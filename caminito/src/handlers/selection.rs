//! The clipboard, primary selection and drag-and-drop, and data-control, through which a
//! clipboard manager reads and sets the selections without the keyboard (68). X clients
//! reach them through `xwayland::selection` and Smithay's XDND bridge (37).
//!
//! Known bugs, accepted with no workarounds in v0.1 (11): they're fixed upstream or not at
//! all, and picked up by bumping the Smithay pin.
//! - XDND's Ask and Private actions aren't bridged.
//! - `CLIPBOARD_MANAGER`'s `SAVE_TARGETS` isn't implemented, so an X client's clipboard
//!   only outlives it through msd's data-control clipboard manager.
//! - A drop can be lost: Smithay #1953.
//!
//! A Wayland drag's icon is kept here from the drag's start until it's dropped or
//! cancelled (48). An X client's drag has none: it draws its own, in an override-redirect
//! window, and Smithay's XDND bridge starts the grab without asking for one.
//!
//! A drag released on another output makes that one the Active output, whatever it's
//! dropped on (CMN-339): a client that opens a window for a drop on nothing, as Firefox
//! does for a tab, has it open there. A drag cancelled, or given up by its Wayland client
//! with the button still held, as GTK does at an Escape, changes nothing. An X client's
//! drag reaches the same `dropped` through the bridge's grab, once it has left the X
//! windows; one dropped on another X window stays within Xwayland, as a plain press and
//! release, and isn't told from one.

use std::any::Any;
use std::os::fd::OwnedFd;
use std::sync::Arc;
use std::time::Duration;

use smithay::desktop::utils::send_frames_surface_tree;
use smithay::input::Seat;
use smithay::input::dnd::{
    DnDGrab, DndAction, DndGrabHandler, DndTarget, GrabType, Source, SourceMetadata,
};
use smithay::input::pointer::Focus;
use smithay::output::Output;
use smithay::reexports::wayland_server::protocol::wl_surface::WlSurface;
use smithay::utils::{IsAlive, Logical, Monotonic, Point, Serial, Time};
use smithay::wayland::compositor::{
    SurfaceAttributes, TraversalAction, with_states, with_surface_tree_downward,
};
use smithay::wayland::selection::data_device::{
    DataDeviceHandler, DataDeviceState, WaylandDndGrabHandler,
};
use smithay::wayland::selection::ext_data_control::{
    DataControlHandler as ExtDataControlHandler, DataControlState as ExtDataControlState,
};
use smithay::wayland::selection::primary_selection::{
    PrimarySelectionHandler, PrimarySelectionState,
};
use smithay::wayland::selection::wlr_data_control::{
    DataControlHandler as WlrDataControlHandler, DataControlState as WlrDataControlState,
};
use smithay::wayland::selection::{SelectionHandler, SelectionSource, SelectionTarget};

use crate::state::State;
use crate::window::send_preferred_scale;

/// The only selections Caminito sets on the seat itself are X clients', so its user data
/// says nothing more. A data-control client's selection comes here as any other client's.
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
    /// is told of its main output's.
    pub(crate) fn send_frame(
        &self,
        output: &Output,
        now: Time<Monotonic>,
        throttle: Option<Duration>,
    ) {
        with_surface_tree_downward(
            &self.surface,
            (),
            |_, _, _| TraversalAction::DoChildren(()),
            |surface, states, _| send_preferred_scale(surface, states, output),
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
        let caminito = &mut self.caminito;
        let Some(icon) = caminito
            .drag_icon
            .as_mut()
            .filter(|icon| icon.surface == *surface)
        else {
            return false;
        };
        icon.offset += take_buffer_delta(surface);
        caminito.queue_redraw_all();
        true
    }

    fn drag_ended(&mut self) {
        self.caminito.drag_source = None;
        if self.caminito.drag_icon.take().is_some() {
            self.caminito.queue_redraw_all();
        }
    }
}

/// A drag's source, shared between the grab and `Caminito::drag_source`, which only asks
/// whether it's alive.
struct SharedSource<S>(Arc<S>);

impl<S: Source> IsAlive for SharedSource<S> {
    fn alive(&self) -> bool {
        self.0.alive()
    }
}

impl<S: Source> Source for SharedSource<S> {
    fn is_client_local(&self, target: &dyn Any) -> bool {
        self.0.is_client_local(target)
    }
    fn metadata(&self) -> Option<SourceMetadata> {
        self.0.metadata()
    }
    fn accepted(&self, mime_type: Option<String>) {
        self.0.accepted(mime_type);
    }
    fn choose_action(&self, action: DndAction) {
        self.0.choose_action(action);
    }
    fn send(&self, mime_type: &str, fd: OwnedFd) {
        self.0.send(mime_type, fd);
    }
    fn drop_performed(&self) {
        self.0.drop_performed();
    }
    fn cancel(&self) {
        self.0.cancel();
    }
    fn finished(&self) {
        self.0.finished();
    }
}

impl DataDeviceHandler for State {
    fn data_device_state(&mut self) -> &mut DataDeviceState {
        &mut self.caminito.data_device_state
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
            let source = Arc::new(source);
            let shared = SharedSource(source.clone());
            let grab =
                DnDGrab::new_pointer(&self.caminito.display_handle, start_data, shared, seat);
            // A drag this one replaces is cancelled here, and takes its icon and source
            // with it, so the new ones are kept only after.
            pointer.set_grab(self, grab, serial, Focus::Keep);
            self.caminito.drag_source = Some(source);
            // The icon may have committed its first offset before the drag began, as GTK's
            // does; the surface is new, so that offset is its only one.
            self.caminito.drag_icon = icon.map(|surface| DragIcon {
                offset: take_buffer_delta(&surface),
                surface,
            });
            self.caminito.queue_redraw_all();
        }
    }
}

impl DndGrabHandler for State {
    fn dropped(
        &mut self,
        _target: Option<DndTarget<'_, Self>>,
        _validated: bool,
        _seat: Seat<Self>,
        location: Point<f64, Logical>,
    ) {
        // An X client's drag has no source kept here.
        let given_up = self
            .caminito
            .drag_source
            .as_ref()
            .is_some_and(|source| !source.alive());
        if !given_up {
            self.caminito.drag_released(location);
        }
        self.drag_ended();
    }

    fn cancelled(&mut self, _seat: Seat<Self>, _location: Point<f64, Logical>) {
        self.drag_ended();
    }
}

impl PrimarySelectionHandler for State {
    fn primary_selection_state(&mut self) -> &mut PrimarySelectionState {
        &mut self.caminito.primary_selection_state
    }
}

impl ExtDataControlHandler for State {
    fn data_control_state(&mut self) -> &mut ExtDataControlState {
        &mut self.caminito.ext_data_control_state
    }
}

impl WlrDataControlHandler for State {
    fn data_control_state(&mut self) -> &mut WlrDataControlState {
        &mut self.caminito.wlr_data_control_state
    }
}
