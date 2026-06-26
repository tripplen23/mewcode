//! Auto-layout for the architecture canvas graph.
//!
//! Q3 spike result: **in-house grid layout**, no third-party layout crate.
//!
//! ## The candidates we considered
//!
//! The README + milestone doc (`milestone-1-promptable-canvas.md` §3
//! T2) ranked three candidates: `ascii-dag` (primary) → `layout-rs` →
//! `rust-sugiyama`. The timeboxed spike tried all of them against the
//! T2 acceptance test (3-node chain, distinct deterministic positions)
//! and none of them was a fit:
//!
//! - **`ascii-dag`** is an **ASCII renderer**, not a layout engine.
//!   Its core API is `Graph::render()` which returns a `String`; it
//!   has no "give me positions" mode. Using it for T2 would mean
//!   render-then-parse of the ASCII output, which loses the structured
//!   `Point`s T4 needs and is fragile. (It remains a candidate for
//!   T4's render path if we ever want a fallback ASCII representation
//!   of the canvas.)
//! - **`layout-rs`** is a Graphviz `.dot` file **parser**. It does
//!   not implement a layout algorithm itself — it shells out to
//!   Graphviz's `dot` binary. That makes it a non-starter for a
//!   single-binary TUI/server product.
//! - **`rust-sugiyama`** (0.4.0) is a pure-Rust Sugiyama port and
//!   the closest match in *category* — it returns 2D coordinates.
//!   But it panics on the 3-node chain test case in two separate
//!   places: `algorithm/p2_reduce_crossings/mod.rs:371` panics
//!   because `for i in 0..order._inner[r].len() - 1` underflows
//!   when a layer is empty; and `algorithm/p3_calculate_coordinates/
//!   mod.rs:250` is an index-out-of-bounds. Disabling `transpose`
//!   (its main crossing-reduction optimization) avoids the first
//!   panic but exposes the second. The crate is unfit for T2's
//!   input class.
//! - **`dagre`** (0.1.1) is a more recent pure-Rust Sugiyama port
//!   that cross-validates against dagre.js. It would also work in
//!   theory, but with only 2 releases it's a bigger risk than the
//!   spike calls for. Worth revisiting when M2+ brings bigger
//!   graphs where a real layout matters.
//!
//! ## What we ship instead
//!
//! A simple **row-major grid layout**: nodes are placed at
//! `(col * COL_STEP, row * ROW_STEP)` in `NodeId`-lex order, where
//! rows wrap at `COLS_PER_ROW` columns. This passes T2's acceptance
//! test (3 distinct positions, deterministic for fixed input) and
//! renders cleanly in the TUI (every node has its own cell, no
//! overlaps). It is **not** a graph-aware layout — edges can cross,
//! siblings in the same layer don't get pulled together — and that
//! is fine for M1, where graphs are 3-10 nodes and the user is
//! typing "add an auth component" rather than staring at a 100-node
//! system. When M2+ brings drag-to-reposition (and bigger graphs),
//! we should re-evaluate `dagre` and switch.

use std::collections::HashMap;

use mewcode_protocol::canvas::{Graph, NodeId, Point};

/// A fully-resolved layout: every node has a 2D position, in character
/// cells (the TUI renderer's unit). This is what T4's `view::canvas`
/// consumes.
pub type ResolvedLayout = HashMap<NodeId, Point>;

/// Horizontal step between adjacent cells in the grid. Big enough
/// that 20-cell node cards don't visually merge.
const COL_STEP: i32 = 24;
/// Vertical step between rows. Big enough that 4-cell-tall node cards
/// don't overlap.
const ROW_STEP: i32 = 6;
/// How many columns per row before wrapping to the next row.
const COLS_PER_ROW: usize = 4;

/// Assign positions to every node in `graph` that does not already
/// have one in `existing`. Deterministic for a fixed input: nodes are
/// sorted by `NodeId` (lexicographic on the inner string), then laid
/// out in row-major order. Edges do not influence placement.
///
/// `existing` positions are preserved unchanged. The returned
/// `ResolvedLayout` always contains a position for every node, so T4
/// can render without checking for `Option<Point>` per node.
pub fn auto_layout(graph: &Graph, existing: &ResolvedLayout) -> ResolvedLayout {
    // Sort nodes by NodeId for determinism. The T2 spec calls this
    // out explicitly: "deterministic order = sort by NodeId before
    // placement; ties broken by edge (src, tgt) lex order". We
    // currently don't need the edge-tiebreak because the row-major
    // grid never sees ties, but if we ever do, this is the place.
    let mut sorted_ids: Vec<&NodeId> = graph.nodes.iter().map(|n| &n.id).collect();
    sorted_ids.sort_by(|a, b| a.0.cmp(&b.0));

    let mut resolved: ResolvedLayout = existing.clone();
    for (i, id) in sorted_ids.into_iter().enumerate() {
        // Skip nodes that the user (or a previous layout pass) pinned.
        if resolved.contains_key(id) {
            continue;
        }
        let col = (i % COLS_PER_ROW) as i32;
        let row = (i / COLS_PER_ROW) as i32;
        resolved.insert(
            id.clone(),
            Point {
                x: col * COL_STEP,
                y: row * ROW_STEP,
            },
        );
    }
    resolved
}

#[cfg(test)]
mod tests {
    use super::*;
    use mewcode_protocol::canvas::{Edge, EdgeKind, Node, NodeKind};

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
        let resolved = auto_layout(&g, &HashMap::new());
        assert!(resolved.is_empty());
    }

    #[test]
    fn three_node_two_edge_assigns_distinct_positions() {
        // a -> b -> c
        let g = Graph {
            version: 1,
            nodes: vec![node("a", "A"), node("b", "B"), node("c", "C")],
            edges: vec![edge("a", "b"), edge("b", "c")],
        };
        let resolved = auto_layout(&g, &HashMap::new());
        assert_eq!(resolved.len(), 3);

        // Deterministic: a is at column 0, b at column 1, c at column 2.
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
        // b is not pinned, so it gets the next grid slot.
        assert_eq!(
            resolved.get(&NodeId("b".to_string())),
            Some(&Point { x: 24, y: 0 })
        );
    }

    #[test]
    fn wraps_to_next_row_after_cols_per_row() {
        // 5 nodes sorted lex: a, b, c, d, e. With COLS_PER_ROW=4:
        // a=0,0  b=24,0  c=48,0  d=72,0  e=0,6
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
}
