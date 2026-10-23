//! `caminito-behaviour` (CMN-184): Workspace Behaviour, the settings of how Caminito itself
//! behaves beyond what MATE's capplets set. One window of pages, in GTK4 without libadwaita,
//! as caminito-displays is (ADR 0014). Hot corners and Key remaps (CMN-189) are its pages
//! for now; the animations and Per-output workspaces join them later, each a tab of its own.

#![forbid(unsafe_code)]

mod hot_corners_page;
mod key_remaps_page;

use caminito_behaviour::hot_corners::HotCorners;
use caminito_behaviour::key_remaps::KeyRemapKeys;
use gtk::prelude::*;
use gtk::{gdk, glib};

/// The log domain of the window's warnings.
const DOMAIN: &str = "caminito-behaviour";
const TITLE: &str = "Workspace Behaviour";
/// The space between the window's parts and around them, in pixels.
const SPACING: i32 = 18;

fn main() -> glib::ExitCode {
    let application = gtk::Application::builder()
        .application_id("org.mate.Caminito.Behaviour")
        .build();
    // A second start activates the first, which raises the window it has.
    application.connect_activate(|application| match application.active_window() {
        Some(window) => window.present(),
        None => open(application),
    });
    // Without the arguments, which GApplication would refuse.
    application.run_with_args::<&str>(&[])
}

/// Opens the window: a tab for each page.
fn open(application: &gtk::Application) {
    let window = gtk::ApplicationWindow::builder()
        .application(application)
        .title(TITLE)
        .icon_name("preferences-system-windows")
        .resizable(false)
        .build();
    let child = match HotCorners::open() {
        Ok(corners) => {
            let pages = gtk::Notebook::new();
            pages.append_page(
                &hot_corners_page::build(corners),
                Some(&gtk::Label::new(Some("Hot corners"))),
            );
            let remaps = match KeyRemapKeys::open() {
                Ok(keys) => key_remaps_page::build(keys),
                Err(err) => {
                    glib::g_critical!(DOMAIN, "can't open the key remaps: {err}");
                    unavailable(&format!("The key remaps can't be opened: {err}."))
                }
            };
            pages.append_page(&remaps, Some(&gtk::Label::new(Some("Key remaps"))));
            pages.upcast::<gtk::Widget>()
        }
        Err(err) => {
            glib::g_critical!(DOMAIN, "can't open the settings: {err}");
            unavailable(&format!("Caminito's settings can't be opened: {err}."))
        }
    };
    window.set_child(Some(&child));

    // No Close button: the titlebar has one, and Escape closes the window too.
    let keys = gtk::EventControllerKey::new();
    keys.connect_key_pressed({
        let window = window.downgrade();
        move |_, key, _, _| {
            if key != gdk::Key::Escape {
                return glib::Propagation::Proceed;
            }
            if let Some(window) = window.upgrade() {
                window.close();
            }
            glib::Propagation::Stop
        }
    });
    window.add_controller(keys);
    window.present();
}

/// What the window or a page shows in place of what can't be opened.
fn unavailable(text: &str) -> gtk::Widget {
    gtk::Label::builder()
        .label(text)
        .margin_top(SPACING)
        .margin_bottom(SPACING)
        .margin_start(SPACING)
        .margin_end(SPACING)
        .build()
        .upcast()
}
