use crate::db;
use crate::feature::{msg_segment_from_string, Feature, MessageContext};
use async_trait::async_trait;
use bot_lib::structs::MessageSegment;
use chrono::TimeZone;
use reqwest::header::HeaderValue;
use serde_json::Value;
use sqlx::Row;
use std::sync::RwLock;
use std::time::{SystemTime, UNIX_EPOCH};

#[derive(Debug, Clone)]
struct LastWarned {
    hash_hex: String,
    user_id: i64,
    user_name: String,
    timestamp: u64,
}

pub struct DupCheckFeature {
    last_warned: RwLock<Option<LastWarned>>,
}

impl DupCheckFeature {
    pub fn new() -> Self {
        tokio::spawn(async {
            let pool = db::pg().await;
            let mut interval = tokio::time::interval(tokio::time::Duration::from_secs(600));
            loop {
                interval.tick().await;
                if let Err(e) = sqlx::query("DELETE FROM image_hashes WHERE expires_at < NOW()")
                    .execute(pool)
                    .await
                {
                    log::warn!("dup_check cleanup error: {}", e);
                }
            }
        });

        Self {
            last_warned: RwLock::new(None),
        }
    }
}

impl Default for DupCheckFeature {
    fn default() -> Self {
        Self::new()
    }
}


fn hash_to_f32_vec(hash: &imagehash::Hash) -> Vec<f32> {
    hash.bits
        .iter()
        .map(|&b| if b { 1.0 } else { 0.0 })
        .collect()
}

pub fn format_timestamp(timestamp: u64) -> String {
    let ts_secs = (timestamp as i64) + 8 * 3600;
    let dt = chrono::Utc.timestamp_opt(ts_secs, 0).single();
    match dt {
        Some(dt) => dt.format("%Y/%m/%d %H:%M:%S").to_string(),
        None => "N/A".to_string(),
    }
}


impl DupCheckFeature {
    pub fn feature_id() -> &'static str {
        "dup_check"
    }

    pub fn feature_name() -> &'static str {
        "火星图出警: -emoji 标记上个出警为表情包"
    }
}

#[async_trait]
impl Feature for DupCheckFeature {
    fn feature_id(&self) -> &str {
        DupCheckFeature::feature_id()
    }

    fn feature_name(&self) -> &str {
        DupCheckFeature::feature_name()
    }

    fn check_command(&self, msg: &Value) -> bool {
        match msg["type"].as_str() {
            Some("image") => true,
            Some("text") => {
                let text = msg["data"]["text"].as_str().unwrap_or("").trim();
                text == "-emoji"
            }
            _ => false,
        }
    }

    async fn deal_with_message(
        &self,
        context: &MessageContext,
        msg: &Value,
    ) -> Option<MessageSegment> {
        let msg_type = msg["type"].as_str().unwrap_or("");

        if msg_type == "text" {
            let last = {
                let guard = self.last_warned.read().unwrap();
                guard.clone()
            };

            if let Some(lw) = last {
                if SystemTime::now()
                    .duration_since(UNIX_EPOCH)
                    .unwrap()
                    .as_secs()
                    > lw.timestamp + 60
                {
                    return Some(msg_segment_from_string(
                        "没有找到 1 分钟内最近被出警的图片，无法标记为表情包。".to_string(),
                    ));
                }
                let pool = db::pg().await;
                let _ = sqlx::query(
                    "UPDATE image_hashes SET hash_type = 'emoji', expires_at = NOW() + INTERVAL '90 days' WHERE hash_hex = $1",
                )
                .bind(&lw.hash_hex)
                .execute(pool)
                .await;

                {
                    let mut guard = self.last_warned.write().unwrap();
                    *guard = None;
                }

                log::info!("Marked hash {} as emoji", lw.hash_hex);
                return Some(msg_segment_from_string(format!(
                    "已将 {} ({}) 刚被出警的图片标记为表情包，后续不再出警。",
                    lw.user_name, lw.user_id
                )));
            } else {
                log::info!("-emoji called but no recent warning to mark");
            }

            return None;
        }

        let image_url = match msg["data"]["url"].as_str() {
            Some(u) if !u.is_empty() => u.to_string(),
            _ => {
                log::debug!("Image segment has no URL, skipping");
                return None;
            }
        };

        let client = reqwest::Client::new();
        let image_bytes = match client
            .get(&image_url)
            .header(
                reqwest::header::USER_AGENT,
                HeaderValue::from_static(
                    "Mozilla/5.0 (Windows NT 10.0; Win64; x64) AppleWebKit/537.36",
                ),
            )
            .header(
                reqwest::header::REFERER,
                HeaderValue::from_static("https://multimedia.nt.qq.com.cn/"),
            )
            .header(
                reqwest::header::ACCEPT,
                HeaderValue::from_static(
                    "text/html,application/xhtml+xml,application/xml;q=0.9,image/avif,image/webp,image/apng,*/*;q=0.8",
                ),
            )
            .header(
                reqwest::header::ACCEPT_ENCODING,
                HeaderValue::from_static("gzip, deflate, br, zstd"),
            )
            .send()
            .await
        {
            Ok(resp) => match resp.bytes().await {
                Ok(b) => b,
                Err(e) => {
                    log::warn!("Failed to read image bytes: {}", e);
                    return None;
                }
            },
            Err(e) => {
                log::warn!("Failed to download image from {}: {}", image_url, e);
                return None;
            }
        };

        let img = match image::load_from_memory(&image_bytes) {
            Ok(i) => i,
            Err(e) => {
                log::warn!("Failed to decode image: {}", e);
                return None;
            }
        };

        let (width, height) = {
            use image::GenericImageView;
            img.dimensions()
        };

        if width < 512 || height < 512 {
            log::debug!(
                "Image {}x{} below 512x512 threshold, skipping",
                width,
                height
            );
            return None;
        }

        let hash = imagehash::perceptual_hash(&img);
        let hash_hex = hash.to_string();
        let f32_vec = hash_to_f32_vec(&hash);

        let pool = db::pg().await;

        // Check emoji hashes — skip if similar to a known emoji
        let emoji_row = sqlx::query(
            r#"SELECT hash_hex, phash_vec <-> $1 AS dist
               FROM image_hashes
               WHERE hash_type = 'emoji' AND expires_at > NOW()
                 AND phash_vec <-> $1 < 2.45
               LIMIT 1"#,
        )
        .bind(&f32_vec)
        .fetch_optional(pool)
        .await;

        if let Ok(Some(row)) = emoji_row {
            let stored_hash: String = row.get("hash_hex");
            if stored_hash == hash_hex {
                log::debug!("Image exactly matches emoji hash, skipping");
                return None;
            }
        }

        // Check image hashes — look for duplicates
        let img_row = sqlx::query(
            r#"SELECT hash_hex, count, user_id, sender, timestamp
               FROM image_hashes
               WHERE hash_type = 'image' AND expires_at > NOW()
                 AND phash_vec <-> $1 < 2.45
               ORDER BY phash_vec <-> $1
               LIMIT 1"#,
        )
        .bind(&f32_vec)
        .fetch_optional(pool)
        .await;

        if let Ok(Some(row)) = img_row {
            let stored_hash: String = row.get("hash_hex");
            if stored_hash != hash_hex {
                log::debug!(
                    "Vector match found but hash mismatch, skipping"
                );
                return None;
            }

            let hit_count: i32 = row.get("count");
            if hit_count < 10 {
                let new_count = hit_count + 1;
                let _ = sqlx::query(
                    "UPDATE image_hashes SET count = count + 1, expires_at = NOW() + INTERVAL '10 days' WHERE hash_hex = $1",
                )
                .bind(&hash_hex)
                .execute(pool)
                .await;

                {
                    let mut guard = self.last_warned.write().unwrap();
                    *guard = Some(LastWarned {
                        hash_hex: hash_hex.clone(),
                        user_id: context.user_id,
                        user_name: context.display_name(),
                        timestamp: SystemTime::now()
                            .duration_since(UNIX_EPOCH)
                            .unwrap()
                            .as_secs(),
                    });
                }

                let record_sender: String = row.get("sender");
                let record_id: i64 = row.get("user_id");
                let record_ts: i64 = row.get("timestamp");

                let name = context.display_name();
                let response = format!(
                    "出警！{} 又在发火星图了！图片由 {} ({}) 于 {} 发过，已经被发过了 {} 次！\n如果这是表情包，请发送 -emoji 来标记，后续不再出警。",
                    name,
                    record_sender,
                    record_id,
                    format_timestamp(record_ts as u64),
                    new_count
                );

                log::info!(
                    "Duplicate image detected for user {} (count: {})",
                    context.user_id,
                    new_count
                );

                return Some(msg_segment_from_string(response));
            } else {
                log::debug!("Duplicate found but count >= 10, not responding");
                return None;
            }
        }

        // No duplicate found — store as new entry
        let now = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .unwrap_or_default()
            .as_secs();
        let sender = context.display_name();

        let _ = sqlx::query(
            r#"INSERT INTO image_hashes (hash_hex, hash_type, phash_vec, count, user_id, sender, timestamp, expires_at)
               VALUES ($1, 'image', $2, 1, $3, $4, $5, NOW() + INTERVAL '10 days')
               ON CONFLICT (hash_hex) DO NOTHING"#,
        )
        .bind(&hash_hex)
        .bind(&f32_vec)
        .bind(context.user_id)
        .bind(sender)
        .bind(now as i64)
        .execute(pool)
        .await;

        None
    }
}
