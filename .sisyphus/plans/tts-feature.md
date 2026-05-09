# TTS Feature Implementation Plan

## Overview
Add a new `-tts <text>` command feature that calls the TTS service and returns voice/audio messages. Follows the exact same pattern as `loli.rs` but skips the LLM chat step - directly synthesizes the user's input text.

## TODOs

### Phase 1: Core Implementation

- [x] Create `bot_run/src/tts.rs` - TTS feature module with Feature trait implementation
  - Structs: `TtsRequest`, `TtsResult`, `TtsSender`, `TtsFeature`
  - Methods reused from loli.rs: `new()`, `tts_base_url_from_env()`, `get_tts_base_url()`, `build_tts_record_segment()`, `synthesize_tts()`, `synthesize_and_deliver()`
  - NOT included: All LLM code (ChatRequest, ChatMessage, ChatResponse, Choice, MESSAGE_CACHE, SYSTEM_PROMPT, chat(), OpenAI methods)
  - Feature id: "tts", command prefix: "-tts "
  - `deal_with_message`: extract text after "-tts ", directly spawn synthesize_and_deliver (no chat step)
  - Unit tests: url defaults, url trimming, record segment wrapping, command prefix detection

- [x] Register `pub mod tts;` and `pub use self::tts::TtsFeature;` in `bot_run/src/lib.rs`

- [x] Wire TtsFeature into `bot_run/src/main.rs`
  - Add `use bot_run::tts::TtsResult;` import
  - Create `(tts_tx, tts_rx)` mpsc channel
  - Clone `ws_arc` for `ws_tts`
  - Register in FeatureManager (same pattern as loli)
  - Add consumer task: `while let Some(result) = tts_rx.recv().await { send_reply(...) }`

### Phase 2: Verification

- [x] Verify: `cargo check -p bot_run` compiles without errors
- [x] Verify: `cargo test -p bot_run` all tests pass
- [x] Verify: `cargo check` workspace compiles

## Final Verification Wave

- [x] F1 - Code Review: Verify tts.rs only contains TTS logic (no LLM code), follows loli.rs patterns
- [x] F2 - Build Check: `cargo check -p bot_run` exit code 0
- [x] F3 - Test Check: `cargo test -p bot_run` all tests pass
- [x] F4 - Integration Check: lib.rs module declarations and main.rs wiring match loli.rs pattern exactly
