use crate::app_error::ServerError;
use crate::cache::memory::InMemoryCache;
use crate::cache::redis::RedisCache;

pub struct LayeredCache {
    redis: RedisCache,
    memory: InMemoryCache,
}

impl LayeredCache {
    pub async fn new(
        redis_url: &'static str,
        in_memory_size: usize,
    ) -> Result<LayeredCache, ServerError> {
        let redis = RedisCache::new(redis_url).await?;
        let memory = InMemoryCache::new(in_memory_size);
        Ok(LayeredCache { redis, memory })
    }

    pub async fn store_value(
        &mut self,
        key: &str,
        data: &str,
        ttl_option: Option<u64>,
    ) -> Result<(), ServerError> {
        println!("Writing {} to the cache", key);
        self.memory.store_value(key, data);
        match self.redis.store_value(key, data, ttl_option).await {
            Err(err) => println("Storing value to cache failed: {:?}"),
            _ => {}
        }
        Ok(())
    }

    pub async fn get_value(&mut self, key: &str) -> Option<String> {
        if let Some(found_in_memory) = self.memory.get_value(key) {
            println!("{} found in the cache", key);
            return Some(found_in_memory);
        }
        if let Ok(value) = self.redis.get_value(key).await {
            println!("{} found in the cache", key);
            self.memory.store_value(key, value.as_str());
            return Some(value);
        }
        None
    }
}
