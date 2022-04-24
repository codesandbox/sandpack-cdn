use cache::layered::LayeredCache;
use std::env;
use std::net::SocketAddr;
use warp::Filter;

mod app_error;
mod cache;
mod package;
mod setup_tracing;
mod transform;
mod utils;
mod router;

#[derive(Clone)]
pub struct AppData {
    data_dir: String,
    cache: LayeredCache,
}

#[tokio::main]
async fn main() -> Result<(), std::io::Error> {
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

    let cors = warp::cors()
        .allow_any_origin()
        .allow_methods(vec!["POST", "GET", "PUT"]);

    let filter = router::routes::routes(app_data)
        .with(warp::trace::request())
        .with(cors)
        .with(warp::compression::gzip());

    let addr: SocketAddr = ([0, 0, 0, 0], port).into();
    println!("Server running on {}", addr);
    warp::serve(filter).run(addr).await;

    Ok(())
}
