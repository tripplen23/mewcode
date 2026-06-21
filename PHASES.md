# mewcode — implementation phases

This file tracks the build order agreed in the kickoff plan. Each phase ends with a checkpoint
(stated below) so progress is reviewable in small slices.

[tool-guide]: https://www.anthropic.com/engineering/writing-tools-for-agents
[anthropic-caching]: https://platform.claude.com/docs/en/build-with-claude/prompt-caching
[skills-guide]: https://resources.anthropic.com/hubfs/The-Complete-Guide-to-Building-Skill-for-Claude.pdf
[mastra-memory]: https://mastra.ai/docs/memory/overview
[mastra-observational]: https://mastra.ai/docs/memory/observational-memory
[mastra-message-history]: https://mastra.ai/docs/memory/message-history
[hermes-memory]: https://github.com/NousResearch/hermes-agent

## Phase 1 — Workspace skeleton ✅
- [x] Cargo workspace at `/home/binhfnef/Projects/personal/mew_code/mewcode`
- [x] Toolchain pin (`rust-toolchain.toml`, stable, edition 2024)
- [x] `Cargo.toml` workspace manifest with shared dep versions
- [x] `.gitignore`, `.env.example`, `rustfmt.toml`
- [x] README with project status
- [x] Four crates: `protocol`, `engine`, `server`, `client`
- [x] Two binaries: `mewcode` (CLI dispatcher / TUI), `mewcode-server`
- [x] Wire-protocol types: `Message`, `MessagePart`, `ToolCall`, `ToolResult`, `ModelId`, `Mode`, `StreamEvent`, `ChatRequest`
- [x] Server endpoints: `GET /health`, `GET /models`, `GET/POST /sessions`, `GET /sessions/{id}`, `POST /chat` (SSE)
- [x] Engine `Harness` with placeholder streaming reply (drives the wire protocol end-to-end)
- [x] `mewcode` CLI subcommands: `tui`, `server`, `version`, `hello`
- [x] 8 unit tests passing in `mewcode-protocol`
- [x] `cargo clippy --workspace --all-targets` clean (2 non-blocking `result_large_err` warnings on figment)

Checkpoint: `cargo build` succeeds, all 8 unit tests pass, and the end-to-end SSE
chat pipeline works against the in-memory placeholder (verified with curl).

## Phase 2 — Anthropic-aligned tools + Skills skeleton ✅
- [x] `protocol::tool` rewritten following the [Anthropic tool guide][tool-guide]:
  - [x] `ToolDescriptor { name, description, input_schema, annotations, examples, max_response_chars }`
  - [x] `ToolAnnotations { read_only, destructive, open_world, idempotent }` (MCP-style)
  - [x] `ResponseFormat` enum (`concise` / `detailed`)
  - [x] `ToolExample { description, input }`
  - [x] `ToolError` variants with optional actionable `hint`
  - [x] `ToolErrorPayload` JSON returned to the model on error
  - [x] `truncate_with_marker()` helper for token-efficient responses
  - [x] `resolve_inside_root()` helper for path safety
  - [x] snake_case tool names (`read_file`, `write_file`, `list_directory`, `glob`, `grep`, `edit_file`, `bash`)
  - [x] 11 unit tests covering all of the above
- [x] `protocol::skill` skeleton following the [Anthropic Skills guide][skills-guide]:
  - [x] `Skill { name, description, body, location, assets }`
  - [x] `parse_skill_md()` for `SKILL.md` YAML frontmatter + markdown body
  - [x] `SkillError` (Read / MalformedFrontmatter / MissingField)
  - [x] Constants: `SKILL_FILE`, `GLOBAL_SKILLS_DIR`, `PROJECT_SKILLS_DIR`
  - [x] 5 unit tests
- [x] `engine::skills` module:
  - [x] `SkillRegistry` with `load_defaults()` (global `~/.config/mewcode/skills/`, project `.mewcode/skills/`, plus dev `./skills/`)
  - [x] `LoadedSkill { skill, source: SkillSource }`
  - [x] `find_project_skills_dir_from()` (walks up to repo root)
  - [x] `find_dev_skills_dir_from()` (dev convenience)
  - [x] `catalog_for_system_prompt()` renders the Anthropic-recommended catalog
  - [x] `resolve_body()` for the `use_skill` tool
  - [x] 6 unit tests
- [x] `engine::tools` module:
  - [x] `ToolRegistry` (registry + dispatch returning `ToolErrorPayload`)
  - [x] `ProjectContext` shared with every tool
  - [x] `ReadFileTool` — first real working tool, with all Anthropic guidance
  - [x] `ListDirectoryTool`
  - [x] `GlobTool` (uses `globset` + `ignore`)
  - [x] `UseSkillTool` — the only way the model can read a skill body
  - [x] `default_registry()` factory
  - [x] 5 unit tests
- [x] `engine::agent`:
  - [x] `build_system_prompt(mode, &skills)` — mode-aware, injects skill catalog
  - [x] 4 unit tests (BUILD mentions write tools, PLAN doesn't, skills inject, no-skills = no catalog)
- [x] `engine::harness`:
  - [x] Takes `Arc<SkillRegistry>` + `Arc<ToolRegistry>`
  - [x] `system_prompt()`, `skill_count()`, `tool_names()` accessors
  - [x] Placeholder reply advertises skill count + tool list
- [x] Two bundled sample skills at `skills/review-pr/SKILL.md` and `skills/write-rust-error/SKILL.md`
- [x] Smoke test that confirms the bundled skills load via `load_defaults()`
- [x] **Progressive disclosure** wiring:
  - [x] `format_tool_descriptors(&ToolRegistry)` in `engine::agent` renders full descriptors
    (name, description, safety, schema, examples) sorted alphabetically
  - [x] `build_system_prompt` accepts `&ToolRegistry` and injects the descriptors after
    the mode-specific prose
  - [x] Skill catalog (name + description only) appended last; body remains on demand
    via `use_skill`
  - [x] `dump_system_prompt` example lets you eyeball the result
  - [x] Tests:
    - `tool_descriptors_are_injected_when_present`
    - `empty_registry_yields_no_tool_block`
    - `tools_are_sorted_alphabetically`
    - `build_mode_includes_tool_descriptors`
    - `plan_mode_excludes_write_tool_descriptors`

Checkpoint: 33 unit tests pass (15 engine + 18 protocol), workspace builds clean,
the SSE chat pipeline advertises its tool + skill count in the placeholder
reply, and the Anthropic-aligned tool design is wired through the
registry.

## Phase 3 — `server` skeleton ✅
- [x] axum app with `GET /health`
- [x] Config loader (figment: env + optional toml)
- [x] Error type with `IntoResponse`

## Phase 4 — Persistence layer (filesystem) ✅
- [x] `SessionStore` trait with `FsStore` (default) + `MemoryStore` backends
- [x] Sessions persist as `meta.json` + `messages.jsonl` per session under the XDG data dir
- [x] Routes: `GET /sessions`, `POST /sessions`, `GET /sessions/{id}`, `DELETE /sessions/{id}`, `GET /storage/status`

## Phase 5 — `client` shell ✅
- [x] ratatui event loop, root layout
- [x] Home screen lists sessions from server
- [x] First `insta` snapshot of home screen

## Phase 6 — New session flow ✅
- [x] Title / mode / model picker dialogs
- [x] POST to server, navigate to session screen

## Phase 7 — Engine v0 ✅
- [x] rig Anthropic-compat client for `https://opencode.ai/zen/go/v1/messages`
- [x] First end-to-end smoke test

## Phase 8 — Conversation history + session resume ✅
- Fix the in-session history bug: `Harness::run_turn` currently sends only
  the latest user turn (`last_user_text` + `agent.prompt(text)`), so the model
  has no context for follow-up questions; this also means the agent on a
  resumed session has zero context
- Replace `Provider::invoke_agent(text)` with a history-aware call: map
  `&[mewcode_protocol::Message]` to `Vec<rig_core::message::Message>` and
  hand it to the agent via `with_history(...)` on `PromptRequest` /
  `StreamingPromptRequest`
- Wrap the history-construction in a `HistoryStrategy` enum so future
  memory modes slot in without breaking the call site:
  ```rust
  enum HistoryStrategy {
      Raw { max_turns: usize },
      // Summarized { max_tokens: u64 },     // observational memory (future)
      // DurableFactInjected { ... },         // memory-scaffold mode (future)
  }
  ```
- Token-aware window: keep the system prompt, keep recent N turns verbatim,
  summarise or drop older turns; start with a conservative N (e.g. 20) and
  tune per model
- Load history from `FsStore` when the client opens a session — the server
  `/chat` endpoint already receives the full `&[Message]`, so the plumbing
  is just: store → deserialize → attach to `ChatRequest`
- Tests: a multi-turn end-to-end against the harness, a property test
  that the model receives every prior turn, and a session-resume test that
  verifies loaded history is passed to `with_history()`
- Refs: [Mastra message history][mastra-message-history]

Checkpoint: follow-up questions in a session have full context,
session resume works, `HistoryStrategy` is wired with `Raw` mode,
all tests pass.

## Phase 9 — Durable memory scaffold ✅
- [x] Design a simple fact store (one `.md` file per profile under
  `~/.mewcode/memories/`) that holds durable user facts — the agent's
  equivalent of the Hermes Agent MEMORY.md / USER.md system
- [x] Each memory file has a name and optional category; content is free-form
  markdown the agent reads and writes
- [x] On harness creation, inject the active memory profile into the system
  prompt as a `# Memory` section, so the agent sees its persistent facts
  every turn
- [x] Add a tool `mewcode_memory` (read/write/list) so the agent can update
  its own memory; the tool dispatches to the fact store on the server
- [x] Server endpoint: `GET/POST /memory` (read / write the active profile)
- [x] CLI stub: `mewcode memory [read|write|list]`
- [x] Wire the fact store into `HistoryStrategy` as a wrapper step:
  durable facts are injected into the prompt preamble, not into the
  conversation message list — they are context, not history
- Ref: [Hermes Agent memory][hermes-memory]

Checkpoint: agent sees durable facts every turn, can update them via tool,
`mewcode memory list` shows the active profile, tests cover read/write/lifecycle.

## Phase 10 — Streaming ✅
- [x] Wire rig streaming completion into SSE on the server
- [x] Tokens stream live to the TUI

## Phase 11 — Tool-calling loop ✅
- [x] Bridge mewcode's `ToolContracts` trait to Rig's `ToolDyn` via a
  `RigToolAdapter` wrapper so the Rig agent can call mewcode tools
  natively
- [x] Wire the `ToolRegistry` (built via `default_registry`) into the Rig
  agent builder in `Provider::invoke_agent_streaming` — pass tools via
  `.tools(Vec<Box<dyn ToolDyn>>)`
- [x] Increase `MAX_AGENT_TURNS` from 1 to 10 to allow multi-turn tool-call →
  result → response cycles (Rig handles the loop internally)
- [x] Emit `StreamEvent::ToolInputAvailable` and `ToolOutputAvailable` from
  `stream_agent_completion` when the stream yields
  `StreamedAssistantContent::ToolCall` and `StreamUserItem` items
- [x] Exercise `read_file` end-to-end: the model asks to read a file, the
  adapter dispatches to `ReadFileTool::execute`, the result goes back
  to the model, and the final reply references the file contents
- [x] Also exercise `mewcode_memory` end-to-end: the model writes a fact,
  the adapter dispatches to `MewcodeMemoryTool::execute`, the fact
  persists to `memories/default.md`
- [x] E2E integration test with real LLM calls + Langfuse trace verification
  (`crates/server/tests/agent_tool_e2e.rs`)
- [x] Addressed Copilot + CodeRabbit review comments (deterministic tool
  ordering, explicit JSON parse errors, canonicalized project root,
  collision-resistant temp paths, accurate comments)
- Ref: [Anthropic tool guide][tool-guide]

Checkpoint: the agent can call `read_file` and `mewcode_memory` during
a chat turn, the TUI sees `ToolInputAvailable`/`ToolOutputAvailable`
events, and all existing tests still pass.

## Phase 12 — Remaining tools + PLAN mode gate + Anthropic prompt caching
Builds directly on the `ToolContracts` / `RigToolAdapter` / `ToolDyn` plumbing
shipped in Phase 11: every new tool is just a `ToolContracts` impl that gets
picked up by `default_registry`, and every dispatch flows through the same
adapter. The mode gate closes the descriptor-vs-dispatch gap. Caching is
included because the system prompt + tool layer is large and stable across
the up-to-10 sub-turns the Rig loop can take, so a missed cache hit burns
real money on every multi-turn chat.

**Write-side tools**
- `write_file` — `ToolAnnotations::WRITE_LOCAL`, refuses to escape the
  project root via `resolve_inside_root`, refuses to overwrite a non-empty
  file unless `overwrite: true` (Anthropic guide: confirm destructive ops)
- `edit_file` — single-target string replace; refuses to edit a file that
  doesn't exist; returns the exact byte range it changed; `WRITE_LOCAL`
- `bash` — `ToolAnnotations::BASH`, timeout-bounded, output truncated
  with `truncate_with_marker`; `destructive: true` so the PLAN mode gate
  blocks it without an extra config knob
- New tools register in `default_registry(ctx, skills, memory, mode)`;
  keep the call signature backwards compatible by defaulting `mode` to
  `Mode::Build`

**Read-side tools**
- `glob` is already in Phase 11 — make sure its `destructive: false` is
  preserved through the mode filter
- `grep` — `ToolAnnotations::READ_ONLY_IDEMPOTENT`, uses the `grep` crate
  already in the workspace, respects `.gitignore` via the `ignore` crate
- `list_directory` is already in Phase 11 — same audit

**PLAN mode gate**
- New `default_registry(ctx, skills, memory, mode)` filters tools by
  `descriptor().annotations.read_only && !annotations.destructive` when
  `mode == Mode::Plan` — matches the descriptors the system prompt
  already excludes, so the model sees the same tool set in both places
- `dispatch()` keeps the existing `ToolNotFound` error path so a model
  that tries to call a filtered tool gets the same error shape as an
  unregistered one (no new error variant)
- `chat_stream` in the server route passes `req.mode` into the registry
  factory; no other call site changes
- Tests: a `plan_mode_filters_write_tools` and a
  `plan_mode_dispatch_rejects_filtered_tool` that both fail closed

**Anthropic prompt caching**
- Refactor `Provider::Anthropic` in `engine/src/agent/mod.rs:85-96`:
  build the `CompletionModel` via `p.client().completion_model(model_id)`
  then call `.with_automatic_caching()` before handing it to
  `AgentBuilder::new(model)` (rig-core 0.38.2's `AgentBuilder` has no
  caching setter, so this is the only way — `client().agent(...)` always
  builds a fresh `CompletionModel` with caching off)
- Don't enable on the OpenAI arm — `cache_control` is Anthropic-specific
  and gets ignored / errors on OpenAI-compatible endpoints
- Pull `cached_input_tokens` and `cache_creation_input_tokens` out
  of the `MultiTurnStreamItem::CompletionCall(call).usage` struct
  (rig-core's cross-provider `Usage` exposes these as `cached_input_tokens`
  and `cache_creation_input_tokens` — the Anthropic-specific struct uses
  `cache_read_input_tokens`, which the conversion in
  `providers/anthropic/completion.rs:139-140` maps to `cached_input_tokens`)
  and record them on the `chat-turn` span via the existing
  `gen_ai.usage.cache_read.input_tokens` / `cache_creation.input_tokens`
  fields (`trace.rs:89-90` already declares them; the wiring was the
  missing piece)
- The upstream `Refactor` (remote commit `90c9227`) already extracted
  the system-prompt composition into `Harness::compose_system_prompt()`
  (`harness/mod.rs:99`), with `system_prompt()` (`harness/mod.rs:92-94`)
  as the public accessor delegating to it. The original "delete the
  dead `Harness::system_prompt()`" bullet is no longer needed; the
  Phase 12 implementer can grow `compose_system_prompt` if the new
  mode-aware tool filtering needs to live there too
- E2E test in `crates/server/tests/agent_tool_e2e.rs` (extend, don't
  duplicate): run a 2-turn chat that reads the same file twice and
  assert `gen_ai.usage.cache_read.input_tokens > 0` on the second
  turn's span. If the assertion fails the test should point at the
  exact span field, not just dump usage. The harness maps rig-core's
  cross-provider `Usage::cached_input_tokens` into that span field —
  the test asserts the field, not the raw rig struct

**E2E coverage**
- `write_file` and `edit_file` round-trip: model writes a file, server
  persists the message, reload from `FsStore` shows the new file
- `bash` runs a `cargo --version` and reports stdout back; assert the
  truncation marker fires above the cap
- `grep` finds a marker string in the workspace and returns matches
- PLAN-mode turn that asks the model to write a file: the request
  fails closed at the registry layer, the model sees a `ToolNotFound`
  error, and no file is created
- A multi-turn chat that exercises caching: read a file → ask a
  follow-up that needs context from the first read; second turn's
  span has `gen_ai.usage.cache_read.input_tokens > 0`
  (the harness maps `Usage::cached_input_tokens` into that span
  field — the e2e test asserts the field, not the raw rig struct)
- Ref: [Anthropic tool guide][tool-guide], [Anthropic prompt caching][anthropic-caching]

Checkpoint: the agent can read, write, edit, glob, grep, and run bash;
PLAN mode refuses to dispatch write tools; Anthropic prompt caching
is on by default; every tool call is traced; all existing tests still
pass.

## Phase 13 — Skills runtime
- Skill hot-reload: pick up new or changed `SKILL.md` files without restarting
- Skill assets: bundle files alongside the body, exposed via `use_skill`
- Lint `SKILL.md` frontmatter on load, surface errors at boot
- More bundled sample skills (`explain-error`, `refactor-rust`)
- Ref: [Anthropic Skills guide][skills-guide]

## Phase 14 — TUI polish
- Markdown rendering (`tui-markdown`)
- Code blocks with `syntect`
- Tool cards, theme switcher, slash command menu, @-mention popover
- Toast, trace pane, animations

## Phase 15 — Config & persistence
- `~/.config/mewcode/config.toml`
- Last-used model, theme, recent sessions

## Phase 16 — Hardening
- Error toasts, Ctrl-C graceful shutdown, retries, command palette
