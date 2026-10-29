//! The settings window (CMN-261), "Notifications" in the control center, in the daemon's
//! process (ADR 0051), in GTK4 without libadwaita (ADR 0014). Each control is bound to its
//! key in `org.mate.Caminito.notifications`, so the daemon follows a change as it's made. The
//! General page holds every global setting; the Apps page joins it with the Notifying apps'
//! own settings (CMN-262).

use std::cell::RefCell;

use caminito_notifications::settings::{self, Corner, Delivery};
use gtk::prelude::*;
use gtk::{gdk, gio, glib};

const TITLE: &str = "Notifications";
/// The space between the window's parts and around them, in pixels.
const SPACING: i32 = 18;
/// The minutes between two of a schedule's times the arrows step through.
const SCHEDULE_STEP: f64 = 15.0;

/// The window, while it's open.
#[derive(Default)]
pub(crate) struct SettingsWindow {
    window: RefCell<glib::WeakRef<gtk::Window>>,
}

impl SettingsWindow {
    /// Opens the window, or raises it if it's open. `app` is the Notifying app whose page it
    /// opens at, which the General page stands for until the Apps page (CMN-262).
    pub(crate) fn open(
        &self,
        application: &gtk::Application,
        settings: &gio::Settings,
        app: Option<&str>,
    ) {
        let _ = app;
        if let Some(window) = self.window.borrow().upgrade() {
            window.present();
            return;
        }
        let window = build(application, settings);
        self.window.borrow().set(Some(&window));
        window.present();
    }
}

fn build(application: &gtk::Application, settings: &gio::Settings) -> gtk::Window {
    let window = gtk::ApplicationWindow::builder()
        .application(application)
        .title(TITLE)
        .icon_name("preferences-system-notifications")
        .resizable(false)
        .build();
    let pages = gtk::Notebook::new();
    pages.append_page(&general(settings), Some(&gtk::Label::new(Some("General"))));
    window.set_child(Some(&pages));

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
    window.upcast()
}

/// The General page: what an app without its own settings gets, the bubbles, and Do Not
/// Disturb.
fn general(settings: &gio::Settings) -> gtk::Widget {
    let page = gtk::Box::builder()
        .orientation(gtk::Orientation::Vertical)
        .spacing(SPACING)
        .margin_top(SPACING)
        .margin_bottom(SPACING)
        .margin_start(SPACING)
        .margin_end(SPACING)
        .build();

    let apps = section(&page, "Notifications");
    row(
        &apps,
        0,
        "Delivery",
        &choice(
            settings,
            settings::DELIVERY,
            &Delivery::ALL.map(|delivery| (delivery.nick(), delivery.label())),
        ),
    );
    full_row(&apps, 1, &check(settings, settings::SOUND, "Play a sound"));
    full_row(
        &apps,
        2,
        &check(settings, settings::PREVIEWS, "Show previews in bubbles"),
    );
    full_row(
        &apps,
        3,
        &check(
            settings,
            settings::APP_ICON,
            "Show the app's icon in bubbles",
        ),
    );

    let bubbles = section(&page, "Bubbles");
    row(
        &bubbles,
        0,
        "Corner",
        &choice(
            settings,
            settings::CORNER,
            &Corner::ALL.map(|corner| (corner.nick(), corner.label())),
        ),
    );
    let timeout = gtk::SpinButton::with_range(1.0, 60.0, 1.0);
    settings.bind(settings::TIMEOUT, &timeout, "value").build();
    let seconds = gtk::Box::new(gtk::Orientation::Horizontal, 6);
    seconds.append(&timeout);
    seconds.append(&gtk::Label::new(Some("seconds")));
    row(&bubbles, 1, "Stay for", &seconds);

    let quiet = section(&page, "Do Not Disturb");
    full_row(
        &quiet,
        0,
        &check(
            settings,
            settings::CRITICAL_BREAKS_THROUGH,
            "Critical notifications break through",
        ),
    );
    let schedule = gtk::Box::new(gtk::Orientation::Horizontal, 6);
    schedule.append(&check(
        settings,
        settings::DO_NOT_DISTURB_SCHEDULE,
        "Every day from",
    ));
    let from = time(settings, settings::DO_NOT_DISTURB_FROM);
    let to = time(settings, settings::DO_NOT_DISTURB_TO);
    schedule.append(&from);
    schedule.append(&gtk::Label::new(Some("to")));
    schedule.append(&to);
    for spin in [&from, &to] {
        settings
            .bind(settings::DO_NOT_DISTURB_SCHEDULE, spin, "sensitive")
            .get()
            .build();
    }
    full_row(&quiet, 1, &schedule);
    full_row(
        &quiet,
        2,
        &check(
            settings,
            settings::DO_NOT_DISTURB_WHILE_FULLSCREEN,
            "While a window is fullscreen",
        ),
    );
    full_row(
        &quiet,
        3,
        &check(
            settings,
            settings::DO_NOT_DISTURB_WHILE_SCREENCASTING,
            "While the screen is shared or recorded",
        ),
    );

    page.upcast()
}

/// A heading on `page`, and the grid of its controls below it.
fn section(page: &gtk::Box, heading: &str) -> gtk::Grid {
    let label = gtk::Label::builder().xalign(0.0).build();
    label.set_markup(&format!("<b>{}</b>", glib::markup_escape_text(heading)));
    page.append(&label);
    let grid = gtk::Grid::builder()
        .row_spacing(6)
        .column_spacing(12)
        .margin_start(12)
        .build();
    page.append(&grid);
    grid
}

/// A control with its label before it, at `line` of `grid`.
fn row(grid: &gtk::Grid, line: i32, label: &str, control: &impl IsA<gtk::Widget>) {
    let label = gtk::Label::builder().label(label).xalign(0.0).build();
    grid.attach(&label, 0, line, 1, 1);
    grid.attach(control, 1, line, 1, 1);
}

/// A control the width of `grid`, at `line`.
fn full_row(grid: &gtk::Grid, line: i32, control: &impl IsA<gtk::Widget>) {
    grid.attach(control, 0, line, 2, 1);
}

/// A check button bound to the boolean `key`.
fn check(settings: &gio::Settings, key: &str, label: &str) -> gtk::CheckButton {
    let check = gtk::CheckButton::with_label(label);
    settings.bind(key, &check, "active").build();
    check
}

/// A drop-down bound to the enum `key`, with a nick and its label for each choice.
fn choice(
    settings: &gio::Settings,
    key: &str,
    choices: &[(&'static str, &'static str)],
) -> gtk::DropDown {
    let labels: Vec<&str> = choices.iter().map(|(_, label)| *label).collect();
    let dropdown = gtk::DropDown::from_strings(&labels);
    let nicks: Vec<&'static str> = choices.iter().map(|(nick, _)| *nick).collect();
    settings
        .bind(key, &dropdown, "selected")
        .mapping({
            let nicks = nicks.clone();
            move |variant, _| {
                let nick = variant.str()?;
                let index = nicks.iter().position(|known| *known == nick)?;
                Some(u32::try_from(index).ok()?.to_value())
            }
        })
        .set_mapping(move |value, _| {
            let index = usize::try_from(value.get::<u32>().ok()?).ok()?;
            Some(nicks.get(index)?.to_variant())
        })
        .build();
    dropdown
}

/// A spin button of the time of day `key` holds, in minutes from midnight, shown as hh:mm.
fn time(settings: &gio::Settings, key: &str) -> gtk::SpinButton {
    let spin = gtk::SpinButton::with_range(0.0, 1439.0, SCHEDULE_STEP);
    spin.set_wrap(true);
    spin.set_numeric(false);
    spin.set_width_chars(5);
    spin.connect_output(|spin| {
        // The adjustment's range keeps it a whole minute of the day.
        let minutes = spin.value() as u32;
        spin.set_text(&format!("{:02}:{:02}", minutes / 60, minutes % 60));
        glib::Propagation::Stop
    });
    spin.connect_input(|spin| {
        let text = spin.text();
        let (hours, minutes) = text.split_once(':').unwrap_or((&text, "0"));
        let hours: u32 = hours.trim().parse().ok()?;
        let minutes: u32 = minutes.trim().parse().ok()?;
        (hours < 24 && minutes < 60).then(|| Ok(f64::from(hours * 60 + minutes)))
    });
    settings.bind(key, &spin, "value").build();
    spin
}
