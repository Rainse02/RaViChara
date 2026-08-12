# RaViChara architecture

## Runtime

The active runtime is a single Rust process:

```text
Browser UI
   │ HTTP/JSON + WebSocket
   ▼
Axum server
   ├─ LLM client ──────────────────── LM Studio native or OpenAI compatible
   ├─ per-character MemoryService ─── SQLite
   ├─ PersonaCard catalog ─────────── YAML/JSON
   ├─ BlenderBridge ───────────────── local TCP plugin
   ├─ stateless MCP endpoint ───────── /mcp
   ├─ interaction broadcast ────────── /api/interactions/stream
   ├─ Blender PNG frame stream ─────── /api/blender/stream
   └─ static file service ─────────── static/
```

Chat requests are serialized with a Tokio mutex so one model generation cannot race another. SQLite work runs through `spawn_blocking`. The two WebSocket paths are bounded: interactions use a 128-event broadcast channel, while Blender captures the next frame only after the previous frame has been sent.

## Chat transaction

1. Validate message size and optional client-time metadata.
2. Load active persona and its isolated memory service.
3. Retrieve recent messages, relevant facts, and relevant episode summaries.
4. Add an authoritative server-local timestamp plus sanitized client time on every turn.
5. Build a bounded system/persona/memory prompt and call the configured LLM.
6. Accept only final `message.content`; internal `reasoning_content` is never exposed.
7. Prefer MCP tool events; otherwise run an isolated behavior-planning request with the current persona and Blender capability snapshot. Legacy control tags remain compatibility input only.
8. Store the user message and clean character reply in one SQLite transaction, only after generation succeeds.
9. Run fact extraction and optional consolidation in a blocking background task.
10. If Blender pose mode is enabled, dispatch one bounded temporary behavior plan (or a preset fallback), then return to persona idle.

Provider failures are returned as structured non-2xx JSON errors. The frontend displays those errors and does not fabricate replies or memory-success notices.

The provider registry is exposed by `GET /api/llm/providers`. It supplies UI defaults for local and hosted OpenAI-compatible services while keeping the actual Base URL and model editable. `custom` accepts any compatible endpoint and can operate with or without an API key.

LM Studio uses its native `/api/v1/chat` endpoint. This exposes `input_tokens`, `total_output_tokens`, and `reasoning_output_tokens`, which are mapped into the common response `usage`. Generic providers continue to use `/v1/chat/completions`. Chat responses also include non-secret `prompt_metrics` so system, example, recent-message, and memory costs can be separated.

The old reasoning-only fallback could resend the complete prompt with a doubled output budget. It is now disabled by default. Qwen3.5 models that do not expose LM Studio's native reasoning setting receive a prompt hint only; the returned `reasoning_tokens` remains the authoritative measurement.

The app exposes a stateless Streamable HTTP MCP with a bounded avatar-control
catalog. Ordinary chat receives capability discovery,
`ravichara_apply_behavior_plan`, and the compatibility
`ravichara_apply_reaction`. A behavior plan contains model-validated semantic
roles or exact advertised bone/morph names, normalized keyframe times and local
XYZ offsets. Requests that explicitly concern
Blender, rigging, animation, materials, or shaders receive only the relevant
inspection and proposal tools. This per-turn tool gating avoids injecting the
full schema catalog into casual first-turn prompts.

Providers without a usable MCP tool channel use the same behavior protocol
through an isolated second LLM request after the visible reply is complete. The
request receives a bounded character-card summary and the active Blender
capability snapshot. Its JSON is parsed outside the visible message, checked
against exact advertised channels, angle-clamped, and rejected to the preset
fallback when invalid. Thus DeepSeek, MiMo and generic OpenAI-compatible APIs do
not need provider-specific MCP support and cannot leak control syntax into chat.

Read-only tools cover render status, compact avatar capabilities, paginated rig,
facial-morph and scene inspection, material listing, and shader inspection.
Persistent pose, Action, exact morph-weight, expression-sequence, shader, and
animation mutations are proposal-only:
the MCP call writes a bounded, ten-minute, memory-only proposal and publishes a
UI event. The mutation executes only after `POST
/api/blender/proposals/:id` receives an explicit UI approval. The queue holds at
most 16 entries and is never written to disk.

LM Studio only injects MCP into model requests when either a configured
`mcp/<server>` plugin ID is present or the MCP URL is remote HTTPS. Loopback
ephemeral MCP is deliberately omitted because LM Studio rejects non-public
dynamic MCP addresses. Chat responses report `mcp_mode`, `mcp_model_access`,
`mcp_tool_scope`, and the exact allowlisted tool count for token diagnostics.

## Configuration and security

`AppConfig` is loaded in this order:

```text
built-in defaults ← config/settings.yaml ← data/overrides.json
```

All structures use serde defaults, followed by explicit validation. Runtime settings are allowlisted. API keys entered in the settings UI are stored in the local `data/overrides.json` file and are never returned by the settings API; portable package creation scrubs these fields. Authenticated LM Studio servers can also supply their token through `LM_API_TOKEN`. Character switches reject absolute paths and paths outside `characters/`. Database migrations add missing legacy columns before creating indexes, so an existing memory database is upgraded in place.

## Memory

Each character uses `data/memories/<safe-name>.db`.

Tables:

- `messages`: original user/character text and consolidation state.
- `facts`: text, importance, vector, access metadata, source message.
- `episodes`: hierarchical summaries, vector, merge state, message range.
- `state` and `meta`: reserved extension points.

Recall combines cosine similarity, importance, and exponential recency. Facts are reinforced instead of duplicated when normalized text matches or vector similarity exceeds the configured threshold.

The default `HashEmbedder` creates deterministic local vectors from Chinese characters/bigrams and alphanumeric tokens. `MemoryEmbedder` is replaceable. The default `HeuristicConsolidator` creates bounded summaries without an external call; `MemoryConsolidator` is the replacement boundary for a future LLM-based implementation.

Consolidation preserves the newest messages, marks older messages as consolidated, creates L0 episodes, then recursively merges sufficiently large episode groups into higher levels. Chat messages, facts, and episodes are persistent and unlimited by default. Each retention setting uses zero for unlimited; a positive value explicitly opts into pruning, and raw-message pruning still applies only to consolidated rows while episode pruning applies only to merged rows. `GET /api/chat/history` restores the active character's messages with bounded cursor pagination. `POST /api/memory/maintenance` can checkpoint the WAL and optionally compact the database. SQLite uses a 256-page WAL auto-checkpoint and a 4 MiB journal size limit.

## Blender

The bridge uses:

```text
4-byte unsigned big-endian payload length
UTF-8 JSON payload:
{version, request_id, command, params, token}
```

The generic bridge implements `system.ping`; inspection covers the scene, MMD models, and expressions. The `virtual_c` profile discovers an unambiguous model/armature and applies supported expressions with the plugin's native parameters. Companion extension v0.5.5 adds bounded camera capture plus RigProfile v3 to the same authenticated dispatcher. It reads mmd_tools metadata and live Blender constraints, separates active IK effectors from FK links, withholds partially blended or ambiguous IK chains, maps common semantic roles, converts character-left/forward/up offsets into each PMX control's real pose channels, and resolves upper-arm, elbow, and wrist intent in anatomical character space instead of imported local Euler axes. It calibrates A/T rest poses into a symmetric relaxed idle, exposes a filtered exact-bone catalog for nonstandard rigs, inventories exact shape keys, and supplies per-axis limits. Generated rotations are relative to the active persona-idle baseline. The extension reuses one temporary Action, restores transient morph values, removes the Action at completion, and resumes idle; it does not save the `.blend`. Continuous preview uses Blender's managed camera renderer rather than timer-driven `GPUOffScreen.draw_view3d`; one unique temporary PNG bridges Blender 4.5's render result to Python and is deleted immediately after reading, while only one frame remains cached in process memory. Every connection and read/write operation has a timeout; response frames are capped at 32 MiB.

The render pane has three independent frontend adapters:

- `2d`: CSS/SVG idle and short emote/motion classes driven by MCP interaction events or safe control-tag fallback;
- `blender`: consumes the no-queue `/api/blender/stream` WebSocket and displays in-memory PNG frames;
- `video`: plays a configured HTTP(S) standard-video or MJPEG/continuous-image URL from a local or remote inference/streaming pipeline.

The video adapter is deliberately outside the chat critical path. RaViChara stores only its URL and playback flags; it does not download or write video frames into project storage. Provider-specific asynchronous video-generation job submission remains an extension point because job schemas differ, while any service that exposes its completed or live output through HTTP(S) can be displayed immediately.

## Voice and desktop runtime

Browser ASR is handled by the Web Speech API. TTS can execute in browser SpeechSynthesis or through Rust adapters for OpenAI-compatible `/audio/speech` and MiMo Chat Completions audio. Historical messages are synthesized again on demand; no audio or Blender frame is written to a runtime cache.

On Windows, Winit owns the native event loop and Wry hosts the UI in the
installed WebView2 Runtime. A multi-thread Tokio runtime serves Axum inside the
same process. Closing the native window closes its WebSockets, requests graceful
server shutdown and releases SQLite connections. Static UI files are embedded
in the executable. A portable marker keeps data beside the EXE; a standalone
EXE without the marker seeds `%LOCALAPPDATA%\RaViChara` (an existing legacy EverChara data directory remains usable without copying it).

## Verification

The offline suite uses temporary SQLite databases, a fake OpenAI-compatible HTTP server, and a fake length-prefixed Blender TCP listener. Live end-to-end verification can additionally target the configured LM Studio server and Blender plugin. The supported build/test path uses only Cargo commands available in Rust's minimal profile; `rustfmt` and `clippy` are not prerequisites.

Stable read-only MCP protocol evaluations are recorded in
`docs/evaluations/ravichara_mcp_behavior.xml`.
