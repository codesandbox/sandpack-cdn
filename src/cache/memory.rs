use moka::future::Cache;
use std::{fmt, sync::Arc};

#[derive(Clone)]
pub struct InMemoryCache {
    cache: Cache<String, Arc<String>>,
}

impl InMemoryCache {
    pub fn new(size: u64) -> InMemoryCache {
        let cache: Cache<String, Arc<String>> = Cache::new(size);
        InMemoryCache { cache }
    }

    pub async fn store_value(&mut self, key: &str, data: &str) {
        self.cache
            .insert(String::from(key), Arc::new(String::from(data)))
            .await;
    }

    pub fn get_value(&self, key: &str) -> Option<Arc<String>> {
        self.cache.get(&String::from(key))
    }
}

impl fmt::Debug for InMemoryCache {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str("InMemoryCache")
    }
}
