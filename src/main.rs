use crate::npm_replicator::{database::NpmDatabase, replication_task};
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

    setup_tracing::setup_tracing();

    // Setup npm registry replicator
    let npm_db = NpmDatabase::new(&npm_db_path).unwrap();
    npm_db.init().unwrap();
    replication_task::spawn_sync_thread(npm_db.clone());

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

    let filter = router::routes::routes(npm_db)
        .with(warp::trace::request())
        .with(cors_headers_filter)
        .with(warp::compression::gzip());

    let addr: SocketAddr = ([0, 0, 0, 0], port).into();
    println!("Server running on {}", addr);
    warp::serve(filter).run(addr).await;

    Ok(())
}
