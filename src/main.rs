use actix_web::middleware::{Compress, Logger};
use actix_web::{
    get,
    http::header::{CacheControl, CacheDirective, ContentEncoding},
    http::StatusCode,
    web, App, HttpResponse, HttpServer, Responder,
};
use env_logger::Env;
use package::collect_dep_tree::collect_dep_tree;
use std::env;
use std::fs;
use std::sync::{Arc, Mutex};

mod app_error;
mod cache;
mod package;
mod transform;
mod utils;

use cache::layered::LayeredCache;
use package::process::process_package_cached;

#[derive(Clone)]
struct AppData {
    data_dir: String,
}

#[get("/package/{package_name}/{package_version}")]
async fn package_req_handler(
    path: web::Path<(String, String)>,
    data: web::Data<AppData>,
    cache_arc: web::Data<Arc<Mutex<LayeredCache>>>,
) -> impl Responder {
    let (package_name, package_version) = path.into_inner();

    let data_dir = data.data_dir.clone();
    let package_content = process_package_cached(
        package_name,
        package_version,
        data_dir,
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
            builder.body(format!("{}\n\n{:?}", error, error))
        }
    }
}

#[get("/versions/{manifest}")]
async fn versions_req_handler(
    path: web::Path<String>,
    data: web::Data<AppData>,
    cache_arc: web::Data<Arc<Mutex<LayeredCache>>>,
) -> impl Responder {
    let manifest = path.into_inner();
    let data_dir = data.data_dir.clone();
    let tree = collect_dep_tree(Vec::new(), data_dir, &mut cache_arc.lock().unwrap()).await;
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
            builder.body(format!("{}\n\n{:?}", error, error))
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

    let server_address = "127.0.0.1:8080";

    println!("Starting server on {}", server_address);

    let data_dir_path = env::current_dir()?.join("temp_files");
    let data_dir = data_dir_path.as_os_str().to_str().unwrap();
    let data = AppData {
        data_dir: String::from(data_dir),
    };

    // create data directory
    fs::create_dir_all(String::from(data_dir))?;

    HttpServer::new(move || {
        App::new()
            .app_data(web::Data::new(data.clone()))
            .app_data(web::Data::new(layered_cache.clone()))
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
