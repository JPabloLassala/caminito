//! `caminito-notifications` (ADR 0051): the notification daemon of a Session, a
//! single-instance `GtkApplication`, which serves `org.freedesktop.Notifications`, draws the
//! Notification bubbles, exports the private API and has the settings window (CMN-261).
//!
//! Caminito starts it before mate-session and starts it again when it exits (CMN-258). The
//! program's own name on the bus, `org.mate.Caminito.Notifications`, keeps a second one from
//! starting. `caminito-notifications --settings`, which the control center's entry runs, has
//! the running daemon open the settings window, and exits.

mod bubbles;
mod settings_window;
mod sound;

use std::cell::RefCell;
use std::ops::ControlFlow;
use std::rc::Rc;

use caminito_notifications::DOMAIN;
use caminito_notifications::private_api::PrivateApi;
use caminito_notifications::server::Server;
use caminito_notifications::settings;
use gtk::prelude::*;
use gtk::{gio, glib};

use settings_window::SettingsWindow;

/// The application's action that opens the settings window.
const OPEN_SETTINGS: &str = "settings";

/// What the primary instance runs: the server and the private API.
struct Daemon {
    _server: Server,
    _private_api: PrivateApi,
}

fn main() -> glib::ExitCode {
    let application = gtk::Application::builder()
        .application_id("org.mate.Caminito.Notifications")
        .build();
    application.add_main_option(
        "settings",
        glib::Char::from(0),
        glib::OptionFlags::NONE,
        glib::OptionArg::None,
        "Open the settings window",
        None,
    );

    // The primary instance's: the daemon, and the hold it takes at its first activation.
    let daemon: Rc<RefCell<Option<Daemon>>> = Rc::default();
    let hold: Rc<RefCell<Option<gio::ApplicationHoldGuard>>> = Rc::default();
    let window = Rc::new(SettingsWindow::default());

    application.connect_startup(move |application| {
        let Some(connection) = application.dbus_connection() else {
            glib::g_warning!(DOMAIN, "can't serve notifications: not on the session bus");
            return;
        };
        let Some(settings) = settings::installed() else {
            glib::g_warning!(
                DOMAIN,
                "can't serve notifications: the schema {} isn't installed",
                settings::SCHEMA
            );
            return;
        };

        let open = gio::SimpleAction::new(OPEN_SETTINGS, None);
        open.connect_activate({
            let application = application.downgrade();
            let settings = settings.clone();
            let window = window.clone();
            move |_, _| {
                if let Some(application) = application.upgrade() {
                    window.open(&application, &settings, None);
                }
            }
        });
        application.add_action(&open);

        let bubbles = Rc::new(bubbles::GtkBubbles::new(application, settings.clone()));
        let sounds = Rc::new(sound::Canberra::default());
        let server = match Server::new(&connection, bubbles, sounds, settings.clone()) {
            Ok(server) => server,
            Err(err) => {
                glib::g_warning!(DOMAIN, "{err}");
                return;
            }
        };
        let private_api = PrivateApi::new(&connection, {
            let application = application.downgrade();
            let window = window.clone();
            move |app| {
                if let Some(application) = application.upgrade() {
                    window.open(&application, &settings, app);
                }
            }
        });
        match private_api {
            Ok(private_api) => {
                *daemon.borrow_mut() = Some(Daemon {
                    _server: server,
                    _private_api: private_api,
                });
            }
            Err(err) => glib::g_warning!(DOMAIN, "{err}"),
        }
    });

    // `--settings` asks the primary instance for the window, which this is too when no
    // daemon runs yet.
    application.connect_handle_local_options(|application, options| {
        if !options.contains("settings") {
            return ControlFlow::Continue(());
        }
        if let Err(err) = application.register(gio::Cancellable::NONE) {
            glib::g_warning!(DOMAIN, "can't open the settings: {err}");
            return ControlFlow::Break(glib::ExitCode::FAILURE);
        }
        application.activate_action(OPEN_SETTINGS, None);
        if application.is_remote() {
            ControlFlow::Break(glib::ExitCode::SUCCESS)
        } else {
            ControlFlow::Continue(())
        }
    });

    // It has no window to keep it, so it holds itself from its first activation.
    application.connect_activate(move |application| {
        hold.borrow_mut().get_or_insert_with(|| application.hold());
    });

    application.run()
}
