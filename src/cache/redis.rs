use crate::app_error::ServerError;

use redis::{aio::MultiplexedConnection, Client};

#[derive(Clone)]
pub struct RedisCache {
    conn: MultiplexedConnection,
}

impl RedisCache {
    pub async fn new(redis_url: &'static str) -> Result<RedisCache, ServerError> {
        let client = Client::open(redis_url)?;
        let conn = client.get_multiplexed_async_connection().await?;
        Ok(RedisCache { conn })
    }

    pub async fn store_value(
        &mut self,
        key: &str,
        data: &str,
    ) -> Result<(), redis::RedisError> {
        let mut write_cmd = redis::Cmd::new();
        let set_res: String = write_cmd
            .arg("SET")
            .arg(key)
            .arg(data)
            .query_async(&mut self.conn)
            .await?;
        Ok(())
    }

    pub async fn get_value(&mut self, key: &str) -> Result<String, redis::RedisError> {
        let mut get_cmd = redis::Cmd::new();
        let result: String = get_cmd
            .arg("GET")
            .arg(key)
            .query_async(&mut self.conn)
            .await?;
        Ok(result)
    }
}
