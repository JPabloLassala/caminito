//! `caminito-notifications` (ADR 0051): the notification daemon of a Session, a
//! single-instance `GtkApplication` with no window of its own, which serves
//! `org.freedesktop.Notifications` and draws the Notification bubbles.
//!
//! Caminito starts it before mate-session and starts it again when it exits (CMN-258). The
//! program's own name on the bus, `org.mate.Caminito.Notifications`, keeps a second one from
//! starting.

mod bubbles;
mod sound;

use std::cell::RefCell;
use std::rc::Rc;

use caminito_notifications::DOMAIN;
use caminito_notifications::server::{DEFAULT_TIMEOUT, Server};
use gtk::prelude::*;
use gtk::{gio, glib};

fn main() -> glib::ExitCode {
    let application = gtk::Application::builder()
        .application_id("org.mate.Caminito.Notifications")
        .build();

    // The primary instance's: the server, and the hold it takes at its first activation.
    let server: Rc<RefCell<Option<Server>>> = Rc::default();
    let hold: Rc<RefCell<Option<gio::ApplicationHoldGuard>>> = Rc::default();

    application.connect_startup(move |application| {
        let Some(connection) = application.dbus_connection() else {
            glib::g_warning!(DOMAIN, "can't serve notifications: not on the session bus");
            return;
        };
        let bubbles = Rc::new(bubbles::GtkBubbles::new(application));
        let sounds = Rc::new(sound::Canberra::default());
        match Server::new(&connection, bubbles, sounds, DEFAULT_TIMEOUT) {
            Ok(started) => *server.borrow_mut() = Some(started),
            Err(err) => glib::g_warning!(DOMAIN, "{err}"),
        }
    });

    // It has no window to keep it, so it holds itself from its first activation.
    application.connect_activate(move |application| {
        hold.borrow_mut().get_or_insert_with(|| application.hold());
    });

    application.run()
}
