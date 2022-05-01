use std::sync::Arc;
use tracing::info;

use crate::app_error::ServerError;

use super::memory::InMemoryCache;

#[derive(Debug, Clone)]
pub struct LayeredCache {
    memory: InMemoryCache,
}

impl LayeredCache {
    pub async fn try_init(in_memory_size: u64) -> Result<LayeredCache, ServerError> {
        let memory = InMemoryCache::new(in_memory_size);
        Ok(LayeredCache { memory })
    }

    async fn store_memory_value(&mut self, key: &str, data: &str) {
        self.memory.store_value(key, data).await;
    }

    pub async fn store_value(&mut self, key: &str, data: &str) -> Result<(), ServerError> {
        info!("Writing {} to the cache", key);
        self.store_memory_value(key, data).await;
        Ok(())
    }

    fn get_memory_value(&self, key: &str) -> Option<Arc<String>> {
        self.memory.get_value(key)
    }

    pub async fn get_value(&self, key: &str) -> Option<Arc<String>> {
        if let Some(found_in_memory) = self.get_memory_value(key) {
            info!("{} found in the memory cache", key);
            return Some(found_in_memory);
        }

        None
    }
}
