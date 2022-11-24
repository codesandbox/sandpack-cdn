use super::database::NpmDatabase;
use crate::app_error::AppResult;
use crate::npm_replicator::changes::ChangesStream;
use crate::npm_replicator::types::changes::Event::Change;
use crate::npm_replicator::types::document::MinimalPackageData;

use std::thread::sleep;
use std::time::Duration;

const FINISHED_DEBOUNCE: u64 = 60000;

fn sync(db: NpmDatabase) -> AppResult<()> {
    let last_seq: i64 = db.get_last_seq()?;
    println!("[NPM-Replication] Last synced sequence {}", last_seq);
    let mut stream = ChangesStream::new(50, last_seq.into());
    loop {
        match stream.fetch_next() {
            Ok(page) => {
                let result_count = { page.results.len() };
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

                if stream.should_wait(result_count) {
                    sleep(Duration::from_millis(FINISHED_DEBOUNCE));
                }
            }
            Err(err) => {
                println!("NPM Registry sync error {:?}", err);
                sleep(Duration::from_millis(FINISHED_DEBOUNCE));
            }
        }
    }
}

pub fn spawn_sync_thread(db: NpmDatabase) {
    println!("[NPM-Replication] Spawning npm sync worker...");
    tokio::task::spawn_blocking(move || loop {
        println!("[NPM-Replication] Starting npm sync worker...");
        if let Err(err) = sync(db.clone()) {
            println!("[NPM-Replication] SYNC WORKER CRASHED {:?}", err);
            sleep(Duration::from_millis(500));
        }
    });
}
