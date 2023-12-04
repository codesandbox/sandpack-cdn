use super::registry::NpmRocksDB;
use crate::app_error::AppResult;
use crate::npm::package_data::download_pkg_metadata;
use crate::npm_replicator::changes::ChangesStream;
use crate::npm_replicator::types::changes::Event::Change;
use crate::npm_replicator::types::document::MinimalPackageData;

use std::time::Duration;
use tokio::time::sleep;

const FINISHED_DEBOUNCE: u64 = 60000;

async fn sync(db: NpmRocksDB) -> AppResult<()> {
    let last_seq: i64 = db.get_last_seq()?;
    println!("[NPM-Replication] Last synced sequence {}", last_seq);
    let mut stream = ChangesStream::new(50, last_seq.into());
    loop {
        match stream.fetch_next().await {
            Ok(page) => {
                let result_count = { page.results.len() };
                for entry in page.results {
                    if let Change(evt) = entry {
                        if evt.deleted {
                            db.delete_package(&evt.id)?;
                            println!("[NPM-Replication] Deleted package {}", evt.id);
                        } else if let Some(doc) = evt.doc {
                            println!("[NPM-Replication] Fetching package {} from npm", evt.id);
                            let metadata_result = download_pkg_metadata(&doc.id).await;
                            match metadata_result {
                                Ok(metadata) => {
                                    let pkg: MinimalPackageData =
                                        MinimalPackageData::from_registry_meta(metadata);
                                    db.write_package(pkg)?;
                                    println!("[NPM-Replication] Wrote package {} to db", evt.id);
                                }
                                Err(_err) => {
                                    db.delete_package(&evt.id)?;
                                    println!("[NPM-Replication] Package {} does not seem to exist, removing it", evt.id);
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

pub fn spawn_sync_thread(db: NpmRocksDB) {
    println!("[NPM-Replication] Spawning npm sync worker...");
    tokio::task::spawn(async move {
        if let Err(err) = sync(db).await {
            println!("[NPM-Replication] SYNC WORKER CRASHED {:?}", err);
            sleep(Duration::from_millis(500)).await;
        }
    });
}
