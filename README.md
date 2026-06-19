# mewcode

```
⠀⠀⠀⠀⠀⠀⠀⠀⠀⠀⠀⠀⠀⠀⠀⠀⢀⡴⠞⢳⠀⠀⠀⠀⠀
⠀⠀⠀⠀⠀⠀⠀⠀⠀⠀⠀⠀⠀⠀⠀⡔⠋⠀⢰⠎⠀⠀⠀⠀⠀
⠀⠀⠀⠀⠀⠀⠀⠀⠀⠀⠀⠀⠀⠀⣼⢆⣤⡞⠃⠀⠀⠀⠀⠀⠀
⠀⠀⠀⠀⠀⠀⠀⠀⠀⠀⠀⠀⠀⣼⢠⠋⠁⠀⠀⠀⠀⠀⠀⠀⠀
⠀⠀⠀⠀⢀⣀⣾⢳⠀⠀⠀⠀⢸⢠⠃⠀⠀⠀⠀⠀⠀⠀⠀⠀⠀
⣀⡤⠴⠊⠉⠀⠀⠈⠳⡀⠀⠀⠘⢎⠢⣀⣀⣀⠀⠀⠀⠀⠀⠀⠀
⠳⣄⠀⠀⡠⡤⡀⠀⠘⣇⡀⠀⠀⠀⠉⠓⠒⠺⠭⢵⣦⡀⠀⠀⠀
⠀⢹⡆⠀⢷⡇⠁⠀⠀⣸⠇⠀⠀⠀⠀⠀⢠⢤⠀⠀⠘⢷⣆⡀⠀
⠀⠀⠘⠒⢤⡄⠖⢾⣭⣤⣄⠀⡔⢢⠀⡀⠎⣸⠀⠀⠀⠀⠹⣿⡀
⠀⠀⢀⡤⠜⠃⠀⠀⠘⠛⣿⢸⠀⡼⢠⠃⣤⡟⠀⠀⠀⠀⠀⣿⡇
⠀⠀⠸⠶⠖⢏⠀⠀⢀⡤⠤⠇⣴⠏⡾⢱⡏⠁⠀⠀⠀⠀⢠⣿⠃
⠀⠀⠀⠀⠀⠈⣇⡀⠿⠀⠀⠀⡽⣰⢶⡼⠇⠀⠀⠀⠀⣠⣿⠟⠀
⠀⠀⠀⠀⠀⠀⠈⠳⢤⣀⡶⠤⣷⣅⡀⠀⠀⠀⣀⡠⢔⠕⠁⠀⠀
⠀⠀⠀⠀⠀⠀⠀⠀⠀⠀⠀⠀⠈⠙⠫⠿⠿⠿⠛⠋⠁⠀⠀⠀⠀
```

**A terminal coding agent that clones, audits, and ships better apps.**

**Inspired by Hermes Agent. Built on Rig. Aimed at replacing the JS/Python stack with Rust.**

## Why

Existing AI coding agents are impressive but closed or JS-bound. mewcode is the
opposite: a native, single-binary agent you run in your terminal. It borrows
architecture from [Hermes Agent](https://hermes-agent.nousresearch.com) (skill
system, memory, session management, streaming, MCP) and re-implements the whole
stack in Rust — not as a clone, but as a synthesis with better performance and
zero runtime overhead.

The long arc: replace Hermes Agent's Python/JS stack with a drop-in Rust-native
equivalent that's faster, more portable, and just as extensible via plugins and
skills.

## Current status

Phase 1–10 complete. Working today:

- **Streaming chat** — tokens stream live to a ratatui terminal UI
- **Persistent sessions** — conversations survive restart, stored as JSONL
- **Conversation history** — multi-turn context with message-count windowing
- **Durable memory** — agent remembers persistent facts across sessions
- **Tool system** — Anthropic-aligned tool descriptors, registry, and dispatch
- **Skills system** — loadable SKILL.md files the agent can reference
- **System prompt builder** — mode-aware (Build / Plan), injects skills + tools
- **OpenTelemetry tracing** — Langfuse integration for observability
- **Provider routing** — both Anthropic-compatible (`/v1/messages`) and
  OpenAI-compatible (`/v1/chat/completions`) providers
- **Memory API** — CLI, server endpoints, and agent tool for read/write/list

Not yet wired (landing in subsequent phases):

- Tool execution loop (the agent can describe tools but not yet call them)
- Streaming rendering with syntax highlighting
- MCP server integration
- Multi-platform bridges (Discord, WhatsApp, SMS)

## Architecture

```
mewcode/
  crates/
    protocol/  shared types, wire format (no I/O)
    engine/    Rig-based agent harness, tools, skills, streaming, memory
    server/    axum backend + session store + memory API
    client/    ratatui terminal UI + CLI dispatcher

  skills/      bundled SKILL.md files (loaded at startup)
  phases.md    build plan, phase-by-phase
```

One binary, three entry points:

```bash
mewcode server   # start the axum backend
mewcode tui      # open the ratatui client
mewcode memory   # read, write, list persistent memory
```

Project is tracked in [`PHASES.md`](PHASES.md), which lists every remaining
feature in build order with checkpoints.

## Getting started

```bash
cp .env.example .env
# Set OPENCODE_GO_API_KEY to an OpenCode Go or compatible endpoint key.

cargo run -p mewcode-server &   # start the backend
cargo run -p mewcode-client -- tui  # open the TUI
```

Requires Rust 1.85+ (edition 2024).

## Philosophy

**Clone Hermes' architecture. Rewrite in Rust.**

Every subsystem in mewcode starts as a faithful Rust re-implementation of a
Hermes Agent mechanism — skills, memory, session management, tool routing,
MCP integration, platform bridges. Then we make it tighter: fewer dependencies,
better type safety, no garbage collector, zero-copy where it counts.

No grand claims. It's just a Rust binary that codes. That's the whole point.

## License

MIT
