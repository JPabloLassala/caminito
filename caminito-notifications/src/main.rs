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
use caminito_notifications::server::{Config, Server, SystemClock};
use caminito_notifications::settings;
use gtk::prelude::*;
use gtk::{gio, glib};

use settings_window::SettingsWindow;

/// The application's action that opens the settings window.
const OPEN_SETTINGS: &str = "settings";
/// The option the control center's entry runs the program with, for the settings window.
const SETTINGS_OPTION: &str = "settings";

/// What the primary instance runs: the server, and the private API unless it can't be
/// exported, which the bubbles don't need.
struct Daemon {
    _server: Server,
    _private_api: Option<PrivateApi>,
}

fn main() -> glib::ExitCode {
    let application = gtk::Application::builder()
        .application_id("org.mate.Caminito.Notifications")
        .build();
    application.add_main_option(
        SETTINGS_OPTION,
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
                "can't serve notifications: the schemas {} and {} aren't installed",
                settings::SCHEMA,
                settings::APP_SCHEMA
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

        let bubbles = Rc::new(bubbles::GtkBubbles::new(
            application,
            settings.global().clone(),
        ));
        let sounds = Rc::new(sound::Canberra::default());
        let history = Config {
            history_file: glib::user_state_dir()
                .join("caminito")
                .join("notification-history"),
            clock: Rc::new(SystemClock),
        };
        let server = match Server::new(&connection, bubbles, sounds, settings.clone(), history) {
            Ok(server) => server,
            Err(err) => {
                glib::g_warning!(DOMAIN, "{err}");
                return;
            }
        };
        let private_api = PrivateApi::new(&connection, server.history_calls(), {
            let application = application.downgrade();
            let window = window.clone();
            move |app| {
                if let Some(application) = application.upgrade() {
                    window.open(&application, &settings, app);
                }
            }
        });
        let private_api = private_api
            .inspect_err(|err| glib::g_warning!(DOMAIN, "{err}"))
            .ok();
        *daemon.borrow_mut() = Some(Daemon {
            _server: server,
            _private_api: private_api,
        });
    });

    // `--settings` asks the primary instance for the window, which this is too when no
    // daemon runs yet.
    application.connect_handle_local_options(|application, options| {
        if !options.contains(SETTINGS_OPTION) {
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
