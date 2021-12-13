use thiserror::Error;

#[derive(Error, Debug)]
pub enum ServerError {
    #[error("invalid semver")]
    InvalidSemver(#[from] semver::Error),
    #[error("some request error occured...")]
    FailedRequest(#[from] reqwest::Error),
    #[error("some request error occured...")]
    RequestErrorStatus(u16),
    #[error("an io error occured...")]
    IoError(#[from] std::io::Error),
    #[error("Could not parse url")]
    UrlParseError(#[from] url::ParseError),
    #[error("Could not parse json string")]
    JSONParseError(#[from] serde_json::Error),
    #[error("Package version not found")]
    PackageVersionNotFound,
    #[error("Infallible error")]
    Infallible(#[from] std::convert::Infallible),
    #[error("Redis error")]
    Redis(#[from] redis::RedisError),
    #[error("Could not parse module")]
    SWCParseError { message: String },
    #[error("Could not download npm package manifest")]
    NpmPackageManifestNotFound,
}

impl From<ServerError> for std::io::Error {
    fn from(err: ServerError) -> Self {
        std::io::Error::new(std::io::ErrorKind::Other, format!("{:?}", err))
    }
}
