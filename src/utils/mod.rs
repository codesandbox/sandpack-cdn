use base64::decode as decode_base64;
use lazy_static::lazy_static;
use regex::Regex;

use crate::app_error::ServerError;

pub mod request;
pub mod test_utils;

lazy_static! {
    static ref VERSION_RE: Regex = Regex::new("^(\\d+)\\((.*)\\)$").unwrap();
    static ref LATEST_VERSION: u64 = 2;
}

pub fn decode_req_part(part: &str) -> Result<String, ServerError> {
    let decoded = decode_base64(part)?;
    let str_value = std::str::from_utf8(&decoded)?;

    if let Some(parts) = VERSION_RE.captures(str_value) {
        if let Some(version_match) = parts.get(1) {
            let version = version_match.as_str().parse::<u64>()?;
            if version > *LATEST_VERSION {
                return Err(ServerError::InvalidCDNVersion);
            }
        }

        if let Some(content_match) = parts.get(2) {
            return Ok(String::from(content_match.as_str()));
        }
    }

    // Fallback to no version
    Ok(String::from(str_value))
}
