use actix_web::middleware::Logger;
use actix_web::{get, http::StatusCode, web, App, HttpResponse, HttpServer, Responder};
use env_logger::Env;

mod app_error;
mod npm;
mod process_package;
use process_package::process_package;

#[get("/package/{package_name}/{package_version}")]
async fn package(path: web::Path<(String, String)>) -> impl Responder {
    let (package_name, package_version) = path.into_inner();
    match process_package(package_name, package_version).await {
        Ok(response) => HttpResponse::Ok().body(response),
        Err(error) => {
            HttpResponse::build(StatusCode::INTERNAL_SERVER_ERROR).body(format!("{:?}", error))
        }
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

    HttpServer::new(|| {
        App::new()
            .wrap(Logger::new("\"%r\" %s %Dms"))
            .service(package)
            .service(versions)
    })
    .bind(server_address)?
    .run()
    .await
}
