use thiserror::Error;

#[derive(Error, Debug)]
pub enum ServerError {
    #[error("invalid semver")]
    InvalidSemver(#[from] semver::Error),
}
