//! Outputs as policy sees them.

use smithay::utils::{Logical, Rectangle};

#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub struct OutputId(pub u64);

/// How far in from each edge of an output something is reserved, as a panel reserves its
/// height. Policy doesn't know what reserves it (64).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub struct ReservedEdges {
    pub left: i32,
    pub right: i32,
    pub top: i32,
    pub bottom: i32,
}

/// An output's descriptor: its place in the global logical space, its scale, and what's
/// reserved along its edges.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct Output {
    pub id: OutputId,
    pub geometry: Rectangle<i32, Logical>,
    pub scale: f64,
    pub reserved: ReservedEdges,
}
