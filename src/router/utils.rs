use crate::app_error::ServerError;

pub fn decode_base64(part: &str) -> Result<String, ServerError> {
    let decoded = base64_simd::Base64::STANDARD
        .decode_to_boxed_bytes(part.as_bytes())
        .map_err(|_e| ServerError::Base64DecodingError())?;
    let val =
        String::from_utf8(decoded.to_vec()).map_err(|_e| ServerError::Base64DecodingError())?;
    Ok(val)
}
