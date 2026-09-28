//! Outputs as policy sees them.

use smithay::utils::{Logical, Rectangle};

#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub struct OutputId(pub u64);

/// An output's descriptor: its place in the global logical space, and its scale.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct Output {
    pub id: OutputId,
    pub geometry: Rectangle<i32, Logical>,
    pub scale: f64,
}
