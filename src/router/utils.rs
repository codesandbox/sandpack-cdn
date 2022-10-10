use lazy_static::lazy_static;
use regex::Regex;

use crate::app_error::ServerError;

lazy_static! {
    static ref VERSION_RE: Regex = Regex::new("^(\\d+)\\((.*)\\)$").unwrap();
    static ref LATEST_VERSION: u64 = 5;
}

pub fn decode_base64(part: &str) -> Result<String, ServerError> {
    let decoded = base64_simd::Base64::STANDARD
        .decode_to_boxed_bytes(part.as_bytes())
        .map_err(|_e| ServerError::Base64DecodingError())?;
    let val =
        String::from_utf8(decoded.to_vec()).map_err(|_e| ServerError::Base64DecodingError())?;
    Ok(val)
}

pub fn decode_req_part(part: &str) -> Result<(u64, String), ServerError> {
    let decoded = decode_base64(part)?;

    if let Some(parts) = VERSION_RE.captures(&decoded) {
        if let Some(version_match) = parts.get(1) {
            let version = version_match.as_str().parse::<u64>()?;
            if version > *LATEST_VERSION {
                return Err(ServerError::InvalidCDNVersion);
            }

            if let Some(content_match) = parts.get(2) {
                return Ok((version, String::from(content_match.as_str())));
            }
        }
    }

    // Fallback to no version
    Ok((1, String::from(decoded)))
}
