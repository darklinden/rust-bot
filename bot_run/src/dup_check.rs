use crate::feature::{msg_segment_from_string, Feature, MessageContext};
use crate::redis_client::redis;
use async_trait::async_trait;
use bot_lib::structs::MessageSegment;
use chrono::TimeZone;
use redis::AsyncCommands;
use reqwest::header::HeaderValue;
use serde_json::Value;
use std::sync::RwLock;
use std::time::{SystemTime, UNIX_EPOCH};
use tokio::sync::OnceCell;

#[derive(Debug, Clone)]
struct LastWarned {
    hash_hex: String,
    user_id: i64,
    user_name: String,
    timestamp: u64,
}

pub struct DupCheckFeature {
    last_warned: RwLock<Option<LastWarned>>,
    vector_search_available: OnceCell<bool>,
}

impl DupCheckFeature {
    pub fn new() -> Self {
        Self {
            last_warned: RwLock::new(None),
            vector_search_available: OnceCell::new(),
        }
    }
}

impl Default for DupCheckFeature {
    fn default() -> Self {
        Self::new()
    }
}

const IMAGE_KEY_PREFIX: &str = "img:";
const EMOJI_KEY_PREFIX: &str = "emj:";
const IMAGE_TTL_SECS: u64 = 10 * 24 * 3600;
const EMOJI_TTL_SECS: u64 = 90 * 24 * 3600;

fn hash_to_f32_vec(hash: &imagehash::Hash) -> Vec<f32> {
    hash.bits
        .iter()
        .map(|&b| if b { 1.0 } else { 0.0 })
        .collect()
}

fn f32_vec_to_bytes(vec: &[f32]) -> Vec<u8> {
    let mut bytes = Vec::with_capacity(vec.len() * 4);
    for &val in vec {
        bytes.extend_from_slice(&val.to_le_bytes());
    }
    bytes
}

fn hex_hamming(a: &str, b: &str) -> u32 {
    let au = u64::from_str_radix(a, 16).unwrap_or(0);
    let bu = u64::from_str_radix(b, 16).unwrap_or(0);
    (au ^ bu).count_ones()
}

async fn redis_del(key: &str) {
    if let Ok(conn) = redis().await {
        let mut c = conn.clone();
        if let Err(e) = c.del::<_, ()>(key).await {
            log::warn!("Redis DEL error for key {}: {}", key, e);
        }
    }
}

pub fn format_timestamp(timestamp: u64) -> String {
    let ts_secs = (timestamp as i64) + 8 * 3600;
    let dt = chrono::Utc.timestamp_opt(ts_secs, 0).single();
    match dt {
        Some(dt) => dt.format("%Y/%m/%d %H:%M:%S").to_string(),
        None => "N/A".to_string(),
    }
}

pub async fn ensure_vector_indexes() -> bool {
    let mut conn = match redis().await {
        Ok(c) => c.clone(),
        Err(e) => {
            log::warn!("Redis unavailable: {}", e);
            return false;
        }
    };

    let create_idx = |idx_name: &str, prefix: &str| -> redis::Cmd {
        let mut cmd = redis::cmd("FT.CREATE");
        cmd.arg(idx_name)
            .arg("ON")
            .arg("HASH")
            .arg("PREFIX")
            .arg("1")
            .arg(prefix)
            .arg("SCHEMA")
            .arg("phash_vec")
            .arg("VECTOR")
            .arg("FLAT")
            .arg("6")
            .arg("TYPE")
            .arg("FLOAT32")
            .arg("DIM")
            .arg("64")
            .arg("DISTANCE_METRIC")
            .arg("L2");
        cmd
    };

    let idx_img = create_idx("idx:img_phash", IMAGE_KEY_PREFIX)
        .query_async::<()>(&mut conn)
        .await;
    if let Err(e) = idx_img {
        let msg = e.to_string();
        if !msg.contains("already exists") {
            log::warn!(
                "Redis Stack not available, falling back to KEYS scan (slower): {}",
                e
            );
            return false;
        }
    }

    let idx_emj = create_idx("idx:emj_phash", EMOJI_KEY_PREFIX)
        .query_async::<()>(&mut conn)
        .await;
    if let Err(e) = idx_emj {
        let msg = e.to_string();
        if !msg.contains("already exists") {
            log::warn!(
                "Redis Stack not available, falling back to KEYS scan (slower): {}",
                e
            );
            return false;
        }
    }
    true
}

#[async_trait]
impl Feature for DupCheckFeature {
    fn feature_name(&self) -> &str {
        "火星图出警: -emoji 标记上个出警为表情包"
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
        let vector_available = self
            .vector_search_available
            .get_or_init(ensure_vector_indexes)
            .await;

        let msg_type = msg["type"].as_str().unwrap_or("");

        if msg_type == "text" {
            let last = {
                let guard = self.last_warned.read().unwrap();
                guard.clone()
            };

            if let Some(lw) = last {
                if std::time::SystemTime::now()
                    .duration_since(std::time::UNIX_EPOCH)
                    .unwrap()
                    .as_secs()
                    > lw.timestamp + 60
                {
                    return Some(msg_segment_from_string(
                        "没有找到 1 分钟内最近被出警的图片，无法标记为表情包。".to_string(),
                    ));
                }
                let old_key = format!("{}{}", IMAGE_KEY_PREFIX, lw.hash_hex);
                let new_key = format!("{}{}", EMOJI_KEY_PREFIX, lw.hash_hex);

                let mut conn = match redis().await {
                    Ok(c) => c,
                    Err(e) => {
                        log::warn!("Redis unavailable: {}", e);
                        return None;
                    }
                }
                .clone();

                let fields: redis::RedisResult<std::collections::HashMap<String, Vec<u8>>> =
                    conn.hgetall(&old_key).await;
                if let Ok(map) = fields {
                    if !map.is_empty() {
                        let mut hset_cmd = redis::cmd("HSET");
                        hset_cmd.arg(&new_key);
                        for (k, v) in map.into_iter() {
                            hset_cmd.arg(k).arg(v);
                        }
                        let _ = hset_cmd.query_async::<()>(&mut conn).await;
                        let _ = conn.expire::<_, ()>(&new_key, EMOJI_TTL_SECS as i64).await;
                        redis_del(&old_key).await;
                    }
                }

                {
                    let mut guard = self.last_warned.write().unwrap();
                    *guard = None;
                }

                log::info!("Marked hash {} as emoji (key: {})", lw.hash_hex, new_key);
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
        let image_bytes = match client.get(&image_url)
            .header(reqwest::header::USER_AGENT, HeaderValue::from_static("Mozilla/5.0 (Windows NT 10.0; Win64; x64) AppleWebKit/537.36 (KHTML, like Gecko) Chrome/58.0.3029.110 Safari/537.36"))
            .header(reqwest::header::REFERER, HeaderValue::from_static("https://multimedia.nt.qq.com.cn/"))
            .header(reqwest::header::ACCEPT, HeaderValue::from_static("text/html,application/xhtml+xml,application/xml;q=0.9,image/avif,image/webp,image/apng,*/*;q=0.8,application/signed-exchange;v=b3;q=0.7"))
            .header(reqwest::header::ACCEPT_ENCODING, HeaderValue::from_static("gzip, deflate, br, zstd"))
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
                log::warn!("Failed to decode image dimensions: {}", e);
                return None;
            }
        };

        let (width, height) = {
            use image::GenericImageView;
            img.dimensions()
        };

        if width < 512 || height < 512 {
            log::debug!(
                "Image {}×{} below 512×512 threshold, skipping",
                width,
                height
            );
            return None;
        }

        let hash = imagehash::perceptual_hash(&img);
        let hash_hex = hash.to_string();
        let f32_vec = hash_to_f32_vec(&hash);
        let blob = f32_vec_to_bytes(&f32_vec);

        let mut conn = match redis().await {
            Ok(c) => c.clone(),
            Err(e) => {
                log::warn!("Redis unavailable: {}", e);
                return None;
            }
        };

        if *vector_available {
            let res: redis::RedisResult<Vec<redis::Value>> = redis::cmd("FT.SEARCH")
                .arg("idx:emj_phash")
                .arg("@phash_vec:[VECTOR_RANGE 2.45 $BLOB]")
                .arg("PARAMS")
                .arg("2")
                .arg("BLOB")
                .arg(&blob)
                .arg("DIALECT")
                .arg("2")
                .query_async(&mut conn)
                .await;

            if let Ok(vec) = res {
                if let Some(redis::Value::Int(count)) = vec.first() {
                    if *count > 0 && vec.len() >= 3 {
                        if let redis::Value::Array(ref fields) = vec[2] {
                            let mut found_hash_hex = String::new();

                            for i in (0..fields.len()).step_by(2) {
                                let key_str = match &fields[i] {
                                    redis::Value::BulkString(k) => {
                                        String::from_utf8_lossy(k).into_owned()
                                    }
                                    redis::Value::SimpleString(k) => k.to_string(),
                                    _ => continue,
                                };
                                let val_str = match &fields[i + 1] {
                                    redis::Value::BulkString(v) => {
                                        String::from_utf8_lossy(v).into_owned()
                                    }
                                    redis::Value::SimpleString(v) => v.to_string(),
                                    _ => continue,
                                };
                                if key_str == "hash_hex" {
                                    found_hash_hex = val_str;
                                    break;
                                }
                            }

                            if found_hash_hex == hash_hex {
                                log::debug!("Image exactly matches emoji hash, skipping");
                                return None;
                            }
                        }
                    }
                }
            }

            let res: redis::RedisResult<Vec<redis::Value>> = redis::cmd("FT.SEARCH")
                .arg("idx:img_phash")
                .arg("@phash_vec:[VECTOR_RANGE 2.45 $BLOB]=>{$YIELD_DISTANCE_AS: dist}")
                .arg("PARAMS")
                .arg("2")
                .arg("BLOB")
                .arg(&blob)
                .arg("SORTBY")
                .arg("dist")
                .arg("ASC")
                .arg("LIMIT")
                .arg("0")
                .arg("1")
                .arg("DIALECT")
                .arg("2")
                .query_async(&mut conn)
                .await;

            if let Ok(vec) = res {
                if let Some(redis::Value::Int(count)) = vec.first() {
                    if *count > 0 && vec.len() >= 3 {
                        if let redis::Value::Array(ref fields) = vec[2] {
                            let mut record_sender = String::from("N/A");
                            let mut record_id: i64 = 0;
                            let mut record_ts: u64 = 0;
                            let mut hit_count: u64 = 1;
                            let mut hit_key = String::new();
                            let mut record_hash_hex = String::new();

                            if let redis::Value::BulkString(ref k) = vec[1] {
                                hit_key = String::from_utf8_lossy(k).into_owned();
                            } else if let redis::Value::SimpleString(ref k) = vec[1] {
                                hit_key = k.to_string();
                            }

                            for i in (0..fields.len()).step_by(2) {
                                let key_str = match &fields[i] {
                                    redis::Value::BulkString(k) => {
                                        String::from_utf8_lossy(k).into_owned()
                                    }
                                    redis::Value::SimpleString(k) => k.to_string(),
                                    _ => continue,
                                };
                                let val_str = match &fields[i + 1] {
                                    redis::Value::BulkString(v) => {
                                        String::from_utf8_lossy(v).into_owned()
                                    }
                                    redis::Value::SimpleString(v) => v.to_string(),
                                    _ => continue,
                                };
                                match key_str.as_ref() {
                                    "sender" => record_sender = val_str,
                                    "user_id" => record_id = val_str.parse().unwrap_or(0),
                                    "timestamp" => record_ts = val_str.parse().unwrap_or(0),
                                    "count" => hit_count = val_str.parse().unwrap_or(1),
                                    "hash_hex" => record_hash_hex = val_str,
                                    _ => {}
                                }
                            }

                            if record_hash_hex != hash_hex {
                                log::debug!(
                                    "Vector match found but hash mismatch (stored: {}, current: {}), skipping",
                                    record_hash_hex, hash_hex
                                );
                                return None;
                            }

                            if hit_count < 10 {
                                let new_count = hit_count + 1;
                                let _ = redis::cmd("HSET")
                                    .arg(&hit_key)
                                    .arg("count")
                                    .arg(new_count.to_string())
                                    .query_async::<()>(&mut conn)
                                    .await;
                                let _ = conn.expire::<_, ()>(&hit_key, IMAGE_TTL_SECS as i64).await;

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

                                let name = context.display_name();
                                let response = format!(
                                    "出警！{} 又在发火星图了！图片由 {} ({}) 于 {} 发过，已经被发过了 {} 次！\n如果这是表情包，请发送 -emoji 来标记，后续不再出警。",
                                    name,
                                    record_sender,
                                    record_id,
                                    format_timestamp(record_ts),
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
                    }
                }
            }
        } else {
            let emoji_keys: Vec<String> = conn
                .keys(format!("{}*", EMOJI_KEY_PREFIX))
                .await
                .unwrap_or_default();
            for key in emoji_keys {
                if let Ok(stored_hex) = conn.hget::<_, _, String>(&key, "hash_hex").await {
                    if hex_hamming(&hash_hex, &stored_hex) <= 6 {
                        log::debug!("Image matches emoji key, skipping");
                        return None;
                    }
                }
            }

            let image_keys: Vec<String> = conn
                .keys(format!("{}*", IMAGE_KEY_PREFIX))
                .await
                .unwrap_or_default();
            for key in image_keys {
                if let Ok(stored_hex) = conn.hget::<_, _, String>(&key, "hash_hex").await {
                    if hex_hamming(&hash_hex, &stored_hex) <= 6 {
                        let fields: std::collections::HashMap<String, String> =
                            conn.hgetall(&key).await.unwrap_or_default();
                        let hit_count: u64 = fields
                            .get("count")
                            .and_then(|c| c.parse().ok())
                            .unwrap_or(1);

                        if hit_count < 10 {
                            let new_count = hit_count + 1;
                            let _ = redis::cmd("HSET")
                                .arg(&key)
                                .arg("count")
                                .arg(new_count.to_string())
                                .query_async::<()>(&mut conn)
                                .await;
                            let _ = conn.expire::<_, ()>(&key, IMAGE_TTL_SECS as i64).await;

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

                            let name = context.display_name();
                            let record_sender = fields
                                .get("sender")
                                .cloned()
                                .unwrap_or_else(|| "N/A".to_string());
                            let record_id: i64 = fields
                                .get("user_id")
                                .and_then(|id| id.parse().ok())
                                .unwrap_or(0);
                            let record_ts: u64 = fields
                                .get("timestamp")
                                .and_then(|ts| ts.parse().ok())
                                .unwrap_or(0);

                            let response = format!(
                                "出警！{} 又在发火星图了！图片由 {} ({}) 于 {} 发过，已经被发过了 {} 次！\n如果这是表情包，请发送 -emoji 来标记，后续不再出警。",
                                name,
                                record_sender,
                                record_id,
                                format_timestamp(record_ts),
                                new_count
                            );

                            return Some(msg_segment_from_string(response));
                        } else {
                            return None;
                        }
                    }
                }
            }
        }

        let now = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .unwrap_or_default()
            .as_secs();

        let new_key = format!("{}{}", IMAGE_KEY_PREFIX, hash_hex);
        let _ = redis::cmd("HSET")
            .arg(&new_key)
            .arg("phash_vec")
            .arg(&blob)
            .arg("hash_hex")
            .arg(&hash_hex)
            .arg("count")
            .arg("1")
            .arg("user_id")
            .arg(context.user_id.to_string())
            .arg("sender")
            .arg(context.display_name())
            .arg("timestamp")
            .arg(now.to_string())
            .query_async::<()>(&mut conn)
            .await;

        let _ = conn.expire::<_, ()>(&new_key, IMAGE_TTL_SECS as i64).await;

        None
    }
}
