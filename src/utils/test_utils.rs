use std::env;
use std::fs;

use crate::app_error::ServerError;

#[allow(dead_code)]
pub fn read_fixture(fixture_name: &str) -> Result<String, ServerError> {
    let fixture_path = env::current_dir()?.join(fixture_name);
    let fixture_content: String = fs::read_to_string(fixture_path)?;
    Ok(fixture_content)
}
