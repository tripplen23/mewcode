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

    // Build `resolved` from the current graph's node set only. Entries
    // in `existing` whose NodeId is no longer in `graph.nodes` are
    // dropped here, so save/render paths downstream never see a
    // position for a node that has been deleted from the graph.
    let mut resolved: ResolvedLayout = HashMap::with_capacity(sorted_ids.len());
    for (i, id) in sorted_ids.into_iter().enumerate() {
        let point = existing.get(id).copied().unwrap_or(Point {
            x: (i % COLS_PER_ROW) as i32 * COL_STEP,
            y: (i / COLS_PER_ROW) as i32 * ROW_STEP,
        });
        resolved.insert(id.clone(), point);
    }
    resolved
}
