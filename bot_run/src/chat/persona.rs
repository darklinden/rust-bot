use sqlx::PgPool;
use sqlx::Row;

#[derive(Debug, Clone)]
pub struct Persona {
    pub id: uuid::Uuid,
    pub name: String,
    pub system_prompt: String,
    pub is_default: bool,
}

pub async fn get_default(pool: &PgPool) -> Result<Option<Persona>, String> {
    let row = sqlx::query("SELECT id, name, system_prompt, is_default FROM personas WHERE is_default = true LIMIT 1")
        .fetch_optional(pool)
        .await
        .map_err(|e| format!("persona get_default: {}", e))?;

    Ok(row.map(|r| Persona {
        id: r.get("id"),
        name: r.get("name"),
        system_prompt: r.get("system_prompt"),
        is_default: r.get("is_default"),
    }))
}

pub async fn get_for_group(pool: &PgPool, group_id: &str) -> Result<Option<Persona>, String> {
    let row = sqlx::query(
        r#"SELECT p.id, p.name, p.system_prompt, p.is_default
           FROM personas p
           JOIN group_personas gp ON p.id = gp.persona_id
           WHERE gp.group_id = $1
           LIMIT 1"#,
    )
    .bind(group_id)
    .fetch_optional(pool)
    .await
    .map_err(|e| format!("persona get_for_group: {}", e))?;

    Ok(row.map(|r| Persona {
        id: r.get("id"),
        name: r.get("name"),
        system_prompt: r.get("system_prompt"),
        is_default: r.get("is_default"),
    }))
}

pub async fn create(pool: &PgPool, name: &str, system_prompt: &str) -> Result<Persona, String> {
    let row = sqlx::query(
        r#"INSERT INTO personas (name, system_prompt) VALUES ($1, $2)
           ON CONFLICT (name) DO UPDATE SET system_prompt = $2
           RETURNING id, name, system_prompt, is_default"#,
    )
    .bind(name)
    .bind(system_prompt)
    .fetch_one(pool)
    .await
    .map_err(|e| format!("persona create: {}", e))?;

    Ok(Persona {
        id: row.get("id"),
        name: row.get("name"),
        system_prompt: row.get("system_prompt"),
        is_default: row.get("is_default"),
    })
}

pub async fn list_all(pool: &PgPool) -> Result<Vec<Persona>, String> {
    let rows = sqlx::query("SELECT id, name, system_prompt, is_default FROM personas ORDER BY name")
        .fetch_all(pool)
        .await
        .map_err(|e| format!("persona list_all: {}", e))?;

    Ok(rows
        .iter()
        .map(|r| Persona {
            id: r.get("id"),
            name: r.get("name"),
            system_prompt: r.get("system_prompt"),
            is_default: r.get("is_default"),
        })
        .collect())
}

pub async fn set_default(pool: &PgPool, id: uuid::Uuid) -> Result<(), String> {
    sqlx::query("UPDATE personas SET is_default = false")
        .execute(pool)
        .await
        .map_err(|e| format!("persona set_default clear: {}", e))?;
    sqlx::query("UPDATE personas SET is_default = true WHERE id = $1")
        .bind(id)
        .execute(pool)
        .await
        .map_err(|e| format!("persona set_default set: {}", e))?;
    Ok(())
}

pub async fn set_group_persona(
    pool: &PgPool,
    group_id: &str,
    persona_id: uuid::Uuid,
) -> Result<(), String> {
    sqlx::query(
        r#"INSERT INTO group_personas (group_id, persona_id) VALUES ($1, $2)
           ON CONFLICT (group_id) DO UPDATE SET persona_id = $2"#,
    )
    .bind(group_id)
    .bind(persona_id)
    .execute(pool)
    .await
    .map_err(|e| format!("set_group_persona: {}", e))?;
    Ok(())
}

/// Seed a default persona if the table is empty.
pub async fn seed_default(pool: &PgPool) -> Result<(), String> {
    let count: i64 = sqlx::query_scalar("SELECT COUNT(*) FROM personas")
        .fetch_one(pool)
        .await
        .map_err(|e| format!("seed_default count: {}", e))?;

    if count == 0 {
        let default_prompt = std::env::var("PERSONA_DEFAULT_PROMPT").unwrap_or_else(|_| {
            "你是 QQ 群聊里的一个暖心助手。请用友好、简洁的中文回复群友的消息，语气自然亲切。".to_string()
        });
        create(pool, "默认", &default_prompt).await?;
        let row = sqlx::query_scalar::<_, uuid::Uuid>("SELECT id FROM personas WHERE name = '默认' LIMIT 1")
            .fetch_one(pool)
            .await
            .map_err(|e| format!("seed_default get id: {}", e))?;
        sqlx::query("UPDATE personas SET is_default = true WHERE id = $1")
            .bind(row)
            .execute(pool)
            .await
            .map_err(|e| format!("seed_default set default: {}", e))?;
    }
    Ok(())
}
