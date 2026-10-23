//! `caminito-network` (ADR 0041): one process through a Session, a single-instance
//! `GtkApplication` with no window, which is NetworkManager's secret agent.
//!
//! The autostart entry starts it at login. A second start finds the first on the session bus
//! and exits. Outside Caminito it exits at once, so that under X11 MATE nm-applet stays the
//! agent: two agents in one session would both ask for each secret.

#![forbid(unsafe_code)]

mod dialogs;

use std::cell::RefCell;
use std::rc::Rc;

use caminito_network::DOMAIN;
use caminito_network::agent::Agent;
use gtk::prelude::*;
use gtk::{gio, glib};

fn main() -> glib::ExitCode {
    if std::env::var_os("MATE_WM_NAME").is_none_or(|wm| wm != "Caminito") {
        return glib::ExitCode::SUCCESS;
    }

    let application = gtk::Application::builder()
        .application_id("org.mate.Caminito.Network")
        .build();

    // The primary instance's: the agent, and the hold it takes at its first activation.
    let agent: Rc<RefCell<Option<Agent>>> = Rc::default();
    let hold: Rc<RefCell<Option<gio::ApplicationHoldGuard>>> = Rc::default();

    application.connect_startup(move |_| {
        let system = match gio::bus_get_sync(gio::BusType::System, gio::Cancellable::NONE) {
            Ok(system) => system,
            Err(err) => {
                glib::g_warning!(
                    DOMAIN,
                    "can't reach NetworkManager on the system bus: {err}"
                );
                return;
            }
        };
        match Agent::new(&system, Rc::new(dialogs::GtkDialogs::default())) {
            Ok(started) => *agent.borrow_mut() = Some(started),
            Err(err) => glib::g_warning!(DOMAIN, "{err}"),
        }
    });

    // The first start is activated as it starts, a second one's activation reaches it over
    // the session bus. Either way it holds itself, since it has no window to keep it.
    application.connect_activate(move |application| {
        hold.borrow_mut().get_or_insert_with(|| application.hold());
    });

    application.run()
}
