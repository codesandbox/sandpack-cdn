use moka::future::Cache as MokaCache;
use std::{fmt, sync::Arc};
use tracing::info;

#[derive(Clone)]
pub struct Cache {
    cache: MokaCache<String, Arc<Vec<u8>>>,
}

impl Cache {
    pub async fn new(size: u64) -> Cache {
        let cache: MokaCache<String, Arc<Vec<u8>>> = MokaCache::builder()
            .weigher(|_key, value: &Arc<Vec<u8>>| -> u32 {
                value.len().try_into().unwrap_or(u32::MAX)
            })
            .max_capacity(size)
            .build();
        Cache { cache }
    }

    pub async fn store_value(&mut self, key: &str, data: Vec<u8>) {
        info!("Writing {} to the cache", key);
        self.cache.insert(String::from(key), Arc::new(data)).await;
    }

    pub async fn get_value(&self, key: &str) -> Option<Arc<Vec<u8>>> {
        info!("Reading {} from cache", key);
        self.cache.get(&String::from(key))
    }
}

impl fmt::Debug for Cache {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str("Cache")
    }
}
