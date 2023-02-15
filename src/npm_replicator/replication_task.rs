use super::registry::NpmRocksDB;
use crate::app_error::AppResult;
use crate::npm_replicator::changes::ChangesStream;
use crate::npm_replicator::sqlite::NpmDatabase;
use crate::npm_replicator::types::changes::Event::Change;
use crate::npm_replicator::types::document::MinimalPackageData;

use std::env;
use std::time::Duration;
use tokio::time::sleep;

const FINISHED_DEBOUNCE: u64 = 60000;

async fn sync(db: NpmRocksDB) -> AppResult<()> {
    let mut last_seq: i64 = db.get_last_seq()?;
    println!("[NPM-Replication] Last synced sequence {}", last_seq);
    if last_seq < 15000000 {
        // Setup SQLite DB
        let npm_db_path =
            env::var("NPM_SQLITE_DB").expect("NPM_SQLITE_DB env variable should be set");
        let npm_db = NpmDatabase::new(&npm_db_path)?;
        npm_db.init()?;

        let packages = npm_db.list_packages()?;
        for package_name in packages {
            let pkg = npm_db.get_package(&package_name)?;
            db.write_package(pkg)?;
            println!("[SQLite => RocksDB] Synced {}", package_name);
        }

        last_seq = npm_db.get_last_seq()?;
        db.update_last_seq(last_seq)?;
        println!("[SQLite => RocksDB] Completed syncing {}", last_seq);
    } else {
        println!("[SQLite => RocksDB] Skipping sync");
    }

    let mut stream = ChangesStream::new(50, last_seq.into());
    loop {
        match stream.fetch_next().await {
            Ok(page) => {
                let result_count = { page.results.len() };
                for entry in page.results {
                    if let Change(evt) = entry {
                        if evt.deleted {
                            db.delete_package(&evt.id)?;
                            // println!("[NPM-Replication] Deleted package {}", evt.id);
                        } else if let Some(doc) = evt.doc {
                            db.write_package(MinimalPackageData::from_doc(doc))?;
                            // println!("[NPM-Replication] Wrote package {} to db", evt.id);
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
