//! The Bluetooth settings window: an empty placeholder until This computer and Devices
//! (257) and Add New Device (258) fill it.

use gtk::prelude::*;

/// Opens the window, or raises the one there is. `scan` is for Add New Device (258), which
/// the placeholder hasn't yet.
pub(crate) fn present(application: &gtk::Application, _scan: bool) {
    if let Some(window) = application.active_window() {
        window.present();
        return;
    }
    let window = gtk::ApplicationWindow::builder()
        .application(application)
        .title("Bluetooth")
        .default_width(720)
        .default_height(480)
        .build();
    window.present();
}
