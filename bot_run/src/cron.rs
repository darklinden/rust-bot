use std::sync::{Arc, Mutex};
use std::time::Duration;

use async_trait::async_trait;
use bot_lib::structs::MessageSegment;
use chrono::{Local, NaiveDateTime, TimeZone};
use redis::AsyncCommands;
use serde::{Deserialize, Serialize};
use serde_json::Value;
use tokio::sync::mpsc;

use crate::feature::{msg_segment_from_string, Feature, MessageContext};
use crate::redis_client::redis;

pub struct CronResult {
    pub context: MessageContext,
    pub message: String,
}

pub type CronSender = mpsc::Sender<CronResult>;

const CRON_QUEUE_KEY: &str = "cron:queue";
const CRON_TASK_PREFIX: &str = "cron:task:";
const CRON_TTL_SECS: i64 = 7 * 24 * 3600;

#[derive(Debug, Clone, Serialize, Deserialize)]
struct CronTask {
    id: u64,
    target_time: i64,
    user_id: i64,
    group_id: Option<i64>,
    nickname: String,
    card: String,
    message: String,
}

#[derive(Clone)]
struct ScheduledTask {
    id: u64,
    target_time: NaiveDateTime,
    context: MessageContext,
    message: String,
}

impl From<CronTask> for ScheduledTask {
    fn from(t: CronTask) -> Self {
        let context = MessageContext {
            self_id: 0,
            user_id: t.user_id,
            group_id: t.group_id,
            message_id: 0,
            message: Vec::new(),
            raw_message: String::new(),
            nickname: t.nickname,
            card: t.card,
        };
        let target_time = Local
            .timestamp_opt(t.target_time, 0)
            .single()
            .map(|dt| dt.naive_local())
            .unwrap_or_else(|| {
                chrono::DateTime::from_timestamp(t.target_time, 0)
                    .unwrap()
                    .with_timezone(&Local)
                    .naive_local()
            });
        ScheduledTask {
            id: t.id,
            target_time,
            context,
            message: t.message,
        }
    }
}

pub struct CronFeature {
    next_id: Arc<Mutex<u64>>,
    tasks: Arc<Mutex<Vec<ScheduledTask>>>,
    _sender: CronSender,
}

impl CronFeature {
    pub fn new(sender: CronSender) -> Self {
        let next_id: Arc<Mutex<u64>> = Arc::new(Mutex::new(1));
        let tasks: Arc<Mutex<Vec<ScheduledTask>>> = Arc::new(Mutex::new(Vec::new()));

        let tasks_bg = tasks.clone();
        let sender_bg = sender.clone();
        let next_id_bg = next_id.clone();

        let tasks_load = tasks.clone();
        let next_id_load = next_id.clone();
        tokio::spawn(async move {
            let load_result = Self::redis_tasks_load().await;
            let redis_tasks = match load_result {
                Ok(t) => t,
                Err(e) => {
                    log::warn!("Cron: failed to load tasks from redis: {}", e);
                    return;
                }
            };

            let now = Local::now().naive_local();
            let mut expired_ids: Vec<u64> = Vec::new();
            let mut to_insert: Vec<ScheduledTask> = Vec::new();
            let mut max_id: u64 = 0;

            for rt in redis_tasks {
                let target = Local
                    .timestamp_opt(rt.target_time, 0)
                    .single()
                    .map(|dt| dt.naive_local())
                    .unwrap_or_else(|| {
                        Local
                            .timestamp_opt(rt.target_time, 0)
                            .single()
                            .map(|dt| dt.naive_local())
                            .unwrap()
                    });
                if target <= now {
                    expired_ids.push(rt.id);
                } else {
                    let st: ScheduledTask = rt.into();
                    let st_id = st.id;
                    log::info!(
                        "Cron: loaded task #{} scheduled for {}",
                        st_id,
                        st.target_time.format("%Y-%m-%d %H:%M")
                    );
                    to_insert.push(st);
                    if st_id > max_id {
                        max_id = st_id;
                    }
                }
            }

            for task in to_insert {
                tasks_load.lock().unwrap().push(task);
            }

            for id in expired_ids {
                log::info!("Cron: task #{} already expired on load, removing", id);
                if let Err(e) = Self::redis_task_remove(id).await {
                    log::warn!("Cron: failed to remove expired task: {}", e);
                }
            }

            if max_id > 0 {
                let mut id_guard = next_id_load.lock().unwrap();
                *id_guard = max_id + 1;
            }
        });

        tokio::spawn(async move {
            let mut interval = tokio::time::interval(Duration::from_secs(1));
            loop {
                interval.tick().await;
                let now = Local::now().naive_local();

                let due_ids: Vec<u64> = {
                    let guard = tasks_bg.lock().unwrap();
                    guard
                        .iter()
                        .filter(|t| t.target_time <= now)
                        .map(|t| t.id)
                        .collect()
                };

                if due_ids.is_empty() {
                    continue;
                }

                let due: Vec<ScheduledTask> = {
                    let mut guard = tasks_bg.lock().unwrap();
                    let mut collected = Vec::new();
                    guard.retain(|t| {
                        if due_ids.contains(&t.id) {
                            collected.push(t.clone());
                            false
                        } else {
                            true
                        }
                    });
                    collected
                };

                for task in due {
                    log::info!("Cron: task #{} is due, sending reminder", task.id);
                    let task_id = task.id;
                    let sender_clone = sender_bg.clone();
                    let _ = next_id_bg.lock().unwrap().checked_add(0);

                    tokio::spawn(async move {
                        if let Err(e) = Self::redis_task_remove(task_id).await {
                            log::warn!(
                                "Cron: failed to remove task #{} from redis: {}",
                                task_id,
                                e
                            );
                        }

                        let result = CronResult {
                            context: task.context,
                            message: task.message,
                        };
                        if sender_clone.send(result).await.is_err() {
                            log::error!("Cron: failed to send result to main.rs");
                        }
                    });
                }
            }
        });

        Self {
            next_id,
            tasks,
            _sender: sender,
        }
    }

    pub fn feature_id() -> &'static str {
        "cron"
    }

    pub fn feature_name() -> &'static str {
        "定时器: -cron 今天|明天 HH:MM 做什么"
    }

    fn parse_command(text: &str) -> Result<(NaiveDateTime, String), String> {
        let rest = text.strip_prefix("-cron ").unwrap_or("").trim();
        if rest.is_empty() {
            return Err("用法: -cron 今天|明天 HH:MM 做什么".to_string());
        }

        let parts: Vec<&str> = rest.split_whitespace().collect();
        if parts.len() != 3 {
            return Err("用法: -cron 今天|明天 HH:MM 做什么".to_string());
        }

        let time_parts = parts[0..2].to_vec();
        let message = parts[2].trim().to_string();
        if message.is_empty() {
            return Err("请指定要提醒的内容".to_string());
        }

        let day_str =
            if !time_parts.is_empty() && (time_parts[0] == "今天" || time_parts[0] == "明天") {
                let d = time_parts[0];
                d
            } else {
                "今天"
            };

        let time_str = if !time_parts.is_empty() {
            time_parts[1]
        } else {
            return Err("请指定时间，格式: HH:MM".to_string());
        };

        let hour_minute: Vec<&str> = time_str.split(':').collect();
        if hour_minute.len() != 2 {
            return Err("时间格式错误，请使用 HH:MM".to_string());
        }

        let hour: u32 = hour_minute[0]
            .parse()
            .map_err(|_| "小时格式错误".to_string())?;
        let minute: u32 = hour_minute[1]
            .parse()
            .map_err(|_| "分钟格式错误".to_string())?;

        if hour > 23 || minute > 59 {
            return Err("时间范围错误: 00:00 ~ 23:59".to_string());
        }

        let now = Local::now();
        let target_date = match day_str {
            "今天" => now.date_naive(),
            "明天" => now.date_naive() + chrono::Duration::days(1),
            _ => return Err("请使用 今天 或 明天".to_string()),
        };

        let target_time = target_date.and_hms_opt(hour, minute, 0).unwrap();
        if day_str == "今天" && target_time <= now.naive_local() {
            return Err("指定的时间已经过了".to_string());
        }

        Ok((target_time, message))
    }

    async fn redis_tasks_load() -> Result<Vec<CronTask>, String> {
        let mut conn = match redis().await {
            Ok(c) => c.clone(),
            Err(e) => return Err(e.to_string()),
        };

        let task_ids: Vec<u64> = redis::cmd("ZRANGEBYSCORE")
            .arg(CRON_QUEUE_KEY)
            .arg(0)
            .arg(i64::MAX)
            .query_async(&mut conn)
            .await
            .map_err(|e| format!("Redis ZRANGEBYSCORE error: {}", e))?;

        let mut tasks = Vec::new();
        for id in task_ids {
            let key = format!("{}{}", CRON_TASK_PREFIX, id);
            let data: Option<String> = conn
                .hget(&key, "data")
                .await
                .map_err(|e| format!("Redis HGET error: {}", e))?;

            if let Some(json) = data {
                match serde_json::from_str::<CronTask>(&json) {
                    Ok(task) => tasks.push(task),
                    Err(e) => {
                        log::warn!(
                            "Cron: failed to parse task #{} from redis: {}, removing",
                            id,
                            e
                        );
                        let _: () = conn
                            .zrem(CRON_QUEUE_KEY, id)
                            .await
                            .map_err(|e| format!("Redis ZREM error: {}", e))?;
                        let _: () = conn
                            .del(&key)
                            .await
                            .map_err(|e| format!("Redis DEL error: {}", e))?;
                    }
                }
            }
        }
        Ok(tasks)
    }

    async fn redis_task_save(task: &CronTask) -> Result<(), String> {
        let mut conn = match redis().await {
            Ok(c) => c.clone(),
            Err(e) => return Err(e.to_string()),
        };

        let json =
            serde_json::to_string(task).map_err(|e| format!("JSON serialize error: {}", e))?;

        let key = format!("{}{}", CRON_TASK_PREFIX, task.id);
        let _: () = conn
            .hset(&key, "data", &json)
            .await
            .map_err(|e| format!("Redis HSET error: {}", e))?;
        let _: () = conn
            .expire(&key, CRON_TTL_SECS)
            .await
            .map_err(|e| format!("Redis EXPIRE error: {}", e))?;

        let _: () = redis::cmd("ZADD")
            .arg(CRON_QUEUE_KEY)
            .arg(task.target_time)
            .arg(task.id)
            .query_async(&mut conn)
            .await
            .map_err(|e| format!("Redis ZADD error: {}", e))?;

        Ok(())
    }

    async fn redis_task_remove(task_id: u64) -> Result<(), String> {
        let mut conn = match redis().await {
            Ok(c) => c.clone(),
            Err(e) => return Err(e.to_string()),
        };

        let key = format!("{}{}", CRON_TASK_PREFIX, task_id);
        let _: () = conn
            .del(&key)
            .await
            .map_err(|e| format!("Redis DEL error: {}", e))?;
        let _: () = conn
            .zrem(CRON_QUEUE_KEY, task_id)
            .await
            .map_err(|e| format!("Redis ZREM error: {}", e))?;
        Ok(())
    }
}

#[async_trait]
impl Feature for CronFeature {
    fn feature_id(&self) -> &str {
        CronFeature::feature_id()
    }

    fn feature_name(&self) -> &str {
        CronFeature::feature_name()
    }

    fn check_command(&self, msg: &Value) -> bool {
        let text = msg
            .get("data")
            .and_then(|d| d.get("text"))
            .and_then(|t| t.as_str())
            .unwrap_or("");
        text.starts_with("-cron ")
    }

    async fn deal_with_message(
        &self,
        context: &MessageContext,
        msg: &Value,
    ) -> Option<MessageSegment> {
        let text = msg
            .get("data")
            .and_then(|d| d.get("text"))
            .and_then(|t| t.as_str())?;

        let (target_time, message) = match Self::parse_command(text) {
            Ok(v) => v,
            Err(e) => return Some(msg_segment_from_string(e)),
        };

        let time_str = target_time.format("%Y-%m-%d %H:%M").to_string();
        let display_name = context.display_name();

        let task_id = {
            let mut id_guard = self.next_id.lock().unwrap();
            let id = *id_guard;
            *id_guard += 1;
            id
        };

        let target_ts = target_time.and_utc().timestamp();

        let redis_task = CronTask {
            id: task_id,
            target_time: target_ts,
            user_id: context.user_id,
            group_id: context.group_id,
            nickname: context.nickname.clone(),
            card: context.card.clone(),
            message: message.clone(),
        };

        if let Err(e) = Self::redis_task_save(&redis_task).await {
            log::warn!("Cron: failed to save task #{} to redis: {}", task_id, e);
        }

        let task = ScheduledTask {
            id: task_id,
            target_time,
            context: context.clone(),
            message: message.clone(),
        };
        self.tasks.lock().unwrap().push(task);

        let response = format!(
            "{} 好的，已为你设置定时任务 #{}\n将在 {} 提醒你: {}",
            display_name, task_id, time_str, message
        );

        Some(msg_segment_from_string(response))
    }
}
