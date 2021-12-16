use std::sync::{Arc, Mutex};

use crate::app_error::ServerError;
use crate::cache::memory::InMemoryCache;

#[derive(Clone)]
pub struct LayeredCache {
    memory: Arc<Mutex<InMemoryCache>>,
}

impl LayeredCache {
    pub async fn try_init(
        in_memory_size: usize,
    ) -> Result<LayeredCache, ServerError> {
        let memory = Arc::new(Mutex::new(InMemoryCache::new(in_memory_size)));
        Ok(LayeredCache { memory })
    }

    fn store_memory_value(&self, key: &str, data: &str) {
        self.memory
            .lock()
            .expect("could not get lock for mem-cache")
            .store_value(key, data);
    }

    pub async fn store_value(&self, key: &str, data: &str) -> Result<(), ServerError> {
        println!("Writing {} to the cache", key);
        self.store_memory_value(key, data);
        Ok(())
    }

    fn get_memory_value(&self, key: &str) -> Option<String> {
        self.memory
            .lock()
            .expect("could not get lock for mem-cache")
            .get_value(key)
    }

    pub async fn get_value(&self, key: &str) -> Option<String> {
        if let Some(found_in_memory) = self.get_memory_value(key) {
            println!("{} found in the memory cache", key);
            return Some(found_in_memory);
        }

        None
    }
}
