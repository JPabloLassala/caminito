//! A minimal `gtk_shell1` (06, 67): GTK3 says only through this GNOME protocol that a window
//! is modal, and rings the bell only through it. GTK4 doesn't bind it, and uses xdg-dialog
//! and xdg-system-bell.
//!
//! This module owns the protocol's objects and each surface's modal hint. It knows nothing
//! of windows: the compositor reads the hint with [`is_modal`], and hears of a change and of
//! the bell through [`GtkShellHandler`]. Every other request is taken and ignored, and no
//! event is sent, since GTK3 needs none to start.
//!
//! It isn't offered to make GTK3 drop its client-side decorations: GTK3 asks for a frame
//! through KDE server-decoration (07).

use std::sync::atomic::{AtomicBool, Ordering};

use smithay::reexports::wayland_server::backend::GlobalId;
use smithay::reexports::wayland_server::protocol::wl_surface::WlSurface;
use smithay::reexports::wayland_server::{
    Client, DataInit, Dispatch, DisplayHandle, GlobalDispatch, New, Resource, Weak,
};
use smithay::wayland::compositor::with_states;
use smithay::wayland::{Dispatch2, GlobalDispatch2};

use crate::protocols::raw::gtk_shell1::{self, GtkShell1};
use crate::protocols::raw::gtk_surface1::{self, GtkSurface1};

/// The version of the protocol Caminito offers: the lowest at which GTK3 uses `set_modal` and
/// `system_bell`, which are both in version 1. GTK3 binds the lower of this and 5
/// (gdkdisplay-wayland.c:429-436, gtk-3-24 at 2776f156), and skips what version 1 lacks:
/// `request_focus` (gdkwindow-wayland.c:4056), `release` (3584-3588), `titlebar_gesture`
/// (5094) and the edge constraints of `configure_edges` (1764), which it then works out
/// itself.
const VERSION: u32 = 1;

/// What the compositor does with what GTK3 says.
pub trait GtkShellHandler: 'static {
    /// `surface` became modal, or stopped being modal. [`is_modal`] has the new hint.
    fn modal_changed(&mut self, surface: &WlSurface);

    /// A client rang the bell for `surface`, which may be gone by now, or for no surface.
    fn system_bell(&mut self, surface: Option<Weak<WlSurface>>);
}

/// The `gtk_shell1` global.
#[derive(Debug)]
pub struct GtkShellState {
    _global: GlobalId,
}

/// The global's data.
#[derive(Debug)]
pub struct GtkShellGlobalData;

/// A `gtk_shell1`'s data.
#[derive(Debug)]
pub struct GtkShellData;

/// A `gtk_surface1`'s data.
#[derive(Debug)]
pub struct GtkSurfaceData {
    surface: Weak<WlSurface>,
}

/// A surface's modal hint, in its data map. It outlives the `gtk_surface1` that set it, as
/// the protocol has no word for what destroying one undoes.
struct Modal(AtomicBool);

impl GtkShellState {
    /// Offers `gtk_shell1` to every client.
    pub fn new<D>(display: &DisplayHandle) -> Self
    where
        D: GlobalDispatch<GtkShell1, GtkShellGlobalData> + 'static,
    {
        let global = display.create_global::<D, GtkShell1, _>(VERSION, GtkShellGlobalData);
        Self { _global: global }
    }
}

/// Whether a client last said, with `gtk_surface1.set_modal`, that `surface` is modal.
pub fn is_modal(surface: &WlSurface) -> bool {
    with_states(surface, |states| {
        states
            .data_map
            .get::<Modal>()
            .is_some_and(|modal| modal.0.load(Ordering::Relaxed))
    })
}

/// Records a surface's modal hint, and returns whether it changed.
fn set_modal(surface: &WlSurface, modal: bool) -> bool {
    with_states(surface, |states| {
        let hint = states
            .data_map
            .get_or_insert_threadsafe(|| Modal(AtomicBool::new(false)));
        hint.0.swap(modal, Ordering::Relaxed) != modal
    })
}

impl<D> GlobalDispatch2<GtkShell1, D> for GtkShellGlobalData
where
    D: GtkShellHandler + Dispatch<GtkShell1, GtkShellData>,
{
    fn bind(
        &self,
        _state: &mut D,
        _display: &DisplayHandle,
        _client: &Client,
        resource: New<GtkShell1>,
        data_init: &mut DataInit<'_, D>,
    ) {
        // No `capabilities` event: Caminito has no global menu and draws no desktop icons.
        data_init.init(resource, GtkShellData);
    }
}

impl<D> Dispatch2<GtkShell1, D> for GtkShellData
where
    D: GtkShellHandler + Dispatch<GtkSurface1, GtkSurfaceData>,
{
    fn request(
        &self,
        state: &mut D,
        _client: &Client,
        _resource: &GtkShell1,
        request: gtk_shell1::Request,
        _display: &DisplayHandle,
        data_init: &mut DataInit<'_, D>,
    ) {
        match request {
            gtk_shell1::Request::GetGtkSurface {
                gtk_surface,
                surface,
            } => {
                let data = GtkSurfaceData {
                    surface: surface.downgrade(),
                };
                data_init.init(gtk_surface, data);
            }
            gtk_shell1::Request::SystemBell { surface } => {
                // A `gtk_surface1` has no destructor at version 1, so its surface can go
                // before it does, and a bell for it then names a surface that's gone.
                let surface = surface
                    .as_ref()
                    .and_then(|gtk_surface| gtk_surface.data::<GtkSurfaceData>())
                    .map(|data| data.surface.clone());
                state.system_bell(surface);
            }
            // Startup notification goes through xdg-activation. `notify_launch` is newer
            // than the version offered.
            gtk_shell1::Request::SetStartupId { .. } | gtk_shell1::Request::NotifyLaunch { .. } => {
            }
        }
    }
}

impl<D: GtkShellHandler> Dispatch2<GtkSurface1, D> for GtkSurfaceData {
    fn request(
        &self,
        state: &mut D,
        _client: &Client,
        _resource: &GtkSurface1,
        request: gtk_surface1::Request,
        _display: &DisplayHandle,
        _data_init: &mut DataInit<'_, D>,
    ) {
        let modal = match request {
            gtk_surface1::Request::SetModal => true,
            gtk_surface1::Request::UnsetModal => false,
            // Caminito has no global menu to give the D-Bus names to, activation goes through
            // xdg-activation, and a titlebar gesture is only made of a client-side titlebar,
            // whose actions GTK3 then takes itself. The last three are newer than the
            // version offered, so the Wayland library lets none through; they're here because the
            // XML has them.
            gtk_surface1::Request::SetDbusProperties { .. }
            | gtk_surface1::Request::Present { .. }
            | gtk_surface1::Request::RequestFocus { .. }
            | gtk_surface1::Request::TitlebarGesture { .. }
            | gtk_surface1::Request::Release => return,
        };
        let Ok(surface) = self.surface.upgrade() else {
            return;
        };
        if set_modal(&surface, modal) {
            state.modal_changed(&surface);
        }
    }
}
