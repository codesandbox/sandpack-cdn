use super::{
    error::{ChangeStreamError, ChangeStreamResult},
    types::changes::ChangesPage,
};
use reqwest::{Client, Method};
use std::{collections::HashMap, sync::Arc};

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
    client: Arc<Client>,
    params: HashMap<String, String>,
}

impl ChangesStream {
    /// Create a new changes stream.
    pub fn new(last_seq: serde_json::Value) -> Self {
        let client = Arc::new(Client::new());
        let mut params = HashMap::new();
        params.insert("feed".to_string(), "longpoll".to_string());
        params.insert("include_docs".to_string(), "true".to_string());
        params.insert("timeout".to_string(), COUCH_MAX_TIMEOUT.to_string());
        params.insert("limit".to_string(), 50.to_string());
        Self {
            client,
            params,
            last_seq,
        }
    }

    pub async fn fetch_next(&mut self) -> ChangeStreamResult<ChangesPage> {
        let mut params = self.params.clone();
        params.insert("since".to_string(), self.last_seq.to_string());
        let request = self
            .client
            .request(Method::GET, "https://replicate.npmjs.com/registry/_changes")
            .query(&params);
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
