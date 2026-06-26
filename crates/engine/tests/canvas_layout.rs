//! Tests for the canvas auto-layout engine.

use std::collections::HashMap;

use mewcode_engine::canvas::layout::{ResolvedLayout, auto_layout};
use mewcode_protocol::canvas::{Edge, EdgeKind, Graph, Node, NodeId, NodeKind, Point};

fn node(id: &str, name: &str) -> Node {
    Node {
        id: NodeId(id.to_string()),
        kind: NodeKind::Component,
        name: name.to_string(),
        bind: None,
        contract: Vec::new(),
        tech: None,
        desc: None,
    }
}

fn edge(from: &str, to: &str) -> Edge {
    Edge {
        from: NodeId(from.to_string()),
        to: NodeId(to.to_string()),
        kind: EdgeKind::Depends,
    }
}

#[test]
fn empty_graph_yields_empty_layout() {
    let g = Graph::default();
    let resolved: ResolvedLayout = auto_layout(&g, &HashMap::new());
    assert!(resolved.is_empty());
}

#[test]
fn three_node_two_edge_assigns_distinct_positions() {
    let g = Graph {
        version: 1,
        nodes: vec![node("a", "A"), node("b", "B"), node("c", "C")],
        edges: vec![edge("a", "b"), edge("b", "c")],
    };
    let resolved = auto_layout(&g, &HashMap::new());
    assert_eq!(resolved.len(), 3);

    assert_eq!(
        resolved.get(&NodeId("a".to_string())),
        Some(&Point { x: 0, y: 0 })
    );
    assert_eq!(
        resolved.get(&NodeId("b".to_string())),
        Some(&Point { x: 24, y: 0 })
    );
    assert_eq!(
        resolved.get(&NodeId("c".to_string())),
        Some(&Point { x: 48, y: 0 })
    );
}

#[test]
fn existing_positions_are_preserved() {
    let g = Graph {
        version: 1,
        nodes: vec![node("a", "A"), node("b", "B")],
        edges: vec![],
    };
    let pinned = HashMap::from([(NodeId("a".to_string()), Point { x: 5, y: 5 })]);
    let resolved = auto_layout(&g, &pinned);
    assert_eq!(
        resolved.get(&NodeId("a".to_string())),
        Some(&Point { x: 5, y: 5 })
    );
    assert_eq!(
        resolved.get(&NodeId("b".to_string())),
        Some(&Point { x: 24, y: 0 })
    );
}

#[test]
fn wraps_to_next_row_after_cols_per_row() {
    let g = Graph {
        version: 1,
        nodes: vec![
            node("a", "A"),
            node("b", "B"),
            node("c", "C"),
            node("d", "D"),
            node("e", "E"),
        ],
        edges: vec![],
    };
    let resolved = auto_layout(&g, &HashMap::new());
    assert_eq!(resolved.len(), 5);
    assert_eq!(
        resolved.get(&NodeId("e".to_string())),
        Some(&Point { x: 0, y: 6 }),
        "e should wrap to column 0 of row 1"
    );
    assert_eq!(
        resolved.get(&NodeId("a".to_string())),
        Some(&Point { x: 0, y: 0 })
    );
}

/// Regression: `existing` may carry positions for `NodeId`s that are
/// no longer in `graph.nodes` (e.g. a node was deleted but its old
/// position is still in `layout.json`). `auto_layout` must drop those
/// stale entries so the returned map covers only the current graph's
/// node set — otherwise a save/render downstream would persist or
/// render a position for a deleted node.
#[test]
fn drops_existing_entries_for_removed_nodes() {
    let g = Graph {
        version: 1,
        nodes: vec![node("a", "A"), node("b", "B")],
        edges: vec![],
    };
    let pinned: ResolvedLayout = HashMap::from([
        (NodeId("a".to_string()), Point { x: 5, y: 5 }),
        (NodeId("b".to_string()), Point { x: 9, y: 9 }),
        // "c" was in the graph previously but has since been deleted.
        (NodeId("c".to_string()), Point { x: 100, y: 100 }),
    ]);
    let resolved = auto_layout(&g, &pinned);
    assert_eq!(resolved.len(), 2, "stale entry for 'c' should be dropped");
    assert!(!resolved.contains_key(&NodeId("c".to_string())));
    assert_eq!(
        resolved.get(&NodeId("a".to_string())),
        Some(&Point { x: 5, y: 5 })
    );
    assert_eq!(
        resolved.get(&NodeId("b".to_string())),
        Some(&Point { x: 9, y: 9 })
    );
}
