//! Window-management policy, free of protocol objects.
//!
//! Only Smithay's geometry types may be used here.

#![forbid(unsafe_code)]

mod action;
mod edges;
mod grid;
mod output;
mod placement;
mod policy;
mod skip_list;
mod stack;
mod window;

#[cfg(test)]
mod test_window;
#[cfg(test)]
mod tests;

pub use action::Action;
pub use grid::{Grid, Orientation, WorkspaceLayout, WrapStyle};
pub use output::{Output, OutputId, ReservedEdges};
pub use placement::PlacementMode;
pub use policy::{
    ActivationToken, Config, Crossing, FocusClaim, FocusMode, FocusNewWindows, GrabKey, GrabStep,
    MAX_WORKSPACES, MapStates, MenuEntry, MenuItem, MenuKey, MenuMark, MenuOp, MenuSelection,
    PointerTarget, Policy, ResizeEdges, SeatId, SwitchList, Switcher, TOKEN_LIFETIME,
    UrgentPlacement, WindowMenu, default_workspace_name,
};
pub use skip_list::ShowDesktopSkipList;
pub use stack::Layer;
pub use window::{
    FrameState, PolicyWindow, SizeHints, Tile, TiledEdges, WindowId, WindowStates, WindowType,
};
