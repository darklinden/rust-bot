pub mod migration;

use sqlx::postgres::PgPoolOptions;
use sqlx::PgPool;
use std::env;
use tokio::sync::OnceCell;

static PG_POOL: OnceCell<PgPool> = OnceCell::const_new();

pub async fn pg() -> &'static PgPool {
    PG_POOL
        .get_or_init(|| async {
            let url = env::var("DATABASE_URL")
                .unwrap_or_else(|_| "postgres://bot:bot123@localhost:5432/bot".into());
            PgPoolOptions::new()
                .max_connections(10)
                .connect(&url)
                .await
                .expect("Failed to connect to PostgreSQL")
        })
        .await
}

pub async fn init() -> Result<(), Box<dyn std::error::Error>> {
    let pool = pg().await;
    migration::run(pool).await?;
    log::info!("Database migrations completed");

    // Seed default persona if none exists
    let count: i64 = sqlx::query_scalar("SELECT COUNT(*) FROM personas")
        .fetch_one(pool)
        .await?;
    if count == 0 {
        let default_prompt = std::env::var("PERSONA_DEFAULT_PROMPT").unwrap_or_else(|_| {
            "你是 QQ 群聊里的一个暖心助手。请用友好、简洁的中文回复群友的消息，语气自然亲切。".to_string()
        });
        sqlx::query("INSERT INTO personas (name, system_prompt, is_default) VALUES ('默认', $1, true)")
            .bind(&default_prompt)
            .execute(pool)
            .await?;
        log::info!("Seeded default persona");
    }
    Ok(())
}
