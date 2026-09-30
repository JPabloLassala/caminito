//! The clipboard, primary selection and drag-and-drop. data-control comes in M5. X
//! clients reach them through `xwayland::selection` and Smithay's XDND bridge (37).
//!
//! Known bugs, accepted with no workarounds in v0.1 (11): they're fixed upstream or not at
//! all, and picked up by bumping the Smithay pin.
//! - XDND's Ask and Private actions aren't bridged.
//! - `CLIPBOARD_MANAGER`'s `SAVE_TARGETS` isn't implemented, so an X client's clipboard
//!   only outlives it through msd's data-control clipboard manager (M5).
//! - A drop can be lost: Smithay #1953.
//!
//! A Wayland drag's icon is kept here from the drag's start until it's dropped or
//! cancelled (48). An X client's drag has none: it draws its own, in an override-redirect
//! window, and Smithay's XDND bridge starts the grab without asking for one.

use std::os::fd::OwnedFd;
use std::time::Duration;

use smithay::desktop::utils::send_frames_surface_tree;
use smithay::input::Seat;
use smithay::input::dnd::{DnDGrab, DndGrabHandler, DndTarget, GrabType, Source};
use smithay::input::pointer::Focus;
use smithay::output::Output;
use smithay::reexports::wayland_server::protocol::wl_surface::WlSurface;
use smithay::utils::{Logical, Monotonic, Point, Serial, Time};
use smithay::wayland::compositor::{
    SurfaceAttributes, TraversalAction, send_surface_state, with_states, with_surface_tree_downward,
};
use smithay::wayland::fractional_scale::with_fractional_scale;
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

/// A Wayland drag's icon, which Smithay has given `DND_ICON_ROLE`.
#[derive(Debug)]
pub(crate) struct DragIcon {
    pub(crate) surface: WlSurface,
    /// Where the surface's origin is from the pointer, moved by each `wl_surface.offset`
    /// or attach offset it commits.
    pub(crate) offset: Point<i32, Logical>,
}

impl DragIcon {
    /// Tells the icon of a frame of `output`, the pointer's, and of its scale, as a window
    /// is told of its primary output's.
    pub(crate) fn send_frame(
        &self,
        output: &Output,
        now: Time<Monotonic>,
        throttle: Option<Duration>,
    ) {
        let scale = output.current_scale();
        let transform = output.current_transform();
        with_surface_tree_downward(
            &self.surface,
            (),
            |_, _, _| TraversalAction::DoChildren(()),
            |surface, states, _| {
                send_surface_state(surface, states, scale.integer_scale(), transform);
                with_fractional_scale(states, |fractional| {
                    fractional.set_preferred_scale(scale.fractional_scale());
                });
            },
            |_, _, _| true,
        );
        send_frames_surface_tree(&self.surface, output, now, throttle, |_, _| {
            Some(output.clone())
        });
    }
}

/// The offset a commit of `surface` moved its content by, taken so it's counted once.
fn take_buffer_delta(surface: &WlSurface) -> Point<i32, Logical> {
    with_states(surface, |states| {
        states
            .cached_state
            .get::<SurfaceAttributes>()
            .current()
            .buffer_delta
            .take()
            .unwrap_or_default()
    })
}

impl State {
    /// A commit of the drag icon's surface: it moves by the offset it commits, and is
    /// drawn again. Returns whether `surface` is the drag icon.
    pub(super) fn commit_if_drag_icon(&mut self, surface: &WlSurface) -> bool {
        let camino = &mut self.camino;
        let Some(icon) = camino
            .drag_icon
            .as_mut()
            .filter(|icon| icon.surface == *surface)
        else {
            return false;
        };
        icon.offset += take_buffer_delta(surface);
        camino.queue_redraw_all();
        true
    }

    fn drag_ended(&mut self) {
        if self.camino.drag_icon.take().is_some() {
            self.camino.queue_redraw_all();
        }
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
        icon: Option<WlSurface>,
        seat: Seat<Self>,
        serial: Serial,
        type_: GrabType,
    ) {
        if let GrabType::Pointer = type_
            && let Some(pointer) = seat.get_pointer()
            && let Some(start_data) = pointer.grab_start_data()
        {
            let grab = DnDGrab::new_pointer(&self.camino.display_handle, start_data, source, seat);
            // A drag this one replaces is cancelled here, and takes its icon with it, so
            // the new icon is kept only after.
            pointer.set_grab(self, grab, serial, Focus::Keep);
            // The icon may have committed its first offset before the drag began, as GTK's
            // does; the surface is new, so that offset is its only one.
            self.camino.drag_icon = icon.map(|surface| DragIcon {
                offset: take_buffer_delta(&surface),
                surface,
            });
            self.camino.queue_redraw_all();
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
        self.drag_ended();
    }

    fn cancelled(&mut self, _seat: Seat<Self>, _location: Point<f64, Logical>) {
        self.drag_ended();
    }
}

impl PrimarySelectionHandler for State {
    fn primary_selection_state(&mut self) -> &mut PrimarySelectionState {
        &mut self.camino.primary_selection_state
    }
}
