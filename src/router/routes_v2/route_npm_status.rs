use serde::{Deserialize, Serialize};
use warp::{Filter, Rejection, Reply};

use crate::app_error::{AppResult, ServerError};
use crate::npm_replicator::registry::NpmRocksDB;

use super::super::custom_reply::CustomReply;
use super::super::error_reply::ErrorReply;
use super::super::routes::with_data;

#[derive(Serialize, Deserialize, PartialEq, Eq, Debug, Clone)]
struct NpmSyncStatus {
    last_seq: i64,
}

async fn get_reply(npm_db: NpmRocksDB) -> Result<CustomReply, ServerError> {
    let status: AppResult<NpmSyncStatus> = tokio::task::spawn_blocking(move || {
        let last_seq = npm_db.get_last_seq()?;

        Ok(NpmSyncStatus { last_seq })
    })
    .await?;

    let mut reply = CustomReply::json(&status?)?;
    let cache_ttl = 300;
    reply.add_header(
        "Cache-Control",
        format!("public, max-age={}", cache_ttl).as_str(),
    );
    Ok(reply)
}

async fn route_handler(npm_db: NpmRocksDB) -> Result<impl Reply, Rejection> {
    match get_reply(npm_db).await {
        Ok(reply) => Ok(reply),
        Err(err) => Ok(ErrorReply::from(err).as_reply(300).unwrap()),
    }
}

pub fn npm_sync_status_route(
    npm_db: NpmRocksDB,
) -> impl Filter<Extract = impl warp::Reply, Error = warp::Rejection> + Clone {
    warp::path!("v2" / "npm_sync_status")
        .and(warp::get())
        .and(with_data(npm_db))
        .and_then(route_handler)
}
