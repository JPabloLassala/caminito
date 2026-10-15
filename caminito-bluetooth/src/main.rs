//! `caminito-bluetooth` (255, ADR 0035): one process through the whole session, a
//! single-instance `GtkApplication`.
//!
//! - `--background`, from the autostart entry at login, opens no window and holds the
//!   process until the session ends, for the pairing agent (256), which every start
//!   registers.
//! - With no flag, the running instance opens or raises the settings window, or a new one
//!   starts and does. That's what the Control Center and the applet's "Configure…" run.
//! - `--scan` does the same on Add New Device (258).
//!
//! A second start passes its command line to the first, over the session bus, and exits.
//! The first holds itself whichever way it started, so closing the window doesn't end it:
//! the agent lives in it.

#![forbid(unsafe_code)]

mod dialogs;
mod window;

use std::cell::RefCell;
use std::rc::Rc;

use caminito_bluetooth::DOMAIN;
use caminito_bluetooth::agent::Agent;
use caminito_bluetooth::bluez::Bluez;
use caminito_bluetooth::rfkill::SystemRfkill;
use caminito_bluetooth::switch::Switch;
use gtk::prelude::*;
use gtk::{gio, glib};

fn main() -> glib::ExitCode {
    let application = gtk::Application::builder()
        .application_id("org.mate.Caminito.Bluetooth")
        .flags(gio::ApplicationFlags::HANDLES_COMMAND_LINE)
        .build();
    application.add_main_option(
        "background",
        glib::Char::from(0),
        glib::OptionFlags::NONE,
        glib::OptionArg::None,
        "Run through the session without opening a window",
        None,
    );
    application.add_main_option(
        "scan",
        glib::Char::from(0),
        glib::OptionFlags::NONE,
        glib::OptionArg::None,
        "Open the window on Add New Device",
        None,
    );

    // The primary instance's: BlueZ's model and Bluetooth's switch, which the window works
    // on, the pairing agent over them, and the hold it takes at its first command line.
    let services: Rc<RefCell<Option<window::Services>>> = Rc::default();
    let agent: Rc<RefCell<Option<Agent>>> = Rc::default();
    let hold: Rc<RefCell<Option<gio::ApplicationHoldGuard>>> = Rc::default();

    application.connect_startup({
        let services = services.clone();
        move |_| {
            let model = match gio::bus_get_sync(gio::BusType::System, gio::Cancellable::NONE) {
                Ok(system) => Bluez::new(&system),
                Err(err) => {
                    glib::g_warning!(DOMAIN, "can't reach BlueZ on the system bus: {err}");
                    return;
                }
            };
            match Agent::new(&model, Rc::new(dialogs::GtkDialogs::default())) {
                Ok(started) => *agent.borrow_mut() = Some(started),
                Err(err) => glib::g_warning!(DOMAIN, "{err}"),
            }
            let switch = Rc::new(Switch::new(&model, Rc::new(SystemRfkill::new())));
            *services.borrow_mut() = Some(window::Services {
                bluez: model,
                switch,
            });
        }
    });

    application.connect_command_line(move |application, command_line| {
        let options = command_line.options_dict();
        hold.borrow_mut().get_or_insert_with(|| application.hold());
        if !options.contains("background") {
            window::present(
                application,
                services.borrow().as_ref(),
                options.contains("scan"),
            );
        }
        glib::ExitCode::SUCCESS
    });

    application.run()
}
