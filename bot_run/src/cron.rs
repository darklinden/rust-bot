use std::sync::{Arc, Mutex};
use std::time::Duration;

use async_trait::async_trait;
use bot_lib::structs::MessageSegment;
use chrono::{Local, NaiveDateTime, TimeZone};
use serde_json::Value;
use sqlx::Row;
use tokio::sync::mpsc;

use crate::db;
use crate::feature::{msg_segment_from_string, Feature, MessageContext};

pub struct CronResult {
    pub context: MessageContext,
    pub message: String,
}

pub type CronSender = mpsc::Sender<CronResult>;

const CRON_TTL_DAYS: i64 = 7;

#[derive(Debug, Clone)]
struct CronTask {
    id: i64,
    target_time: i64,
    user_id: i64,
    group_id: Option<i64>,
    nickname: String,
    card: String,
    message: String,
}

#[derive(Clone)]
struct ScheduledTask {
    id: i64,
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
    tasks: Arc<Mutex<Vec<ScheduledTask>>>,
    _sender: CronSender,
}

impl CronFeature {
    pub fn new(sender: CronSender) -> Self {
        let tasks: Arc<Mutex<Vec<ScheduledTask>>> = Arc::new(Mutex::new(Vec::new()));

        let tasks_bg = tasks.clone();
        let sender_bg = sender.clone();

        let tasks_load = tasks.clone();
        tokio::spawn(async move {
            match Self::pg_tasks_load().await {
                Ok(loaded) => {
                    let now = Local::now().naive_local();
                    let mut expired_ids: Vec<i64> = Vec::new();
                    let mut to_insert: Vec<ScheduledTask> = Vec::new();

                    for rt in loaded {
                        let target = Local
                            .timestamp_opt(rt.target_time, 0)
                            .single()
                            .map(|dt| dt.naive_local());
                        if let Some(target) = target {
                            if target <= now {
                                expired_ids.push(rt.id);
                            } else {
                                let st: ScheduledTask = rt.into();
                                log::info!(
                                    "Cron: loaded task #{} scheduled for {}",
                                    st.id,
                                    st.target_time.format("%Y-%m-%d %H:%M")
                                );
                                to_insert.push(st);
                            }
                        }
                    }

                    for task in to_insert {
                        tasks_load.lock().unwrap().push(task);
                    }

                    for id in expired_ids {
                        log::info!("Cron: task #{} already expired on load, removing", id);
                        if let Err(e) = Self::pg_task_remove(id).await {
                            log::warn!("Cron: failed to remove expired task: {}", e);
                        }
                    }
                }
                Err(e) => {
                    log::warn!("Cron: failed to load tasks from pg: {}", e);
                }
            }
        });

        tokio::spawn(async move {
            let mut interval = tokio::time::interval(Duration::from_secs(1));
            loop {
                interval.tick().await;
                let now = Local::now().naive_local();

                let due_ids: Vec<i64> = {
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

                    tokio::spawn(async move {
                        if let Err(e) = Self::pg_task_remove(task_id).await {
                            log::warn!(
                                "Cron: failed to remove task #{} from pg: {}",
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
                time_parts[0]
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

    async fn pg_tasks_load() -> Result<Vec<CronTask>, String> {
        let pool = db::pg().await;
        let now = Local::now().naive_local();
        let cutoff = now - chrono::Duration::days(CRON_TTL_DAYS);

        let rows = sqlx::query(
            "SELECT id, target_time, user_id, group_id, nickname, card, message FROM cron_tasks WHERE target_time > $1 ORDER BY target_time",
        )
        .bind(cutoff.and_utc().timestamp())
        .fetch_all(pool)
        .await
        .map_err(|e| format!("pg cron load error: {}", e))?;

        let mut tasks = Vec::new();
        for row in rows {
            tasks.push(CronTask {
                id: row.get("id"),
                target_time: row.get("target_time"),
                user_id: row.get("user_id"),
                group_id: row.get("group_id"),
                nickname: row.get("nickname"),
                card: row.get("card"),
                message: row.get("message"),
            });
        }
        Ok(tasks)
    }

    async fn pg_task_save(task: &CronTask) -> Result<i64, String> {
        let pool = db::pg().await;
        let row = sqlx::query(
            r#"INSERT INTO cron_tasks (target_time, user_id, group_id, nickname, card, message)
               VALUES ($1, $2, $3, $4, $5, $6)
               RETURNING id"#,
        )
        .bind(task.target_time)
        .bind(task.user_id)
        .bind(task.group_id)
        .bind(&task.nickname)
        .bind(&task.card)
        .bind(&task.message)
        .fetch_one(pool)
        .await
        .map_err(|e| format!("pg cron save error: {}", e))?;
        Ok(row.get("id"))
    }

    async fn pg_task_remove(task_id: i64) -> Result<(), String> {
        let pool = db::pg().await;
        sqlx::query("DELETE FROM cron_tasks WHERE id = $1")
            .bind(task_id)
            .execute(pool)
            .await
            .map_err(|e| format!("pg cron remove error: {}", e))?;
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

        let target_ts = target_time.and_local_timezone(Local).unwrap().timestamp();

        let pg_task = CronTask {
            id: 0,
            target_time: target_ts,
            user_id: context.user_id,
            group_id: context.group_id,
            nickname: context.nickname.clone(),
            card: context.card.clone(),
            message: message.clone(),
        };

        let task_id = match Self::pg_task_save(&pg_task).await {
            Ok(id) => id,
            Err(e) => {
                log::warn!("Cron: failed to save task to pg: {}", e);
                return Some(msg_segment_from_string(format!(
                    "保存定时任务失败：{}",
                    e
                )));
            }
        };

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
