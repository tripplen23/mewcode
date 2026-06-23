# mewcode — current state

This file is the short, current-state map. For the full per-phase checklists,
decisions, and WHY-comments, see [`PHASES_HISTORY.md`](PHASES_HISTORY.md).
Load `PHASES.md` first; only descend into the history if you need the detail.

## Build status

| # | Phase | Status | Summary |
|---|---|---|---|
| 1 | Workspace skeleton | ✅ | Cargo workspace, 4 crates (`protocol`, `engine`, `server`, `client`), 2 binaries (`mewcode`, `mewcode-server`), wire protocol types |
| 2 | Anthropic-aligned tools + Skills skeleton | ✅ | `ToolDescriptor` / `Skill` / `parse_skill_md` / `ToolRegistry` / `build_system_prompt` |
| 3 | `server` skeleton | ✅ | axum + figment config |
| 4 | Persistence (filesystem) | ✅ | `SessionStore` trait, `FsStore` + `MemoryStore`, XDG data dir layout |
| 5 | `client` shell | ✅ | ratatui event loop, home screen |
| 6 | New session flow | ✅ | Title / mode / model pickers → POST → session screen |
| 7 | Engine v0 | ✅ | rig Anthropic-compat client + first e2e smoke |
| 8 | Conversation history + session resume | ✅ | `HistoryStrategy::Raw`, multi-turn context, resume from `FsStore` |
| 9 | Durable memory scaffold | ✅ | Fact store, `# Memory` section in preamble, `mewcode_memory` tool |
| 10 | Streaming | ✅ | rig streaming → SSE → TUI live tokens |
| 11 | Tool-calling loop | ✅ | `RigToolAdapter`, `MAX_AGENT_TURNS=10`, `ToolInput/OutputAvailable` events, e2e via `agent_tool_e2e.rs` |
| 12 | Remaining tools + PLAN mode gate + Anthropic prompt caching | ✅ | `write_file` / `edit_file` / `bash` / `grep`; mode-aware `default_registry`; `with_automatic_caching()` on Anthropic arm; `cache_read_input_tokens` recorded on spans |
| 13 | Skills runtime | ⬜ | Hot-reload, assets, frontmatter lint, more bundled skills |
| 14 | TUI polish | ⬜ | Markdown render, code blocks, tool cards, theme switcher, slash menu, @-mention popover |
| 15 | Config & persistence | ⬜ | `~/.config/mewcode/config.toml`, last-used model, recent sessions |
| 16 | Hardening | ⬜ | Error toasts, Ctrl-C graceful shutdown, retries, command palette (overlaps Phase 17) |
| 17 | Trace ingestion latency | ⬜ | Active. See below. |

Legend: ✅ done · ⬜ todo

## Active phase — 17. Trace ingestion latency

Traces take ~13 min to appear in Langfuse. Three confirmed root causes
(all verifiable from `opentelemetry_sdk-0.31.0` and
`opentelemetry-langfuse-0.6.1` source, plus [Langfuse's v4 FAQ][langfuse-v4]):

1. **Missing `x-langfuse-ingestion-version: 4` header** — Langfuse's
   Fast Preview path needs this; without it traces land in the S3
   batched path which the FAQ itself documents as "multi-minute
   delays". The langfuse crate's `exporter.rs:185-199` only injects
   `Authorization`, not this header.
2. **Unconfigured `BatchConfig` defaults** — `main.rs:116` uses
   defaults (5s tick, 30s export timeout, batch 512, queue 2048).
3. **No graceful shutdown + no per-turn `force_flush`** — Ctrl-C drops
   in-flight spans; the 5s ticker is the only flush driver.

Fix shape:
- Set the v4 header via `OTEL_EXPORTER_OTLP_HEADERS` (env var is the
  least resistance path; the langfuse builder doesn't expose header
  injection).
- Tune `BatchConfigBuilder`: `scheduled_delay=2s`, `export_timeout=10s`,
  `batch=256`, `queue=4096`.
- Wrap `axum::serve` in `with_graceful_shutdown(tokio::signal::ctrl_c())`
  so the existing `provider.shutdown()` is actually reached.
- Add `force_flush()` at the end of `Harness::run_turn` and the chat
  stream forwarder.

E2E: extend `crates/server/tests/agent_tool_e2e.rs` to assert that a
trace sent through `/chat` returns from a Langfuse API query in <5s.

[langfuse-v4]: https://langfuse.com/faq/all/explore-observations-in-v4

## Key entry points

| Concern | Location |
|---|---|
| Wire protocol types | `crates/protocol/src/` |
| Harness (session/turn orchestrator) | `crates/engine/src/harness/` |
| System prompt composition | `crates/engine/src/agent/prompt.rs` |
| Trace field constants + helpers | `crates/engine/src/harness/trace.rs` |
| Tool adapter (rig `ToolDyn` ↔ mewcode `ToolContracts`) | `crates/engine/src/tools/adapter.rs` |
| Tool registry / mode gate | `crates/engine/src/tools/mod.rs` (`default_registry`) |
| Memory store (durable facts) | `crates/engine/src/memory.rs` |
| OTel/Langfuse init | `crates/server/src/main.rs:73-120` |
| `/chat` SSE route | `crates/server/src/routes/chat.rs` |
| Engine e2e (real LLM + Langfuse verification) | `crates/server/tests/agent_tool_e2e.rs` |

## Architectural decisions (WHY)

- **`HistoryStrategy` enum** (Phase 8) wraps history construction so
  observational/durable memory modes slot in without breaking the call
  site. Currently only `Raw { max_turns }` is implemented.
- **Progressive disclosure for tools + skills** (Phase 2): the system
  prompt carries full tool descriptors but only skill names +
  descriptions. The model calls `use_skill` to read a skill body.
  Keeps the prompt small and stable for cache hits.
- **Mode gate on tool dispatch** (Phase 12): `default_registry` filters
  write tools when `Mode::Plan`; dispatch uses the same filter, so the
  descriptor set the model sees matches the tools it can call.
- **Anthropic caching only** (Phase 12): `with_automatic_caching()` is
  applied on the Anthropic arm only; `cache_control` is
  Anthropic-specific and would be ignored/error on OpenAI-compatible
  endpoints.
- **Span field discipline** (Phase 12 review): `gen_ai.*` fields are
  rig's responsibility; we only set `langfuse.*` fields rig doesn't
  emit. See `trace.rs` for the split.

## References

[tool-guide]: https://www.anthropic.com/engineering/writing-tools-for-agents
[anthropic-caching]: https://platform.claude.com/docs/en/build-with-claude/prompt-caching
[skills-guide]: https://resources.anthropic.com/engineering/writing-skills-for-claude
[mastra-message-history]: https://mastra.ai/docs/memory/message-history
[hermes-memory]: https://github.com/NousResearch/hermes-agent
[langfuse-v4]: https://langfuse.com/faq/all/explore-observations-in-v4
