use app_error::ServerError;
use base64::decode as decode_base64;
use lazy_static::lazy_static;
use package::collect_dep_tree::{collect_dep_tree, process_dep_map, DependencyList};
use regex::Regex;
use serde::{self, Deserialize, Serialize};
use std::collections::HashMap;
use std::env;
use std::net::SocketAddr;
use warp::{Filter, Rejection, Reply};

mod app_error;
mod cache;
mod custom_reply;
mod package;
mod routes;
mod setup_tracing;
mod transform;
mod utils;

use cache::layered::LayeredCache;
use custom_reply::CustomReply;
use package::process::{transform_module_cached, MinimalCachedModule};

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

// #[get("/package/{package_specifier}")]
async fn package_req_handler(
    path: String,
    data: AppData,
    cache: LayeredCache,
) -> Result<impl Reply, Rejection> {
    let decoded_specifier = decode_req_part(path.as_str())?;
    let package_content =
        transform_module_cached(decoded_specifier.as_str(), data.data_dir.as_str(), &cache).await?;

    let mut reply = CustomReply::json(&package_content);
    reply.add_header(
        "cache-control",
        format!("public, max-age={}", 365 * 24 * 3600).as_str(),
    );
    Ok(reply)

    // match package_content {
    //     Ok(response) => {
    //         let mut builder = HttpResponse::Ok();
    //         // 1 year
    //
    //         builder.insert_header(CacheControl(vec![
    //             CacheDirective::Public,
    //             CacheDirective::MaxAge(cache_ttl),
    //         ]));
    //         builder.json(response)
    //     }
    //     Err(error) => {
    //         let mut builder = HttpResponse::build(StatusCode::INTERNAL_SERVER_ERROR);
    //         // 6 hours
    //         let cache_ttl: u32 = 6 * 3600;
    //         builder.insert_header(CacheControl(vec![
    //             CacheDirective::Public,
    //             CacheDirective::MaxAge(cache_ttl),
    //         ]));
    //         builder.json(ErrorResponse::new(
    //             format!("{}", error),
    //             format!("{:?}", error),
    //         ))
    //     }
    // }
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

// #[get("/dep_tree/{dependencies}")]
// async fn versions_req_handler(
//     path: web::Path<String>,
//     data: web::Data<AppData>,
//     cache_arc: web::Data<LayeredCache>,
// ) -> impl Responder {
//     let cache = cache_arc.into_inner();
//     let tree = process_dep_tree(path.into_inner().as_str(), data.data_dir.as_str(), &cache).await;

//     // 15 minutes cache ttl
//     let cache_ttl: u32 = 15 * 60;
//     match tree {
//         Ok(response) => {
//             let mut builder = HttpResponse::Ok();
//             builder.insert_header(CacheControl(vec![
//                 CacheDirective::Public,
//                 CacheDirective::MaxAge(cache_ttl),
//             ]));
//             builder.json(response)
//         }
//         Err(error) => {
//             let mut builder = HttpResponse::build(StatusCode::INTERNAL_SERVER_ERROR);
//             builder.insert_header(CacheControl(vec![
//                 CacheDirective::Public,
//                 CacheDirective::MaxAge(cache_ttl),
//             ]));
//             builder.json(ErrorResponse::new(
//                 format!("{}", error),
//                 format!("{:?}", error),
//             ))
//         }
//     }
// }

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
