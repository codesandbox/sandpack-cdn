use serde::{self, Deserialize, Serialize};
use warp::hyper::StatusCode;

use crate::app_error::ServerError;

use super::custom_reply::CustomReply;

#[derive(Clone, Serialize, Deserialize)]
pub struct ErrorReply {
    status: u16,
    message: String,
    details: String,
}

impl ErrorReply {
    pub fn new(status: u16, message: String, details: String) -> Self {
        ErrorReply {
            status,
            message,
            details,
        }
    }

    pub fn as_reply(&self, cache_ttl: u32) -> Result<CustomReply, ServerError> {
        let mut reply = CustomReply::json(self)?;
        reply.set_status(StatusCode::from_u16(self.status)?);
        reply.add_header(
            "cache-control",
            format!("public, max-age={}", cache_ttl).as_str(),
        );
        Ok(reply)
    }
}

impl From<ServerError> for ErrorReply {
    fn from(err: ServerError) -> Self {
        ErrorReply::new(500, format!("{}", err), format!("{:?}", err))
    }
}
