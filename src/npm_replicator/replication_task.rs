use super::database::NpmDatabase;
use crate::app_error::AppResult;
use crate::npm_replicator::changes::ChangesStream;
use crate::npm_replicator::types::changes::Event::Change;
use crate::npm_replicator::types::document::MinimalPackageData;

use std::time::Duration;
use tokio::time::sleep;

async fn sync(db_path: String) -> AppResult<()> {
    let db: NpmDatabase = NpmDatabase::new(&db_path)?;
    let last_seq: i64 = db.get_last_seq()?;
    println!("[NPM-Replication] Last synced sequence {}", last_seq);
    let mut stream = ChangesStream::new(last_seq.into());
    loop {
        match stream.fetch_next().await {
            Ok(page) => {
                for entry in page.results {
                    if let Change(evt) = entry {
                        if evt.deleted {
                            db.delete_package(&evt.id)?;
                            println!("[NPM-Replication] Deleted package {}", evt.id);
                        } else if let Some(doc) = evt.doc {
                            db.write_package(MinimalPackageData::from_doc(doc))?;
                            println!("[NPM-Replication] Wrote package {} to db", evt.id);
                        }
                    }
                }

                println!("[NPM-Replication] Updated last seq to {}", page.last_seq);
                db.update_last_seq(page.last_seq)?;
            }
            Err(err) => {
                println!("NPM Registry sync error {:?}", err);
                sleep(Duration::from_millis(250)).await;
            }
        }
    }
}

pub fn spawn_sync_thread(db_path: String) {
    println!("[NPM-Replication] Spawning npm sync worker...");
    tokio::spawn(async move {
        loop {
            println!("[NPM-Replication] Starting npm sync worker...");
            if let Err(err) = sync(db_path.clone()).await {
                println!("[NPM-Replication] SYNC WORKER CRASHED {:?}", err);
                sleep(Duration::from_millis(500)).await;
            }
        }
    });
}
