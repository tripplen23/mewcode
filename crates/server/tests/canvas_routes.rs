//! Canvas route round-trip: `GET /canvas/{graph,layout}` returns
//! the project's `.mewcode/canvas/{graph,layout}.json` as JSON.
//!
//! The route resolves `project_root` from `ServerConfig`. Tests
//! inject the project root via `with_canvas_project_root` so they
//! can run in parallel without racing on process CWD (the previous
//! version mutated `std::env::current_dir` and CI's `cargo test`
//! runs with the default thread count, which flaked).

use std::sync::Arc;

use axum::body::Body;
use axum::http::{Request, StatusCode};
use http_body_util::BodyExt;
use mewcode_engine::memory::MemoryStore as FactStore;
use mewcode_protocol::routes::{CANVAS_GRAPH, CANVAS_LAYOUT};
use mewcode_server::store::SessionStore;
use mewcode_server::store::memory::MemoryStore as SessionMemStore;
use mewcode_server::{AppState, ServerConfig, build_app};
use serde_json::Value;
use tempfile::TempDir;
use tower::ServiceExt;

fn test_config(project_root: &std::path::Path) -> ServerConfig {
    ServerConfig {
        host: "127.0.0.1".into(),
        port: 0,
        opencode_go_api_key: "test-key".into(),
        default_model: None,
        log: "off".into(),
        skills: Default::default(),
        canvas_project_root_override: Some(project_root.to_path_buf()),
    }
}

fn app(project_root: &std::path::Path) -> axum::Router {
    let store: Arc<dyn SessionStore> = Arc::new(SessionMemStore::default());
    let fact_store = FactStore::new(std::env::temp_dir().join(uuid::Uuid::new_v4().to_string()));
    build_app(AppState::new(test_config(project_root), store, fact_store))
}

async fn body_json(resp: axum::response::Response) -> (StatusCode, Value) {
    let status = resp.status();
    let body = resp.into_body().collect().await.unwrap().to_bytes();
    let json: Value = serde_json::from_slice(&body).unwrap_or(Value::Null);
    (status, json)
}

/// First-run: no `.mewcode/canvas/` on disk -> both endpoints return
/// the protocol's empty defaults, never an error. Mirrors the
/// engine's `load_or_default` behaviour at the HTTP boundary.
#[tokio::test]
async fn empty_project_returns_empty_defaults() {
    let tmp = TempDir::new().unwrap();

    let (g_status, g_body) = body_json(
        app(tmp.path())
            .oneshot(
                Request::builder()
                    .uri(CANVAS_GRAPH)
                    .body(Body::empty())
                    .unwrap(),
            )
            .await
            .unwrap(),
    )
    .await;
    assert_eq!(g_status, StatusCode::OK);
    assert_eq!(g_body["nodes"], serde_json::json!([]));
    assert_eq!(g_body["edges"], serde_json::json!([]));

    let (l_status, l_body) = body_json(
        app(tmp.path())
            .oneshot(
                Request::builder()
                    .uri(CANVAS_LAYOUT)
                    .body(Body::empty())
                    .unwrap(),
            )
            .await
            .unwrap(),
    )
    .await;
    assert_eq!(l_status, StatusCode::OK);
    assert_eq!(l_body["positions"], serde_json::json!({}));
}

/// Round-trip: a 3-node/2-edge graph written to disk via the
/// engine's `save_graph` is returned by `GET /canvas/graph` with
/// every field preserved.
#[tokio::test]
async fn written_graph_is_returned_unchanged() {
    use mewcode_engine::canvas::io::save_graph;
    use mewcode_protocol::canvas::{Edge, EdgeKind, Graph, Node, NodeId, NodeKind};

    let tmp = TempDir::new().unwrap();

    let graph = Graph {
        version: 1,
        nodes: vec![
            Node {
                id: NodeId("a".into()),
                kind: NodeKind::Component,
                name: "A".into(),
                bind: None,
                contract: vec![],
                tech: None,
                desc: None,
            },
            Node {
                id: NodeId("b".into()),
                kind: NodeKind::Container,
                name: "B".into(),
                bind: None,
                contract: vec![],
                tech: None,
                desc: None,
            },
        ],
        edges: vec![Edge {
            from: NodeId("a".into()),
            to: NodeId("b".into()),
            kind: EdgeKind::Depends,
        }],
    };
    save_graph(tmp.path(), &graph).unwrap();

    let (status, body) = body_json(
        app(tmp.path())
            .oneshot(
                Request::builder()
                    .uri(CANVAS_GRAPH)
                    .body(Body::empty())
                    .unwrap(),
            )
            .await
            .unwrap(),
    )
    .await;
    assert_eq!(status, StatusCode::OK);
    assert_eq!(body["nodes"].as_array().unwrap().len(), 2);
    assert_eq!(body["edges"].as_array().unwrap().len(), 1);
    assert_eq!(body["nodes"][0]["id"], "a");
    assert_eq!(body["nodes"][0]["kind"], "component");
    assert_eq!(body["nodes"][1]["kind"], "container");
    assert_eq!(body["edges"][0]["kind"], "depends");
}
