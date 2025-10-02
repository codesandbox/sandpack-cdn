use super::database::NpmDatabase;
use crate::app_error::AppResult;
use crate::npm_replicator::changes::ChangesStream;
use crate::npm_replicator::types::changes::Event::Change;
use crate::npm_replicator::types::document::{MinimalPackageData, RegistryDocument};

use reqwest::Client;
use std::time::Duration;
use tokio::time::sleep;

const FINISHED_DEBOUNCE: u64 = 60000;

async fn fetch_packument(
    client: &Client,
    package_id: &str,
) -> Result<MinimalPackageData, reqwest::Error> {
    let url = format!("https://registry.npmjs.org/{}", package_id);
    let response = client
        .get(url)
        .header("accept", "application/json")
        .send()
        .await?
        .error_for_status()?;

    let document: RegistryDocument = response.json().await?;
    Ok(MinimalPackageData::from_doc(document))
}

async fn sync(db: NpmDatabase) -> AppResult<()> {
    let last_seq: i64 = db.get_last_seq()?;
    println!("[NPM-Replication] Last synced sequence {}", last_seq);
    let mut stream = ChangesStream::new(50, last_seq.into());
    let package_client = Client::builder().timeout(Duration::from_secs(60)).build()?;
    loop {
        match stream.fetch_next().await {
            Ok(page) => {
                let result_count = { page.results.len() };
                for entry in page.results {
                    if let Change(evt) = entry {
                        if evt.deleted {
                            db.delete_package(&evt.id)?;
                            println!("[NPM-Replication] Deleted package {}", evt.id);
                        } else {
                            match fetch_packument(&package_client, &evt.id).await {
                                Ok(pkg) => {
                                    db.write_package(pkg)?;
                                    println!("[NPM-Replication] Wrote package {} to db", evt.id);
                                }
                                Err(err) => {
                                    println!(
                                        "[NPM-Replication] Failed to fetch package {}: {:?}",
                                        evt.id, err
                                    );
                                }
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
