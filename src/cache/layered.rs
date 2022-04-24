use parking_lot::Mutex;
use std::sync::Arc;

use crate::app_error::ServerError;

use super::memory::InMemoryCache;

#[derive(Debug, Clone)]
pub struct LayeredCache {
    memory: Arc<Mutex<InMemoryCache>>,
}

impl LayeredCache {
    pub async fn try_init(in_memory_size: u64) -> Result<LayeredCache, ServerError> {
        let memory = Arc::new(Mutex::new(InMemoryCache::new(in_memory_size)));
        Ok(LayeredCache { memory })
    }

    fn store_memory_value(&self, key: &str, data: &str) {
        self.memory.lock().store_value(key, data);
    }

    pub async fn store_value(&self, key: &str, data: &str) -> Result<(), ServerError> {
        println!("Writing {} to the cache", key);
        self.store_memory_value(key, data);
        Ok(())
    }

    fn get_memory_value(&self, key: &str) -> Option<String> {
        self.memory.lock().get_value(key)
    }

    pub async fn get_value(&self, key: &str) -> Option<String> {
        if let Some(found_in_memory) = self.get_memory_value(key) {
            println!("{} found in the memory cache", key);
            return Some(found_in_memory);
        }

        None
    }
}
