//! xdg-decoration and KDE server-decoration (07). KDE's default mode is server side, and
//! xdg-decoration grants the mode the client asks for, server side when it has no preference.
//! So MATE, plain GTK and Qt apps get frames, and header-bar and libadwaita apps, which ask
//! for client side or bind neither protocol, draw their own.

use std::sync::Mutex;

use smithay::reexports::wayland_protocols::xdg::decoration::zv1::server::zxdg_toplevel_decoration_v1::Mode;
use smithay::reexports::wayland_protocols_misc::server_decoration::server::org_kde_kwin_server_decoration::{
    Mode as KdeMode, OrgKdeKwinServerDecoration,
};
use smithay::reexports::wayland_server::WEnum;
use smithay::reexports::wayland_server::protocol::wl_surface::WlSurface;
use smithay::wayland::compositor::with_states;
use smithay::wayland::shell::kde::decoration::{KdeDecorationHandler, KdeDecorationState};
use smithay::wayland::shell::xdg::ToplevelSurface;
use smithay::wayland::shell::xdg::decoration::XdgDecorationHandler;

use crate::state::State;

impl XdgDecorationHandler for State {
    fn new_decoration(&mut self, toplevel: ToplevelSurface) {
        set_mode(&toplevel, Mode::ServerSide);
    }

    fn request_mode(&mut self, toplevel: ToplevelSurface, mode: Mode) {
        set_mode(&toplevel, mode);
    }

    fn unset_mode(&mut self, toplevel: ToplevelSurface) {
        set_mode(&toplevel, Mode::ServerSide);
    }
}

/// Grants `mode`, sending it once the toplevel has had its initial configure. The frame
/// follows when the client acks it and commits.
fn set_mode(toplevel: &ToplevelSurface, mode: Mode) {
    toplevel.with_pending_state(|state| state.decoration_mode = Some(mode));
    if toplevel.is_initial_configure_sent() {
        toplevel.send_pending_configure();
    }
}

/// The mode KDE server-decoration last granted a surface, if it has a decoration object.
struct KdeModeData(Mutex<Option<KdeMode>>);

/// Whether KDE server-decoration has granted `surface` server-side decorations.
pub(crate) fn kde_server_side(surface: &WlSurface) -> bool {
    with_states(surface, |states| {
        states
            .data_map
            .get::<KdeModeData>()
            .is_some_and(|data| *data.0.lock().unwrap() == Some(KdeMode::Server))
    })
}

impl State {
    /// Grants a KDE server-decoration mode, which takes effect at once.
    fn set_kde_mode(&mut self, surface: &WlSurface, mode: Option<KdeMode>) {
        with_states(surface, |states| {
            let data = states
                .data_map
                .get_or_insert_threadsafe(|| KdeModeData(Mutex::new(None)));
            *data.0.lock().unwrap() = mode;
        });
        self.camino.decoration_changed(surface);
    }
}

impl KdeDecorationHandler for State {
    fn kde_decoration_state(&self) -> &KdeDecorationState {
        &self.camino.kde_decoration_state
    }

    fn new_decoration(&mut self, surface: &WlSurface, decoration: &OrgKdeKwinServerDecoration) {
        // A new decoration starts in the default mode.
        decoration.mode(KdeMode::Server);
        self.set_kde_mode(surface, Some(KdeMode::Server));
    }

    fn request_mode(
        &mut self,
        surface: &WlSurface,
        decoration: &OrgKdeKwinServerDecoration,
        mode: WEnum<KdeMode>,
    ) {
        if let WEnum::Value(mode) = mode {
            decoration.mode(mode);
            self.set_kde_mode(surface, Some(mode));
        }
    }

    fn release(&mut self, _decoration: &OrgKdeKwinServerDecoration, surface: &WlSurface) {
        self.set_kde_mode(surface, None);
    }
}
