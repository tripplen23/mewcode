//! Integration tests for `mewcode_protocol::canvas`.
//!
//! Round-trips the §5 example `graph.json` and `layout.json` from
//! `docs/architecture-canvas/README.md` through serde, plus a few
//! wire-form checks that pin the on-the-wire spelling of the C4
//! taxonomy. This is the T1 acceptance test from
//! `milestone-1-promptable-canvas.md` §3.T1.

use mewcode_protocol::{EdgeKind, Graph, Layout, NodeId, NodeKind, Point};

const GRAPH_JSON: &str = r#"{
  "version": 1,
  "nodes": [
    {
      "id": "auth",
      "kind": "component",
      "name": "Authenticator",
      "bind": "crates/engine/src/auth.rs#Authenticator",
      "contract": [
        "fn verify(&self, token: &str) -> Result<Claims, AuthError>"
      ],
      "tech": "rust",
      "desc": "Validates bearer tokens"
    }
  ],
  "edges": [
    { "from": "auth", "to": "session_store", "kind": "depends" }
  ]
}"#;

const LAYOUT_JSON: &str = r#"{
  "version": 1,
  "positions": { "auth": { "x": 12, "y": 4 }, "session_store": { "x": 12, "y": 10 } },
  "theme": "default"
}"#;

#[test]
fn graph_roundtrips() {
    let parsed: Graph = serde_json::from_str(GRAPH_JSON).expect("graph json parses");
    assert_eq!(parsed.version, 1);
    assert_eq!(parsed.nodes.len(), 1);
    let n = &parsed.nodes[0];
    assert_eq!(n.id.as_str(), "auth");
    assert_eq!(n.kind, NodeKind::Component);
    assert_eq!(n.name, "Authenticator");
    assert_eq!(
        n.bind.as_deref(),
        Some("crates/engine/src/auth.rs#Authenticator")
    );
    assert_eq!(
        n.contract,
        vec!["fn verify(&self, token: &str) -> Result<Claims, AuthError>"]
    );
    assert_eq!(n.tech.as_deref(), Some("rust"));
    assert_eq!(n.desc.as_deref(), Some("Validates bearer tokens"));

    assert_eq!(parsed.edges.len(), 1);
    assert_eq!(parsed.edges[0].from.as_str(), "auth");
    assert_eq!(parsed.edges[0].to.as_str(), "session_store");
    assert_eq!(parsed.edges[0].kind, EdgeKind::Depends);

    let serialised = serde_json::to_string(&parsed).expect("graph serialises");
    let reparsed: Graph = serde_json::from_str(&serialised).expect("graph reparses");
    assert_eq!(reparsed, parsed);
}

#[test]
fn layout_roundtrips() {
    let parsed: Layout = serde_json::from_str(LAYOUT_JSON).expect("layout json parses");
    assert_eq!(parsed.version, 1);
    assert_eq!(parsed.theme, "default");
    assert_eq!(parsed.positions.len(), 2);
    assert_eq!(
        parsed.positions.get(&"auth".to_string().into()),
        Some(&Point { x: 12, y: 4 })
    );
    assert_eq!(
        parsed.positions.get(&"session_store".to_string().into()),
        Some(&Point { x: 12, y: 10 })
    );

    let serialised = serde_json::to_string(&parsed).expect("layout serialises");
    let reparsed: Layout = serde_json::from_str(&serialised).expect("layout reparses");
    assert_eq!(reparsed, parsed);
}

#[test]
fn empty_graph_serialises_to_default() {
    // `Graph::default()` is what the loader will return when no file
    // exists (see T2); verify it round-trips to the same shape that
    // the loader will then write back to disk.
    let g = Graph::default();
    assert_eq!(g.version, 1);
    assert!(g.nodes.is_empty());
    assert!(g.edges.is_empty());

    let serialised = serde_json::to_string(&g).expect("graph serialises");
    let reparsed: Graph = serde_json::from_str(&serialised).expect("graph reparses");
    assert_eq!(reparsed, g);
}

#[test]
fn empty_layout_serialises_to_default() {
    let l = Layout::default();
    assert_eq!(l.version, 1);
    assert!(l.positions.is_empty());
    assert_eq!(l.theme, "default");

    let serialised = serde_json::to_string(&l).expect("layout serialises");
    let reparsed: Layout = serde_json::from_str(&serialised).expect("layout reparses");
    assert_eq!(reparsed, l);
}

#[test]
fn node_kind_wire_form() {
    // The on-the-wire form is the C4 kebab-case spelling per the §5
    // example (`"kind": "component"`).
    assert_eq!(
        serde_json::to_string(&NodeKind::Component).unwrap(),
        "\"component\""
    );
    assert_eq!(
        serde_json::to_string(&NodeKind::System).unwrap(),
        "\"system\""
    );
    assert_eq!(
        serde_json::to_string(&NodeKind::Container).unwrap(),
        "\"container\""
    );
}

#[test]
fn edge_kind_wire_form() {
    assert_eq!(
        serde_json::to_string(&EdgeKind::Depends).unwrap(),
        "\"depends\""
    );
    assert_eq!(
        serde_json::to_string(&EdgeKind::Calls).unwrap(),
        "\"calls\""
    );
    assert_eq!(
        serde_json::to_string(&EdgeKind::Implements).unwrap(),
        "\"implements\""
    );
    assert_eq!(serde_json::to_string(&EdgeKind::Owns).unwrap(), "\"owns\"");
}

#[test]
fn node_id_is_stable() {
    // Same id parsed back is the same value — this is what `Layout`
    // relies on for its HashMap keying.
    let id: NodeId = serde_json::from_str("\"auth\"").unwrap();
    assert_eq!(id.as_str(), "auth");
    let again: NodeId = serde_json::from_str("\"auth\"").unwrap();
    assert_eq!(id, again);
}
