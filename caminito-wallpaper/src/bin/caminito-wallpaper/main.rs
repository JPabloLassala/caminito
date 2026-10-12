//! `caminito-wallpaper` (212, ADR 0031): the Wallpaper dialog of a Session's Control Center,
//! which sets each output's wallpaper for `caminito-wallpaperd` to draw. One window, in GTK4
//! without libadwaita, as caminito-displays is (ADR 0014).
//!
//! `--demo` shows the dialog with made-up outputs and no store, for looking at it where
//! there's no Caminito, or no settings to change.

#![forbid(unsafe_code)]

mod decoder;
mod window;

use gtk::prelude::*;
use gtk::{gio, glib};

fn main() -> glib::ExitCode {
    let demo = std::env::args()
        .skip(1)
        .any(|argument| argument == "--demo");
    // A second start shows the window there is, but a demo is nobody's second.
    let flags = if demo {
        gio::ApplicationFlags::NON_UNIQUE
    } else {
        gio::ApplicationFlags::empty()
    };
    let application = gtk::Application::builder()
        .application_id("org.mate.Caminito.Wallpaper")
        .flags(flags)
        .build();
    application.connect_activate(move |application| match application.active_window() {
        Some(window) => window.present(),
        None => window::open(application, demo),
    });
    // Without the arguments, which GApplication would refuse.
    application.run_with_args::<&str>(&[])
}
