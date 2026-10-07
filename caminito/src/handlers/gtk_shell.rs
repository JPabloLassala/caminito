//! `gtk_shell1` (67): a GTK3 window's modal hint, which joins xdg-dialog's, and GTK3's bell.

use smithay::reexports::wayland_server::Resource;
use smithay::reexports::wayland_server::protocol::wl_surface::WlSurface;

use crate::protocols::gtk_shell::GtkShellHandler;
use crate::state::State;

impl GtkShellHandler for State {
    fn modal_changed(&mut self, surface: &WlSurface) {
        // A pending window's hint is read when it maps.
        if let Some(id) = self.caminito.mapped_for_surface(surface) {
            self.caminito.window_changed(id);
        }
    }

    /// M6 joins it to the one bell event (17).
    fn system_bell(&mut self, surface: Option<WlSurface>) {
        match surface {
            Some(surface) => tracing::debug!("gtk_shell1 rang the bell for {:?}", surface.id()),
            None => tracing::debug!("gtk_shell1 rang the bell for no window"),
        }
    }
}
