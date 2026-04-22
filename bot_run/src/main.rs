use bot_lib::{logger, NapcatWebSocket, Segment};
use bot_run::feature::{Feature, FeatureConfig, MessageContext, FEATURE_MANAGER};
use bot_run::image_matting::{ImageMattingResult, MsgQueue};
use bot_run::loli::LoliTtsResult;
use bot_run::sdimage::SdImageResult;
use bot_run::video_prompt::VideoPromptResult;
use dotenvy::dotenv;
use std::collections::VecDeque;
use std::env;
use std::sync::Arc;
use tokio::sync::{mpsc, Mutex};

const MSG_QUEUE_MAX_LEN: usize = 50;

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    dotenv().ok();

    // for item in dotenvy::dotenv_iter()? {
    //     let (key, val) = item?;
    //     println!("{}={}", key, val);
    // }

    logger::init();
    logger::set_level(log::LevelFilter::Debug);

    let ws_url = env::var("NAPCAT_WS_URL").unwrap_or_else(|_| "ws://127.0.0.1:3001".to_string());
    let access_token =
        env::var("NAPCAT_ACCESS_TOKEN").unwrap_or_else(|_| "NAPCAT_ACCESS_TOKEN".to_string());

    log::info!("Starting bot with WebSocket URL: {}", ws_url);

    let ws = NapcatWebSocket::with_options(
        bot_lib::WebSocketOptions::from_url(&ws_url)
            .with_access_token(&access_token)
            .with_debug(true),
    );

    let ws_arc = std::sync::Arc::new(ws);

    ws_arc
        .on("socket.open", |_| {
            log::info!("Socket connected");
        })
        .await;

    ws_arc
        .on("socket.close", |data| {
            log::info!("Socket closed: {:?}", data);
        })
        .await;

    ws_arc
        .on("socket.error", |data| {
            log::error!("Socket error: {:?}", data);
        })
        .await;

    ws_arc
        .on("meta_event.lifecycle.connect", |_| {
            log::info!("Lifecycle connect");
        })
        .await;

    let (tx, mut sd_rx) = mpsc::channel::<SdImageResult>(32);
    let (cron_tx, mut cron_rx) = mpsc::channel::<bot_run::cron::CronResult>(32);
    let (vp_tx, mut vp_rx) = mpsc::channel::<VideoPromptResult>(32);
    let (matting_tx, mut matting_rx) = mpsc::channel::<ImageMattingResult>(32);
    let (loli_tts_tx, mut loli_tts_rx) = mpsc::channel::<LoliTtsResult>(32);
    let msg_queue: MsgQueue = Arc::new(Mutex::new(VecDeque::new()));
    let ws_sd = ws_arc.clone();
    let ws_cron = ws_arc.clone();
    let ws_vp = ws_arc.clone();
    let ws_matting = ws_arc.clone();
    let ws_loli_tts = ws_arc.clone();

    let features_enabled: Vec<String> = env::var("FEATURES_ENABLED")
        .unwrap_or_default()
        .split(',')
        .map(|s| s.trim().to_string())
        .filter(|s| !s.is_empty())
        .collect();

    {
        let mut manager = FEATURE_MANAGER.lock().unwrap();
        manager.register(
            bot_run::choice::ChoiceFeature::feature_id(),
            bot_run::choice::ChoiceFeature::feature_name(),
            || Arc::new(bot_run::choice::ChoiceFeature) as Arc<dyn Feature + Send + Sync>,
        );
        manager.register(
            bot_run::draw5k::Draw5kFeature::feature_id(),
            bot_run::draw5k::Draw5kFeature::feature_name(),
            || Arc::new(bot_run::draw5k::Draw5kFeature) as Arc<dyn Feature + Send + Sync>,
        );
        manager.register(
            bot_run::dup_check::DupCheckFeature::feature_id(),
            bot_run::dup_check::DupCheckFeature::feature_name(),
            || {
                Arc::new(bot_run::dup_check::DupCheckFeature::new())
                    as Arc<dyn Feature + Send + Sync>
            },
        );
        manager.register(
            bot_run::gold::GoldFeature::feature_id(),
            bot_run::gold::GoldFeature::feature_name(),
            || Arc::new(bot_run::gold::GoldFeature::new()) as Arc<dyn Feature + Send + Sync>,
        );
        manager.register(
            bot_run::jrrp::JrrpFeature::feature_id(),
            bot_run::jrrp::JrrpFeature::feature_name(),
            || Arc::new(bot_run::jrrp::JrrpFeature::new()) as Arc<dyn Feature + Send + Sync>,
        );
        manager.register(
            bot_run::loli::LoliFeature::feature_id(),
            bot_run::loli::LoliFeature::feature_name(),
            move || {
                Arc::new(bot_run::loli::LoliFeature::new(loli_tts_tx.clone()))
                    as Arc<dyn Feature + Send + Sync>
            },
        );
        manager.register(
            bot_run::sdimage::SdImageFeature::feature_id(),
            bot_run::sdimage::SdImageFeature::feature_name(),
            move || {
                Arc::new(bot_run::sdimage::SdImageFeature::new(tx.clone()))
                    as Arc<dyn Feature + Send + Sync>
            },
        );
        manager.register(
            bot_run::cron::CronFeature::feature_id(),
            bot_run::cron::CronFeature::feature_name(),
            move || {
                Arc::new(bot_run::cron::CronFeature::new(cron_tx.clone()))
                    as Arc<dyn Feature + Send + Sync>
            },
        );
        manager.register(
            bot_run::video_prompt::VideoPromptFeature::feature_id(),
            bot_run::video_prompt::VideoPromptFeature::feature_name(),
            move || {
                Arc::new(bot_run::video_prompt::VideoPromptFeature::new(
                    vp_tx.clone(),
                )) as Arc<dyn Feature + Send + Sync>
            },
        );
        let matting_queue = msg_queue.clone();
        manager.register(
            bot_run::image_matting::ImageMattingFeature::feature_id(),
            bot_run::image_matting::ImageMattingFeature::feature_name(),
            move || {
                Arc::new(bot_run::image_matting::ImageMattingFeature::new(
                    matting_tx.clone(),
                    matting_queue.clone(),
                )) as Arc<dyn Feature + Send + Sync>
            },
        );

        for feat in &features_enabled {
            if let Err(e) = manager.load_feature(feat.trim()) {
                log::warn!("Failed to load feature '{}': {}", feat, e);
            }
        }

        manager
            .loaded
            .push(Arc::new(FeatureConfig) as Arc<dyn Feature + Send + Sync>);
    }

    let feature_names: Vec<String> = {
        let manager = FEATURE_MANAGER.lock().unwrap();
        manager.list_loaded()
    };
    log::info!("Loaded features:\n{:?}", feature_names.join("\n"));

    let ws_features = ws_arc.clone();
    let fm = FEATURE_MANAGER.clone();
    let event_msg_queue = msg_queue.clone();
    tokio::spawn(async move {
        let mut rx = ws_features.event_receiver();
        while let Ok(json) = rx.recv().await {
            let post_type = json.get("post_type").and_then(|v| v.as_str()).unwrap_or("");
            if post_type != "message" && post_type != "message_sent" {
                continue;
            }

            let message_type = json
                .get("message_type")
                .and_then(|v| v.as_str())
                .unwrap_or("");
            if message_type != "group" && message_type != "private" {
                continue;
            }

            // Push message into history queue for reference lookups
            {
                let mut queue = event_msg_queue.lock().await;
                queue.push_back(json.clone());
                while queue.len() > MSG_QUEUE_MAX_LEN {
                    queue.pop_front();
                }
            }

            let context = MessageContext::from_json(&json);
            let messages = json
                .get("message")
                .and_then(|v| v.as_array())
                .cloned()
                .unwrap_or_default();

            for msg in messages {
                let current_features = {
                    let manager = fm.lock().unwrap();
                    manager.loaded.clone()
                };

                for feature in &current_features {
                    if feature.check_command(&msg) {
                        let ctx = context.clone();
                        let msg_clone = msg.clone();
                        let ws_clone = ws_features.clone();
                        let feat = Arc::clone(feature);

                        tokio::spawn(async move {
                            let result = feat.deal_with_message(&ctx, &msg_clone).await;
                            if let Some(segment) = result {
                                let _ = send_reply(&ws_clone, &ctx, vec![segment]).await;
                            }
                        });

                        break;
                    }
                }
            }
        }
    });

    let ws_sd_cb = ws_sd.clone();
    tokio::spawn(async move {
        while let Some(result) = sd_rx.recv().await {
            let name = if result.context.display_name().is_empty() {
                result.context.nickname.clone()
            } else {
                result.context.display_name()
            };
            let msg = format!("@{} ({}) 已生成图片：", name, result.context.user_id);

            let segments = vec![Segment::text(msg), result.segment];
            let _ = send_reply(&ws_sd_cb, &result.context, segments).await;
        }
    });

    tokio::spawn(async move {
        while let Some(result) = cron_rx.recv().await {
            let segments = vec![
                Segment::at(result.context.user_id),
                Segment::text(format!(" 提醒你：{}", result.message)),
            ];
            let _ = send_reply(&ws_cron, &result.context, segments).await;
        }
    });

    tokio::spawn(async move {
        while let Some(result) = vp_rx.recv().await {
            let name = if result.context.display_name().is_empty() {
                result.context.nickname.clone()
            } else {
                result.context.display_name()
            };
            let segments = vec![
                Segment::text(format!("@{} 视频提示词生成完成：\n", name)),
                result.segment,
            ];
            let _ = send_reply(&ws_vp, &result.context, segments).await;
        }
    });

    tokio::spawn(async move {
        while let Some(result) = matting_rx.recv().await {
            let segments = vec![result.segment];
            let _ = send_reply(&ws_matting, &result.context, segments).await;
        }
    });

    tokio::spawn(async move {
        while let Some(result) = loli_tts_rx.recv().await {
            let segments = vec![result.segment];
            let _ = send_reply(&ws_loli_tts, &result.context, segments).await;
        }
    });

    log::info!(
        "Connecting to napcat WebSocket {} with token {} ...",
        ws_url,
        access_token
    );
    if let Err(e) = ws_arc.run().await {
        log::error!("WebSocket error: {}", e);
        return Err(Box::new(e) as Box<dyn std::error::Error>);
    }

    Ok(())
}

async fn send_reply(
    ws: &std::sync::Arc<NapcatWebSocket>,
    context: &MessageContext,
    segments: Vec<bot_lib::structs::MessageSegment>,
) -> Result<(), Box<dyn std::error::Error>> {
    if let Some(group_id) = context.group_id {
        let params = bot_lib::websocket_api::SendGroupMsgParams {
            group_id,
            message: segments,
            auto_escape: Some(false),
        };
        ws.send_group_msg(params).await?;
    } else {
        let params = bot_lib::websocket_api::SendPrivateMsgParams {
            user_id: context.user_id,
            message: segments,
            auto_escape: Some(false),
        };
        ws.send_private_msg(params).await?;
    }
    Ok(())
}
