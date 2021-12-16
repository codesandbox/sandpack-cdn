use crate::app_error::ServerError;

use redis::{aio::ConnectionManager, Client};

pub struct RedisCache {
    client: Client,
    conn_manager: ConnectionManager,
}

impl RedisCache {
    pub async fn try_init(redis_url: &'static str) -> Result<RedisCache, ServerError> {
        let client: Client = Client::open(redis_url)?;
        let conn_manager: ConnectionManager = client.get_tokio_connection_manager().await?;
        Ok(RedisCache {
            client,
            conn_manager,
        })
    }

    pub async fn store_value(&mut self, key: &str, data: &str) -> Result<(), redis::RedisError> {
        let mut write_cmd = redis::Cmd::new();
        let set_res: String = write_cmd
            .arg("SET")
            .arg(key)
            .arg(data)
            .query_async(&mut self.conn_manager)
            .await?;
        Ok(())
    }

    pub async fn get_value(&mut self, key: &str) -> Result<String, redis::RedisError> {
        let mut get_cmd = redis::Cmd::new();
        let result: String = get_cmd
            .arg("GET")
            .arg(key)
            .query_async(&mut self.conn_manager)
            .await?;
        Ok(result)
    }
}
