//! Marco's stacking layers, and restacking a workspace to respect them.

use std::collections::{BTreeMap, BTreeSet};

use crate::WindowId;

/// Marco's stacking layers (`MetaStackLayer`), bottom to top.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord)]
pub enum Layer {
    Desktop,
    /// Windows kept below; reached from M3's `_NET_WM_STATE_BELOW`.
    Bottom,
    Normal,
    /// Docks, and windows kept above.
    Top,
    /// A fullscreen window while it or one of its transients has focus.
    Fullscreen,
}

/// Reorders a stack, bottom to top, so layers never decrease and every transient is above its
/// parent, as Marco's `ensure_above` does. Otherwise the order is kept.
///
/// A transient's layer must be at least its parent's, and `parent` must be acyclic.
pub(crate) fn restack(
    stack: &mut Vec<WindowId>,
    layer: impl Fn(WindowId) -> Layer,
    parent: impl Fn(WindowId) -> Option<WindowId>,
) {
    stack.sort_by_key(|id| layer(*id));

    // A transient met below its parent waits, and goes in just above the parent.
    let mut waiting: BTreeMap<WindowId, Vec<WindowId>> = BTreeMap::new();
    let mut placed = BTreeSet::new();
    let mut out = Vec::with_capacity(stack.len());
    fn place(
        id: WindowId,
        out: &mut Vec<WindowId>,
        placed: &mut BTreeSet<WindowId>,
        waiting: &mut BTreeMap<WindowId, Vec<WindowId>>,
    ) {
        out.push(id);
        placed.insert(id);
        for child in waiting.remove(&id).unwrap_or_default() {
            place(child, out, placed, waiting);
        }
    }
    for &id in stack.iter() {
        match parent(id) {
            Some(p) if stack.contains(&p) && !placed.contains(&p) => {
                waiting.entry(p).or_default().push(id)
            }
            _ => place(id, &mut out, &mut placed, &mut waiting),
        }
    }
    debug_assert!(waiting.is_empty(), "transient parents are acyclic");
    *stack = out;
}
