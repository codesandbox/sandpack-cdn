use crate::app_error::ServerError;
use reqwest::{Client, ClientBuilder};
use std::time::Duration;

pub fn get_client(timeout_secs: u64) -> Result<Client, ServerError> {
    let client_builder = ClientBuilder::new()
        .timeout(Duration::new(timeout_secs, 0))
        .deflate(true)
        .gzip(true)
        .brotli(true);
    let client = client_builder.build()?;
    Ok(client)
}
