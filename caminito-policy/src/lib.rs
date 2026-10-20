//! Window-management policy, free of protocol objects.
//!
//! Only Smithay's geometry types may be used here.

#![forbid(unsafe_code)]

mod action;
mod edges;
mod functions;
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

pub use action::{Action, Anchor};
pub use functions::{FunctionFacts, Functions, functions};
pub use grid::{Direction, Grid, Orientation, WorkspaceLayout, WrapStyle};
pub use output::{Output, OutputId, ReservedEdges};
pub use placement::PlacementMode;
pub use policy::{
    ActivationSource, ActivationToken, Config, Crossing, FocusClaim, FocusMode, FocusNewWindows,
    GrabKey, GrabStep, Interaction, LayerSurfaceId, MAX_WORKSPACES, MapStates, MenuEntry, MenuItem,
    MenuKey, MenuMark, MenuOp, MenuSelection, MinimizedPlacement, OutputSwitch, Overview,
    OverviewDrag, OverviewOutput, PointerTarget, Policy, PreMapConfigure, PreMapWindow,
    ResizeEdges, SeatId, StateRequest, SwitchEntry, SwitchList, Switcher, TOKEN_LIFETIME,
    TileShare, UrgentPlacement, WindowMenu, WorkspacePopup, default_workspace_name,
};
pub use skip_list::ShowDesktopSkipList;
pub use stack::Layer;
pub use window::{
    FrameState, PolicyWindow, SizeHints, Tile, TiledEdges, WindowId, WindowStates, WindowType,
};
