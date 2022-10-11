use thiserror::Error;
use warp::{hyper::http, reject};

use crate::cached::CachedError;

#[derive(Error, Debug)]
pub enum ServerError {
    #[error("invalid semver")]
    InvalidSemver(#[from] node_semver::SemverError),
    #[error("Failed request")]
    FailedRequest(#[from] reqwest_middleware::Error),
    #[error(transparent)]
    RequestFailed(#[from] reqwest::Error),
    #[error("Response has a non-200 status code")]
    RequestErrorStatus { status_code: u16 },
    #[error("IO Operation failed")]
    IoError(#[from] std::io::Error),
    #[error("Could not parse url")]
    UrlParseError(#[from] url::ParseError),
    #[error("Could not parse json string")]
    JSONParseError(#[from] serde_json::Error),
    #[error("Package version not found {0}@{1}")]
    PackageVersionNotFound(String, String),
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
    #[error("Could not download tarball package")]
    TarballDownloadError {
        status_code: u16,
        url: String,
    },
    #[error("Could not download npm package manifest")]
    NpmManifestDownloadError {
        status_code: u16,
        package_name: String,
    },
    #[error("Invalid package specifier")]
    InvalidPackageSpecifier,
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
    #[error("Failed to serialize to msgpack")]
    SerializeError(),
    #[error("Failed to deserialize from msgpack")]
    DeserializeError(),
    #[error("Failed to decode base64 string")]
    Base64DecodingError(),
    #[error("Cached error")]
    CachedError(#[from] CachedError),
    #[error("Resource hasn't changed")]
    NotChanged,
    #[error("Invalid query")]
    InvalidQuery,
}

impl From<ServerError> for std::io::Error {
    fn from(err: ServerError) -> Self {
        std::io::Error::new(std::io::ErrorKind::Other, format!("{:?}", err))
    }
}

impl reject::Reject for ServerError {}
