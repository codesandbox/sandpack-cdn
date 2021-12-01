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
}
