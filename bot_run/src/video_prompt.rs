use crate::feature::{msg_segment_from_string, Feature, MessageContext};
use async_trait::async_trait;
use bot_lib::structs::MessageSegment;
use serde::{Deserialize, Serialize};
use serde_json::Value;
use std::env;
use tokio::sync::mpsc;

// ── Skill prompts (embedded at compile time) ───────────────────────────────

const VISUAL_BRIEF_SKILL: &str = include_str!("../assets/skills/visual-brief-skill.md");
const SHOT_PLAN_SKILL: &str = include_str!("../assets/skills/shot-plan-skill.md");
const SHOT_PROMPT_SKILL: &str = include_str!("../assets/skills/seedance2.0-prompt-skill.md");

// ── Channel result ─────────────────────────────────────────────────────────

pub struct VideoPromptResult {
    pub context: MessageContext,
    pub segment: MessageSegment,
}

pub type VideoPromptSender = mpsc::Sender<VideoPromptResult>;

// ── JSON intermediate structures ───────────────────────────────────────────

#[derive(Debug, Clone, Serialize, Deserialize)]
struct VisualBrief {
    style_keywords: Vec<String>,
    tone: String,
    setting: SettingDefaults,
    continuity_rules: Vec<String>,
    characters: Vec<CharacterBrief>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
struct SettingDefaults {
    era: String,
    location: String,
    time_of_day: String,
    weather: String,
    architecture: String,
    palette: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
struct CharacterBrief {
    id: String,
    name: String,
    appearance: String,
    wardrobe: String,
    props: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
struct ShotSpec {
    shot_id: u32,
    beat_summary: String,
    character_ids: Vec<String>,
    primary_subject: String,
    primary_action: String,
    setting: String,
    camera: String,
    duration_sec: u32,
    must_include: Vec<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
struct ShotPromptResult {
    shot_id: u32,
    prompt: String,
    #[serde(default)]
    prompt_zh: String,
}

// ── Lightweight LLM client ─────────────────────────────────────────────────

#[derive(Serialize)]
struct ChatRequest {
    model: String,
    max_tokens: u64,
    messages: Vec<ChatMessage>,
}

#[derive(Serialize)]
struct ChatMessage {
    role: String,
    content: String,
}

#[derive(Deserialize)]
struct ChatResponse {
    choices: Vec<Choice>,
}

#[derive(Deserialize)]
struct Choice {
    message: AssistantMsg,
}

#[derive(Deserialize)]
struct AssistantMsg {
    content: String,
}

async fn llm_chat(
    client: &reqwest::Client,
    base_url: &str,
    api_key: &str,
    model: &str,
    system_prompt: &str,
    user_prompt: &str,
) -> Result<String, String> {
    let url = format!("{}/chat/completions", base_url.trim_end_matches('/'));

    let body = ChatRequest {
        model: model.to_string(),
        max_tokens: 8192,
        messages: vec![
            ChatMessage {
                role: "system".to_string(),
                content: system_prompt.to_string(),
            },
            ChatMessage {
                role: "user".to_string(),
                content: user_prompt.to_string(),
            },
        ],
    };

    let resp = client
        .post(&url)
        .header("Authorization", format!("Bearer {}", api_key))
        .header("Content-Type", "application/json")
        .json(&body)
        .send()
        .await
        .map_err(|e| format!("LLM request failed: {}", e))?;

    if !resp.status().is_success() {
        let status = resp.status();
        let text = resp.text().await.unwrap_or_default();
        return Err(format!("LLM API returned {}: {}", status, text));
    }

    let chat_resp: ChatResponse = resp
        .json()
        .await
        .map_err(|e| format!("LLM response parse error: {}", e))?;

    let mut content = chat_resp
        .choices
        .into_iter()
        .next()
        .map(|c| c.message.content)
        .ok_or_else(|| "LLM returned no choices".to_string())?;

    if let (Some(_start), Some(end)) = (content.find("<think>"), content.find("</think>")) {
        content = content[end + 8..].trim().to_string();
    }

    Ok(content)
}

// ── JSON extraction helper ─────────────────────────────────────────────────

/// Extract JSON from LLM response that may contain markdown fences or extra text.
fn extract_json_str(raw: &str) -> &str {
    let trimmed = raw.trim();

    if let Some(start) = trimmed.find("```json") {
        let after_fence = &trimmed[start + 7..];
        if let Some(end) = after_fence.find("```") {
            return after_fence[..end].trim();
        }
    }

    if let Some(start) = trimmed.find("```") {
        let after_fence = &trimmed[start + 3..];
        if let Some(end) = after_fence.find("```") {
            return after_fence[..end].trim();
        }
    }

    let first_brace = trimmed.find('{');
    let first_bracket = trimmed.find('[');
    let start = match (first_brace, first_bracket) {
        (Some(a), Some(b)) => Some(a.min(b)),
        (Some(a), None) => Some(a),
        (None, Some(b)) => Some(b),
        (None, None) => None,
    };

    if let Some(s) = start {
        let expected_close = if trimmed.as_bytes()[s] == b'{' { '}' } else { ']' };
        if let Some(e) = trimmed.rfind(expected_close) {
            return &trimmed[s..=e];
        }
    }

    trimmed
}

// ── Validation functions ───────────────────────────────────────────────────

const BANNED_PHRASES: &[&str] = &[
    "avoid ",
    "no ",
    "without ",
    "don't ",
    "do not ",
    "never ",
    "exclude ",
    "remove ",
];

const LABEL_PREFIXES: &[&str] = &[
    "subject:",
    "action:",
    "scene:",
    "camera:",
    "style:",
    "lighting:",
    "environment:",
];

struct ValidationFailure {
    reasons: Vec<String>,
}

fn validate_shot_prompt(prompt: &str, _duration_sec: u32) -> Result<(), ValidationFailure> {
    let mut reasons = Vec::new();
    let word_count = prompt.split_whitespace().count();

    if word_count < 30 {
        reasons.push(format!(
            "Prompt has {} words, minimum is 30.",
            word_count
        ));
    }
    if word_count > 80 {
        reasons.push(format!(
            "Prompt has {} words, maximum is 80. Cut to 80.",
            word_count
        ));
    }
    if word_count > 100 {
        reasons.push("Prompt exceeds 100 words — hard limit. Completely rewrite shorter.".to_string());
    }

    let lower = prompt.to_lowercase();
    for phrase in BANNED_PHRASES {
        if lower.contains(phrase) {
            reasons.push(format!(
                "Contains banned negative phrase '{}'. Seedance does not use negative prompts. Remove it.",
                phrase.trim()
            ));
        }
    }

    for label in LABEL_PREFIXES {
        if lower.contains(label) {
            reasons.push(format!(
                "Contains label prefix '{}'. Write a flowing sentence, not labeled fields.",
                label
            ));
        }
    }

    if reasons.is_empty() {
        Ok(())
    } else {
        Err(ValidationFailure { reasons })
    }
}

// ── Pipeline ───────────────────────────────────────────────────────────────

async fn run_pipeline(story: String, context: MessageContext, sender: VideoPromptSender) {
    let base_url =
        env::var("OPENAI_BASE_URL").unwrap_or_else(|_| "https://api.openai.com/v1".to_string());
    let api_key = env::var("OPENAI_API_KEY").unwrap_or_default();
    let model = env::var("OPENAI_API_MODEL").unwrap_or_else(|_| "gpt-4o".to_string());

    let client = match reqwest::Client::builder()
        .timeout(std::time::Duration::from_secs(300))
        .build()
    {
        Ok(c) => c,
        Err(e) => {
            log::error!("[video_prompt] Failed to build HTTP client: {}", e);
            let _ = sender
                .send(VideoPromptResult {
                    context,
                    segment: msg_segment_from_string(format!(
                        "视频提示词生成失败：无法创建 HTTP 客户端 ({})",
                        e
                    )),
                })
                .await;
            return;
        }
    };

    // ────────────────────────────────────────────────────────────────────
    // Step 1: Visual Brief (story → compact JSON with world + characters)
    // ────────────────────────────────────────────────────────────────────
    log::info!("[video_prompt] Step 1/3: Generating Visual Brief...");

    let brief_user_prompt = format!(
        "Analyze the following story and produce the Visual Brief JSON.\n\n\
         ---\n\
         STORY TEXT:\n{}\n\
         ---",
        story
    );

    let visual_brief: VisualBrief = match llm_call_with_json_retry(
        &client,
        &base_url,
        &api_key,
        &model,
        VISUAL_BRIEF_SKILL,
        &brief_user_prompt,
        "Visual Brief",
    )
    .await
    {
        Ok(v) => v,
        Err(e) => {
            log::error!("[video_prompt] Visual Brief failed: {}", e);
            let _ = sender
                .send(VideoPromptResult {
                    context,
                    segment: msg_segment_from_string(format!(
                        "视频提示词生成失败（视觉概要阶段）：{}",
                        e
                    )),
                })
                .await;
            return;
        }
    };

    let brief_json = serde_json::to_string_pretty(&visual_brief).unwrap_or_default();
    log::info!(
        "[video_prompt] Step 1/3 complete. Visual Brief:\n{}",
        brief_json
    );

    // ────────────────────────────────────────────────────────────────────
    // Step 2: Shot Plan (story + brief → JSON array of shot specs)
    // ────────────────────────────────────────────────────────────────────
    log::info!("[video_prompt] Step 2/3: Generating Shot Plan...");

    let plan_user_prompt = format!(
        "Break the following story into shots. Use the Visual Brief for setting, style, and character references.\n\n\
         ---\n\
         STORY TEXT:\n{}\n\
         ---\n\
         VISUAL BRIEF:\n{}\n\
         ---",
        story, brief_json
    );

    let shot_plan: Vec<ShotSpec> = match llm_call_with_json_retry(
        &client,
        &base_url,
        &api_key,
        &model,
        SHOT_PLAN_SKILL,
        &plan_user_prompt,
        "Shot Plan",
    )
    .await
    {
        Ok(v) => v,
        Err(e) => {
            log::error!("[video_prompt] Shot Plan failed: {}", e);
            let _ = sender
                .send(VideoPromptResult {
                    context,
                    segment: msg_segment_from_string(format!(
                        "视频提示词生成失败（分镜规划阶段）：{}",
                        e
                    )),
                })
                .await;
            return;
        }
    };

    let plan_json = serde_json::to_string_pretty(&shot_plan).unwrap_or_default();
    log::info!(
        "[video_prompt] Step 2/3 complete. Shot Plan ({} shots):\n{}",
        shot_plan.len(),
        plan_json
    );

    // ────────────────────────────────────────────────────────────────────
    // Step 3: Per-Shot Prompts (for each shot: brief + shot spec → prompt)
    // ────────────────────────────────────────────────────────────────────
    log::info!(
        "[video_prompt] Step 3/3: Generating prompts for {} shots...",
        shot_plan.len()
    );

    let mut final_prompts: Vec<ShotPromptResult> = Vec::with_capacity(shot_plan.len());
    let mut prev_prompt_hint: Option<String> = None;

    for (i, shot) in shot_plan.iter().enumerate() {
        log::info!(
            "[video_prompt] Generating prompt for shot {}/{}...",
            i + 1,
            shot_plan.len()
        );

        let referenced_chars: Vec<&CharacterBrief> = visual_brief
            .characters
            .iter()
            .filter(|c| shot.character_ids.contains(&c.id))
            .collect();

        let chars_json = serde_json::to_string(&referenced_chars).unwrap_or_else(|_| "[]".to_string());
        let shot_json = serde_json::to_string(shot).unwrap_or_default();

        let mut user_prompt = format!(
            "Generate the Seedance 2.0 prompt for this shot.\n\n\
             ---\n\
             VISUAL BRIEF (style + setting):\n{}\n\
             ---\n\
             SHOT SPEC:\n{}\n\
             ---\n\
             REFERENCED CHARACTERS:\n{}\n\
             ---",
            brief_json, shot_json, chars_json
        );

        if let Some(ref prev) = prev_prompt_hint {
            user_prompt.push_str(&format!(
                "\n\
                 PREVIOUS SHOT PROMPT (for continuity):\n{}\n\
                 ---",
                prev
            ));
        }

        let mut prompt_result: Option<ShotPromptResult> = None;
        let mut last_error = String::new();

        for attempt in 0..3 {
            let raw = match llm_chat(
                &client,
                &base_url,
                &api_key,
                &model,
                SHOT_PROMPT_SKILL,
                &user_prompt,
            )
            .await
            {
                Ok(v) => v,
                Err(e) => {
                    last_error = format!("LLM call failed: {}", e);
                    log::error!(
                        "[video_prompt] Shot {} attempt {} LLM error: {}",
                        shot.shot_id,
                        attempt + 1,
                        e
                    );
                    continue;
                }
            };

            log::info!(
                "[video_prompt] Shot {} attempt {} raw response:\n{}",
                shot.shot_id,
                attempt + 1,
                raw
            );

            let json_str = extract_json_str(&raw);
            let parsed: ShotPromptResult = match serde_json::from_str(json_str) {
                Ok(v) => v,
                Err(e) => {
                    last_error = format!("JSON parse error: {} — raw: {}", e, &raw[..raw.len().min(200)]);
                    log::warn!(
                        "[video_prompt] Shot {} attempt {} JSON parse failed: {}",
                        shot.shot_id,
                        attempt + 1,
                        last_error
                    );
                    // Retry with parse error feedback
                    user_prompt = format!(
                        "Your previous response was not valid JSON. Error: {}\n\
                         Raw response: {}\n\n\
                         Please output ONLY the JSON object with shot_id and prompt fields. No markdown fences, no extra text.\n\n\
                         ---\n\
                         VISUAL BRIEF:\n{}\n\
                         ---\n\
                         SHOT SPEC:\n{}\n\
                         ---\n\
                         REFERENCED CHARACTERS:\n{}\n\
                         ---",
                        e, &raw[..raw.len().min(500)], brief_json, shot_json, chars_json
                    );
                    continue;
                }
            };

            match validate_shot_prompt(&parsed.prompt, shot.duration_sec) {
                Ok(()) => {
                    log::info!(
                        "[video_prompt] Shot {} validated OK (attempt {})",
                        shot.shot_id,
                        attempt + 1
                    );
                    prompt_result = Some(parsed);
                    break;
                }
                Err(failure) => {
                    let reasons_str = failure.reasons.join("\n- ");
                    log::warn!(
                        "[video_prompt] Shot {} attempt {} validation failed:\n- {}",
                        shot.shot_id,
                        attempt + 1,
                        reasons_str
                    );

                    if attempt < 2 {
                        user_prompt = format!(
                            "Your previous prompt failed validation. Fix these issues:\n- {}\n\n\
                             Your previous prompt was:\n\"{}\"\n\n\
                             Rewrite the prompt fixing all listed issues. Output ONLY the JSON object.\n\n\
                             ---\n\
                             VISUAL BRIEF:\n{}\n\
                             ---\n\
                             SHOT SPEC:\n{}\n\
                             ---\n\
                             REFERENCED CHARACTERS:\n{}\n\
                             ---",
                            reasons_str, parsed.prompt, brief_json, shot_json, chars_json
                        );
                    } else {
                        log::warn!(
                            "[video_prompt] Shot {} used after 3 failed validations",
                            shot.shot_id
                        );
                        prompt_result = Some(parsed);
                    }
                }
            }
        }

        match prompt_result {
            Some(result) => {
                prev_prompt_hint = Some(result.prompt.clone());
                final_prompts.push(result);
            }
            None => {
                log::error!(
                    "[video_prompt] Shot {} completely failed after 3 attempts: {}",
                    shot.shot_id,
                    last_error
                );
                final_prompts.push(ShotPromptResult {
                    shot_id: shot.shot_id,
                    prompt: format!("[GENERATION FAILED: {}]", last_error),
                    prompt_zh: "生成失败".to_string(),
                });
            }
        }
    }

    // ────────────────────────────────────────────────────────────────────
    // Format final output for the user
    // ────────────────────────────────────────────────────────────────────
    log::info!(
        "[video_prompt] Pipeline complete. {} prompts generated.",
        final_prompts.len()
    );

    let mut output = String::new();
    output.push_str(&format!(
        "🎬 Seedance 2.0 Prompts — {} shots\n\n",
        final_prompts.len()
    ));

    for (result, shot) in final_prompts.iter().zip(shot_plan.iter()) {
        let word_count = result.prompt.split_whitespace().count();
        output.push_str(&format!(
            "=== Shot {} ===\n建议时长 / Suggested Duration: {}s\n\n\
             [EN] {}\n({} words)\n\n\
             [中文] {}\n\n",
            result.shot_id,
            shot.duration_sec,
            result.prompt,
            word_count,
            result.prompt_zh
        ));
    }

    output.push_str("--- Continuity Rules ---\n");
    for rule in &visual_brief.continuity_rules {
        output.push_str(&format!("• {}\n", rule));
    }

    let _ = sender
        .send(VideoPromptResult {
            context,
            segment: msg_segment_from_string(output),
        })
        .await;
}

/// Call LLM and parse JSON response with up to 2 retries on parse failure.
async fn llm_call_with_json_retry<T: serde::de::DeserializeOwned>(
    client: &reqwest::Client,
    base_url: &str,
    api_key: &str,
    model: &str,
    system_prompt: &str,
    initial_user_prompt: &str,
    step_name: &str,
) -> Result<T, String> {
    let mut user_prompt = initial_user_prompt.to_string();

    for attempt in 0..3 {
        let raw = llm_chat(client, base_url, api_key, model, system_prompt, &user_prompt).await?;

        log::info!(
            "[video_prompt] {} attempt {} raw response:\n{}",
            step_name,
            attempt + 1,
            raw
        );

        let json_str = extract_json_str(&raw);
        match serde_json::from_str::<T>(json_str) {
            Ok(parsed) => return Ok(parsed),
            Err(e) => {
                log::warn!(
                    "[video_prompt] {} attempt {} JSON parse failed: {}",
                    step_name,
                    attempt + 1,
                    e
                );
                if attempt < 2 {
                    user_prompt = format!(
                        "Your previous response was not valid JSON. Parse error: {}\n\
                         Raw response (truncated): {}\n\n\
                         Please output ONLY raw JSON. No markdown fences, no explanation text, no commentary.\n\n\
                         Original request:\n{}",
                        e,
                        &raw[..raw.len().min(500)],
                        initial_user_prompt
                    );
                } else {
                    return Err(format!(
                        "{} failed after 3 JSON parse attempts. Last error: {}",
                        step_name, e
                    ));
                }
            }
        }
    }

    Err(format!("{} failed: exhausted retries", step_name))
}

// ── Feature implementation ─────────────────────────────────────────────────

pub struct VideoPromptFeature {
    sender: VideoPromptSender,
}

impl VideoPromptFeature {
    pub fn new(sender: VideoPromptSender) -> Self {
        Self { sender }
    }

    pub fn feature_id() -> &'static str {
        "video_prompt"
    }

    pub fn feature_name() -> &'static str {
        "视频提示词生成: -video-prompt <故事内容>"
    }
}

#[async_trait]
impl Feature for VideoPromptFeature {
    fn feature_id(&self) -> &str {
        VideoPromptFeature::feature_id()
    }

    fn feature_name(&self) -> &str {
        VideoPromptFeature::feature_name()
    }

    fn check_command(&self, msg: &Value) -> bool {
        if msg["type"].as_str() != Some("text") {
            return false;
        }
        let text = msg["data"]["text"].as_str().unwrap_or("").trim();
        text.starts_with("-video-prompt ")
    }

    async fn deal_with_message(
        &self,
        context: &MessageContext,
        msg: &Value,
    ) -> Option<MessageSegment> {
        let text = msg["data"]["text"]
            .as_str()
            .unwrap_or("")
            .trim()
            .to_string();

        let story = match text.strip_prefix("-video-prompt ") {
            Some(s) => s.trim().to_string(),
            None => {
                return Some(msg_segment_from_string(
                    "用法: -video-prompt <故事内容>".to_string(),
                ));
            }
        };

        if story.is_empty() {
            return Some(msg_segment_from_string(
                "请输入故事内容。用法: -video-prompt <故事内容>".to_string(),
            ));
        }

        let video_accept_senders = env::var("VIDEO_ACCEPT_SENDERS")
            .unwrap_or_default()
            .split(',')
            .filter_map(|s| s.trim().parse::<i64>().ok())
            .collect::<Vec<_>>();
        if !video_accept_senders.contains(&context.user_id) {
            log::info!(
                "[video_prompt] User {} is not authorized to use video prompt feature",
                context.user_id
            );
            return Some(msg_segment_from_string(
                "您没有权限使用视频提示词生成功能。".to_string(),
            ));
        }

        let ctx = context.clone();
        let sender = self.sender.clone();

        tokio::spawn(run_pipeline(story, ctx, sender));

        Some(msg_segment_from_string(
            "已收到视频提示词生成请求，正在执行 3 步流程：视觉概要 → 分镜规划 → 逐镜头提示词生成。\n整个流程可能需要几分钟，请耐心等待..."
                .to_string(),
        ))
    }
}
