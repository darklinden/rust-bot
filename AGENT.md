# CLAUDE.md

This file provides guidance to Claude Code (claude.ai/code) when working with code in this repository.

## Build & Run

```bash
# Build the bot binary
cargo build --release -p bot_run

# Run tests
cargo test -p bot_lib
cargo test -p bot_run

# Run a specific test with output
RUST_LOG=info cargo test -p bot_run -- --nocapture

# Run ignored tests (e.g. TL;DR e2e test)
RUST_LOG=info cargo test -p bot_run tldr::tests -- --nocapture --ignored

# Lint
cargo clippy -- -D warnings
```

## Architecture

This is a Rust workspace with three crates that together form a QQ bot using the OneBot 11 protocol via napcat.

### bot_lib — OneBot 11 WebSocket client library
- `websocket_base.rs` — Core WebSocket connection with reconnection, `send_raw` request/response pattern using nanoid echo keys, message parsing with CQ code decoding
- `websocket_api.rs` — Typed API wrappers (send_group_msg, set_group_ban, etc.) over `send_raw`
- `event_bus.rs` — Hierarchical event system (`socket.open`, `message.group.normal`, etc.) with `on`/`once`/`off` and wildcard propagation (events bubble up to parent namespaces)
- `structs.rs` — Message segment types (Text, Image, At, Reply, etc.)
- `utils.rs` — CQ code encode/decode, JSON conversion, logger init

### bot_run — Bot application with pluggable features
- `main.rs` — Connects to napcat WebSocket, registers features via `FeatureManager`, spawns async result-handling loops (each feature gets its own mpsc channel to send results back to be replied)
- `feature.rs` — `Feature` trait, `FeatureManager` (global static `FEATURE_MANAGER`), `MessageContext` struct. Features are loaded based on the `FEATURES_ENABLED` env var (comma-separated list of feature IDs). The built-in `FeatureConfig` feature enables runtime feature management via chat commands (`-features list/load/unload`)
- Individual features under `bot_run/src/` implement the `Feature` trait: `check_command` validates the incoming message, `deal_with_message` returns an `Option<MessageSegment>`; async features spawn background work and send results through an mpsc channel

#### Key feature files
| File | ID | Purpose |
|------|-----|---------|
| `jrrp.rs` | `jrrp` | Daily luck score (SHA256 of user_id + date) |
| `gold.rs` | `gold` | Gold price queries via external APIs, cached in PostgreSQL |
| `dup_check.rs` | `dup_check` | Duplicate image detection via perceptual hashing + pgvector |
| `tldr.rs` | `tldr` | Web page summarization via OpenAI-compatible API, uses sliding-window chunking |
| `tts.rs` | `tts` | Text-to-speech via external TTS API |
| `sdimage.rs` | `sdimage` | Stable Diffusion image generation via ComfyUI |
| `grsai_gpt_image.rs` | `gpt_image` | GPT image generation via grsai API |
| `loli.rs` | `loli` | Loli voice TTS |
| `choice.rs` | `choice` | Random choice helper |
| `draw5k.rs` | `draw5k` | Draw/roll feature |
| `cron.rs` | `cron` | Scheduled reminders, persisted in PostgreSQL |
| `video_prompt.rs` | `video_prompt` | Video prompt generation |
| `image_matting.rs` | `image_matting` | Image background removal |
| `media_file.rs` | — | Shared media file helper (writes base64/images to temp files) |
| `db/mod.rs` | — | PostgreSQL connection pool singleton (`db::pg()`) and init with migration + persona seeding |
| `db/migration.rs` | — | Idempotent `CREATE TABLE IF NOT EXISTS` for all tables (image_hashes, cache_entries, cron_tasks, personas, group_personas, knowledge_chunks, conversations) |
| `chat/mod.rs` | `chat` | Passive feature: RAG-powered chat with persona + knowledge + conversation history |
| `chat/embedding.rs` | — | Qwen3-Embedding-4B client with concurrent batch support |
| `chat/knowledge.rs` | — | Knowledge base pgvector cosine search + chunk import |
| `chat/persona.rs` | — | Persona CRUD + group-persona binding |
| `chat/session.rs` | — | Conversation history recent/save/prune |
| `chat/prompt.rs` | — | Assembles system prompt from persona + knowledge + history |
| `chat/manage.rs` | `persona_manage` | Persona management commands (`-p list/create/set/show`) |

### kb_tool — Knowledge base ingestion CLI
- `src/main.rs` — Scans `.md` files recursively, chunks text, embeds via Qwen3-Embedding-4B, inserts into `knowledge_chunks` table. Flag `--dry-run` to preview chunking without writing.

### Event flow
1. napcat pushes JSON events over WebSocket → `NapcatWebSocketBase::run` parses them, decodes CQ codes, emits to `EventBus` and `broadcast` channel
2. `main.rs` spawns a listener on the broadcast channel that filters for `message`/`message_sent` events, then runs two-phase dispatch:
   - **Active features**: `is_passive() == false`, first `check_command` match wins and breaks the loop
   - **Passive features**: `is_passive() == true`, all matching features are called (e.g. `chat` feature handles every text message)
3. Sync features return `MessageSegment` directly; async features spawn background tasks and return results via mpsc to dedicated handler loops

### CQ codes
Messages may arrive in CQ code string format (`[CQ:image,file=...]`). `bot_lib::utils::cq_decode` converts them to JSON arrays. Use `Segment::text()`, `Segment::image()`, `Segment::at()` etc. to build reply messages.

## Docker deployment

```bash
# Build the Docker image
./export-docker.sh   # produces qq-bot-0.0.1.tar.gz

# Start full stack (napcat + postgres + bot)
NAPCAT_UID=$(id -u) NAPCAT_GID=$(id -g) docker compose up -d
```

The stack runs three services: napcat (QQ framework), postgres (pgvector/pg17 for storage, caching, and vector search), and the bot binary.

## Environment

Copy `.env.example` to `.env` and configure:
- `NAPCAT_WS_URL` / `NAPCAT_ACCESS_TOKEN` — napcat connection
- `FEATURES_ENABLED` — comma-separated feature IDs to load at startup
- `DATABASE_URL` — PostgreSQL connection string (pgvector)
- `OPENAI_BASE_URL` / `OPENAI_API_KEY` / `OPENAI_API_MODEL` — for TL;DR, chat, and other LLM features
- `EMBEDDING_URL` — Embedding service endpoint (Qwen3-Embedding-4B)
- `PERSONA_DEFAULT_PROMPT` — Optional default persona system prompt for chat feature
- Feature-specific keys: `JISU_API_TOKEN`, `GOLD_API_TOKEN`, `TTS_URL`, `GRSAI_API_KEY`, etc.
