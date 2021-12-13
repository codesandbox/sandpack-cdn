use lru::LruCache;

pub struct InMemoryCache {
    cache: LruCache<String, String>,
}

impl InMemoryCache {
    pub fn new(size: usize) -> InMemoryCache {
        let cache: LruCache<String, String> = LruCache::new(size);
        InMemoryCache { cache }
    }

    pub fn store_value(&mut self, key: &str, data: &str) {
        self.cache.put(String::from(key), String::from(data));
    }

    pub fn get_value(&mut self, key: &str) -> Option<String> {
        let val = self.cache.get(&String::from(key));
        val.map(|v| v.clone())
    }
}
