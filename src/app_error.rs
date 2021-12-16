use thiserror::Error;

#[derive(Error, Debug)]
pub enum ServerError {
    #[error("invalid semver")]
    InvalidSemver(#[from] node_semver::SemverError),
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
    #[error("Could not parse module")]
    SWCParseError { message: String },
    #[error("Could not download npm package manifest")]
    NpmPackageManifestNotFound,
    #[error("Invalid package specifier")]
    InvalidPackageSpecifier,
    #[error("Invalid Base64 string")]
    InvalidBase64(#[from] base64::DecodeError),
    #[error("Invalid byte buffer")]
    InvalidString(#[from] std::str::Utf8Error)
}

impl From<ServerError> for std::io::Error {
    fn from(err: ServerError) -> Self {
        std::io::Error::new(std::io::ErrorKind::Other, format!("{:?}", err))
    }
}
