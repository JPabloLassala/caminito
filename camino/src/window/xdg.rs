//! The xdg-shell adapter: what `Mapped` reads from an xdg toplevel and asks of it, through
//! the protocols that extend it (xdg-decoration, KDE server-decoration, xdg-dialog,
//! `gtk_shell1` and xdg-toplevel-icon).

use std::sync::atomic::{AtomicBool, Ordering};

use camino_policy::{SizeHints, WindowStates, WindowType};
use smithay::reexports::wayland_protocols::xdg::decoration::zv1::server::zxdg_toplevel_decoration_v1::Mode as DecorationMode;
use smithay::reexports::wayland_protocols::xdg::shell::server::xdg_toplevel;
use smithay::reexports::wayland_server::{DisplayHandle, Resource};
use smithay::utils::{FrameExtents, Logical, Point, Rectangle};
use smithay::wayland::compositor::with_states;
use smithay::wayland::shell::xdg::dialog::ToplevelDialogHint;
use smithay::wayland::shell::xdg::{SurfaceCachedState, ToplevelSurface, XdgToplevelSurfaceData};
use smithay::wayland::xdg_toplevel_icon::ToplevelIconCachedState;

use super::{Adapter, PreMapConfigure, Skips, WindowKey};
use crate::decoration::IconSource;
use crate::handlers::kde_server_side;
use crate::input::KeyboardFocus;
use crate::protocols::gtk_shell;

/// The client asked for a state, so it's owed a configure even if nothing changed.
#[derive(Default)]
struct ConfigureOwed(AtomicBool);

/// Owes the client a configure in reply to a request, whether or not policy changed
/// anything, as xdg-shell expects.
pub(crate) fn owe_configure(toplevel: &ToplevelSurface) {
    with_states(toplevel.wl_surface(), |states| {
        states
            .data_map
            .get_or_insert_threadsafe(ConfigureOwed::default)
            .0
            .store(true, Ordering::Relaxed);
    });
}

/// Takes whether the client is owed a configure.
fn take_configure_owed(toplevel: &ToplevelSurface) -> bool {
    with_states(toplevel.wl_surface(), |states| {
        states
            .data_map
            .get::<ConfigureOwed>()
            .is_some_and(|owed| owed.0.swap(false, Ordering::Relaxed))
    })
}

/// Sets `states` on the toplevel's pending configure.
fn set_pending_states(toplevel: &ToplevelSurface, states: &[(xdg_toplevel::State, bool)]) {
    toplevel.with_pending_state(|pending| {
        for &(state, on) in states {
            if on {
                pending.states.set(state);
            } else {
                pending.states.unset(state);
            }
        }
    });
}

impl Adapter for ToplevelSurface {
    fn title(&self) -> String {
        with_states(self.wl_surface(), |states| {
            let data = states.data_map.get::<XdgToplevelSurfaceData>()?;
            data.lock().unwrap().title.clone()
        })
        .unwrap_or_default()
    }

    fn app_id(&self) -> Option<String> {
        with_states(self.wl_surface(), |states| {
            states
                .data_map
                .get::<XdgToplevelSurfaceData>()?
                .lock()
                .unwrap()
                .app_id
                .clone()
        })
    }

    /// A modal dialog from xdg-dialog or `gtk_shell1`, a dialog if it has a transient
    /// parent, or else normal.
    fn window_type(&self, has_parent: bool) -> WindowType {
        let modal = with_states(self.wl_surface(), |states| {
            states
                .data_map
                .get::<XdgToplevelSurfaceData>()
                .is_some_and(|data| data.lock().unwrap().dialog_hint == ToplevelDialogHint::Modal)
        });
        if modal || gtk_shell::is_modal(self.wl_surface()) {
            WindowType::ModalDialog
        } else if has_parent {
            WindowType::Dialog
        } else {
            WindowType::Normal
        }
    }

    fn size_hints(&self) -> SizeHints {
        with_states(self.wl_surface(), |states| {
            let mut cached = states.cached_state.get::<SurfaceCachedState>();
            let current = cached.current();
            SizeHints {
                min: current.min_size,
                max: current.max_size,
            }
        })
    }

    fn requested_position(
        &self,
        _extents: FrameExtents<i32, Logical>,
    ) -> Option<Point<i32, Logical>> {
        // xdg toplevels can't ask for a position.
        None
    }

    /// From the client's credentials.
    fn pid(&self) -> Option<u32> {
        let surface = self.wl_surface();
        let dh = DisplayHandle::from(surface.handle().upgrade()?);
        let credentials = surface.client()?.get_credentials(&dh).ok()?;
        u32::try_from(credentials.pid).ok()
    }

    /// The last mode it acked from xdg-decoration, or KDE server-decoration's.
    fn wants_frame(&self) -> bool {
        let xdg = self.with_committed_state(|state| state.and_then(|s| s.decoration_mode));
        xdg == Some(DecorationMode::ServerSide) || kde_server_side(self.wl_surface())
    }

    /// The mode its next configure grants.
    fn wants_frame_pending(&self) -> bool {
        let xdg = self.with_pending_state(|state| state.decoration_mode);
        xdg == Some(DecorationMode::ServerSide) || kde_server_side(self.wl_surface())
    }

    /// xdg-toplevel-icon's, or else the app id's.
    fn icon(&self) -> IconSource {
        let app_id = self.app_id();
        let toplevel_icon = with_states(self.wl_surface(), |states| {
            let mut cached = states.cached_state.get::<ToplevelIconCachedState>();
            let icon = cached.current();
            let name = icon.icon_name().map(str::to_owned);
            let buffers = icon.buffers().to_vec();
            (name.is_some() || !buffers.is_empty()).then(|| IconSource::Toplevel {
                name,
                buffers,
                app_id: app_id.clone(),
            })
        });
        toplevel_icon.unwrap_or(IconSource::AppId(app_id))
    }

    fn parent(&self) -> Option<WindowKey> {
        ToplevelSurface::parent(self).map(WindowKey::Wayland)
    }

    /// xdg-activation's urgency comes with focus-stealing prevention in M8.
    fn urgent(&self) -> bool {
        false
    }

    /// xdg-shell has no workspaces.
    fn requested_workspace(&self) -> Option<usize> {
        None
    }

    fn takes_input(&self) -> bool {
        true
    }

    /// Only foreign-toplevel's clients would ask, and they come in M5.
    fn skips(&self) -> Skips {
        Skips::default()
    }

    fn set_geometry(&self, geometry: Rectangle<i32, Logical>, states: WindowStates) {
        // Where the window is is Camino's alone; the client is told its size and states.
        self.with_pending_state(|pending| pending.size = Some(geometry.size));
        set_pending_states(
            self,
            &[
                (xdg_toplevel::State::Maximized, states.maximized),
                (xdg_toplevel::State::Fullscreen, states.fullscreen),
                (xdg_toplevel::State::Resizing, states.resizing),
            ],
        );
    }

    /// Sends what's pending since the last configure, if anything, or a configure the
    /// client is owed, once it has had its initial configure.
    fn flush(&self) {
        if !self.is_initial_configure_sent() {
            return;
        }
        if take_configure_owed(self) {
            self.send_configure();
        } else {
            self.send_pending_configure();
        }
    }

    fn set_activated(&self, activated: bool) {
        set_pending_states(self, &[(xdg_toplevel::State::Activated, activated)]);
    }

    fn close(&self) {
        self.send_close();
    }

    /// Sets the pre-map configure, and sends it if the initial configure has gone out; the
    /// initial one goes out on the initial commit.
    fn configure_pending(&self, configure: &PreMapConfigure) {
        self.with_pending_state(|pending| {
            pending.bounds = configure.bounds;
            pending.size = configure.size;
        });
        set_pending_states(
            self,
            &[
                (xdg_toplevel::State::Maximized, configure.maximized),
                (xdg_toplevel::State::Fullscreen, configure.fullscreen),
            ],
        );
        if self.is_initial_configure_sent() {
            self.send_pending_configure();
        }
    }

    fn keyboard_target(&self) -> KeyboardFocus {
        KeyboardFocus::Surface(self.wl_surface().clone())
    }
}
