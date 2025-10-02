use super::database::NpmDatabase;
use crate::app_error::AppResult;
use crate::npm_replicator::changes::ChangesStream;
use crate::npm_replicator::types::changes::Event::Change;
use crate::npm_replicator::types::document::{MinimalPackageData, RegistryDocument};

use futures::{stream, StreamExt};
use reqwest::{Client, StatusCode};
use std::{env, time::Duration};
use tokio::time::sleep;

const FINISHED_DEBOUNCE: u64 = 5000;
const PACKUMENT_FETCH_CONCURRENCY: usize = 32;
const CHANGE_PAGE_LIMIT: usize = 1000;
const PACKUMENT_FETCH_RETRY_MAX_ATTEMPTS: usize = 5;
const PACKUMENT_FETCH_RETRY_BASE_DELAY_MS: u64 = 500;

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

async fn sync(db: NpmDatabase) -> AppResult<()> {
    let db_seq: i64 = db.get_last_seq()?;
    let forced_seq = env::var("NPM_REPLICATION_FORCE_SINCE")
        .ok()
        .and_then(|raw| {
            let trimmed = raw.trim();
            if trimmed.is_empty() {
                None
            } else {
                match trimmed.parse::<i64>() {
                    Ok(value) => Some(value),
                    Err(err) => {
                        println!(
                            "[NPM-Replication] Invalid NPM_REPLICATION_FORCE_SINCE '{}': {:?}",
                            trimmed, err
                        );
                        None
                    }
                }
            }
        });

    let start_seq = forced_seq.unwrap_or(db_seq);

    if let Some(seq) = forced_seq {
        println!(
            "[NPM-Replication] Starting from forced sequence {} (db sequence {})",
            seq, db_seq
        );
    } else {
        println!("[NPM-Replication] Last synced sequence {}", start_seq);
    }

    let mut stream = ChangesStream::new(CHANGE_PAGE_LIMIT, start_seq.into());
    let package_client = Client::builder().timeout(Duration::from_secs(60)).build()?;
    loop {
        match stream.fetch_next().await {
            Ok(page) => {
                let result_count = { page.results.len() };
                let mut pending_ids: Vec<String> = Vec::new();
                for entry in page.results {
                    if let Change(evt) = entry {
                        if evt.deleted {
                            db.delete_package(&evt.id)?;
                            println!("[NPM-Replication] Deleted package {}", evt.id);
                        } else {
                            pending_ids.push(evt.id);
                        }
                    }
                }

                if !pending_ids.is_empty() {
                    let fetch_stream = stream::iter(pending_ids.into_iter()).map(|package_id| {
                        let client = package_client.clone();
                        async move {
                            let result = fetch_packument(&client, &package_id).await;
                            (package_id, result)
                        }
                    });

                    let mut fetch_stream =
                        fetch_stream.buffer_unordered(PACKUMENT_FETCH_CONCURRENCY);
                    while let Some((package_id, result)) = fetch_stream.next().await {
                        match result {
                            Ok(pkg) => {
                                db.write_package(pkg)?;
                                println!("[NPM-Replication] Wrote package {} to db", package_id);
                            }
                            Err(err) => {
                                println!(
                                    "[NPM-Replication] Failed to fetch package {}: {:?}",
                                    package_id, err
                                );
                            }
                        }
                    }
                }

                println!("[NPM-Replication] Updated last seq to {}", page.last_seq);
                db.update_last_seq(page.last_seq)?;

                if stream.should_wait(result_count) {
                    sleep(Duration::from_millis(FINISHED_DEBOUNCE)).await;
                }
            }
            Err(err) => {
                println!("NPM Registry sync error {:?}", err);
                sleep(Duration::from_millis(FINISHED_DEBOUNCE)).await;
            }
        }
    }
}

pub fn spawn_sync_thread(db: NpmDatabase) {
    println!("[NPM-Replication] Spawning npm sync worker...");
    tokio::task::spawn(async move {
        println!("[NPM-Replication] Starting npm sync worker...");
        if let Err(err) = sync(db).await {
            println!("[NPM-Replication] SYNC WORKER CRASHED {:?}", err);
            sleep(Duration::from_millis(500)).await;
        }
    });
}
