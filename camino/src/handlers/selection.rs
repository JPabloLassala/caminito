//! The clipboard, primary selection and drag-and-drop. data-control comes in M5. X
//! clients reach them through `xwayland::selection` and Smithay's XDND bridge (37).
//!
//! Known bugs, accepted with no workarounds in v0.1 (11): they're fixed upstream or not at
//! all, and picked up by bumping the Smithay pin.
//! - XDND's Ask and Private actions aren't bridged.
//! - `CLIPBOARD_MANAGER`'s `SAVE_TARGETS` isn't implemented, so an X client's clipboard
//!   only outlives it through msd's data-control clipboard manager (M5).
//! - A drop can be lost: Smithay #1953.

use std::os::fd::OwnedFd;

use smithay::input::Seat;
use smithay::input::dnd::{DnDGrab, DndGrabHandler, DndTarget, GrabType, Source};
use smithay::input::pointer::Focus;
use smithay::reexports::wayland_server::protocol::wl_surface::WlSurface;
use smithay::utils::{Logical, Point, Serial};
use smithay::wayland::selection::data_device::{
    DataDeviceHandler, DataDeviceState, WaylandDndGrabHandler,
};
use smithay::wayland::selection::primary_selection::{
    PrimarySelectionHandler, PrimarySelectionState,
};
use smithay::wayland::selection::{SelectionHandler, SelectionSource, SelectionTarget};

use crate::state::State;

/// The only selections Camino sets on the seat itself are X clients', so its user data
/// says nothing more.
impl SelectionHandler for State {
    type SelectionUserData = ();

    fn new_selection(
        &mut self,
        ty: SelectionTarget,
        source: Option<SelectionSource>,
        _seat: Seat<Self>,
    ) {
        self.wayland_selection_changed(ty, source.map(|source| source.mime_types()));
    }

    fn send_selection(
        &mut self,
        ty: SelectionTarget,
        mime_type: String,
        fd: OwnedFd,
        _seat: Seat<Self>,
        _user_data: &(),
    ) {
        self.send_x_selection_to_wayland(ty, mime_type, fd);
    }
}

impl DataDeviceHandler for State {
    fn data_device_state(&mut self) -> &mut DataDeviceState {
        &mut self.camino.data_device_state
    }
}

impl WaylandDndGrabHandler for State {
    fn dnd_requested<S: Source>(
        &mut self,
        source: S,
        _icon: Option<WlSurface>,
        seat: Seat<Self>,
        serial: Serial,
        type_: GrabType,
    ) {
        // Drag icons aren't drawn yet.
        if let GrabType::Pointer = type_
            && let Some(pointer) = seat.get_pointer()
            && let Some(start_data) = pointer.grab_start_data()
        {
            let grab = DnDGrab::new_pointer(&self.camino.display_handle, start_data, source, seat);
            pointer.set_grab(self, grab, serial, Focus::Keep);
        }
    }
}

impl DndGrabHandler for State {
    fn dropped(
        &mut self,
        _target: Option<DndTarget<'_, Self>>,
        _validated: bool,
        _seat: Seat<Self>,
        _location: Point<f64, Logical>,
    ) {
    }
}

impl PrimarySelectionHandler for State {
    fn primary_selection_state(&mut self) -> &mut PrimarySelectionState {
        &mut self.camino.primary_selection_state
    }
}
