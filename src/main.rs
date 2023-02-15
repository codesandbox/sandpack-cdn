use crate::npm_replicator::{sqlite::NpmDatabase, registry::NpmRocksDB, replication_task};
use dotenv::dotenv;
use std::env;
use std::net::SocketAddr;
use warp::http::header::{HeaderMap, HeaderValue};
use warp::Filter;

mod app_error;
mod cached;
mod npm;
mod npm_replicator;
mod package;
mod router;
mod setup_tracing;
mod utils;

#[tokio::main]
async fn main() -> Result<(), std::io::Error> {
    dotenv().ok();

    let port = match env::var("PORT") {
        Ok(var) => var,
        Err(_) => String::from("8080"),
    }
    .parse::<u16>()
    .unwrap();

    let npm_db_path = env::var("NPM_SQLITE_DB").expect("NPM_SQLITE_DB env variable should be set");
    let npm_registry_path = env::var("NPM_ROCKS_DB").expect("NPM_ROCKS_DB env variable should be set");

    setup_tracing::setup_tracing();

    // Setup npm db
    let npm_fs_db = NpmRocksDB::new(&npm_registry_path);
    replication_task::spawn_sync_thread(npm_fs_db.clone());

    // cors headers
    let mut headers = HeaderMap::new();
    headers.insert("Access-Control-Allow-Origin", HeaderValue::from_static("*"));
    headers.insert(
        "Access-Control-Allow-Headers",
        HeaderValue::from_static("*"),
    );
    headers.insert(
        "Access-Control-Allow-Methods",
        HeaderValue::from_static("GET, POST, OPTIONS"),
    );
    let cors_headers_filter = warp::reply::with::headers(headers);

    let filter = router::routes::routes(npm_fs_db)
        .with(warp::trace::request())
        .with(cors_headers_filter)
        .with(warp::compression::gzip());

    let addr: SocketAddr = ([0, 0, 0, 0], port).into();
    println!("Server running on {}", addr);
    warp::serve(filter).run(addr).await;

    Ok(())
}
