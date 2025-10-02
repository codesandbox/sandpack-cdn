use super::{
    error::{ChangeStreamError, ChangeStreamResult},
    types::changes::ChangesPage,
};
use reqwest::{Client, Method};
use std::{collections::HashMap, time::Duration};

/// The stream for the `_changes` endpoint.
///
/// This is returned from [Database::changes].
pub struct ChangesStream {
    client: Client,
    last_seq: serde_json::Value,
    params: HashMap<String, String>,
    pub limit: usize,
}

impl ChangesStream {
    /// Create a new changes stream.
    pub fn new(limit: usize, last_seq: serde_json::Value) -> Self {
        let mut params = HashMap::new();
        params.insert("limit".to_string(), limit.to_string());
        let client = Client::builder()
            .timeout(Duration::from_secs(120))
            .build()
            .unwrap();
        Self {
            params,
            last_seq,
            limit,
            client,
        }
    }

    pub fn should_wait(&self, last_result_count: usize) -> bool {
        last_result_count < (self.limit / 2)
    }

    pub async fn fetch_next(&mut self) -> ChangeStreamResult<ChangesPage> {
        self.params
            .insert("since".to_string(), self.last_seq.to_string());
        let request = self
            .client
            .request(Method::GET, "https://replicate.npmjs.com/registry/_changes")
            .header("npm-replication-opt-in", "true")
            .query(&self.params);
        // println!("{:?}", request);
        let res = request.send().await?;
        if !res.status().is_success() {
            return Err(ChangeStreamError::new(
                res.status().into(),
                Some(res.text().await.unwrap_or_else(|_| String::from(""))),
            ));
        }
        let page: ChangesPage = res.json().await?;
        self.last_seq = page.last_seq.into();
        Ok(page)
    }
}
