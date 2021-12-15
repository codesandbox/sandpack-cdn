use actix_cors::Cors;
use actix_web::middleware::{Compress, Logger};
use actix_web::{
    get,
    http::header::{CacheControl, CacheDirective, ContentEncoding},
    http::StatusCode,
    web, App, HttpResponse, HttpServer, Responder,
};
use app_error::ServerError;
use base64::decode as decode_base64;
use env_logger::Env;
use package::collect_dep_tree::{collect_dep_tree, process_dep_map, DependencyList};
use serde::{self, Deserialize, Serialize};
use std::collections::HashMap;
use std::env;
use std::fs;
use std::sync::{Arc, Mutex, MutexGuard};

mod app_error;
mod cache;
mod package;
mod transform;
mod utils;

use cache::layered::LayeredCache;
use package::process::{transform_module_cached, MinimalCachedModule};

#[derive(Clone)]
struct AppData {
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
    Ok(String::from(str_value))
}

async fn do_package_req(
    path: &str,
    data_dir: &str,
    cache: &mut MutexGuard<'_, LayeredCache>,
) -> Result<MinimalCachedModule, ServerError> {
    let decoded_specifier = decode_req_part(path)?;
    transform_module_cached(decoded_specifier.as_str(), data_dir, cache).await
}

#[get("/package/{package_specifier}")]
async fn package_req_handler(
    path: web::Path<String>,
    data: web::Data<AppData>,
    cache_arc: web::Data<Arc<Mutex<LayeredCache>>>,
) -> impl Responder {
    let package_content = do_package_req(
        path.as_str(),
        data.data_dir.as_str(),
        &mut cache_arc.lock().unwrap(),
    )
    .await;

    match package_content {
        Ok(response) => {
            let mut builder = HttpResponse::Ok();
            let cache_ttl: u32 = 86400 * 365;
            builder.insert_header(CacheControl(vec![
                CacheDirective::Public,
                CacheDirective::MaxAge(cache_ttl),
            ]));
            builder.json(response)
        }
        Err(error) => {
            let mut builder = HttpResponse::build(StatusCode::INTERNAL_SERVER_ERROR);
            let cache_ttl: u32 = 86400;
            builder.insert_header(CacheControl(vec![
                CacheDirective::Public,
                CacheDirective::MaxAge(cache_ttl),
            ]));
            builder.json(ErrorResponse::new(
                format!("{}", error),
                format!("{:?}", error),
            ))
        }
    }
}

async fn process_dep_tree(
    raw_deps_str: &str,
    data_dir: &str,
    cache: Arc<Arc<Mutex<LayeredCache>>>,
) -> Result<DependencyList, ServerError> {
    let decoded_deps_str = decode_req_part(raw_deps_str)?;
    let dep_map: HashMap<String, String> = serde_json::from_str(decoded_deps_str.as_str())?;
    let dep_requests = process_dep_map(dep_map, 0)?;
    return collect_dep_tree(dep_requests, data_dir, cache).await;
}

#[get("/dep_tree/{dependencies}")]
async fn versions_req_handler(
    path: web::Path<String>,
    data: web::Data<AppData>,
    cache_arc: web::Data<Arc<Mutex<LayeredCache>>>,
) -> impl Responder {
    let cache = cache_arc.into_inner();
    let tree = process_dep_tree(path.into_inner().as_str(), data.data_dir.as_str(), cache).await;

    // 15 minutes cache ttl
    let cache_ttl: u32 = 15 * 60;
    match tree {
        Ok(response) => {
            let mut builder = HttpResponse::Ok();
            builder.insert_header(CacheControl(vec![
                CacheDirective::Public,
                CacheDirective::MaxAge(cache_ttl),
            ]));
            builder.json(response)
        }
        Err(error) => {
            let mut builder = HttpResponse::build(StatusCode::INTERNAL_SERVER_ERROR);
            builder.insert_header(CacheControl(vec![
                CacheDirective::Public,
                CacheDirective::MaxAge(cache_ttl),
            ]));
            builder.json(ErrorResponse::new(
                format!("{}", error),
                format!("{:?}", error),
            ))
        }
    }
}

#[actix_web::main]
async fn main() -> Result<(), std::io::Error> {
    env_logger::Builder::from_env(Env::default().default_filter_or("info")).init();

    let layered_cache = Arc::new(Mutex::new(
        LayeredCache::new(
            "rediss://:c4d5cb5cb1dd4ad49bc9af0f565e9d53@eu1-tidy-bison-33953.upstash.io:33953",
            1000,
        )
        .await?,
    ));

    let data_dir_path = env::current_dir()?.join("temp_files");
    let data_dir = data_dir_path.as_os_str().to_str().unwrap();
    let data = AppData {
        data_dir: String::from(data_dir),
    };

    // create data directory
    fs::create_dir_all(String::from(data_dir))?;

    let port = match env::var("CASE_INSENSITIVE") {
        Ok(var) => var,
        Err(_) => String::from("8080"),
    };

    let server_address = format!("0.0.0.0:{}", port);
    println!("Starting server on {}", server_address);
    HttpServer::new(move || {
        let mut cors = Cors::default();
        cors = cors.allow_any_header();
        cors = cors.allow_any_method();
        cors = cors.allow_any_origin();

        App::new()
            .app_data(web::Data::new(data.clone()))
            .app_data(web::Data::new(layered_cache.clone()))
            .wrap(cors)
            .wrap(Logger::new("\"%r\" %s %Dms"))
            // TODO: Remove this and let cloudflare handle encoding?
            .wrap(Compress::new(ContentEncoding::Auto))
            .service(package_req_handler)
            .service(versions_req_handler)
    })
    .bind(server_address)?
    .run()
    .await
}
