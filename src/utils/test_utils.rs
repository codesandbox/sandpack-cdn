use crate::app_error::ServerError;

use std::env;
use std::fs;

pub fn read_fixture(fixture_name: &str) -> Result<String, ServerError> {
    let fixture_path = env::current_dir()?.join(fixture_name);
    let fixture_content: String = fs::read_to_string(fixture_path)?;
    Ok(fixture_content)
}
