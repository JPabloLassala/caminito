//! caminito-displays (84, ADR 0014): Caminito's display settings program, which takes the
//! place of MATE's Display capplet in a Session. One window, in GTK4 without libadwaita, with
//! two tabs: Displays, and Night Light (142).
//!
//! - `--demo` shows the tabs with made-up monitors and a made-up Night Light, and changes
//!   nothing outside the window, for looking at them where there's no Caminito, or none to
//!   rearrange.
//! - `--night-light` opens the window on Night Light, or switches the open one to it. The
//!   Night Light entry in Preferences runs it.
//! - `--toggle-night-light` turns Night Light on or off and exits with no window, for a
//!   custom shortcut (100).
//!
//! A second start passes its command line to the first, over the session bus, and exits.

mod arrangement;
mod compositor;
mod identify;
mod night_light_tab;
mod window;

use gtk::prelude::*;
use gtk::{gio, glib};

fn main() -> glib::ExitCode {
    let demo = std::env::args()
        .skip(1)
        .any(|argument| argument == "--demo");
    // A second start shows the window there is, but a demo is nobody's second.
    let flags = if demo {
        gio::ApplicationFlags::HANDLES_COMMAND_LINE | gio::ApplicationFlags::NON_UNIQUE
    } else {
        gio::ApplicationFlags::HANDLES_COMMAND_LINE
    };
    let application = gtk::Application::builder()
        .application_id("org.mate.Caminito.Displays")
        .flags(flags)
        .build();
    let options = [
        (
            "demo",
            "Show made-up monitors and Night Light, and change nothing outside the window",
        ),
        ("night-light", "Open the window on Night Light"),
        (
            "toggle-night-light",
            "Turn Night Light on or off, with no window",
        ),
    ];
    for (name, description) in options {
        application.add_main_option(
            name,
            glib::Char::from(0),
            glib::OptionFlags::NONE,
            glib::OptionArg::None,
            description,
            None,
        );
    }

    // Before the application is registered on the bus, so this start neither opens a
    // window nor reaches a running one.
    application.connect_handle_local_options(|_, options| {
        if options.contains("toggle-night-light") {
            std::ops::ControlFlow::Break(night_light_tab::toggle())
        } else {
            std::ops::ControlFlow::Continue(())
        }
    });

    application.connect_command_line(move |application, command_line| {
        let tab = if command_line.options_dict().contains("night-light") {
            window::Tab::NightLight
        } else {
            window::Tab::Displays
        };
        match application.active_window() {
            Some(window) => {
                if tab == window::Tab::NightLight {
                    window::show(&window, tab);
                }
                window.present();
            }
            None => window::open(application, demo, tab),
        }
        glib::ExitCode::SUCCESS
    });

    application.run()
}
