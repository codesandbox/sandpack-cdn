use reqwest_middleware::{ClientBuilder, ClientWithMiddleware};
use reqwest_retry::{policies::ExponentialBackoff, RetryTransientMiddleware};
use std::time::Duration;

pub fn get_client(timeout_secs: u64) -> ClientWithMiddleware {
    let retry_policy = ExponentialBackoff::builder().build_with_max_retries(3);

    let client_builder = reqwest::ClientBuilder::new()
        .timeout(Duration::new(timeout_secs, 0))
        .deflate(true)
        .gzip(true)
        .brotli(true);
    let base_client = client_builder
        .build()
        .expect("reqwest::ClientBuilder::build()");

    ClientBuilder::new(base_client)
        .with(RetryTransientMiddleware::new_with_policy(retry_policy))
        .build()
}
