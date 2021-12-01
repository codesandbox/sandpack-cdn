use crate::app_error::ServerError;
use crate::file_utils;

use std::env;

pub fn read_fixture(fixture_name: &str) -> Result<String, ServerError> {
    let fixture_path = env::current_dir()?.join(fixture_name);
    let fixture_content: String = file_utils::read_text_file(fixture_path)?;
    Ok(fixture_content)
}
