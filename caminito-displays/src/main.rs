//! caminito-displays (84, ADR 0014): Caminito's display settings program, which takes the
//! place of MATE's Display capplet in a Session. One window, in GTK4 without libadwaita.
//!
//! `--demo` shows the page with made-up monitors and changes nothing outside the window,
//! for looking at the page where there's no Caminito, or none to rearrange.

mod arrangement;
mod compositor;
mod identify;
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
        .application_id("org.mate.Caminito.Displays")
        .flags(flags)
        .build();
    application.connect_activate(move |application| match application.active_window() {
        Some(window) => window.present(),
        None => window::open(application, demo),
    });
    // Without the arguments, which GApplication would refuse.
    application.run_with_args::<&str>(&[])
}
