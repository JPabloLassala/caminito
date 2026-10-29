//! The settings window (CMN-261), "Notifications" in the control center, in the daemon's
//! process (ADR 0051), in GTK4 without libadwaita (ADR 0014). Each control writes its key in
//! `org.mate.Caminito.notifications`, or in a Notifying app's own path, so the daemon follows
//! a change as it's made. The General page holds every global setting; the Apps page is a
//! table of the Notifying apps' own settings, the default first (CMN-262).

use std::cell::{Cell, RefCell};
use std::rc::{Rc, Weak};

use caminito_notifications::apps::{self, App};
use caminito_notifications::server::Icon;
use caminito_notifications::settings::{self, AppSettings, Corner, Delivery, Settings};
use gtk::prelude::*;
use gtk::{gdk, gio, glib};

const TITLE: &str = "Notifications";
/// The space between the window's parts and around them, in pixels.
const SPACING: i32 = 18;
/// The minutes between two of a schedule's times the arrows step through.
const SCHEDULE_STEP: f64 = 15.0;
/// The Apps page's columns after Delivery: each key, its title, and its full name, which is
/// the title's tooltip and the check button's.
const FLAGS: [(&str, &str, &str); 6] = [
    (settings::SOUND, "Sound", "Play a sound"),
    (settings::PREVIEWS, "Previews", "Show previews in bubbles"),
    (settings::APP_ICON, "Icon", "Show the app's icon in bubbles"),
    (settings::GROUP, "Group", "Group notifications"),
    (settings::BADGE, "Badge", "Show a badge on its Dock item"),
    (
        settings::BREAKS_THROUGH,
        "Breaks DND",
        "Break through Do Not Disturb",
    ),
];
/// The Apps page's least height for its table, in pixels.
const TABLE_HEIGHT: i32 = 360;

/// The window, while it's open.
#[derive(Default)]
pub(crate) struct SettingsWindow {
    window: RefCell<glib::WeakRef<gtk::Window>>,
    /// The window's pages, and its Apps page.
    pages: RefCell<Option<(gtk::Notebook, Rc<AppsPage>)>>,
}

impl SettingsWindow {
    /// Opens the window, or raises it if it's open: at the General page, or at the Apps page
    /// with the row of the Notifying app `app` selected.
    pub(crate) fn open(
        &self,
        application: &gtk::Application,
        settings: &Settings,
        app: Option<&str>,
    ) {
        let window = self.window.borrow().upgrade();
        let window = window.unwrap_or_else(|| {
            let (window, pages, apps) = build(application, settings);
            self.window.borrow().set(Some(&window));
            *self.pages.borrow_mut() = Some((pages, apps));
            window
        });
        if let Some(app) = app
            && let Some((pages, apps)) = self.pages.borrow().as_ref()
        {
            pages.set_current_page(Some(APPS_PAGE));
            apps.select(app);
        }
        window.present();
    }
}

/// The Apps page's place among the pages.
const APPS_PAGE: u32 = 1;

fn build(
    application: &gtk::Application,
    settings: &Settings,
) -> (gtk::Window, gtk::Notebook, Rc<AppsPage>) {
    let window = gtk::ApplicationWindow::builder()
        .application(application)
        .title(TITLE)
        .icon_name("preferences-system-notifications")
        .resizable(false)
        .build();
    let pages = gtk::Notebook::new();
    pages.append_page(
        &general(settings.global()),
        Some(&gtk::Label::new(Some("General"))),
    );
    let apps = AppsPage::new(settings);
    pages.append_page(&apps.page, Some(&gtk::Label::new(Some("Apps"))));
    window.set_child(Some(&pages));
    window.connect_destroy({
        let apps = apps.clone();
        move |_| apps.disconnect()
    });

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
    (window.upcast(), pages, apps)
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
    full_row(
        &apps,
        4,
        &check(settings, settings::GROUP, "Group notifications"),
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

/// The Apps page: a search, and a table of one row per Notifying app, the default first, with
/// a column per setting. A row follows its key in the global settings or the app's own path
/// as it changes, from here or elsewhere.
struct AppsPage {
    settings: Settings,
    page: gtk::Box,
    search: gtk::SearchEntry,
    list: gtk::ListBox,
    viewport: gtk::Viewport,
    /// One per column, to line the rows' cells up under the titles.
    columns: Vec<gtk::SizeGroup>,
    /// The default's row first, then the apps', in the order they were added.
    rows: RefCell<Vec<Rc<Row>>>,
    /// What the global settings call back while the window is open.
    handlers: RefCell<Vec<glib::SignalHandlerId>>,
}

/// One row of the Apps page.
struct Row {
    /// The app, or `None` for the default.
    app: Option<App>,
    /// The app's own settings, for the app's row.
    own: Option<gio::Settings>,
    row: gtk::ListBoxRow,
    delivery: gtk::DropDown,
    checks: Vec<gtk::CheckButton>,
    /// "Use the default", on an app's row while it has its own settings.
    reset: gtk::Button,
    /// Set while the row shows what the settings hold, when its controls' changes aren't the
    /// user's.
    syncing: Cell<bool>,
}

impl AppsPage {
    fn new(settings: &Settings) -> Rc<AppsPage> {
        let page = gtk::Box::builder()
            .orientation(gtk::Orientation::Vertical)
            .spacing(6)
            .margin_top(SPACING)
            .margin_bottom(SPACING)
            .margin_start(SPACING)
            .margin_end(SPACING)
            .build();
        let search = gtk::SearchEntry::builder()
            .placeholder_text("Search apps")
            .build();
        page.append(&search);

        let columns: Vec<gtk::SizeGroup> = (0..FLAGS.len() + 3)
            .map(|_| gtk::SizeGroup::new(gtk::SizeGroupMode::Horizontal))
            .collect();
        let titles = cells(&columns);
        let title = |text: &str, tooltip: Option<&str>| {
            let label = gtk::Label::builder().xalign(0.0).build();
            label.set_markup(&format!("<b>{}</b>", glib::markup_escape_text(text)));
            label.set_tooltip_text(tooltip);
            label
        };
        titles[0].append(&title("App", None));
        titles[1].append(&title("Delivery", None));
        for (cell, (_, name, full)) in titles[2..].iter().zip(FLAGS) {
            cell.append(&title(name, Some(full)));
        }
        let header = gtk::Box::builder()
            .spacing(10)
            .margin_start(6)
            .margin_end(6)
            .build();
        for cell in &titles {
            header.append(cell);
        }
        page.append(&header);

        let list = gtk::ListBox::new();
        list.set_selection_mode(gtk::SelectionMode::Single);
        let viewport = gtk::Viewport::builder().child(&list).build();
        let scrolled = gtk::ScrolledWindow::builder()
            .child(&viewport)
            .hscrollbar_policy(gtk::PolicyType::Never)
            .propagate_natural_width(true)
            .min_content_height(TABLE_HEIGHT)
            .vexpand(true)
            .has_frame(true)
            .build();
        page.append(&scrolled);

        let this = Rc::new(AppsPage {
            settings: settings.clone(),
            page,
            search,
            list,
            viewport,
            columns,
            rows: RefCell::default(),
            handlers: RefCell::default(),
        });
        this.add(None);
        for app in apps::listed(&settings.known_apps()) {
            this.add(Some(app));
        }

        let weak = Rc::downgrade(&this);
        this.list.set_sort_func({
            let weak = weak.clone();
            move |a, b| {
                let Some(this) = weak.upgrade() else {
                    return gtk::Ordering::Equal;
                };
                let key = |row: &gtk::ListBoxRow| {
                    this.row_of(row).map(|row| match &row.app {
                        None => (0, String::new()),
                        Some(app) => (1, app.name.to_lowercase()),
                    })
                };
                key(a).cmp(&key(b)).into()
            }
        });
        this.list.set_filter_func({
            let weak = weak.clone();
            move |row| {
                let Some(this) = weak.upgrade() else {
                    return true;
                };
                let query = this.search.text().to_lowercase();
                this.row_of(row).is_none_or(|row| match &row.app {
                    None => true,
                    Some(app) => {
                        app.name.to_lowercase().contains(&query)
                            || app.key.to_lowercase().contains(&query)
                    }
                })
            }
        });
        this.search.connect_search_changed({
            let weak = weak.clone();
            move |_| {
                if let Some(this) = weak.upgrade() {
                    this.list.invalidate_filter();
                }
            }
        });

        let global = settings.global();
        let changed = global.connect_changed(None, {
            let weak = weak.clone();
            move |_, key| {
                let Some(this) = weak.upgrade() else {
                    return;
                };
                if key == settings::KNOWN_APPS {
                    this.add_known();
                } else if settings::APP_KEYS.contains(&key) {
                    // The default's row, and every app that follows it.
                    let rows = this.rows.borrow().clone();
                    for row in rows {
                        row.sync(&this.settings);
                    }
                }
            }
        });
        this.handlers.borrow_mut().push(changed);
        this
    }

    /// Stops following the global settings, once the window is gone.
    fn disconnect(&self) {
        for handler in self.handlers.take() {
            self.settings.global().disconnect(handler);
        }
    }

    /// The row of `list_row`.
    fn row_of(&self, list_row: &gtk::ListBoxRow) -> Option<Rc<Row>> {
        self.rows
            .borrow()
            .iter()
            .find(|row| row.row == *list_row)
            .cloned()
    }

    /// The row of the app whose key is `key`.
    fn row_of_app(&self, key: &str) -> Option<Rc<Row>> {
        self.rows
            .borrow()
            .iter()
            .find(|row| row.app.as_ref().is_some_and(|app| app.key == key))
            .cloned()
    }

    /// Adds the rows of the known apps that have none, as they join.
    fn add_known(self: &Rc<Self>) {
        let known = self.settings.known_apps();
        for app in apps::listed(&known) {
            if self.row_of_app(&app.key).is_none() && known.contains(&app.key) {
                self.add(Some(app));
            }
        }
    }

    /// Selects the row of the app whose key is `key`, with a row of its own if it has none,
    /// and scrolls to it. The search is cleared, so it shows.
    fn select(self: &Rc<Self>, key: &str) {
        let row = self.row_of_app(key).unwrap_or_else(|| {
            let listed = apps::listed(&[key.to_owned()]);
            let app = listed.into_iter().find(|app| app.key == key);
            self.add(Some(app.unwrap_or_else(|| App {
                key: key.to_owned(),
                name: key.to_owned(),
                icon: None,
            })))
        });
        self.search.set_text("");
        self.list.select_row(Some(&row.row));
        row.row.grab_focus();
        // Once the window that's opening has laid the table out.
        let viewport = self.viewport.clone();
        glib::idle_add_local_once(move || viewport.scroll_to(&row.row, None));
    }

    /// Adds the row of `app`, or of the default for `None`.
    fn add(self: &Rc<Self>, app: Option<App>) -> Rc<Row> {
        let cells = cells(&self.columns);
        let image = match app.as_ref().and_then(|app| app.icon.as_ref()) {
            _ if app.is_none() => gtk::Image::from_icon_name("preferences-system-notifications"),
            Some(Icon::Name(name)) => gtk::Image::from_icon_name(name),
            Some(Icon::File(path)) => gtk::Image::from_file(path),
            None => gtk::Image::from_icon_name("application-x-executable"),
        };
        image.set_pixel_size(20);
        let name = gtk::Label::builder()
            .xalign(0.0)
            .ellipsize(gtk::pango::EllipsizeMode::End)
            .max_width_chars(22)
            .build();
        match &app {
            None => {
                name.set_markup("<b>Default</b>");
                name.set_tooltip_text(Some("What an app without its own settings gets"));
            }
            Some(app) => {
                name.set_text(&app.name);
                name.set_tooltip_text(Some(&app.key));
            }
        }
        cells[0].set_spacing(8);
        cells[0].append(&image);
        cells[0].append(&name);

        let labels: Vec<&str> = Delivery::ALL
            .iter()
            .map(|delivery| delivery.label())
            .collect();
        let delivery = gtk::DropDown::from_strings(&labels);
        delivery.set_hexpand(true);
        cells[1].append(&delivery);
        let checks: Vec<gtk::CheckButton> = FLAGS
            .iter()
            .zip(&cells[2..])
            .map(|((_, _, full), cell)| {
                let check = gtk::CheckButton::builder().tooltip_text(*full).build();
                cell.set_halign(gtk::Align::Fill);
                check.set_hexpand(true);
                check.set_halign(gtk::Align::Center);
                cell.append(&check);
                check
            })
            .collect();
        let reset = gtk::Button::builder()
            .icon_name("edit-undo-symbolic")
            .tooltip_text("Use the default")
            .has_frame(false)
            .build();
        cells[FLAGS.len() + 2].append(&reset);

        let line = gtk::Box::builder()
            .spacing(10)
            .margin_top(2)
            .margin_bottom(2)
            .margin_start(6)
            .margin_end(6)
            .build();
        for cell in &cells {
            line.append(cell);
        }
        let list_row = gtk::ListBoxRow::builder().child(&line).build();
        let own = app.as_ref().map(|app| self.settings.app(&app.key));
        let row = Rc::new(Row {
            app,
            own,
            row: list_row,
            delivery,
            checks,
            reset,
            syncing: Cell::new(false),
        });
        row.sync(&self.settings);
        self.list.append(&row.row);
        self.rows.borrow_mut().push(row.clone());

        let settings = self.settings.clone();
        let weak = Rc::downgrade(&row);
        row.delivery.connect_selected_notify({
            let (settings, weak) = (settings.clone(), weak.clone());
            move |dropdown| {
                let Some(delivery) = usize::try_from(dropdown.selected())
                    .ok()
                    .and_then(|index| Delivery::ALL.get(index))
                else {
                    return;
                };
                set(
                    &settings,
                    &weak,
                    settings::DELIVERY,
                    delivery.nick().to_variant(),
                );
            }
        });
        for ((key, _, _), check) in FLAGS.iter().zip(&row.checks) {
            check.connect_toggled({
                let (settings, weak) = (settings.clone(), weak.clone());
                move |check| set(&settings, &weak, key, check.is_active().to_variant())
            });
        }
        row.reset.connect_clicked({
            let (settings, weak) = (settings.clone(), weak.clone());
            move |_| {
                if let Some(app) = weak.upgrade().and_then(|row| row.app.clone()) {
                    settings.use_default(&app.key);
                }
            }
        });
        if let Some(own) = &row.own {
            own.connect_changed(None, move |_, _| {
                if let Some(row) = weak.upgrade() {
                    row.sync(&settings);
                }
            });
        }
        row
    }
}

/// Writes `value` to `key` for the row `row`: the default's, or the app's own, which it
/// first gets all of if it followed the default.
fn set(settings: &Settings, row: &Weak<Row>, key: &str, value: glib::Variant) {
    let Some(row) = row.upgrade() else {
        return;
    };
    if row.syncing.get() {
        return;
    }
    let written = match (&row.app, &row.own) {
        (Some(app), Some(own)) => {
            settings.make_own(&app.key);
            own.set_value(key, &value)
        }
        _ => settings.global().set_value(key, &value),
    };
    if let Err(err) = written {
        glib::g_warning!(caminito_notifications::DOMAIN, "can't set {key}: {err}");
    }
}

impl Row {
    /// Shows what the app gets, the default's for an app that follows it, with the settings
    /// its Delivery leaves without effect greyed out.
    fn sync(&self, settings: &Settings) {
        self.syncing.set(true);
        let values = match &self.app {
            Some(app) => settings.effective(&app.key),
            None => settings.defaults(),
        };
        let position = Delivery::ALL
            .iter()
            .position(|delivery| *delivery == values.delivery)
            .and_then(|position| u32::try_from(position).ok())
            .unwrap_or(0);
        self.delivery.set_selected(position);
        for ((key, _, _), check) in FLAGS.iter().zip(&self.checks) {
            check.set_active(flag(&values, key));
            check.set_sensitive(settings::applies(values.delivery, key));
        }
        let own = self
            .app
            .as_ref()
            .is_some_and(|app| settings.has_own(&app.key));
        // Kept in its place, so the column stays as wide when no row has one.
        self.reset.set_opacity(if own { 1.0 } else { 0.0 });
        self.reset.set_sensitive(own);
        self.syncing.set(false);
    }
}

/// The value of the boolean `key` of [`FLAGS`] in `values`.
fn flag(values: &AppSettings, key: &str) -> bool {
    match key {
        settings::SOUND => values.sound,
        settings::PREVIEWS => values.previews,
        settings::APP_ICON => values.app_icon,
        settings::GROUP => values.group,
        settings::BADGE => values.badge,
        _ => values.breaks_through,
    }
}

/// One empty cell for each of `columns`, in its size group.
fn cells(columns: &[gtk::SizeGroup]) -> Vec<gtk::Box> {
    columns
        .iter()
        .map(|column| {
            let cell = gtk::Box::new(gtk::Orientation::Horizontal, 0);
            column.add_widget(&cell);
            cell
        })
        .collect()
}
