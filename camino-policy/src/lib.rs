//! Window-management policy, free of protocol objects.
//!
//! Only Smithay's geometry types may be used here.

#![forbid(unsafe_code)]

mod action;
mod grid;
mod output;
mod placement;
mod policy;
mod stack;
mod window;

#[cfg(test)]
mod test_window;
#[cfg(test)]
mod tests;

pub use action::Action;
pub use grid::{Grid, Orientation, WorkspaceLayout};
pub use output::{Output, OutputId, ReservedEdges};
pub use policy::{
    Config, MAX_WORKSPACES, Policy, ResizeEdges, SeatId, SwitchList, default_workspace_name,
};
pub use stack::Layer;
pub use window::{FrameState, PolicyWindow, SizeHints, WindowId, WindowStates, WindowType};
