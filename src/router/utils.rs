use crate::app_error::ServerError;

pub fn decode_base64(part: &str) -> Result<String, ServerError> {
    let decoded = base64_simd::STANDARD
        .decode_to_vec(part.as_bytes())
        .map_err(|_e| ServerError::Base64DecodingError())?;
    let val = String::from_utf8(decoded).map_err(|_e| ServerError::Base64DecodingError())?;
    Ok(val)
}
