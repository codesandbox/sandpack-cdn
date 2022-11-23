use reqwest::StatusCode;

#[derive(Clone, Debug)]
pub struct ChangeStreamError {
    pub status: u16,
    pub message: Option<String>,
}

impl ChangeStreamError {
    pub fn new(status: u16, message: Option<String>) -> Self {
        Self { status, message }
    }
}

impl From<reqwest::Error> for ChangeStreamError {
    fn from(e: reqwest::Error) -> Self {
        ChangeStreamError::new(
            e.status()
                .unwrap_or(StatusCode::INTERNAL_SERVER_ERROR)
                .into(),
            Some(e.to_string()),
        )
    }
}

pub type ChangeStreamResult<T> = Result<T, ChangeStreamError>;
