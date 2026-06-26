//! Architecture canvas engine support: load/save + auto-layout.
//!
//! The canvas model is "graph is truth, layout is presentation" — see
//! `mewcode_protocol::canvas` for the data shapes and
//! `docs/architecture-canvas/README.md` §5 for the design. This module
//! is the engine-side glue: reading `graph.json` + `layout.json` off
//! disk, and filling in any missing layout positions via the chosen
//! auto-layout algorithm.
//!
//! See `layout.rs` for the layout crate choice (Q3 spike) and the
//! `ponytail:` comment that names the ceiling.

pub mod layout;
