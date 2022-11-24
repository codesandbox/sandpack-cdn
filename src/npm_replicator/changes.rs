use super::{
    error::{ChangeStreamError, ChangeStreamResult},
    types::changes::ChangesPage,
};
use reqwest::{Client, Method};
use std::collections::HashMap;

/// The max timeout value for longpoll/continous HTTP requests
/// that CouchDB supports (see [1]).
///
/// [1]: https://docs.couchdb.org/en/stable/api/database/changes.html
const COUCH_MAX_TIMEOUT: usize = 60000;

/// The stream for the `_changes` endpoint.
///
/// This is returned from [Database::changes].
pub struct ChangesStream {
    last_seq: serde_json::Value,
    params: HashMap<String, String>,
    pub limit: usize,
}

impl ChangesStream {
    /// Create a new changes stream.
    pub fn new(limit: usize, last_seq: serde_json::Value) -> Self {
        let mut params = HashMap::new();
        params.insert("feed".to_string(), "longpoll".to_string());
        params.insert("include_docs".to_string(), "true".to_string());
        params.insert("timeout".to_string(), COUCH_MAX_TIMEOUT.to_string());
        params.insert("limit".to_string(), limit.to_string());
        Self {
            params,
            last_seq,
            limit,
        }
    }

    pub fn get_client(&self) -> Client {
        Client::new()
    }

    pub fn should_wait(&self, last_result_count: usize) -> bool {
        last_result_count < (self.limit / 2)
    }

    pub async fn fetch_next(&mut self) -> ChangeStreamResult<ChangesPage> {
        let client = self.get_client();
        self.params
            .insert("since".to_string(), self.last_seq.to_string());
        let request = client
            .request(Method::GET, "https://replicate.npmjs.com/registry/_changes")
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
