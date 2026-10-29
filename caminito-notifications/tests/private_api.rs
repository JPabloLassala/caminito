//! The daemon's private API on a private bus (ADR 0051): the test calls it as the
//! Notifications applet or the Dock applet would, over its own connection. Opening the
//! settings window is a fake, which records the app it was asked for (CMN-261).

mod bus;

use std::cell::RefCell;
use std::rc::Rc;

use caminito_notifications::private_api::PrivateApi;
use gio::glib::{self, Variant};
use gio::prelude::*;

use bus::Bus;

const INTERFACE: &str = "org.mate.Caminito.Notifications";
const PATH: &str = "/org/mate/Caminito/Notifications";

/// Calls the private API's `method` on `daemon`, without blocking the main context, which
/// runs the daemon too.
fn call(bus: &Bus, daemon: &gio::DBusConnection, method: &str, parameters: Variant) {
    let applet = bus.connect();
    let answer: Rc<RefCell<Option<Result<Variant, glib::Error>>>> = Rc::default();
    applet.call(
        daemon.unique_name().as_deref(),
        PATH,
        INTERFACE,
        method,
        Some(&parameters),
        None,
        gio::DBusCallFlags::NONE,
        -1,
        gio::Cancellable::NONE,
        {
            let answer = answer.clone();
            move |result| *answer.borrow_mut() = Some(result)
        },
    );
    bus.wait_until(&format!("{method}'s answer"), || answer.borrow().is_some());
    let answer = answer.take().expect("it answered");
    answer.unwrap_or_else(|err| panic!("{method} answers: {err}"));
}

#[test]
fn open_settings_opens_the_window_at_the_general_page_or_an_apps() {
    bus::run(|bus| {
        let daemon = bus.connect();
        let opened: Rc<RefCell<Vec<Option<String>>>> = Rc::default();
        let _api = PrivateApi::new(&daemon, {
            let opened = opened.clone();
            move |app| opened.borrow_mut().push(app.map(str::to_owned))
        })
        .expect("the private API registers");

        call(bus, &daemon, "OpenSettings", ("",).to_variant());
        call(bus, &daemon, "OpenSettings", ("firefox",).to_variant());

        assert_eq!(*opened.borrow(), [None, Some("firefox".to_owned())]);
    });
}
