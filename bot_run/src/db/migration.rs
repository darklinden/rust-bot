use sqlx::PgPool;

pub async fn run(pool: &PgPool) -> Result<(), Box<dyn std::error::Error>> {
    sqlx::query("CREATE EXTENSION IF NOT EXISTS vector")
        .execute(pool)
        .await?;

    sqlx::query(
        r#"CREATE TABLE IF NOT EXISTS image_hashes (
            hash_hex TEXT PRIMARY KEY,
            hash_type TEXT NOT NULL DEFAULT 'image',
            phash_vec vector(64),
            count INTEGER NOT NULL DEFAULT 1,
            user_id BIGINT NOT NULL,
            sender TEXT NOT NULL,
            timestamp BIGINT NOT NULL,
            created_at TIMESTAMPTZ NOT NULL DEFAULT NOW(),
            expires_at TIMESTAMPTZ NOT NULL
        )"#,
    )
    .execute(pool)
    .await?;

    sqlx::query(
        r#"CREATE INDEX IF NOT EXISTS idx_hashes_vec
            ON image_hashes USING hnsw (phash_vec vector_l2_ops)"#,
    )
    .execute(pool)
    .await?;

    sqlx::query(
        r#"CREATE INDEX IF NOT EXISTS idx_hashes_expires
            ON image_hashes (expires_at)"#,
    )
    .execute(pool)
    .await?;

    sqlx::query(
        r#"CREATE TABLE IF NOT EXISTS cache_entries (
            key TEXT PRIMARY KEY,
            value JSONB NOT NULL,
            expires_at TIMESTAMPTZ
        )"#,
    )
    .execute(pool)
    .await?;

    sqlx::query(
        r#"CREATE INDEX IF NOT EXISTS idx_cache_expires
            ON cache_entries (expires_at)"#,
    )
    .execute(pool)
    .await?;

    sqlx::query(
        r#"CREATE TABLE IF NOT EXISTS cron_tasks (
            id BIGSERIAL PRIMARY KEY,
            target_time BIGINT NOT NULL,
            user_id BIGINT NOT NULL,
            group_id BIGINT,
            nickname TEXT NOT NULL,
            card TEXT NOT NULL,
            message TEXT NOT NULL,
            created_at TIMESTAMPTZ NOT NULL DEFAULT NOW()
        )"#,
    )
    .execute(pool)
    .await?;

    sqlx::query(
        r#"CREATE INDEX IF NOT EXISTS idx_cron_target
            ON cron_tasks (target_time)"#,
    )
    .execute(pool)
    .await?;

    sqlx::query(
        r#"CREATE TABLE IF NOT EXISTS personas (
            id UUID PRIMARY KEY DEFAULT gen_random_uuid(),
            name TEXT NOT NULL UNIQUE,
            system_prompt TEXT NOT NULL,
            is_default BOOLEAN NOT NULL DEFAULT false,
            created_at TIMESTAMPTZ NOT NULL DEFAULT NOW()
        )"#,
    )
    .execute(pool)
    .await?;

    sqlx::query(
        r#"CREATE TABLE IF NOT EXISTS group_personas (
            group_id TEXT PRIMARY KEY,
            persona_id UUID NOT NULL REFERENCES personas(id)
        )"#,
    )
    .execute(pool)
    .await?;

    sqlx::query(
        r#"CREATE TABLE IF NOT EXISTS knowledge_chunks (
            id UUID PRIMARY KEY DEFAULT gen_random_uuid(),
            source TEXT NOT NULL,
            text TEXT NOT NULL,
            embedding vector(4096),
            created_at TIMESTAMPTZ NOT NULL DEFAULT NOW()
        )"#,
    )
    .execute(pool)
    .await?;

    sqlx::query(
        r#"CREATE INDEX IF NOT EXISTS idx_knowledge_embedding
            ON knowledge_chunks USING hnsw (embedding vector_cosine_ops)"#,
    )
    .execute(pool)
    .await?;

    sqlx::query(
        r#"CREATE TABLE IF NOT EXISTS conversations (
            id UUID PRIMARY KEY DEFAULT gen_random_uuid(),
            user_id TEXT NOT NULL,
            group_id TEXT,
            persona_id UUID REFERENCES personas(id),
            role TEXT NOT NULL,
            content TEXT NOT NULL,
            created_at TIMESTAMPTZ NOT NULL DEFAULT NOW()
        )"#,
    )
    .execute(pool)
    .await?;

    sqlx::query(
        r#"CREATE INDEX IF NOT EXISTS idx_conv_user
            ON conversations (user_id, group_id, created_at DESC)"#,
    )
    .execute(pool)
    .await?;

    Ok(())
}
