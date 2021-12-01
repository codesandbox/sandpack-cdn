use actix_web::middleware::Logger;
use actix_web::{get, http::StatusCode, web, App, HttpResponse, HttpServer, Responder};
use env_logger::Env;
use std::env;
use std::fs;

mod app_error;
mod npm;
mod package_json;
mod process_package;
mod test_utils;
mod file_utils;

use process_package::process_package;

#[derive(Clone, Debug)]
struct AppData {
    data_dir: String,
}

#[get("/package/{package_name}/{package_version}")]
async fn package(path: web::Path<(String, String)>, data: web::Data<AppData>) -> impl Responder {
    let (package_name, package_version) = path.into_inner();
    let data_dir = data.data_dir.clone();
    match process_package(package_name, package_version, data_dir).await {
        Ok(response) => HttpResponse::Ok().json(response),
        Err(error) => HttpResponse::build(StatusCode::INTERNAL_SERVER_ERROR)
            .body(format!("{}\n\n{:?}", error, error)),
    }
}

#[get("/versions/{manifest}")]
async fn versions(path: web::Path<String>) -> impl Responder {
    let manifest = path.into_inner();
    HttpResponse::Ok().body(format!("Versions of {}", manifest))
}

#[actix_web::main]
async fn main() -> std::io::Result<()> {
    env_logger::Builder::from_env(Env::default().default_filter_or("info")).init();

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
            .wrap(Logger::new("\"%r\" %s %Dms"))
            .service(package)
            .service(versions)
    })
    .bind(server_address)?
    .run()
    .await
}
