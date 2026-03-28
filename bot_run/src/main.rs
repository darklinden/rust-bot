use bot_lib::{logger, NapcatWebSocket, Segment};
use bot_run::feature::{Feature, MessageContext};
use bot_run::sdimage::{SdImageFeature, SdImageResult};
use dotenvy::dotenv;
use serde_json::Value;
use std::env;
use std::sync::Arc;
use tokio::sync::mpsc;

#[allow(dead_code)]
fn check_text_command(msg: &Value, prefixes: &[&str]) -> bool {
    if msg.get("type").and_then(|v| v.as_str()) != Some("text") {
        return false;
    }
    let text = msg
        .get("data")
        .and_then(|d| d.get("text"))
        .and_then(|t| t.as_str())
        .unwrap_or("");
    prefixes.iter().any(|p| text.starts_with(p))
}

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
    let ws_sd = ws_arc.clone();

    let mut features: Vec<Arc<dyn Feature + Send + Sync>> = vec![];

    let features_enabled: Vec<String> = env::var("FEATURES_ENABLED")
        .unwrap_or_default()
        .split(',')
        .map(|s| s.trim().to_string())
        .filter(|s| !s.is_empty())
        .collect();

    for feat in &features_enabled {
        match feat.trim() {
            "choice" => features
                .push(Arc::new(bot_run::choice::ChoiceFeature) as Arc<dyn Feature + Send + Sync>),
            "draw5k" => features
                .push(Arc::new(bot_run::draw5k::Draw5kFeature) as Arc<dyn Feature + Send + Sync>),
            "dup_check" => features.push(Arc::new(bot_run::dup_check::DupCheckFeature::new())
                as Arc<dyn Feature + Send + Sync>),
            "gold" => features
                .push(Arc::new(bot_run::gold::GoldFeature) as Arc<dyn Feature + Send + Sync>),
            "jrrp" => features
                .push(Arc::new(bot_run::jrrp::JrrpFeature) as Arc<dyn Feature + Send + Sync>),
            "loli" => features
                .push(Arc::new(bot_run::loli::LoliFeature) as Arc<dyn Feature + Send + Sync>),
            "sdimage" => features
                .push(Arc::new(SdImageFeature::new(tx.clone())) as Arc<dyn Feature + Send + Sync>),
            _ => log::warn!("Unknown feature '{}' in FEATURES_ENABLED", feat),
        }
    }

    let features: Arc<Vec<Arc<dyn Feature + Send + Sync>>> = Arc::new(features);

    let feature_names: Vec<String> = features
        .iter()
        .map(|f| f.feature_name().to_string())
        .collect();
    log::info!("Loaded features: {:?}", feature_names);

    let ws_features = ws_arc.clone();
    let features_for_task = features.clone();
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

            let context = MessageContext::from_json(&json);
            let messages = json
                .get("message")
                .and_then(|v| v.as_array())
                .cloned()
                .unwrap_or_default();

            for msg in messages {
                let mut handled = false;

                for feature in features_for_task.iter() {
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

                        handled = true;
                        break;
                    }
                }

                if !handled {
                    if let Some(text) = msg
                        .get("data")
                        .and_then(|d| d.get("text"))
                        .and_then(|t| t.as_str())
                    {
                        if text.trim() == "echo features" {
                            let list = feature_names.join(", ");
                            let reply = format!("当前已加载的功能有：\n{}", list);
                            let _ = send_reply(&ws_features, &context, vec![Segment::text(reply)])
                                .await;
                        }
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
