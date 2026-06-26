//! Auto-layout for the architecture canvas graph.
//!
//! Assigns a deterministic 2D position (in TUI cell units) to every
//! node. In-house row-major grid layout: no third-party layout crate
//! survived the spike.
//! Fine for 3-10 node graphs; revisit `dagre` when bigger graphs
//! need real graph-aware layout.

use std::collections::HashMap;

use mewcode_protocol::canvas::{Graph, NodeId, Point};

/// A fully-resolved layout: every node has a 2D position, in character
/// cells (the TUI renderer's unit).
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
/// `ResolvedLayout` always contains a position for every node, so
/// callers don't have to handle `Option<Point>` per node.
pub fn auto_layout(graph: &Graph, existing: &ResolvedLayout) -> ResolvedLayout {
    // Sort by NodeId for determinism.
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
