use thiserror::Error;
use warp::hyper::http;

#[derive(Error, Debug)]
pub enum ServerError {
    #[error("invalid semver")]
    InvalidSemver(#[from] node_semver::SemverError),
    #[error("Failed request")]
    FailedRequest(#[from] reqwest_middleware::Error),
    #[error("Request failed")]
    RequestFailed(#[from] reqwest::Error),
    #[error("Response has a non-200 status code")]
    RequestErrorStatus { status_code: u16 },
    #[error("IO Operation failed")]
    IoError(#[from] std::io::Error),
    #[error("Could not parse url")]
    UrlParseError(#[from] url::ParseError),
    #[error("Could not parse json string")]
    JSONParseError(#[from] serde_json::Error),
    #[error("Package version not found")]
    PackageVersionNotFound,
    #[error("Infallible error")]
    Infallible(#[from] std::convert::Infallible),
    #[error("Could not parse module")]
    SWCParseError { message: String },
    #[error("Could not download npm package")]
    NpmPackageDownloadError {
        status_code: u16,
        package_name: String,
        package_version: String,
    },
    #[error("Could not download npm package manifest")]
    NpmManifestDownloadError {
        status_code: u16,
        package_name: String,
    },
    #[error("Invalid package specifier")]
    InvalidPackageSpecifier,
    #[error("Invalid Base64 string")]
    InvalidBase64(#[from] base64::DecodeError),
    #[error("Invalid byte buffer")]
    InvalidString(#[from] std::str::Utf8Error),
    #[error("Join error")]
    JoinError(#[from] tokio::task::JoinError),
    #[error("Invalid CDN version")]
    InvalidCDNVersion,
    #[error("Could not parse integer")]
    IntegerParse(#[from] std::num::ParseIntError),
    #[error("Invalid status code")]
    InvalidStatusCode(#[from] http::status::InvalidStatusCode),
}

impl From<ServerError> for std::io::Error {
    fn from(err: ServerError) -> Self {
        std::io::Error::new(std::io::ErrorKind::Other, format!("{:?}", err))
    }
}
