use anyhow::{Context, Result};
use clap::Parser;
use indicatif::{ProgressBar, ProgressStyle};
use sqlx::postgres::PgPoolOptions;
use std::path::Path;

#[derive(Parser)]
#[command(name = "kb-tool", about = "知识库入库工具")]
struct Cli {
    #[arg(short, long, default_value = ".")]
    dir: String,

    #[arg(long, default_value = "http://127.0.0.1:8080", env = "EMBEDDING_URL")]
    em_api: String,

    #[arg(long, default_value = "", env = "EMBEDDING_API_KEY")]
    em_key: String,

    #[arg(long, default_value = "Qwen3-Embedding-4B")]
    em_model: String,

    #[arg(
        long,
        default_value = "postgres://bot:bot123@localhost:5432/bot",
        env = "DATABASE_URL"
    )]
    pg_url: String,

    #[arg(long, default_value = "800")]
    chunk_size: usize,

    #[arg(long, default_value = "100")]
    chunk_overlap: usize,

    #[arg(long, default_value = "4")]
    concurrency: usize,

    #[arg(long)]
    dry_run: bool,
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

fn collect_md_files(dir: &str) -> Result<Vec<(String, String)>> {
    let mut files = Vec::new();
    let root = Path::new(dir);
    if !root.exists() {
        anyhow::bail!("Directory not found: {}", dir);
    }
    for entry in walkdir::WalkDir::new(dir)
        .into_iter()
        .filter_map(|e| e.ok())
        .filter(|entry| {
            entry
                .path()
                .extension()
                .map(|ext| ext == "md")
                .unwrap_or(false)
        })
    {
        let path = entry.path();
        let content = std::fs::read_to_string(path)
            .with_context(|| format!("Failed to read: {}", path.display()))?;
        let rel_path = path
            .strip_prefix(root)
            .unwrap_or(path)
            .display()
            .to_string();
        files.push((rel_path, content));
    }
    Ok(files)
}

async fn embed_text(client: &reqwest::Client, api: &str, model: &str, text: &str) -> Result<Vec<f32>> {
    let body = serde_json::json!({
        "input": text,
        "model": model,
        "encoding_format": "float",
    });
    let resp = client
        .post(format!("{}/v1/embeddings", api))
        .json(&body)
        .send()
        .await?;
    let json: serde_json::Value = resp.json().await?;
    let embedding = json["data"][0]["embedding"]
        .as_array()
        .context("No embedding in response")?
        .iter()
        .map(|v| v.as_f64().unwrap_or(0.0) as f32)
        .collect();
    Ok(embedding)
}

#[tokio::main]
async fn main() -> Result<()> {
    let cli = Cli::parse();

    println!("Scanning directory: {}", cli.dir);
    let files = collect_md_files(&cli.dir)?;
    if files.is_empty() {
        println!("No .md files found.");
        return Ok(());
    }
    println!("Found {} markdown file(s).", files.len());

    let mut all_chunks: Vec<(String, String)> = Vec::new();
    for (path, content) in &files {
        let chunks = chunk_text(content);
        for chunk in chunks {
            all_chunks.push((path.clone(), chunk));
        }
    }
    println!("Total chunks: {}", all_chunks.len());

    if cli.dry_run {
        println!("[Dry run] Would import {} chunks. Exiting.", all_chunks.len());
        return Ok(());
    }

    let pool = PgPoolOptions::new()
        .max_connections(5)
        .connect(&cli.pg_url)
        .await?;

    sqlx::query("CREATE EXTENSION IF NOT EXISTS vector")
        .execute(&pool)
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
    .execute(&pool)
    .await?;

    let client = reqwest::Client::new();
    let pb = ProgressBar::new(all_chunks.len() as u64);
    pb.set_style(
        ProgressStyle::default_bar()
            .template("{spinner} [{elapsed_precise}] [{bar:40}] {pos}/{len} ({eta})")
            .unwrap(),
    );

    let mut handles = Vec::new();
    for (source, text) in all_chunks {
        let client = client.clone();
        let api = cli.em_api.clone();
        let model = cli.em_model.clone();
        let pool = pool.clone();
        let pb = pb.clone();

        let handle = tokio::spawn(async move {
            let vec = embed_text(&client, &api, &model, &text).await?;
            sqlx::query(
                "INSERT INTO knowledge_chunks (source, text, embedding) VALUES ($1, $2, $3)",
            )
            .bind(&source)
            .bind(&text)
            .bind(&vec)
            .execute(&pool)
            .await?;
            pb.inc(1);
            Ok::<_, anyhow::Error>(())
        });
        handles.push(handle);

        if handles.len() >= cli.concurrency {
            let (result, _, remaining) = futures_util::future::select_all(handles).await;
            result??;
            handles = remaining;
        }
    }

    for handle in handles {
        handle.await??;
    }

    pb.finish_with_message("Import complete!");
    println!("All chunks imported successfully.");
    Ok(())
}
