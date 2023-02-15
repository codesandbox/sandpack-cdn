use crate::npm_replicator::{registry::NpmRocksDB, replication_task, sqlite::NpmDatabase};
use dotenv::dotenv;
use std::env;
use std::net::SocketAddr;
use warp::http::header::{HeaderMap, HeaderValue};
use warp::Filter;

mod app_error;
mod cached;
mod minimal_pkg_capnp;
mod npm;
mod npm_replicator;
mod package;
mod router;
mod setup_tracing;
mod utils;

#[tokio::main]
async fn main() -> Result<(), std::io::Error> {
    dotenv().ok();
    setup_tracing::setup_tracing();

    let port = match env::var("PORT") {
        Ok(var) => var,
        Err(_) => String::from("8080"),
    }
    .parse::<u16>()
    .unwrap();

    // Setup SQLite DB
    let npm_db_path = env::var("NPM_SQLITE_DB").expect("NPM_SQLITE_DB env variable should be set");
    let npm_db = NpmDatabase::new(&npm_db_path).unwrap();
    npm_db.init().unwrap();

    // Setup npm db
    let npm_registry_path =
        env::var("NPM_ROCKS_DB").expect("NPM_ROCKS_DB env variable should be set");
    let npm_fs_db = NpmRocksDB::new(&npm_registry_path);

    let packages = npm_db.list_packages()?;
    for package_name in packages {
        let pkg = npm_db.get_package(&package_name)?;
        npm_fs_db.write_package(pkg)?;
    }

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
