use cache::layered::LayeredCache;
use dotenv::dotenv;
use std::env;
use std::net::SocketAddr;
use warp::http::header::{HeaderMap, HeaderValue};
use warp::Filter;

mod app_error;
mod cache;
mod package;
mod router;
mod setup_tracing;
mod transform;
mod utils;

#[derive(Clone)]
pub struct AppData {
    data_dir: String,
    cache: LayeredCache,
}

#[tokio::main]
async fn main() -> Result<(), std::io::Error> {
    dotenv().ok();

    let port = match env::var("PORT") {
        Ok(var) => var,
        Err(_) => String::from("8080"),
    }
    .parse::<u16>()
    .unwrap();

    setup_tracing::setup_tracing();

    // TODO: Calculate cache size dynamically based on available memory?
    // 1 module ~ 512Mb
    let layered_cache = LayeredCache::try_init(2500).await?;

    let data_dir_path = env::current_dir()?.join("temp_files");
    let data_dir = data_dir_path.as_os_str().to_str().unwrap();
    let app_data = AppData {
        data_dir: String::from(data_dir),
        cache: layered_cache,
    };

    // create data directory
    tokio::fs::create_dir_all(String::from(data_dir)).await?;

    // cors headers
    let mut headers = HeaderMap::new();
    headers.insert("Access-Control-Allow-Origin", HeaderValue::from_static("*"));
    headers.insert("Access-Control-Allow-Headers", HeaderValue::from_static("*"));
    headers.insert("Access-Control-Allow-Methods", HeaderValue::from_static("GET, POST, OPTIONS"));
    let cors_headers_filter = warp::reply::with::headers(headers);

    let filter = router::routes::routes(app_data)
        .with(warp::trace::request())
        .with(cors_headers_filter)
        .with(warp::compression::gzip());

    let addr: SocketAddr = ([0, 0, 0, 0], port).into();
    println!("Server running on {}", addr);
    warp::serve(filter).run(addr).await;

    Ok(())
}
