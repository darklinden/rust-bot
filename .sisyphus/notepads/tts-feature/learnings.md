## 2026-05-09 Task: TTS Feature Implementation

### What was built
Added a new `-tts <text>` command feature to the QQ bot (bot_run crate).

### Files created/modified
- **Created**: `bot_run/src/tts.rs` (217 lines) - Pure TTS feature, no LLM code
- **Modified**: `bot_run/src/lib.rs` (+2 lines) - Module declaration and re-export
- **Modified**: `bot_run/src/main.rs` (+18 lines) - Import, channel, ws clone, registration, consumer

### Key patterns learned
- Feature registration follows: channel → ws_arc.clone() → FeatureManager::register → consumer task
- Audio segments use `Segment::record(format!("base64://{}", BASE64.encode(bytes)))`
- TTS service at `{TTS_URL}/synthesize` (POST JSON `{"text": "..."}`, returns `audio/wav`)
- Features that send results via channel return `None` from `deal_with_message`
- `check_command` must validate `msg["type"] == "text"` before checking prefix

### Difference from loli.rs
- loli.rs: `-loli <text>` → chat(LLM) → synthesize_and_deliver(LLM_response)
- tts.rs: `-tts <text>` → synthesize_and_deliver(text) directly
- No ChatRequest/ChatMessage/ChatResponse/Choice/MESSAGE_CACHE/SYSTEM_PROMPT

### Verification
- `cargo check -p bot_run` → 0 errors
- `cargo test -p bot_run -- tts` → 4/4 tests pass
- Full loli + tts test suite: 8/8 unit tests pass
- Pre-existing choice.rs failures unrelated (BOT_MESSAGE_PREFIX env var)
