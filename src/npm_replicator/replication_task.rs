use super::database::NpmDatabase;
use crate::app_error::AppResult;
use crate::npm_replicator::changes::ChangesStream;
use crate::npm_replicator::types::document::{MinimalPackageData, RegistryDocument};

use tokio_stream::StreamExt;

async fn sync(db_path: String) -> AppResult<()> {
    // let db: NpmDatabase = NpmDatabase::new(&db_path)?;
    // let last_seq: i64 = db.get_last_seq()?;
    // println!("Last synced sequence {}", last_seq);
    // let mut stream = ChangesStream::new(Some(last_seq.into()));
    // while let Some(val) = stream.next().await {
    //     if let Ok(change) = val {
    //         if let Some(doc) = change.doc {
    //             let parsed: RegistryDocument = serde_json::from_value(doc)?;

    //             if parsed.deleted {
    //                 db.delete_package(&parsed.id)?;
    //                 println!("Deleted package {}", parsed.id);
    //             } else {
    //                 db.write_package(MinimalPackageData::from_doc(parsed.clone()))?;
    //                 println!("Wrote package {} to db", parsed.id);
    //             }
    //         }

    //         let last_seq: i64 = serde_json::from_value(change.seq)?;
    //         db.update_last_seq(last_seq)?;
    //     }
    // }

    let mut stream = ChangesStream::new(None);
    while let Some(val) = stream.next().await {
        // do nothing really
    }

    Ok(())
}

async fn spawn_sync_thread(db_path: String) -> AppResult<()> {
    tokio::spawn(async move {
        if let Err(err) = sync(db_path.clone()).await {
            println!("Sync script stopped with the following error: {:?}", err);
        } else {
            println!("Sync script stopped unexpectedly without an error");
        }
    });

    Ok(())
}
