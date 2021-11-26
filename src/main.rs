use actix_web::middleware::Logger;
use actix_web::{get, http::StatusCode, web, App, HttpResponse, HttpServer, Responder};
use env_logger::Env;
use semver::Version;

mod app_error;
use app_error::ServerError;

async fn fetch_package(
    package_name: String,
    package_version: String,
) -> Result<String, ServerError> {
    let parsed_version = Version::parse(package_version.as_str())?;

    return Ok(format!("Package {}@{}", package_name, parsed_version.major));
}

#[get("/package/{package_name}/{package_version}")]
async fn package(
    web::Path((package_name, package_version)): web::Path<(String, String)>,
) -> impl Responder {
    match fetch_package(package_name, package_version).await {
        Ok(response) => HttpResponse::Ok().body(response),
        Err(error) => {
            HttpResponse::build(StatusCode::INTERNAL_SERVER_ERROR).body(format!("{:?}", error))
        }
    }
}

#[get("/versions/{manifest}")]
async fn versions(web::Path(manifest): web::Path<String>) -> impl Responder {
    HttpResponse::Ok().body(format!("Versions of {}", manifest))
}

#[actix_web::main]
async fn main() -> std::io::Result<()> {
    env_logger::Builder::from_env(Env::default().default_filter_or("info")).init();

    HttpServer::new(|| {
        App::new()
            .wrap(Logger::new("\"%r\" %s %Dms"))
            .service(package)
            .service(versions)
    })
    .bind("127.0.0.1:8080")?
    .run()
    .await
}
