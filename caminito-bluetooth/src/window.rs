//! The Bluetooth settings window (257): a list beside a detail pane, the prototype's
//! "Window C". The list has a header with Bluetooth's switch, This computer pinned at the
//! top, then the paired devices, and a **+** button for Add New Device (258). The pane shows
//! the selection: This computer, a device, or Add New Device, which looks for devices and
//! pairs the one that is clicked.
//!
//! The window draws [`Settings`] and calls it, and Bluetooth's [`Switch`]. It keeps nothing
//! of BlueZ's itself: each change is drawn again from the state, in place, so that a name
//! being typed or the list's focus survives a battery's change.

use std::cell::{Cell, RefCell};
use std::rc::Rc;

use caminito_bluetooth::DOMAIN;
use caminito_bluetooth::bluez::{Adapter, Bluez, Device};
use caminito_bluetooth::settings::{self, Attempt, LOW_BATTERY, PairError, Selection, Settings};
use caminito_bluetooth::switch::Switch;
use gtk::prelude::*;
use gtk::{gdk, gio, glib};

const CSS: &str = "
levelbar.caminito-bluetooth-low > trough > block.filled { background-color: @error_color; }
label.caminito-bluetooth-failed { color: mix(@error_color, @theme_fg_color, 0.45); }
";

/// What the window works on, which the program made at its start.
pub(crate) struct Services {
    pub(crate) bluez: Bluez,
    pub(crate) switch: Rc<Switch>,
}

/// Sets up what every window shares, once, at the program's start: the window's CSS and
/// Ctrl+W.
pub(crate) fn set_up(application: &gtk::Application) {
    application.set_accels_for_action("window.close", &["<Control>w"]);
    if let Some(display) = gdk::Display::default() {
        let provider = gtk::CssProvider::new();
        provider.load_from_string(CSS);
        gtk::style_context_add_provider_for_display(
            &display,
            &provider,
            gtk::STYLE_PROVIDER_PRIORITY_APPLICATION,
        );
    }
}

/// Opens the window, or raises the one there is. `services` is None when BlueZ can't be
/// reached at all. `scan` shows Add New Device, in the window there is or in a new one.
pub(crate) fn present(application: &gtk::Application, services: Option<&Services>, scan: bool) {
    // Any of its windows, not only the active one, which a hidden window isn't.
    if let Some(window) = application.windows().first() {
        if scan {
            show_add_device(window);
        }
        window.present();
        return;
    }
    let window = gtk::ApplicationWindow::builder()
        .application(application)
        .title("Bluetooth")
        .default_width(720)
        .default_height(480)
        .build();
    match services {
        Some(services) => {
            View::build(&window, services);
            if scan {
                show_add_device(&window);
            }
        }
        None => {
            let unreachable = dim("Bluetooth can't be reached: BlueZ isn't on the system bus.");
            unreachable.set_halign(gtk::Align::Center);
            window.set_child(Some(&unreachable));
        }
    }
    window.present();
}

/// Shows Add New Device in `window`, through its action, since the window's state is its own.
/// A window with no state, as when BlueZ can't be reached, has no such action and shows
/// nothing.
fn show_add_device(window: &impl IsA<gtk::Window>) {
    if let Some(window) = window.dynamic_cast_ref::<gtk::ApplicationWindow>() {
        gio::prelude::ActionGroupExt::activate_action(window, ADD_DEVICE, None);
    }
}

/// The window's action that shows Add New Device.
const ADD_DEVICE: &str = "add-device";

/// The window's widgets over the state.
struct View {
    window: gtk::ApplicationWindow,
    settings: Settings,
    switch: Rc<Switch>,
    /// Set while the view sets its widgets from the state, so that their signals aren't
    /// taken for the user's.
    syncing: Cell<bool>,
    /// Bluetooth's switch in the list's header.
    header_switch: gtk::Switch,
    /// Bluetooth's switch in This computer.
    computer_switch: gtk::Switch,
    /// Whether rfkill's block couldn't be read, warned of once until it can again.
    unreadable: Cell<bool>,
    list: gtk::ListBox,
    rows: RefCell<Vec<Row>>,
    /// The **+** button, which is down while Add New Device is shown.
    add_button: gtk::ToggleButton,
    pane: gtk::Stack,
    computer: ComputerPane,
    device: DevicePane,
    add: AddPane,
    /// The last action's failure, under the pane.
    message: gtk::Label,
}

/// What one of the list's rows shows.
struct Entry<'a> {
    selection: Selection,
    icon: &'a str,
    name: String,
    /// The line under the name, which is hidden when empty.
    status: String,
}

/// One of the list's rows.
struct Row {
    selection: Selection,
    row: gtk::ListBoxRow,
    icon: gtk::Image,
    name: gtk::Label,
    status: gtk::Label,
}

/// This computer's pane.
struct ComputerPane {
    note: gtk::Label,
    name: NameEntry,
    chooser_row: gtk::Box,
    chooser: gtk::DropDown,
    /// The chooser's adapters, in its order.
    chosen_from: RefCell<Vec<String>>,
    address: gtk::Label,
}

/// Add New Device's pane.
struct AddPane {
    spinner: gtk::Spinner,
    note: gtk::Label,
    /// What to do when nothing has been found, which is hidden when something has.
    hint: gtk::Label,
    scroll: gtk::ScrolledWindow,
    list: gtk::ListBox,
    rows: RefCell<Vec<NewRow>>,
}

/// One of Add New Device's rows: a device found.
struct NewRow {
    path: String,
    row: gtk::ListBoxRow,
    icon: gtk::Image,
    name: gtk::Label,
    /// Its type, or that it's being paired, or why it wasn't.
    status: gtk::Label,
}

/// A device's pane.
struct DevicePane {
    icon: gtk::Image,
    title: gtk::Label,
    status: gtk::Label,
    battery_row: gtk::Box,
    battery: gtk::LevelBar,
    percentage: gtk::Label,
    details: gtk::Label,
    name: NameEntry,
    connect: gtk::Button,
    forget: gtk::Button,
    /// The device shown, its path and name.
    shown: RefCell<Option<Device>>,
    /// Whether a connection or disconnection hasn't been answered yet.
    busy: Cell<bool>,
}

/// A name field, which renames the adapter or device at `path` when it's activated or left.
struct NameEntry {
    entry: gtk::Entry,
    path: RefCell<String>,
    /// The name BlueZ last gave, which the field shows unless the user is changing it.
    alias: RefCell<String>,
}

impl View {
    fn build(window: &gtk::ApplicationWindow, services: &Services) {
        let settings = Settings::new(&services.bluez);
        let list = gtk::ListBox::builder()
            .selection_mode(gtk::SelectionMode::Single)
            .vexpand(true)
            .build();
        let pane = gtk::Stack::new();
        let computer = ComputerPane::build();
        let device = DevicePane::build();
        let add = AddPane::build();
        let computer_switch = switch();
        pane.add_named(&computer.page(&computer_switch), Some("computer"));
        pane.add_named(&device.page(), Some("device"));
        pane.add_named(&add.page(), Some("add"));
        let message = gtk::Label::builder()
            .xalign(0.0)
            .wrap(true)
            .css_classes(["error"])
            .build();
        let view = Rc::new(View {
            window: window.clone(),
            settings: settings.clone(),
            switch: services.switch.clone(),
            syncing: Cell::new(false),
            header_switch: switch(),
            computer_switch,
            unreadable: Cell::new(false),
            list,
            rows: RefCell::default(),
            add_button: gtk::ToggleButton::builder()
                .icon_name("list-add-symbolic")
                .tooltip_text("Add New Device")
                .halign(gtk::Align::Start)
                .css_classes(["flat"])
                .build(),
            pane,
            computer,
            device,
            add,
            message,
        });
        window.set_child(Some(&view.layout()));
        view.connect_signals();

        let weak = Rc::downgrade(&view);
        settings.connect_changed(move |_| {
            if let Some(view) = weak.upgrade() {
                view.update();
            }
        });
        // The window's handler holds the view, which lives as long as the window does.
        let holder = view.clone();
        window.connect_close_request(move |_| {
            let settings = holder.settings.clone();
            glib::spawn_future_local(async move {
                if let Err(err) = settings.close().await {
                    glib::g_warning!(DOMAIN, "{err}");
                }
            });
            glib::Propagation::Proceed
        });
        view.settings.open();
        view.update();
    }

    fn layout(&self) -> gtk::Box {
        let header = gtk::Box::new(gtk::Orientation::Horizontal, 8);
        let title = gtk::Label::builder()
            .label("Bluetooth")
            .xalign(0.0)
            .hexpand(true)
            .css_classes(["heading"])
            .build();
        header.append(&title);
        header.append(&self.header_switch);
        margins(&header, 8);

        let add = &self.add_button;
        margins(add, 4);

        let left = gtk::Box::new(gtk::Orientation::Vertical, 0);
        left.set_size_request(240, -1);
        // Not the header's title's, which would widen the list with the window.
        left.set_hexpand(false);
        left.append(&header);
        left.append(&gtk::Separator::new(gtk::Orientation::Horizontal));
        left.append(
            &gtk::ScrolledWindow::builder()
                .child(&self.list)
                .hscrollbar_policy(gtk::PolicyType::Never)
                .vexpand(true)
                .build(),
        );
        left.append(&gtk::Separator::new(gtk::Orientation::Horizontal));
        left.append(add);

        let right = gtk::Box::new(gtk::Orientation::Vertical, 12);
        right.set_hexpand(true);
        margins(&right, 24);
        right.append(&self.pane);
        right.append(&self.message);

        let both = gtk::Box::new(gtk::Orientation::Horizontal, 0);
        both.append(&left);
        both.append(&gtk::Separator::new(gtk::Orientation::Vertical));
        both.append(&right);
        both
    }

    fn connect_signals(self: &Rc<Self>) {
        for switch in [&self.header_switch, &self.computer_switch] {
            let weak = Rc::downgrade(self);
            switch.connect_state_set(move |_, on| {
                let Some(view) = weak.upgrade() else {
                    return glib::Propagation::Proceed;
                };
                if view.syncing.get() {
                    return glib::Propagation::Proceed;
                }
                view.message.set_label("");
                glib::spawn_future_local(async move {
                    if let Err(err) = view.switch.set_on(on).await {
                        let what = if on { "on" } else { "off" };
                        view.fail(&err, &format!("Bluetooth can't be turned {what}."));
                    }
                    view.update();
                });
                // The switch shows the new state once it's set, or the old if it can't be.
                glib::Propagation::Stop
            });
        }

        let weak = Rc::downgrade(self);
        self.list.connect_row_selected(move |_, row| {
            let (Some(view), Some(row)) = (weak.upgrade(), row) else {
                return;
            };
            if view.syncing.get() {
                return;
            }
            let selection = view
                .rows
                .borrow()
                .iter()
                .find(|known| &known.row == row)
                .map(|known| known.selection.clone());
            if let Some(selection) = selection {
                view.message.set_label("");
                view.rename_from(&view.device.name);
                view.settings.select(selection);
            }
        });

        let weak = Rc::downgrade(self);
        self.computer
            .chooser
            .connect_selected_notify(move |chooser| {
                let Some(view) = weak.upgrade() else {
                    return;
                };
                if view.syncing.get() {
                    return;
                }
                let path = view
                    .computer
                    .chosen_from
                    .borrow()
                    .get(chooser.selected() as usize)
                    .cloned();
                if let Some(path) = path {
                    view.rename_from(&view.computer.name);
                    view.settings.choose(&path);
                }
            });

        NameEntry::connect(self, |view| &view.computer.name);
        NameEntry::connect(self, |view| &view.device.name);

        // The + button and --scan.
        let action = gio::SimpleAction::new(ADD_DEVICE, None);
        let weak = Rc::downgrade(self);
        action.connect_activate(move |_, _| {
            if let Some(view) = weak.upgrade() {
                view.show_add_device();
            }
        });
        self.window.add_action(&action);
        let weak = Rc::downgrade(self);
        self.add_button.connect_toggled(move |_| {
            if let Some(view) = weak.upgrade()
                && !view.syncing.get()
            {
                view.show_add_device();
            }
        });
        let weak = Rc::downgrade(self);
        self.add.list.connect_row_activated(move |_, row| {
            let path = weak.upgrade().and_then(|view| {
                let path = view
                    .add
                    .rows
                    .borrow()
                    .iter()
                    .find(|known| &known.row == row)
                    .map(|known| known.path.clone());
                path.map(|path| (view, path))
            });
            if let Some((view, path)) = path {
                view.pair(&path);
            }
        });

        let weak = Rc::downgrade(self);
        self.device.connect.connect_clicked(move |_| {
            if let Some(view) = weak.upgrade() {
                view.connect_or_disconnect();
            }
        });
        let weak = Rc::downgrade(self);
        self.device.forget.connect_clicked(move |_| {
            if let Some(view) = weak.upgrade() {
                view.forget();
            }
        });
    }

    /// Draws the state again.
    fn update(&self) {
        self.syncing.set(true);
        let on = match self.switch.is_on() {
            Ok(on) => {
                self.unreadable.set(false);
                on
            }
            Err(err) => {
                if !self.unreadable.replace(true) {
                    glib::g_warning!(DOMAIN, "{err}");
                }
                true
            }
        };
        for switch in [&self.header_switch, &self.computer_switch] {
            switch.set_active(on);
            switch.set_state(on);
        }
        let adapter = self.settings.adapter();
        let devices = self.settings.devices();
        self.update_list(adapter.as_ref(), &devices);
        self.computer.update(&self.settings, adapter.as_ref(), on);

        let selection = self.settings.selection();
        self.add_button
            .set_active(selection == Selection::AddDevice);
        let shown = match &selection {
            Selection::Device(path) => devices.into_iter().find(|device| &device.path == path),
            Selection::Computer | Selection::AddDevice => None,
        };
        match (selection, shown) {
            (Selection::AddDevice, _) => {
                self.add.update(&self.settings, adapter.as_ref(), on);
                self.pane.set_visible_child_name("add");
            }
            (_, Some(device)) => {
                self.device.update(&device, on);
                self.pane.set_visible_child_name("device");
            }
            (_, None) => self.pane.set_visible_child_name("computer"),
        }
        self.syncing.set(false);
    }

    fn update_list(&self, adapter: Option<&Adapter>, devices: &[Device]) {
        let mut wanted = vec![Entry {
            selection: Selection::Computer,
            icon: "computer",
            name: "This computer".to_owned(),
            status: adapter
                .map(|adapter| adapter.alias.clone())
                .unwrap_or_default(),
        }];
        wanted.extend(devices.iter().map(|device| Entry {
            selection: Selection::Device(device.path.clone()),
            icon: icon_name(device),
            name: device.alias.clone(),
            status: if device.connected { "Connected" } else { "" }.to_owned(),
        }));

        let mut rows = self.rows.borrow_mut();
        let same = rows.len() == wanted.len()
            && rows
                .iter()
                .zip(&wanted)
                .all(|(row, entry)| row.selection == entry.selection);
        if !same {
            self.list.remove_all();
            *rows = wanted
                .iter()
                .map(|entry| {
                    let row = Row::build(entry.selection.clone());
                    self.list.append(&row.row);
                    row
                })
                .collect();
        }
        for (row, entry) in rows.iter().zip(&wanted) {
            row.icon.set_icon_name(Some(entry.icon));
            row.name.set_label(&entry.name);
            row.status.set_label(&entry.status);
            row.status.set_visible(!entry.status.is_empty());
        }
        let selection = self.settings.selection();
        let selected = rows.iter().find(|row| row.selection == selection);
        self.list.select_row(selected.map(|row| &row.row));
    }

    /// Shows Add New Device, which looks for devices while it's shown.
    fn show_add_device(self: &Rc<Self>) {
        self.message.set_label("");
        self.rename_from(&self.device.name);
        self.rename_from(&self.computer.name);
        // Told to the window even when it's shown already, so the button is down again.
        self.settings.select(Selection::AddDevice);
    }

    /// Pairs the device at `path`, which Add New Device lists. The agent's dialogs come up
    /// as BlueZ asks, and the row says that it's pairing, or why it didn't.
    fn pair(self: &Rc<Self>, path: &str) {
        let Some(device) = self
            .settings
            .new_devices()
            .into_iter()
            .find(|device| device.path == path)
        else {
            return;
        };
        self.message.set_label("");
        let view = self.clone();
        glib::spawn_future_local(async move {
            match view.settings.pair(&device.path).await {
                Ok(()) => {}
                // Another is being paired, which the list doesn't allow.
                Err(PairError::Busy) => {}
                // The row says why.
                Err(err @ PairError::Pair(_)) => glib::g_warning!(DOMAIN, "{err}"),
                // It's one of the devices now, where it can be connected again.
                Err(err @ PairError::Connect(_)) => view.fail(
                    &err,
                    &format!("{} is paired, but can't connect.", device.alias),
                ),
            }
        });
    }

    fn connect_or_disconnect(self: &Rc<Self>) {
        let Some(device) = self.device.shown.borrow().clone() else {
            return;
        };
        self.device.busy.set(true);
        self.message.set_label("");
        self.update();
        let view = self.clone();
        glib::spawn_future_local(async move {
            let (answer, what) = if device.connected {
                (view.settings.disconnect(&device.path).await, "disconnect")
            } else {
                (view.settings.connect(&device.path).await, "connect")
            };
            view.device.busy.set(false);
            if let Err(err) = answer {
                view.fail(&err, &format!("Can't {what} {}.", device.alias));
            }
            view.update();
        });
    }

    fn forget(self: &Rc<Self>) {
        let Some(device) = self.device.shown.borrow().clone() else {
            return;
        };
        self.message.set_label("");
        let view = self.clone();
        glib::spawn_future_local(async move {
            let confirmation = gtk::AlertDialog::builder()
                .message(format!("Forget {}?", device.alias))
                .detail("It has to be paired again before it can connect.")
                .buttons(["Cancel", "Forget"])
                .cancel_button(0)
                .default_button(0)
                .modal(true)
                .build();
            if confirmation.choose_future(Some(&view.window)).await != Ok(1) {
                return;
            }
            if let Err(err) = view.settings.forget(&device.path).await {
                view.fail(&err, &format!("Can't forget {}.", device.alias));
            }
        });
    }

    /// Renames what `name` is the field of, if the user changed its text.
    fn rename_from(self: &Rc<Self>, name: &NameEntry) {
        let text = name.entry.text().to_string();
        let path = name.path.borrow().clone();
        if path.is_empty() || text == *name.alias.borrow() {
            return;
        }
        // Taken as given, so that the field's leaving after a new selection doesn't ask
        // again.
        *name.alias.borrow_mut() = text.clone();
        self.message.set_label("");
        let view = self.clone();
        glib::spawn_future_local(async move {
            if let Err(err) = view.settings.rename(&path, &text).await {
                view.fail(&err, &format!("Can't rename it “{text}”."));
            }
        });
    }

    /// Warns of `err`, and tells the user `text` under the pane.
    fn fail(&self, err: &dyn std::error::Error, text: &str) {
        glib::g_warning!(DOMAIN, "{err}");
        self.message.set_label(text);
    }
}

impl Row {
    fn build(selection: Selection) -> Row {
        let icon = gtk::Image::builder().pixel_size(24).build();
        let name = gtk::Label::builder()
            .xalign(0.0)
            .ellipsize(gtk::pango::EllipsizeMode::End)
            .build();
        let status = dim("");
        let text = gtk::Box::new(gtk::Orientation::Vertical, 0);
        text.set_valign(gtk::Align::Center);
        text.append(&name);
        text.append(&status);
        let content = gtk::Box::new(gtk::Orientation::Horizontal, 8);
        content.append(&icon);
        content.append(&text);
        margins(&content, 6);
        let row = gtk::ListBoxRow::builder().child(&content).build();
        Row {
            selection,
            row,
            icon,
            name,
            status,
        }
    }
}

impl ComputerPane {
    fn build() -> ComputerPane {
        let chooser = gtk::DropDown::from_strings(&[]);
        chooser.set_hexpand(true);
        ComputerPane {
            note: dim(""),
            name: NameEntry::build(),
            chooser_row: gtk::Box::new(gtk::Orientation::Horizontal, 8),
            chooser,
            chosen_from: RefCell::default(),
            address: dim(""),
        }
    }

    /// The page, with `switch`, Bluetooth's, which the list's header has too.
    fn page(&self, switch: &gtk::Switch) -> gtk::Box {
        let page = gtk::Box::new(gtk::Orientation::Vertical, 12);
        page.append(&header("computer", "This computer", &self.note));
        page.append(&field("Bluetooth", switch));
        page.append(&field("Name", &self.name.entry));
        self.chooser_row.append(&caption("Adapter"));
        self.chooser_row.append(&self.chooser);
        page.append(&self.chooser_row);
        page.append(&self.address);
        page
    }

    fn update(&self, settings: &Settings, adapter: Option<&Adapter>, on: bool) {
        let note = match adapter {
            None => "No Bluetooth adapter.".to_owned(),
            Some(_) if !on => "Bluetooth is off.".to_owned(),
            Some(adapter) if adapter.discoverable => format!(
                "Visible to other devices as “{}” while this window is open.",
                adapter.alias
            ),
            Some(_) => "Not visible to other devices.".to_owned(),
        };
        self.note.set_label(&note);
        self.name.entry.set_sensitive(adapter.is_some());
        if let Some(adapter) = adapter {
            self.name.show(&adapter.path, &adapter.alias);
            self.address
                .set_label(&format!("Address {}", adapter.address));
        }
        self.address.set_visible(adapter.is_some());

        let chooser = settings.chooser().unwrap_or_default();
        self.chooser_row.set_visible(!chooser.is_empty());
        let paths: Vec<String> = chooser.iter().map(|known| known.path.clone()).collect();
        if *self.chosen_from.borrow() != paths {
            let names: Vec<String> = chooser
                .iter()
                .map(|known| format!("{} ({})", known.alias, adapter_name(&known.path)))
                .collect();
            let names: Vec<&str> = names.iter().map(String::as_str).collect();
            self.chooser.set_model(Some(&gtk::StringList::new(&names)));
            *self.chosen_from.borrow_mut() = paths;
        }
        if let Some(index) = adapter.and_then(|adapter| {
            self.chosen_from
                .borrow()
                .iter()
                .position(|path| path == &adapter.path)
        }) {
            self.chooser.set_selected(index as u32);
        }
    }
}

impl AddPane {
    fn build() -> AddPane {
        let list = gtk::ListBox::builder()
            .selection_mode(gtk::SelectionMode::None)
            .activate_on_single_click(true)
            .css_classes(["boxed-list"])
            .build();
        AddPane {
            spinner: gtk::Spinner::builder().valign(gtk::Align::Center).build(),
            note: dim(""),
            hint: dim("Nothing found yet. Put the device in pairing mode, close to this computer."),
            scroll: gtk::ScrolledWindow::builder()
                .child(&list)
                .hscrollbar_policy(gtk::PolicyType::Never)
                .vexpand(true)
                .build(),
            list,
            rows: RefCell::default(),
        }
    }

    fn page(&self) -> gtk::Box {
        let top = header("bluetooth", "Add New Device", &self.note);
        self.note.set_hexpand(true);
        top.append(&self.spinner);
        let page = gtk::Box::new(gtk::Orientation::Vertical, 12);
        page.append(&top);
        page.append(&self.hint);
        page.append(&self.scroll);
        page
    }

    /// Draws what is found, in place, so that a device's row survives the others' coming
    /// and going.
    fn update(&self, settings: &Settings, adapter: Option<&Adapter>, on: bool) {
        let ready = on && adapter.is_some_and(|adapter| adapter.powered);
        let note = match adapter {
            None => "No Bluetooth adapter.",
            Some(_) if !ready => "Bluetooth is off.",
            Some(adapter) if adapter.discovering => "Looking for devices…",
            Some(_) => "Not looking for devices.",
        };
        self.note.set_label(note);
        self.spinner
            .set_spinning(ready && adapter.is_some_and(|adapter| adapter.discovering));

        let devices = if ready {
            settings.new_devices()
        } else {
            Vec::new()
        };
        self.hint.set_visible(ready && devices.is_empty());
        self.scroll.set_visible(!devices.is_empty());
        // One device is paired at a time: the agent asks one question at a time.
        let pairing = devices
            .iter()
            .any(|device| settings.attempt(&device.path) == Some(Attempt::Pairing));
        self.list.set_sensitive(!pairing);

        let mut rows = self.rows.borrow_mut();
        let same = rows.len() == devices.len()
            && rows
                .iter()
                .zip(&devices)
                .all(|(row, device)| row.path == device.path);
        if !same {
            self.list.remove_all();
            *rows = devices
                .iter()
                .map(|device| {
                    let row = NewRow::build(&device.path);
                    self.list.append(&row.row);
                    row
                })
                .collect();
        }
        for (row, device) in rows.iter().zip(&devices) {
            row.icon.set_icon_name(Some(icon_name(device)));
            row.name.set_label(&device.alias);
            let (status, failed) = match settings.attempt(&device.path) {
                Some(Attempt::Pairing) => ("Pairing…".to_owned(), false),
                Some(Attempt::Failed(reason)) => (format!("Couldn't pair: {reason}"), true),
                None => (settings::kind(device).to_owned(), false),
            };
            row.status.set_label(&status);
            set_class(&row.status, "caminito-bluetooth-failed", failed);
            set_class(&row.status, "dim-label", !failed);
        }
    }
}

impl NewRow {
    fn build(path: &str) -> NewRow {
        let icon = gtk::Image::builder().pixel_size(24).build();
        let name = gtk::Label::builder()
            .xalign(0.0)
            .ellipsize(gtk::pango::EllipsizeMode::End)
            .build();
        let status = dim("");
        let text = gtk::Box::new(gtk::Orientation::Vertical, 0);
        text.set_valign(gtk::Align::Center);
        text.append(&name);
        text.append(&status);
        let content = gtk::Box::new(gtk::Orientation::Horizontal, 8);
        content.append(&icon);
        content.append(&text);
        margins(&content, 6);
        let row = gtk::ListBoxRow::builder().child(&content).build();
        NewRow {
            path: path.to_owned(),
            row,
            icon,
            name,
            status,
        }
    }
}

impl DevicePane {
    fn build() -> DevicePane {
        let battery = gtk::LevelBar::for_interval(0.0, 100.0);
        battery.set_hexpand(true);
        battery.set_valign(gtk::Align::Center);
        // The bar is red below LOW_BATTERY, and the theme's colour otherwise.
        for offset in ["low", "high", "full"] {
            battery.remove_offset_value(Some(offset));
        }
        let connect = gtk::Button::new();
        connect.add_css_class("suggested-action");
        let forget = gtk::Button::with_label("Forget…");
        forget.add_css_class("destructive-action");
        DevicePane {
            icon: gtk::Image::builder().pixel_size(64).build(),
            title: gtk::Label::builder()
                .xalign(0.0)
                .css_classes(["title-2"])
                .build(),
            status: dim(""),
            battery_row: gtk::Box::new(gtk::Orientation::Horizontal, 8),
            battery,
            percentage: gtk::Label::new(None),
            details: dim(""),
            name: NameEntry::build(),
            connect,
            forget,
            shown: RefCell::default(),
            busy: Cell::new(false),
        }
    }

    fn page(&self) -> gtk::Box {
        let text = gtk::Box::new(gtk::Orientation::Vertical, 4);
        text.set_valign(gtk::Align::Center);
        text.append(&self.title);
        text.append(&self.status);
        let top = gtk::Box::new(gtk::Orientation::Horizontal, 12);
        top.append(&self.icon);
        top.append(&text);

        self.battery_row.append(&caption("Battery"));
        self.battery_row.append(&self.battery);
        self.battery_row.append(&self.percentage);

        let buttons = gtk::Box::new(gtk::Orientation::Horizontal, 8);
        buttons.append(&self.connect);
        buttons.append(&self.forget);

        let page = gtk::Box::new(gtk::Orientation::Vertical, 12);
        page.append(&top);
        page.append(&self.battery_row);
        page.append(&self.details);
        page.append(&field("Name", &self.name.entry));
        page.append(&buttons);
        page
    }

    fn update(&self, device: &Device, on: bool) {
        self.icon.set_icon_name(Some(icon_name(device)));
        self.title.set_label(&device.alias);
        let status = match (self.busy.get(), device.connected) {
            (true, true) => "Disconnecting…",
            (true, false) => "Connecting…",
            (false, true) => "Connected",
            (false, false) => "Not connected",
        };
        self.status.set_label(status);

        self.battery_row.set_visible(device.battery.is_some());
        if let Some(battery) = device.battery {
            let low = battery < LOW_BATTERY;
            self.battery.set_value(f64::from(battery));
            set_class(&self.battery, "caminito-bluetooth-low", low);
            self.percentage.set_label(&format!("{battery}%"));
        }
        self.details
            .set_label(&format!("{} · {}", settings::kind(device), device.address));
        self.name.show(&device.path, &device.alias);
        self.connect.set_label(if device.connected {
            "Disconnect"
        } else {
            "Connect"
        });
        self.connect.set_sensitive(on && !self.busy.get());
        *self.shown.borrow_mut() = Some(device.clone());
    }
}

impl NameEntry {
    fn build() -> NameEntry {
        NameEntry {
            entry: gtk::Entry::builder().hexpand(true).build(),
            path: RefCell::default(),
            alias: RefCell::default(),
        }
    }

    /// Shows the name of the adapter or device at `path`. The text being typed is kept
    /// unless the name changed, or the field is another's now.
    fn show(&self, path: &str, alias: &str) {
        if *self.path.borrow() == path && *self.alias.borrow() == alias {
            return;
        }
        *self.path.borrow_mut() = path.to_owned();
        *self.alias.borrow_mut() = alias.to_owned();
        self.entry.set_text(alias);
    }

    /// Renames when the field of `view` that `of` gives is activated or left.
    fn connect(view: &Rc<View>, of: fn(&View) -> &NameEntry) {
        let weak = Rc::downgrade(view);
        let rename = Rc::new(move || {
            if let Some(view) = weak.upgrade() {
                view.rename_from(of(&view));
            }
        });
        let entry = &of(view).entry;
        entry.connect_activate({
            let rename = rename.clone();
            move |_| rename()
        });
        let focus = gtk::EventControllerFocus::new();
        focus.connect_leave(move |_| rename());
        entry.add_controller(focus);
    }
}

/// A pane's top: a large icon, its title and a line under it.
fn header(icon: &str, title: &str, note: &gtk::Label) -> gtk::Box {
    let image = gtk::Image::builder().icon_name(icon).pixel_size(64).build();
    let text = gtk::Box::new(gtk::Orientation::Vertical, 4);
    text.set_valign(gtk::Align::Center);
    text.append(
        &gtk::Label::builder()
            .label(title)
            .xalign(0.0)
            .css_classes(["title-2"])
            .build(),
    );
    text.append(note);
    let top = gtk::Box::new(gtk::Orientation::Horizontal, 12);
    top.append(&image);
    top.append(&text);
    top
}

/// A labelled row of a pane.
fn field(label: &str, widget: &impl IsA<gtk::Widget>) -> gtk::Box {
    let row = gtk::Box::new(gtk::Orientation::Horizontal, 8);
    row.append(&caption(label));
    row.append(widget);
    row
}

fn caption(label: &str) -> gtk::Label {
    gtk::Label::builder()
        .label(label)
        .xalign(0.0)
        .width_chars(8)
        .build()
}

fn dim(label: &str) -> gtk::Label {
    gtk::Label::builder()
        .label(label)
        .xalign(0.0)
        .wrap(true)
        .css_classes(["dim-label"])
        .build()
}

fn switch() -> gtk::Switch {
    gtk::Switch::builder().valign(gtk::Align::Center).build()
}

fn margins(widget: &impl IsA<gtk::Widget>, margin: i32) {
    widget.set_margin_top(margin);
    widget.set_margin_bottom(margin);
    widget.set_margin_start(margin);
    widget.set_margin_end(margin);
}

fn set_class(widget: &impl IsA<gtk::Widget>, class: &str, on: bool) {
    if on {
        widget.add_css_class(class);
    } else {
        widget.remove_css_class(class);
    }
}

/// A device's icon, BlueZ's for its class, or Bluetooth's when it gives none.
fn icon_name(device: &Device) -> &str {
    device.icon.as_deref().unwrap_or("bluetooth")
}

/// An adapter's kernel name, as `hci0`, from its path.
fn adapter_name(path: &str) -> &str {
    path.rsplit('/').next().unwrap_or(path)
}
