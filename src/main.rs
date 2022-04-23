use app_error::ServerError;
use base64::decode as decode_base64;
use lazy_static::lazy_static;
use package::collect_dep_tree::{collect_dep_tree, process_dep_map, DependencyList};
use regex::Regex;
use serde::{self, Deserialize, Serialize};
use std::collections::HashMap;
use std::env;
use std::net::SocketAddr;
use warp::Filter;
use cache::layered::LayeredCache;

mod app_error;
mod cache;
mod custom_reply;
mod package;
mod routes;
mod setup_tracing;
mod transform;
mod utils;

lazy_static! {
    static ref VERSION_RE: Regex = Regex::new("^(\\d+)\\((.*)\\)$").unwrap();
    static ref LATEST_VERSION: u64 = 2;
}

#[derive(Clone)]
pub struct AppData {
    data_dir: String,
}

#[derive(Clone, Serialize, Deserialize)]
struct ErrorResponse {
    message: String,
    details: String,
}

impl ErrorResponse {
    pub fn new(message: String, details: String) -> Self {
        ErrorResponse { message, details }
    }
}

fn decode_req_part(part: &str) -> Result<String, ServerError> {
    let decoded = decode_base64(part)?;
    let str_value = std::str::from_utf8(&decoded)?;

    if let Some(parts) = VERSION_RE.captures(str_value) {
        if let Some(version_match) = parts.get(1) {
            let version = version_match.as_str().parse::<u64>()?;
            if version > *LATEST_VERSION {
                return Err(ServerError::InvalidCDNVersion);
            }
        }

        if let Some(content_match) = parts.get(2) {
            return Ok(String::from(content_match.as_str()));
        }
    }

    // Fallback to no version
    Ok(String::from(str_value))
}

async fn process_dep_tree(
    raw_deps_str: &str,
    data_dir: &str,
    cache: &LayeredCache,
) -> Result<DependencyList, ServerError> {
    let decoded_deps_str = decode_req_part(raw_deps_str)?;
    let dep_map: HashMap<String, String> = serde_json::from_str(decoded_deps_str.as_str())?;
    let dep_requests = process_dep_map(dep_map, 0)?;
    return collect_dep_tree(dep_requests, data_dir, cache).await;
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
    };

    // create data directory
    tokio::fs::create_dir_all(String::from(data_dir)).await?;

    let cors = warp::cors()
        .allow_any_origin()
        .allow_methods(vec!["POST", "GET", "PUT"]);

    let filter = routes::routes(app_data, layered_cache)
        .with(warp::trace::request())
        .with(cors)
        .with(warp::compression::gzip());

    let addr: SocketAddr = ([0, 0, 0, 0], port).into();
    println!("Server running on {}", addr);
    warp::serve(filter).run(addr).await;

    Ok(())
}
