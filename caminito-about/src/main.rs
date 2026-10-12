//! caminito-about (182): the "About Caminito" of a Session's Control Center. One small
//! window that says what Caminito is and which build is installed, in GTK4 without
//! libadwaita, as caminito-displays is.
//!
//! The version is the installed build's, which `caminito --version` prints too, and not one
//! asked of the running compositor: the two differ only between an upgrade and the next
//! login.

#![forbid(unsafe_code)]

use gtk::prelude::*;
use gtk::{gdk, glib};

/// The logo's name in the icon theme (109).
const ICON: &str = "caminito";
const NAME: &str = "Caminito";
const DESCRIPTION: &str = "A Wayland compositor for the MATE desktop";
/// What Caminito is built on, and where each is kept.
const BUILT_ON: [(&str, &str); 2] = [
    ("Marco", "https://github.com/mate-desktop/marco"),
    ("Smithay", "https://github.com/Smithay/smithay"),
];
/// The licence's SPDX name, and the GNU page of its text.
const LICENCE: (&str, &str) = (
    "GPL-2.0-or-later",
    "https://www.gnu.org/licenses/old-licenses/gpl-2.0.html",
);
const REPOSITORY: &str = env!("CARGO_PKG_REPOSITORY");

/// The space between the window's parts and around them, in pixels.
const SPACING: i32 = 18;

fn main() -> glib::ExitCode {
    let application = gtk::Application::builder()
        .application_id("org.mate.Caminito.About")
        .build();
    // A second start activates the first, which raises the window it has.
    application.connect_activate(|application| match application.active_window() {
        Some(window) => window.present(),
        None => open(application),
    });
    // Without the arguments, which GApplication would refuse.
    application.run_with_args::<&str>(&[])
}

/// Opens the window: the prototype's compact card (182). The logo is on the left, and
/// beside it the name with the version, the description, and a small dim line of links.
fn open(application: &gtk::Application) {
    let card = gtk::Box::builder()
        .spacing(SPACING)
        .margin_top(SPACING)
        .margin_bottom(SPACING)
        .margin_start(SPACING)
        .margin_end(SPACING)
        .build();

    let logo = gtk::Image::builder()
        .icon_name(ICON)
        .pixel_size(64)
        .valign(gtk::Align::Start)
        .build();
    card.append(&logo);

    let text = gtk::Box::new(gtk::Orientation::Vertical, 4);
    card.append(&text);

    // Selectable, so the version can be copied into a bug report.
    let title = gtk::Label::builder()
        .label(title_markup(caminito_version::VERSION))
        .use_markup(true)
        .selectable(true)
        .xalign(0.0)
        .build();
    text.append(&title);
    text.append(&gtk::Label::builder().label(DESCRIPTION).xalign(0.0).build());

    // GTK opens a link of a label in the user's browser by itself.
    let small = gtk::Label::builder()
        .label(small_markup())
        .use_markup(true)
        .xalign(0.0)
        .margin_top(8)
        .css_classes(["dim-label"])
        .build();
    text.append(&small);

    // No Close button: the titlebar has one, and Escape closes the window too.
    let window = gtk::ApplicationWindow::builder()
        .application(application)
        .title("About Caminito")
        .icon_name(ICON)
        .resizable(false)
        .child(&card)
        .build();
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
    // GTK gives a new window's first widget the focus, and a selectable label that takes it
    // selects all its text.
    GtkWindowExt::set_focus(&window, gtk::Widget::NONE);
    title.select_region(0, 0);
}

/// The name in large bold type, and the version string after it on the same line.
fn title_markup(version: &str) -> String {
    format!(
        "<span size=\"x-large\" weight=\"bold\">{NAME}</span>  {}",
        glib::markup_escape_text(version)
    )
}

/// The small line: what Caminito is built on, its licence, and under them the repository.
fn small_markup() -> String {
    let built_on = BUILT_ON.map(|(name, url)| link(name, url)).join(" and ");
    let (licence, licence_text) = LICENCE;
    let shown = REPOSITORY.strip_prefix("https://").unwrap_or(REPOSITORY);
    format!(
        "<small>Built on {built_on}. {}.\n{}</small>",
        link(licence, licence_text),
        link(shown, REPOSITORY)
    )
}

/// A link of a label's markup.
fn link(text: &str, url: &str) -> String {
    format!(
        "<a href=\"{}\">{}</a>",
        glib::markup_escape_text(url),
        glib::markup_escape_text(text)
    )
}
