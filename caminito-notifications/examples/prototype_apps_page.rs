//! PROTOTYPE, throw away (CMN-262). Three variants of the settings window's Apps page,
//! switchable from the bar at the bottom, with ← and →, or from the first argument:
//!
//!     cargo run -p caminito-notifications --example prototype_apps_page -- B
//!
//! A is a sidebar and a detail pane, B a table, C a list of expanders. The question was what
//! the page should look like, and the developer chose B, which it opens on. Everything is in memory: nothing is read from or written to
//! `org.mate.Caminito.notifications`, and the General page is a placeholder. The state is
//! printed under the page after every change.

use std::cell::{Cell, RefCell};
use std::rc::Rc;

use caminito_notifications::settings::Delivery;
use gtk::prelude::*;
use gtk::{gdk, gio, glib};

const SPACING: i32 = 18;
const VARIANTS: [(&str, &str); 3] = [("A", "Sidebar"), ("B", "Table"), ("C", "Expanders")];
/// The six boolean settings: the long label, the column title, and the state's word.
const FLAGS: [(&str, &str, &str); 6] = [
    ("Play a sound", "Sound", "sound"),
    ("Show previews in bubbles", "Previews", "previews"),
    ("Show the app's icon in bubbles", "Icon", "icon"),
    ("Group in the Notification center", "Group", "group"),
    ("Show a badge on its Dock item", "Badge", "badge"),
    ("Break through Do Not Disturb", "Breaks DND", "dnd"),
];
/// Badge and Break through Do Not Disturb, which `org.mate.Caminito.notifications` has no
/// default key for, so the Default entry can't change them.
const NOT_IN_DEFAULT: [usize; 2] = [4, 5];

#[derive(Clone, Copy, PartialEq)]
struct Prefs {
    delivery: Delivery,
    flags: [bool; 6],
}

struct App {
    id: String,
    name: String,
    icon: gio::Icon,
    own: Option<Prefs>,
}

/// The apps, the default at 0, whose `own` is always set.
type Model = Rc<RefCell<Vec<App>>>;
type OnChange = Rc<dyn Fn(usize)>;

fn effective(model: &Model, index: usize) -> Prefs {
    let apps = model.borrow();
    apps[index].own.or(apps[0].own).unwrap()
}

/// Whether flag `k` means anything under `delivery`. An open question, not the spec's.
fn applies(delivery: Delivery, k: usize) -> bool {
    match delivery {
        Delivery::Bubble => true,
        Delivery::History => k == 3 || k == 4,
        Delivery::Off => false,
    }
}

fn summary(prefs: Prefs) -> String {
    let mut parts = vec![prefs.delivery.label().to_owned()];
    for (k, (_, _, word)) in FLAGS.iter().enumerate() {
        let default = k != 5;
        if applies(prefs.delivery, k) && prefs.flags[k] != default {
            parts.push(if prefs.flags[k] {
                (*word).to_owned()
            } else {
                format!("no {word}")
            });
        }
    }
    parts.join(" · ")
}

fn apps() -> Vec<App> {
    let mut apps = vec![App {
        id: "default".into(),
        name: "Default".into(),
        icon: gio::ThemedIcon::new("preferences-system-notifications").upcast(),
        own: Some(Prefs {
            delivery: Delivery::Bubble,
            flags: [true, true, true, true, true, false],
        }),
    }];
    // Ones that "have sent a notification", then the installed ones that say they send them.
    let sent = [
        ("firefox", "Firefox", "firefox"),
        (
            "org.mozilla.Thunderbird",
            "Thunderbird",
            "org.mozilla.Thunderbird",
        ),
        ("org.signal.Signal", "Signal", "org.signal.Signal"),
        ("slack", "Slack", "slack"),
        ("code", "Visual Studio Code", "vscode"),
        (
            "caminito-accessx",
            "Accessibility keys",
            "preferences-desktop-accessibility",
        ),
        ("caminito-network", "Network", "network-wireless"),
    ];
    let installed = desktop_entries();
    let mut known: Vec<App> = sent
        .iter()
        .map(
            |(id, name, icon)| match installed.iter().find(|entry| entry.id == *id) {
                Some(entry) => App {
                    id: entry.id.clone(),
                    name: entry.name.clone(),
                    icon: entry.icon.clone(),
                    own: None,
                },
                None => App {
                    id: (*id).into(),
                    name: (*name).into(),
                    icon: gio::ThemedIcon::new(icon).upcast(),
                    own: None,
                },
            },
        )
        .collect();
    for entry in installed {
        if entry.uses_notifications && !known.iter().any(|app| app.id == entry.id) {
            known.push(App {
                id: entry.id,
                name: entry.name,
                icon: entry.icon,
                own: None,
            });
        }
    }
    known.sort_by_key(|app| app.name.to_lowercase());
    // Something to look at.
    if let Some(slack) = known.iter_mut().find(|app| app.id == "slack") {
        slack.own = Some(Prefs {
            delivery: Delivery::History,
            flags: [false, true, true, true, true, false],
        });
    }
    apps.extend(known);
    apps
}

struct Entry {
    id: String,
    name: String,
    icon: gio::Icon,
    uses_notifications: bool,
}

/// The installed desktop entries, read by hand: `DesktopAppInfo` is in gio-unix, which the
/// crate doesn't depend on.
fn desktop_entries() -> Vec<Entry> {
    let mut dirs = vec![glib::user_data_dir()];
    dirs.extend(glib::system_data_dirs());
    let mut entries: Vec<Entry> = Vec::new();
    for dir in dirs {
        let Ok(files) = std::fs::read_dir(dir.join("applications")) else {
            continue;
        };
        for file in files.flatten() {
            let path = file.path();
            let Some(id) = path
                .file_name()
                .and_then(|name| name.to_str())
                .and_then(|name| name.strip_suffix(".desktop"))
            else {
                continue;
            };
            if entries.iter().any(|entry| entry.id == id) {
                continue;
            }
            let keys = glib::KeyFile::new();
            if keys
                .load_from_file(&path, glib::KeyFileFlags::NONE)
                .is_err()
            {
                continue;
            }
            let group = "Desktop Entry";
            if keys.boolean(group, "NoDisplay").unwrap_or(false) {
                continue;
            }
            let Ok(name) = keys.locale_string(group, "Name", None) else {
                continue;
            };
            let icon = keys.string(group, "Icon").map_or_else(
                |_| gio::ThemedIcon::new("application-x-executable").upcast(),
                |icon| {
                    if icon.starts_with('/') {
                        gio::FileIcon::new(&gio::File::for_path(icon.as_str())).upcast()
                    } else {
                        gio::ThemedIcon::new(&icon).upcast()
                    }
                },
            );
            entries.push(Entry {
                id: id.to_owned(),
                name: name.into(),
                icon,
                uses_notifications: keys
                    .boolean(group, "X-GNOME-UsesNotifications")
                    .unwrap_or(false),
            });
        }
    }
    entries
}

fn state_text(model: &Model) -> String {
    let apps = model.borrow();
    let line = |id: &str, prefs: Prefs| {
        let flags: Vec<String> = FLAGS
            .iter()
            .enumerate()
            .map(|(k, (_, _, word))| {
                if prefs.flags[k] {
                    (*word).to_owned()
                } else {
                    format!("-{word}")
                }
            })
            .collect();
        format!("{id:<24} {:<8} {}", prefs.delivery.nick(), flags.join(" "))
    };
    let mut lines = Vec::new();
    let mut followers = Vec::new();
    for app in apps.iter() {
        match app.own {
            Some(prefs) => lines.push(line(&app.id, prefs)),
            None => followers.push(app.id.as_str()),
        }
    }
    lines.push(format!(
        "follow the default ({}): {}",
        followers.len(),
        followers.join(", ")
    ));
    lines.join("\n")
}

/// The six controls of one app, laid out by the variant.
struct Controls {
    index: usize,
    model: Model,
    delivery: gtk::DropDown,
    checks: Vec<gtk::CheckButton>,
    reset: gtk::Button,
    syncing: Cell<bool>,
}

impl Controls {
    fn new(model: &Model, index: usize, labelled: bool, on_change: &OnChange) -> Rc<Controls> {
        let labels: Vec<&str> = Delivery::ALL
            .iter()
            .map(|delivery| delivery.label())
            .collect();
        let delivery = gtk::DropDown::from_strings(&labels);
        let checks = FLAGS
            .iter()
            .map(|(long, _, _)| {
                let check = if labelled {
                    gtk::CheckButton::with_label(long)
                } else {
                    gtk::CheckButton::new()
                };
                check.set_tooltip_text(Some(long));
                check
            })
            .collect();
        let reset = if labelled {
            gtk::Button::with_label("Use the default")
        } else {
            let button = gtk::Button::from_icon_name("edit-undo-symbolic");
            button.set_tooltip_text(Some("Use the default"));
            button.add_css_class("flat");
            button
        };
        let this = Rc::new(Controls {
            index,
            model: model.clone(),
            delivery,
            checks,
            reset,
            syncing: Cell::new(false),
        });
        this.sync();

        this.delivery.connect_selected_notify({
            let this = this.clone();
            let on_change = on_change.clone();
            move |dropdown| {
                if this.syncing.get() {
                    return;
                }
                let mut prefs = effective(&this.model, this.index);
                prefs.delivery = Delivery::ALL[dropdown.selected() as usize];
                this.store(prefs);
                on_change(this.index);
            }
        });
        for (k, check) in this.checks.iter().enumerate() {
            check.connect_toggled({
                let this = this.clone();
                let on_change = on_change.clone();
                move |check| {
                    if this.syncing.get() {
                        return;
                    }
                    let mut prefs = effective(&this.model, this.index);
                    prefs.flags[k] = check.is_active();
                    this.store(prefs);
                    on_change(this.index);
                }
            });
        }
        this.reset.connect_clicked({
            let this = this.clone();
            let on_change = on_change.clone();
            move |_| {
                this.model.borrow_mut()[this.index].own = None;
                this.sync();
                on_change(this.index);
            }
        });
        this
    }

    fn store(&self, prefs: Prefs) {
        self.model.borrow_mut()[self.index].own = Some(prefs);
        self.sync();
    }

    /// Shows what the app gets, the default's settings for an app that follows it.
    fn sync(&self) {
        self.syncing.set(true);
        let prefs = effective(&self.model, self.index);
        let position = Delivery::ALL
            .iter()
            .position(|d| *d == prefs.delivery)
            .unwrap_or(0);
        self.delivery.set_selected(position as u32);
        for (k, check) in self.checks.iter().enumerate() {
            check.set_active(prefs.flags[k]);
            let fixed = self.index == 0 && NOT_IN_DEFAULT.contains(&k);
            check.set_sensitive(applies(prefs.delivery, k) && !fixed);
            if fixed {
                check.set_tooltip_text(Some(
                    "No default key for this in org.mate.Caminito.notifications",
                ));
            }
        }
        let own = self.model.borrow()[self.index].own.is_some();
        self.reset.set_visible(self.index != 0 && own);
        self.syncing.set(false);
    }

    /// The controls one under the other, for A and C.
    fn column(&self) -> gtk::Box {
        let column = gtk::Box::new(gtk::Orientation::Vertical, 6);
        let delivery = gtk::Box::new(gtk::Orientation::Horizontal, 12);
        delivery.append(&gtk::Label::new(Some("Delivery")));
        delivery.append(&self.delivery);
        column.append(&delivery);
        for check in &self.checks {
            column.append(check);
        }
        self.reset.set_halign(gtk::Align::Start);
        column.append(&self.reset);
        column
    }
}

fn icon(app: &App, size: i32) -> gtk::Image {
    let image = gtk::Image::from_gicon(&app.icon);
    image.set_pixel_size(size);
    image
}

fn dim(text: &str) -> gtk::Label {
    let label = gtk::Label::builder().label(text).xalign(0.0).build();
    label.add_css_class("dim-label");
    label
}

fn search() -> gtk::SearchEntry {
    let entry = gtk::SearchEntry::new();
    entry.set_placeholder_text(Some("Search apps"));
    entry
}

fn matches(model: &Model, index: usize, query: &str) -> bool {
    index == 0
        || model.borrow()[index]
            .name
            .to_lowercase()
            .contains(&query.to_lowercase())
}

fn status(model: &Model, index: usize) -> String {
    if index == 0 {
        "What an app without its own settings gets. The General page has the same five.".into()
    } else if model.borrow()[index].own.is_some() {
        "Its own settings".into()
    } else {
        "Follows the default".into()
    }
}

fn scrolled(child: &impl IsA<gtk::Widget>) -> gtk::ScrolledWindow {
    gtk::ScrolledWindow::builder()
        .child(child)
        .hscrollbar_policy(gtk::PolicyType::Never)
        .vexpand(true)
        .build()
}

/// A: the apps on the left, the chosen one's settings on the right.
fn sidebar(model: &Model, on_change: &OnChange) -> gtk::Widget {
    let root = gtk::Box::new(gtk::Orientation::Horizontal, SPACING);
    let left = gtk::Box::new(gtk::Orientation::Vertical, 6);
    let entry = search();
    left.append(&entry);
    let list = gtk::ListBox::new();
    let mut marks = Vec::new();
    for (index, app) in model.borrow().iter().enumerate() {
        let row = gtk::Box::new(gtk::Orientation::Horizontal, 8);
        row.set_margin_top(4);
        row.set_margin_bottom(4);
        row.set_margin_start(6);
        row.set_margin_end(6);
        row.append(&icon(app, 24));
        let name = gtk::Label::builder()
            .label(&app.name)
            .xalign(0.0)
            .hexpand(true)
            .build();
        if index == 0 {
            name.set_markup("<b>Default</b>");
        }
        row.append(&name);
        let mark = dim("Own");
        mark.set_visible(index != 0 && app.own.is_some());
        row.append(&mark);
        marks.push(mark);
        list.append(&row);
    }
    let left_scroll = scrolled(&list);
    left_scroll.set_min_content_width(240);
    left_scroll.add_css_class("frame");
    left.append(&left_scroll);
    root.append(&left);

    let right = gtk::Box::new(gtk::Orientation::Vertical, 12);
    right.set_hexpand(true);
    root.append(&right);
    let status_label: Rc<RefCell<gtk::Label>> = Rc::new(RefCell::new(dim("")));
    let on_change: OnChange = Rc::new({
        let model = model.clone();
        let status_label = status_label.clone();
        let on_change = on_change.clone();
        move |index| {
            marks[index].set_visible(index != 0 && model.borrow()[index].own.is_some());
            status_label.borrow().set_text(&status(&model, index));
            on_change(index);
        }
    });
    let current: Rc<RefCell<Option<Rc<Controls>>>> = Rc::default();
    let show = {
        let model = model.clone();
        move |index: usize| {
            while let Some(child) = right.first_child() {
                right.remove(&child);
            }
            let header = gtk::Box::new(gtk::Orientation::Horizontal, 12);
            header.append(&icon(&model.borrow()[index], 48));
            let titles = gtk::Box::new(gtk::Orientation::Vertical, 2);
            let title = gtk::Label::builder().xalign(0.0).build();
            title.set_markup(&format!(
                "<big><b>{}</b></big>",
                glib::markup_escape_text(&model.borrow()[index].name)
            ));
            titles.append(&title);
            let label = dim(&status(&model, index));
            label.set_wrap(true);
            titles.append(&label);
            *status_label.borrow_mut() = label;
            header.append(&titles);
            right.append(&header);
            let controls = Controls::new(&model, index, true, &on_change);
            right.append(&controls.column());
            *current.borrow_mut() = Some(controls);
        }
    };
    let query: Rc<RefCell<String>> = Rc::default();
    list.set_filter_func({
        let model = model.clone();
        let query = query.clone();
        move |row| matches(&model, row.index() as usize, &query.borrow())
    });
    entry.connect_search_changed({
        let list = list.clone();
        move |entry| {
            *query.borrow_mut() = entry.text().into();
            list.invalidate_filter();
        }
    });
    list.connect_row_selected(move |_, row| {
        if let Some(row) = row {
            show(row.index() as usize);
        }
    });
    list.select_row(list.row_at_index(0).as_ref());
    root.upcast()
}

/// B: one row per app, one column per setting.
fn table(model: &Model, on_change: &OnChange) -> gtk::Widget {
    let root = gtk::Box::new(gtk::Orientation::Vertical, 6);
    let entry = search();
    root.append(&entry);
    let grid = gtk::Grid::builder()
        .row_spacing(4)
        .column_spacing(10)
        .build();
    let titles = ["App", "Delivery"]
        .into_iter()
        .chain(FLAGS.iter().map(|(_, title, _)| *title));
    for (column, title) in titles.enumerate() {
        let label = gtk::Label::builder().xalign(0.0).build();
        label.set_markup(&format!("<b>{title}</b>"));
        if column > 1 {
            label.set_halign(gtk::Align::Center);
        }
        grid.attach(&label, column as i32, 0, 1, 1);
    }
    let all: Rc<RefCell<Vec<Rc<Controls>>>> = Rc::default();
    let on_change: OnChange = Rc::new({
        let all = all.clone();
        let on_change = on_change.clone();
        move |index| {
            // The default's change shows in every app that follows it.
            if index == 0 {
                for controls in all.borrow().iter() {
                    controls.sync();
                }
            }
            on_change(index);
        }
    });
    let mut rows: Vec<Vec<gtk::Widget>> = Vec::new();
    let count = model.borrow().len();
    for index in 0..count {
        let line = index as i32 + 1;
        let app_box = gtk::Box::new(gtk::Orientation::Horizontal, 8);
        app_box.append(&icon(&model.borrow()[index], 20));
        let name = gtk::Label::builder()
            .label(&model.borrow()[index].name)
            .xalign(0.0)
            .ellipsize(gtk::pango::EllipsizeMode::End)
            .max_width_chars(22)
            .build();
        if index == 0 {
            name.set_markup("<b>Default</b>");
        }
        app_box.append(&name);
        grid.attach(&app_box, 0, line, 1, 1);
        let controls = Controls::new(model, index, false, &on_change);
        grid.attach(&controls.delivery, 1, line, 1, 1);
        let mut widgets: Vec<gtk::Widget> =
            vec![app_box.upcast(), controls.delivery.clone().upcast()];
        for (k, check) in controls.checks.iter().enumerate() {
            check.set_halign(gtk::Align::Center);
            grid.attach(check, k as i32 + 2, line, 1, 1);
            widgets.push(check.clone().upcast());
        }
        grid.attach(&controls.reset, 8, line, 1, 1);
        widgets.push(controls.reset.clone().upcast());
        rows.push(widgets);
        all.borrow_mut().push(controls);
    }
    entry.connect_search_changed({
        let model = model.clone();
        let all = all.clone();
        move |entry| {
            for (index, widgets) in rows.iter().enumerate() {
                let shown = matches(&model, index, &entry.text());
                for widget in widgets {
                    widget.set_visible(shown);
                }
                if shown {
                    // The reset button hides itself for an app that follows the default.
                    all.borrow()[index].sync();
                }
            }
        }
    });
    root.append(&scrolled(&grid));
    root.upcast()
}

/// C: one line per app with what it gets, opening onto its settings.
fn expanders(model: &Model, on_change: &OnChange) -> gtk::Widget {
    let root = gtk::Box::new(gtk::Orientation::Vertical, 6);
    let entry = search();
    root.append(&entry);
    let list = gtk::Box::new(gtk::Orientation::Vertical, 2);
    let summaries: Rc<RefCell<Vec<gtk::Label>>> = Rc::default();
    let text = {
        let model = model.clone();
        move |index: usize| {
            let prefs = effective(&model, index);
            if index != 0 && model.borrow()[index].own.is_none() {
                format!("Default: {}", summary(prefs))
            } else {
                summary(prefs)
            }
        }
    };
    let text = Rc::new(text);
    let on_change: OnChange = Rc::new({
        let summaries = summaries.clone();
        let text = text.clone();
        let on_change = on_change.clone();
        move |index| {
            for (other, label) in summaries.borrow().iter().enumerate() {
                if other == index || index == 0 {
                    label.set_text(&text(other));
                }
            }
            on_change(index);
        }
    });
    let mut items = Vec::new();
    let count = model.borrow().len();
    for index in 0..count {
        let head = gtk::Box::new(gtk::Orientation::Horizontal, 8);
        head.append(&icon(&model.borrow()[index], 24));
        let name = gtk::Label::builder()
            .label(&model.borrow()[index].name)
            .xalign(0.0)
            .build();
        if index == 0 {
            name.set_markup("<b>Default</b>");
        }
        head.append(&name);
        let line = dim(&text(index));
        line.set_hexpand(true);
        line.set_xalign(1.0);
        head.append(&line);
        summaries.borrow_mut().push(line);
        let controls = Controls::new(model, index, true, &on_change);
        let body = controls.column();
        body.set_margin_start(32);
        body.set_margin_top(6);
        body.set_margin_bottom(10);
        let expander = gtk::Expander::builder()
            .label_widget(&head)
            .child(&body)
            .build();
        expander.set_margin_top(4);
        expander.set_margin_bottom(4);
        list.append(&expander);
        list.append(&gtk::Separator::new(gtk::Orientation::Horizontal));
        items.push(expander);
    }
    entry.connect_search_changed({
        let model = model.clone();
        move |entry| {
            for (index, expander) in items.iter().enumerate() {
                let shown = matches(&model, index, &entry.text());
                expander.set_visible(shown);
                if let Some(separator) = expander.next_sibling() {
                    separator.set_visible(shown);
                }
            }
        }
    });
    root.append(&scrolled(&list));
    root.upcast()
}

fn build(application: &gtk::Application, start: usize) {
    let model: Model = Rc::new(RefCell::new(apps()));
    let window = gtk::ApplicationWindow::builder()
        .application(application)
        .icon_name("preferences-system-notifications")
        .default_width(820)
        .default_height(600)
        .build();
    let page = gtk::Box::new(gtk::Orientation::Vertical, 0);
    let pages = gtk::Notebook::new();
    pages.set_vexpand(true);
    pages.append_page(
        &dim("Placeholder: the real General page is CMN-261's."),
        Some(&gtk::Label::new(Some("General"))),
    );
    let host = gtk::Box::new(gtk::Orientation::Vertical, 0);
    host.set_margin_top(SPACING);
    host.set_margin_bottom(SPACING);
    host.set_margin_start(SPACING);
    host.set_margin_end(SPACING);
    pages.append_page(&host, Some(&gtk::Label::new(Some("Apps"))));
    page.append(&pages);

    let state = gtk::Label::builder()
        .xalign(0.0)
        .selectable(true)
        .margin_start(12)
        .margin_end(12)
        .margin_top(6)
        .margin_bottom(6)
        .build();
    state.add_css_class("monospace");
    state.set_text(&state_text(&model));
    page.append(&state);

    let bar = gtk::Box::new(gtk::Orientation::Horizontal, 6);
    bar.set_halign(gtk::Align::Center);
    bar.set_margin_bottom(8);
    bar.add_css_class("osd");
    bar.add_css_class("toolbar");
    let previous = gtk::Button::from_icon_name("go-previous-symbolic");
    let label = gtk::Label::new(None);
    let next = gtk::Button::from_icon_name("go-next-symbolic");
    bar.append(&previous);
    bar.append(&label);
    bar.append(&next);
    page.append(&bar);
    window.set_child(Some(&page));

    let current = Rc::new(Cell::new(start));
    let show: Rc<dyn Fn(usize)> = Rc::new({
        let window = window.clone();
        let current = current.clone();
        move |variant: usize| {
            current.set(variant);
            while let Some(child) = host.first_child() {
                host.remove(&child);
            }
            let on_change: OnChange = Rc::new({
                let model = model.clone();
                let state = state.clone();
                move |_| state.set_text(&state_text(&model))
            });
            let content = match variant {
                0 => sidebar(&model, &on_change),
                1 => table(&model, &on_change),
                _ => expanders(&model, &on_change),
            };
            host.append(&content);
            state.set_text(&state_text(&model));
            let (key, name) = VARIANTS[variant];
            label.set_markup(&format!("<b>PROTOTYPE</b>  {key} ({name})"));
            window.set_title(Some(&format!("Notifications — prototype {key} ({name})")));
        }
    });
    let step = {
        let show = show.clone();
        move |by: usize| show((current.get() + by) % VARIANTS.len())
    };
    let step = Rc::new(step);
    previous.connect_clicked({
        let step = step.clone();
        move |_| step(VARIANTS.len() - 1)
    });
    next.connect_clicked({
        let step = step.clone();
        move |_| step(1)
    });
    let keys = gtk::EventControllerKey::new();
    keys.set_propagation_phase(gtk::PropagationPhase::Capture);
    keys.connect_key_pressed({
        let window = window.downgrade();
        move |_, key, _, _| {
            let typing = window
                .upgrade()
                .and_then(|window| gtk::prelude::RootExt::focus(&window))
                .is_some_and(|focus| focus.is::<gtk::Editable>());
            match key {
                gdk::Key::Left if !typing => step(VARIANTS.len() - 1),
                gdk::Key::Right if !typing => step(1),
                _ => return glib::Propagation::Proceed,
            }
            glib::Propagation::Stop
        }
    });
    window.add_controller(keys);
    show(start);
    pages.set_current_page(Some(1));
    window.present();
}

fn main() -> glib::ExitCode {
    let start = std::env::args()
        .nth(1)
        .and_then(|arg| {
            VARIANTS
                .iter()
                .position(|(key, _)| key.eq_ignore_ascii_case(arg.trim_start_matches("--variant=")))
        })
        .unwrap_or(1);
    let application = gtk::Application::builder()
        .application_id("org.mate.Caminito.Notifications.AppsPagePrototype")
        .build();
    application.connect_activate(move |application| build(application, start));
    application.run_with_args::<&str>(&[])
}
