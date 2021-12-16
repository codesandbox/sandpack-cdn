use std::sync::{Arc, Mutex};

use crate::app_error::ServerError;
use crate::cache::memory::InMemoryCache;
use crate::cache::redis::RedisCache;

#[derive(Clone)]
pub struct LayeredCache {
    redis: Arc<tokio::sync::Mutex<RedisCache>>,
    memory: Arc<Mutex<InMemoryCache>>,
}

impl LayeredCache {
    pub async fn try_init(
        redis_url: &'static str,
        in_memory_size: usize,
    ) -> Result<LayeredCache, ServerError> {
        let redis = Arc::new(tokio::sync::Mutex::new(
            RedisCache::try_init(redis_url).await?,
        ));
        let memory = Arc::new(Mutex::new(InMemoryCache::new(in_memory_size)));
        Ok(LayeredCache { redis, memory })
    }

    fn store_memory_value(&self, key: &str, data: &str) {
        self.memory
            .lock()
            .expect("could not get lock for mem-cache")
            .store_value(key, data);
    }

    async fn store_redis_value(&self, key: &str, data: &str) -> Result<(), ServerError> {
        let mut redis = self.redis.lock().await;
        redis.store_value(key, data).await?;
        Ok(())
    }

    pub async fn store_value(&self, key: &str, data: &str) -> Result<(), ServerError> {
        println!("Writing {} to the cache", key);
        self.store_memory_value(key, data);
        // match self.store_redis_value(key, data).await {
        //     Err(err) => println!("Storing value to cache failed: {:?}", err),
        //     _ => {}
        // }
        Ok(())
    }

    fn get_memory_value(&self, key: &str) -> Option<String> {
        self.memory
            .lock()
            .expect("could not get lock for mem-cache")
            .get_value(key)
    }

    async fn get_redis_value(&self, key: &str) -> Option<String> {
        let mut redis = self.redis.lock().await;
        if let Ok(value) = redis.get_value(key).await {
            return Some(value);
        } else {
            return None;
        }
    }

    pub async fn get_value(&self, key: &str) -> Option<String> {
        if let Some(found_in_memory) = self.get_memory_value(key) {
            println!("{} found in the memory cache", key);
            return Some(found_in_memory);
        }

        // if let Some(value) = self.get_redis_value(key).await {
        //     println!("{} found in the redis cache", key);
        //     self.store_memory_value(key, value.as_str());
        //     return Some(value);
        // }

        None
    }
}
