use serde::{Deserialize, Serialize};
use warp::{Filter, Rejection, Reply};

use crate::app_error::{AppResult, ServerError};
use crate::npm_replicator::database::NpmDatabase;
use crate::npm_replicator::types::document::{MinimalPackageData, RegistryDocument};

use super::super::custom_reply::CustomReply;
use super::super::error_reply::ErrorReply;
use super::super::routes::with_data;

use reqwest::{Client, StatusCode};
use std::time::Duration;
use tokio::time::sleep;

const PACKUMENT_FETCH_RETRY_MAX_ATTEMPTS: usize = 5;
const PACKUMENT_FETCH_RETRY_BASE_DELAY_MS: u64 = 500;

#[derive(Serialize, Deserialize, Debug, Clone)]
struct ForceSyncRequest {
    packages: Vec<String>,
}

#[derive(Serialize, Deserialize, Debug, Clone)]
struct PackageSyncResult {
    package: String,
    success: bool,
    error: Option<String>,
}

#[derive(Serialize, Deserialize, Debug, Clone)]
struct ForceSyncResponse {
    results: Vec<PackageSyncResult>,
    total: usize,
    succeeded: usize,
    failed: usize,
}

fn should_retry_status(status: StatusCode) -> bool {
    status.is_server_error() || status == StatusCode::TOO_MANY_REQUESTS
}

fn backoff_delay(attempt: usize) -> Duration {
    let capped_attempt = attempt.saturating_sub(1).min(6);
    let shift = capped_attempt as u32;
    let factor = 1u64 << shift;
    Duration::from_millis(PACKUMENT_FETCH_RETRY_BASE_DELAY_MS.saturating_mul(factor))
}

fn should_retry_error(err: &reqwest::Error) -> bool {
    if err.is_decode() {
        return false;
    }
    if let Some(status) = err.status() {
        return should_retry_status(status);
    }
    err.is_timeout() || err.is_connect() || err.is_request()
}

async fn fetch_packument(
    client: &Client,
    package_id: &str,
) -> Result<MinimalPackageData, reqwest::Error> {
    let url = format!("https://registry.npmjs.org/{}", package_id);
    let mut attempt = 0;
    loop {
        attempt += 1;
        let response_result = client
            .get(&url)
            .header("accept", "application/json")
            .send()
            .await;

        match response_result {
            Ok(response) => {
                let status = response.status();
                if !status.is_success() {
                    let err = response.error_for_status().unwrap_err();
                    if !should_retry_error(&err) || attempt >= PACKUMENT_FETCH_RETRY_MAX_ATTEMPTS {
                        return Err(err);
                    }
                    sleep(backoff_delay(attempt)).await;
                    continue;
                }

                match response.json::<RegistryDocument>().await {
                    Ok(document) => return Ok(MinimalPackageData::from_doc(document)),
                    Err(err) => {
                        if !should_retry_error(&err)
                            || attempt >= PACKUMENT_FETCH_RETRY_MAX_ATTEMPTS
                        {
                            return Err(err);
                        }
                        sleep(backoff_delay(attempt)).await;
                    }
                }
            }
            Err(err) => {
                if !should_retry_error(&err) || attempt >= PACKUMENT_FETCH_RETRY_MAX_ATTEMPTS {
                    return Err(err);
                }
                sleep(backoff_delay(attempt)).await;
            }
        }
    }
}

async fn force_sync_packages(
    npm_db: NpmDatabase,
    packages: Vec<String>,
) -> AppResult<ForceSyncResponse> {
    let client = Client::builder().timeout(Duration::from_secs(60)).build()?;

    let mut results = Vec::new();
    let mut succeeded = 0;
    let mut failed = 0;

    for package_name in packages.iter() {
        println!("[Force-Sync] Fetching package: {}", package_name);

        match fetch_packument(&client, package_name).await {
            Ok(pkg) => match npm_db.write_package(pkg) {
                Ok(_) => {
                    println!("[Force-Sync] Successfully synced: {}", package_name);
                    results.push(PackageSyncResult {
                        package: package_name.clone(),
                        success: true,
                        error: None,
                    });
                    succeeded += 1;
                }
                Err(e) => {
                    let error_msg = format!("Database write error: {}", e);
                    println!(
                        "[Force-Sync] Failed to write {}: {}",
                        package_name, error_msg
                    );
                    results.push(PackageSyncResult {
                        package: package_name.clone(),
                        success: false,
                        error: Some(error_msg),
                    });
                    failed += 1;
                }
            },
            Err(e) => {
                let error_msg = format!("Fetch error: {}", e);
                println!(
                    "[Force-Sync] Failed to fetch {}: {}",
                    package_name, error_msg
                );
                results.push(PackageSyncResult {
                    package: package_name.clone(),
                    success: false,
                    error: Some(error_msg),
                });
                failed += 1;
            }
        }
    }

    Ok(ForceSyncResponse {
        total: results.len(),
        succeeded,
        failed,
        results,
    })
}

async fn get_reply(
    npm_db: NpmDatabase,
    request: ForceSyncRequest,
) -> Result<CustomReply, ServerError> {
    if request.packages.is_empty() {
        return Err(ServerError::BadRequest("No packages specified".to_string()));
    }

    // Limit to prevent abuse
    if request.packages.len() > 100 {
        return Err(ServerError::BadRequest(
            "Too many packages (max 100)".to_string(),
        ));
    }

    let response = force_sync_packages(npm_db, request.packages).await?;

    let mut reply = CustomReply::json(&response)?;
    // Don't cache this endpoint
    reply.add_header("Cache-Control", "no-cache, no-store, must-revalidate");
    Ok(reply)
}

async fn route_handler(
    request: ForceSyncRequest,
    npm_db: NpmDatabase,
) -> Result<impl Reply, Rejection> {
    match get_reply(npm_db, request).await {
        Ok(reply) => Ok(reply),
        Err(err) => Ok(ErrorReply::from(err).as_reply(0).unwrap()),
    }
}

pub fn force_sync_route(
    npm_db: NpmDatabase,
) -> impl Filter<Extract = impl warp::Reply, Error = warp::Rejection> + Clone {
    warp::path!("v2" / "admin" / "force_sync")
        .and(warp::post())
        .and(warp::body::json())
        .and(with_data(npm_db))
        .and_then(route_handler)
}
