use crate::app_error::ServerError;

use std::path::PathBuf;
use std::fs;

pub fn read_text_file(file_path: PathBuf) -> Result<String, ServerError> {
    let fixture_content: String = String::from_utf8_lossy(&fs::read(file_path)?).parse()?;
    Ok(fixture_content)
}
