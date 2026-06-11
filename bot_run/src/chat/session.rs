use sqlx::PgPool;
use sqlx::Row;

#[derive(Debug, Clone)]
pub struct Message {
    pub role: String,
    pub content: String,
}

pub async fn recent(
    pool: &PgPool,
    user_id: &str,
    group_id: Option<&str>,
    limit: u32,
) -> Result<Vec<Message>, String> {
    let rows = sqlx::query(
        r#"SELECT role, content FROM conversations
           WHERE user_id = $1 AND ($2::text IS NULL OR group_id = $2)
           ORDER BY created_at DESC
           LIMIT $3"#,
    )
    .bind(user_id)
    .bind(group_id)
    .bind(limit as i64)
    .fetch_all(pool)
    .await
    .map_err(|e| format!("session recent: {}", e))?;

    let mut messages: Vec<Message> = rows
        .iter()
        .map(|r| Message {
            role: r.get("role"),
            content: r.get("content"),
        })
        .collect();
    messages.reverse();
    Ok(messages)
}

pub async fn save(
    pool: &PgPool,
    user_id: &str,
    group_id: Option<&str>,
    persona_id: uuid::Uuid,
    role: &str,
    content: &str,
) -> Result<(), String> {
    sqlx::query(
        r#"INSERT INTO conversations (user_id, group_id, persona_id, role, content)
           VALUES ($1, $2, $3, $4, $5)"#,
    )
    .bind(user_id)
    .bind(group_id)
    .bind(persona_id)
    .bind(role)
    .bind(content)
    .execute(pool)
    .await
    .map_err(|e| format!("session save: {}", e))?;
    Ok(())
}

pub async fn prune(pool: &PgPool, days: i32) -> Result<u64, String> {
    let result = sqlx::query("DELETE FROM conversations WHERE created_at < NOW() - INTERVAL '1 day' * $1")
        .bind(days)
        .execute(pool)
        .await
        .map_err(|e| format!("session prune: {}", e))?;
    Ok(result.rows_affected())
}
