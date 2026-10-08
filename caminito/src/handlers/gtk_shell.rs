//! `gtk_shell1` (67): a GTK3 window's modal hint, which joins xdg-dialog's, and GTK3's bell, which
//! is one of the bell event's sources (105).

use smithay::reexports::wayland_server::Weak;
use smithay::reexports::wayland_server::protocol::wl_surface::WlSurface;

use crate::bell::{BellFor, BellSource};
use crate::protocols::gtk_shell::{GtkShellHandler, GtkShellState};
use crate::state::State;

impl GtkShellHandler for State {
    fn gtk_shell_state(&mut self) -> &mut GtkShellState {
        &mut self.caminito.gtk_shell_state
    }

    fn modal_changed(&mut self, surface: &WlSurface) {
        // A pending window's hint is read when it maps.
        if let Some(id) = self.caminito.mapped_for_surface(surface) {
            self.caminito.window_changed(id);
        }
    }

    fn system_bell(&mut self, surface: Option<Weak<WlSurface>>) {
        let bell_for = match surface {
            Some(surface) => BellFor::Window(
                surface
                    .upgrade()
                    .ok()
                    .and_then(|surface| self.caminito.bell_window(&surface)),
            ),
            None => BellFor::Focus,
        };
        self.caminito.ring_bell(BellSource::GtkShell, bell_for);
    }
}
