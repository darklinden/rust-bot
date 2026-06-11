use sqlx::PgPool;
use sqlx::Row;

pub struct Chunk {
    pub id: uuid::Uuid,
    pub source: String,
    pub text: String,
    pub similarity: f64,
}

pub struct KnowledgeBase;

impl KnowledgeBase {
    pub async fn search(
        pool: &PgPool,
        query_vec: &[f32],
        limit: u32,
    ) -> Result<Vec<Chunk>, String> {
        let rows = sqlx::query(
            r#"SELECT id, source, text, 1 - (embedding <=> $1) AS similarity
               FROM knowledge_chunks
               ORDER BY embedding <=> $1
               LIMIT $2"#,
        )
        .bind(query_vec)
        .bind(limit as i64)
        .fetch_all(pool)
        .await
        .map_err(|e| format!("knowledge search error: {}", e))?;

        let chunks = rows
            .iter()
            .map(|row| Chunk {
                id: row.get("id"),
                source: row.get("source"),
                text: row.get("text"),
                similarity: row.get("similarity"),
            })
            .collect();

        Ok(chunks)
    }

    pub async fn insert(
        pool: &PgPool,
        source: &str,
        text: &str,
        vec: &[f32],
    ) -> Result<(), String> {
        sqlx::query(
            r#"INSERT INTO knowledge_chunks (source, text, embedding)
               VALUES ($1, $2, $3)"#,
        )
        .bind(source)
        .bind(text)
        .bind(vec)
        .execute(pool)
        .await
        .map_err(|e| format!("knowledge insert error: {}", e))?;
        Ok(())
    }

    pub async fn import_markdown(
        pool: &PgPool,
        embed: &super::embedding::EmbeddingClient,
        source: &str,
        content: &str,
    ) -> Result<usize, String> {
        let chunks = chunk_text(content);
        let texts: Vec<String> = chunks.iter().map(|c| c.to_string()).collect();
        let embeddings = embed.embed_batch(&texts, 4).await?;

        let mut count = 0;
        for (text, vec) in texts.iter().zip(embeddings.iter()) {
            Self::insert(pool, source, text, vec).await?;
            count += 1;
        }
        Ok(count)
    }
}

const CHUNK_SIZE: usize = 6000;
const CHUNK_OVERLAP: usize = 1000;

fn chunk_text(text: &str) -> Vec<String> {
    let chars: Vec<char> = text.chars().collect();
    let total = chars.len();

    if total <= CHUNK_SIZE {
        return vec![text.to_string()];
    }

    let mut chunks = Vec::new();
    let mut start = 0;

    while start < total {
        let end = (start + CHUNK_SIZE).min(total);
        let mut chunk: String = chars[start..end].iter().collect();

        if end < total {
            let overlap_start = if chunk.chars().count() > CHUNK_OVERLAP {
                chunk.chars().count() - CHUNK_OVERLAP
            } else {
                0
            };
            let byte_pos = chunk
                .char_indices()
                .rev()
                .find(|&(i, ch)| {
                    let char_idx = chunk[..i].chars().count();
                    char_idx >= overlap_start && (ch == '。' || ch == '\n' || ch == '；')
                })
                .map(|(i, ch)| i + ch.len_utf8());

            if let Some(pos) = byte_pos {
                chunk.truncate(pos);
            }
        }

        let chunk_len = chunk.chars().count();
        chunks.push(chunk);

        let advance = if chunk_len > CHUNK_OVERLAP {
            chunk_len - CHUNK_OVERLAP
        } else {
            chunk_len
        };
        start += advance;
    }

    chunks
}
