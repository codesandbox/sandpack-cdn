use serde::{Deserialize, Serialize};
use warp::{Filter, Rejection, Reply};

use crate::app_error::{AppResult, ServerError};
use crate::npm_replicator::database::NpmDatabase;

use super::super::custom_reply::CustomReply;
use super::super::error_reply::ErrorReply;
use super::super::routes::with_data;

#[derive(Serialize, Deserialize, PartialEq, Eq, Debug, Clone)]
struct NpmSyncStatus {
    doc_count: i64,
    last_seq: i64,
}

async fn get_reply(npm_db: NpmDatabase) -> Result<CustomReply, ServerError> {
    let status: AppResult<NpmSyncStatus> = tokio::task::spawn_blocking(move || {
        let last_seq = npm_db.get_last_seq()?;
        let doc_count = npm_db.get_package_count()?;

        Ok(NpmSyncStatus {
            last_seq,
            doc_count,
        })
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

async fn route_handler(npm_db: NpmDatabase) -> Result<impl Reply, Rejection> {
    match get_reply(npm_db).await {
        Ok(reply) => Ok(reply),
        Err(err) => Ok(ErrorReply::from(err).as_reply(3600).unwrap()),
    }
}

pub fn npm_sync_status_route(
    npm_db: NpmDatabase,
) -> impl Filter<Extract = impl warp::Reply, Error = warp::Rejection> + Clone {
    let cloned_db = npm_db.clone();
    warp::path!("v2" / "npm_sync_status")
        .and(warp::get())
        .and(with_data(cloned_db))
        .and_then(route_handler)
}
