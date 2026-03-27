use redis::aio::ConnectionManager;
use redis::Client;
use std::env;
use tokio::sync::OnceCell;

pub fn get_redis_url() -> String {
    env::var("REDIS_URL").unwrap_or_else(|_| "redis://127.0.0.1:6379".to_string())
}

static REDIS_MGR: OnceCell<Result<ConnectionManager, String>> = OnceCell::const_new();

async fn init_redis() -> Result<ConnectionManager, String> {
    let redis_url = get_redis_url();
    let client = Client::open(redis_url.clone()).map_err(|e| {
        let msg = format!("Redis URL '{}' invalid: {}", redis_url, e);
        log::warn!("{}", msg);
        msg
    })?;
    client.get_connection_manager().await.map_err(|e| {
        let msg = format!("Redis connection failed: {}", e);
        log::warn!("{}", msg);
        msg
    })
}

pub async fn redis() -> Result<&'static ConnectionManager, &'static str> {
    let r = REDIS_MGR.get_or_init(init_redis).await;
    match r {
        Ok(m) => Ok(m),
        Err(e) => Err(e),
    }
}
