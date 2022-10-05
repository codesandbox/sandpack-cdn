use std::collections::HashMap;

use serde::Serialize;
use warp::{
    http::HeaderValue,
    hyper::{header::HeaderName, StatusCode},
    reply::Response,
    Reply,
};

use crate::{app_error::ServerError, utils::msgpack::serialize_msgpack};

pub struct CustomReply {
    body: Vec<u8>,
    status: StatusCode,
    headers: HashMap<String, String>,
}

impl CustomReply {
    pub fn json<T>(value: &T) -> Result<CustomReply, ServerError>
    where
        T: Serialize,
    {
        let mut reply = CustomReply {
            body: serde_json::to_vec(value)?,
            status: StatusCode::OK,
            headers: HashMap::new(),
        };
        reply.add_header("content-type", "application/json");
        Ok(reply)
    }

    pub fn msgpack<T>(value: &T) -> Result<CustomReply, ServerError>
    where
        T: Serialize,
    {
        let buf = serialize_msgpack(value)?;
        let mut reply = CustomReply {
            body: buf,
            status: StatusCode::OK,
            headers: HashMap::new(),
        };
        // It should really be application/msgpack but
        // this is a hack to get cloudflare to encode it
        // using gzip/brotli
        reply.add_header("content-type", "application/javascript");
        Ok(reply)
    }

    pub fn add_header(&mut self, name: &str, value: &str) {
        self.headers.insert(name.to_string(), value.to_string());
    }

    pub fn set_status(&mut self, status: StatusCode) {
        self.status = status;
    }
}

impl Reply for CustomReply {
    #[inline]
    fn into_response(self) -> Response {
        let mut response = Response::new(self.body.into());
        *response.status_mut() = self.status;
        for (key, value) in self.headers {
            response.headers_mut().insert(
                HeaderName::try_from(key.as_str()).unwrap(),
                HeaderValue::try_from(value.as_str()).unwrap(),
            );
        }
        response
    }
}
