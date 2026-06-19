//! `GET/POST /memory` — read and write the active memory profile.
//!
//! Memory is stored as a single markdown file per profile in the
//! server's data directory under `memories/<profile>.md`.  The active
//! profile is wired through the shared app state.

use axum::Json;
use axum::extract::State;
use serde::{Deserialize, Serialize};

use crate::AppState;

/// `GET /memory` — return the current memory content.
#[derive(Serialize)]
pub struct MemoryResponse {
    /// Profile name.
    pub profile: String,
    /// Markdown content of the memory file.
    pub content: String,
}

/// `POST /memory` body.
#[derive(Deserialize)]
pub struct MemoryWriteRequest {
    /// New memory content (plain markdown). Overwrites the current file.
    pub content: String,
}

/// `GET /memory`
pub async fn get_memory(State(state): State<AppState>) -> Json<MemoryResponse> {
    let content = state.memory.read();
    Json(MemoryResponse {
        profile: "default".to_string(),
        content,
    })
}

/// `POST /memory`
pub async fn post_memory(
    State(state): State<AppState>,
    Json(req): Json<MemoryWriteRequest>,
) -> Json<MemoryResponse> {
    if let Err(e) = state.memory.write(&req.content) {
        tracing::warn!(error = %e, "failed to write memory");
    }
    let content = state.memory.read();
    Json(MemoryResponse {
        profile: "default".to_string(),
        content,
    })
}
