//! The Night Light tab (142, ADR 0023): the switch, the schedule, the temperatures it uses,
//! and the pause, over `org.mate.Caminito`'s `night-light-*` keys and Caminito's
//! `NightLight` property. The tab's state is the library's `night_light`; this is its widgets,
//! its keys through gio and its calls to Caminito.
//!
//! Each change is written as it's made, with no Apply. A change from elsewhere, Caminito's
//! own writes and `--toggle-night-light` among them, comes back through the keys.

use std::cell::{Cell, RefCell};
use std::rc::{Rc, Weak};

use caminito_displays::night_light::{
    self, ENABLED, MAX_TEMPERATURE, MIN_TEMPERATURE, PAUSED_UNTIL, Pause, SCHEDULE, STEP, Schedule,
    Settings, Tab, Temperature, Write,
};
use gtk::prelude::*;
use gtk::{gio, glib};

use crate::compositor::{self, NAME, PATH};

/// The schema of the keys.
const SCHEMA: &str = "org.mate.Caminito";
/// Caminito's property for the tab.
const NIGHT_LIGHT: &str = "NightLight";

/// Turns Night Light on or off through Caminito's `ToggleNightLight()`, for
/// `--toggle-night-light`. Caminito writes the key, so this works whichever schema source
/// the caller has.
pub(crate) fn toggle() -> glib::ExitCode {
    let toggled =
        gio::bus_get_sync(gio::BusType::Session, gio::Cancellable::NONE).and_then(|bus| {
            bus.call_sync(
                Some(NAME),
                PATH,
                NAME,
                "ToggleNightLight",
                None,
                None,
                gio::DBusCallFlags::NO_AUTO_START,
                compositor::NOW_TIMEOUT,
                gio::Cancellable::NONE,
            )
        });
    match toggled {
        Ok(_) => glib::ExitCode::SUCCESS,
        Err(err) => {
            glib::g_warning!("caminito-displays", "can't toggle Night Light: {err}");
            glib::ExitCode::FAILURE
        }
    }
}

/// Where the keys are, and who pauses.
enum Store {
    /// The keys, and Caminito on the session bus if it's there.
    Keys {
        settings: gio::Settings,
        caminito: Option<gio::DBusProxy>,
    },
    /// The demo's: the keys are the window's own.
    Demo,
    /// `org.mate.Caminito` isn't installed.
    NoSchema,
}

/// One temperature's row.
struct Slider {
    which: Temperature,
    name: gtk::Label,
    row: gtk::Box,
    scale: gtk::Scale,
    value: gtk::Label,
}

/// The tab's widgets and state.
pub(crate) struct NightLightTab {
    tab: RefCell<Tab>,
    store: Store,
    /// Set while the widgets are given the tab's values, so that their handlers don't take
    /// them for the user's.
    rendering: Cell<bool>,
    /// Shows the tab again when the pause on screen ends, in case Caminito's write of 0
    /// is late or doesn't come.
    pause_ends: RefCell<Option<glib::SourceId>>,
    /// The tab itself, for the timer of `pause_ends`.
    this: Weak<NightLightTab>,

    content: gtk::Box,
    form: gtk::Grid,
    enabled: gtk::Switch,
    sun: gtk::ToggleButton,
    constant: gtk::ToggleButton,
    sliders: Vec<Slider>,
    pause: gtk::Button,
    paused: gtk::Label,
    pause_row: gtk::Box,
    note: gtk::Label,
    message: gtk::Label,
}

/// Reads the keys.
fn read(settings: &gio::Settings) -> Settings {
    let schedule = Schedule::from_nick(&settings.string(SCHEDULE));
    Settings {
        enabled: settings.boolean(ENABLED),
        schedule: schedule.unwrap_or(Schedule::Sun),
        noon: settings.uint(Temperature::Noon.key()),
        night: settings.uint(Temperature::Night.key()),
        constant: settings.uint(Temperature::Constant.key()),
        paused_until: settings.int64(PAUSED_UNTIL),
    }
}

/// `NightLight`'s `location-source`, while Caminito is there to publish it.
fn location_source(caminito: &gio::DBusProxy) -> Option<String> {
    let property = caminito.cached_property(NIGHT_LIGHT)?;
    glib::VariantDict::new(Some(&property))
        .lookup::<String>("location-source")
        .ok()
        .flatten()
}

/// The hour and the minute of a Unix time, in the local time zone.
fn local(time: i64) -> (u32, u32) {
    glib::DateTime::from_unix_local(time)
        .map(|at| (at.hour() as u32, at.minute() as u32))
        .unwrap_or_default()
}

fn now() -> i64 {
    glib::real_time() / 1_000_000
}

/// The next 06:42 in the local time zone, the demo's sunrise.
fn demo_sunrise() -> i64 {
    let Ok(at) = glib::DateTime::now_local() else {
        return now() + 8 * 3600;
    };
    let today = glib::DateTime::from_local(at.year(), at.month(), at.day_of_month(), 6, 42, 0.0);
    let today = today.map(|today| today.to_unix()).unwrap_or_default();
    if today > now() { today } else { today + 86_400 }
}

/// A line of quieter text.
fn note() -> gtk::Label {
    let note = gtk::Label::builder().xalign(0.0).build();
    note.add_css_class("dim-label");
    note
}

fn row(children: &[&gtk::Widget]) -> gtk::Box {
    let row = gtk::Box::new(gtk::Orientation::Horizontal, 10);
    for child in children {
        row.append(*child);
    }
    row
}

impl NightLightTab {
    /// The tab over the keys and `caminito`, or over made-up ones for `demo`.
    pub(crate) fn new(demo: bool, caminito: Option<gio::DBusProxy>) -> Rc<Self> {
        let installed = gio::SettingsSchemaSource::default()
            .and_then(|source| source.lookup(SCHEMA, true))
            .is_some();
        let (store, tab) = if demo {
            let settings = Settings {
                enabled: true,
                paused_until: demo_sunrise(),
                ..Settings::default()
            };
            (Store::Demo, Tab::new(settings, Some("none".to_owned())))
        } else if installed {
            let settings = gio::Settings::new(SCHEMA);
            let source = caminito.as_ref().and_then(location_source);
            let tab = Tab::new(read(&settings), source);
            (Store::Keys { settings, caminito }, tab)
        } else {
            (Store::NoSchema, Tab::new(Settings::default(), None))
        };
        let ui = Rc::new_cyclic(|this| NightLightTab {
            this: this.clone(),
            ..Self::build(store, tab)
        });
        ui.connect();
        ui.render();
        ui
    }

    /// The tab's page.
    pub(crate) fn widget(&self) -> &gtk::Box {
        &self.content
    }

    fn build(store: Store, tab: Tab) -> Self {
        let form = gtk::Grid::builder()
            .row_spacing(12)
            .column_spacing(14)
            .margin_top(18)
            .margin_bottom(18)
            .margin_start(18)
            .margin_end(18)
            .build();
        let mut at = 0;
        let mut name = |text: &str, child: &gtk::Widget| {
            let label = gtk::Label::builder().label(text).xalign(1.0).build();
            label.add_css_class("dim-label");
            label.set_size_request(150, -1);
            form.attach(&label, 0, at, 1, 1);
            form.attach(child, 1, at, 1, 1);
            at += 1;
            label
        };

        let enabled = gtk::Switch::builder().halign(gtk::Align::Start).build();
        enabled.update_property(&[gtk::accessible::Property::Label("Night Light")]);
        name("Night Light", enabled.upcast_ref());

        // Two linked toggle buttons, not radio buttons: GTK 4.24 gives a radio button no
        // accessible action, so a screen reader can't choose one.
        let sun = gtk::ToggleButton::with_label(Schedule::Sun.label());
        let constant = gtk::ToggleButton::with_label(Schedule::Constant.label());
        constant.set_group(Some(&sun));
        let schedules = row(&[sun.upcast_ref(), constant.upcast_ref()]);
        schedules.set_spacing(0);
        schedules.add_css_class("linked");
        schedules.set_halign(gtk::Align::Start);
        name("Schedule", schedules.upcast_ref());

        let (least, most, step) = (
            f64::from(MIN_TEMPERATURE),
            f64::from(MAX_TEMPERATURE),
            f64::from(STEP),
        );
        let sliders = [Temperature::Noon, Temperature::Night, Temperature::Constant]
            .into_iter()
            .map(|which| {
                let scale = gtk::Scale::with_range(gtk::Orientation::Horizontal, least, most, step);
                scale.set_draw_value(false);
                scale.set_size_request(300, -1);
                scale.add_mark(least, gtk::PositionType::Bottom, Some("Warm"));
                scale.add_mark(most, gtk::PositionType::Bottom, Some("Neutral"));
                scale.update_property(&[gtk::accessible::Property::Label(which.label())]);
                let value = gtk::Label::new(None);
                value.set_width_chars(7);
                value.set_xalign(1.0);
                value.set_valign(gtk::Align::Start);
                let row = row(&[scale.upcast_ref(), value.upcast_ref()]);
                let name = name(which.label(), row.upcast_ref());
                Slider {
                    which,
                    name,
                    row,
                    scale,
                    value,
                }
            })
            .collect();

        let pause = gtk::Button::with_label(Pause::Pause.label());
        let paused = note();
        let pause_row = row(&[pause.upcast_ref(), paused.upcast_ref()]);
        pause.set_halign(gtk::Align::Start);
        name("Pause", pause_row.upcast_ref());

        let note = note();
        note.set_margin_start(18);
        note.set_margin_end(18);
        let message = gtk::Label::builder()
            .xalign(0.0)
            .wrap(true)
            .margin_start(18)
            .margin_end(18)
            .margin_bottom(12)
            .visible(false)
            .build();

        let content = gtk::Box::new(gtk::Orientation::Vertical, 0);
        content.append(&form);
        content.append(&note);
        content.append(&message);

        NightLightTab {
            tab: RefCell::new(tab),
            store,
            rendering: Cell::new(false),
            pause_ends: RefCell::new(None),
            this: Weak::new(),
            content,
            form,
            enabled,
            sun,
            constant,
            sliders,
            pause,
            paused,
            pause_row,
            note,
            message,
        }
    }

    /// Joins the widgets, the keys and Caminito to what they do.
    fn connect(self: &Rc<Self>) {
        let ui = self.clone();
        self.enabled.connect_active_notify(move |switch| {
            ui.change(|tab| tab.set_enabled(switch.is_active()));
        });
        for (check, schedule) in [
            (&self.sun, Schedule::Sun),
            (&self.constant, Schedule::Constant),
        ] {
            let ui = self.clone();
            check.connect_toggled(move |check| {
                if check.is_active() {
                    ui.change(|tab| tab.set_schedule(schedule));
                }
            });
        }
        for slider in &self.sliders {
            let ui = self.clone();
            let which = slider.which;
            slider.scale.connect_value_changed(move |scale| {
                let kelvin = scale.value().round() as u32;
                ui.change(|tab| tab.set_temperature(which, kelvin));
            });
        }
        let ui = self.clone();
        self.pause.connect_clicked(move |_| ui.pause_or_resume());

        if let Store::Keys { settings, caminito } = &self.store {
            let ui = self.clone();
            settings.connect_changed(None, move |settings, key| {
                if key.starts_with("night-light-") && ui.tab.borrow_mut().refresh(read(settings)) {
                    ui.render();
                }
            });
            if let Some(caminito) = caminito {
                let ui = self.clone();
                caminito.connect_local("g-properties-changed", false, move |values| {
                    ui.caminito_changed(values[0].get::<gio::DBusProxy>().ok().as_ref());
                    None
                });
                // Caminito starting or going: its property comes and goes with it.
                let ui = self.clone();
                caminito.connect_notify_local(Some("g-name-owner"), move |caminito, _| {
                    ui.caminito_changed(Some(caminito));
                });
            }
        }
    }

    fn caminito_changed(&self, caminito: Option<&gio::DBusProxy>) {
        let source = caminito.and_then(location_source);
        if self.tab.borrow_mut().set_location_source(source) {
            self.render();
        }
    }

    /// Changes the tab as a widget says, unless it's the tab that is setting the widget,
    /// and writes the key that changed.
    fn change(&self, change: impl FnOnce(&mut Tab) -> Option<Write>) {
        if self.rendering.get() {
            return;
        }
        let write = change(&mut self.tab.borrow_mut());
        if let (Some(write), Store::Keys { settings, .. }) = (write, &self.store) {
            let written = match write {
                Write::Enabled(enabled) => settings.set_boolean(write.key(), enabled),
                Write::Schedule(schedule) => settings.set_string(write.key(), schedule.nick()),
                Write::Temperature(_, kelvin) => settings.set_uint(write.key(), kelvin),
            };
            if let Err(err) = written {
                self.say(&format!("Night Light wasn't changed: {err}."));
            }
        }
        self.render();
    }

    /// Pauses until sunrise or resumes, through Caminito, which writes the key.
    fn pause_or_resume(self: &Rc<Self>) {
        self.say("");
        let pause = self.tab.borrow().pause(now());
        let caminito = match &self.store {
            Store::Keys { caminito, .. } => caminito.clone(),
            Store::Demo => {
                let until = match pause {
                    Pause::Pause => demo_sunrise(),
                    Pause::Resume { .. } => 0,
                };
                let settings = Settings {
                    paused_until: until,
                    ..self.tab.borrow().settings().clone()
                };
                self.tab.borrow_mut().refresh(settings);
                return self.render();
            }
            Store::NoSchema => None,
        };
        let Some(caminito) = caminito else {
            return self.say("Night Light can't be paused: Caminito isn't on the session bus.");
        };
        let (method, failed) = match pause {
            Pause::Pause => ("PauseNightLight", "Night Light wasn't paused"),
            Pause::Resume { .. } => ("ResumeNightLight", "Night Light wasn't resumed"),
        };
        let call = caminito.call_future(method, None, gio::DBusCallFlags::NONE, -1);
        let ui = self.clone();
        glib::spawn_future_local(async move {
            if let Err(err) = call.await {
                ui.say(&format!("{failed}: {}.", compositor::refused(&err)));
            }
        });
    }

    /// Shows `text` under the settings, or nothing for an empty one.
    fn say(&self, text: &str) {
        self.message.set_label(text);
        self.message.set_visible(!text.is_empty());
    }

    /// Gives every widget what the tab has.
    fn render(&self) {
        self.rendering.set(true);
        let tab = self.tab.borrow();
        let settings = tab.settings();

        self.enabled.set_active(settings.enabled);
        match settings.schedule {
            Schedule::Sun => self.sun.set_active(true),
            Schedule::Constant => self.constant.set_active(true),
        }

        let shown = tab.sliders();
        for slider in &self.sliders {
            let kelvin = shown
                .iter()
                .find(|(which, _)| *which == slider.which)
                .map(|&(_, kelvin)| kelvin);
            slider.name.set_visible(kelvin.is_some());
            slider.row.set_visible(kelvin.is_some());
            let kelvin = kelvin.unwrap_or_else(|| settings.temperature(slider.which));
            slider.scale.set_value(f64::from(kelvin));
            slider.value.set_label(&night_light::kelvin(kelvin));
        }

        let pause = tab.pause(now());
        if let Some(running) = self.pause_ends.take() {
            running.remove();
        }
        if let Pause::Resume { until } = pause {
            let this = self.this.clone();
            let left = u64::try_from(until - now()).unwrap_or_default() + 1;
            let over = glib::timeout_add_seconds_local_once(left as u32, move || {
                if let Some(ui) = this.upgrade() {
                    ui.pause_ends.take();
                    ui.render();
                }
            });
            self.pause_ends.replace(Some(over));
        }
        self.pause.set_label(pause.label());
        let paused = pause.text(local);
        self.paused.set_label(paused.as_deref().unwrap_or_default());
        self.pause_row.set_sensitive(tab.can_pause());

        self.note.set_label(tab.note().unwrap_or_default());
        self.note.set_visible(tab.note().is_some());

        let installed = !matches!(self.store, Store::NoSchema);
        self.form.set_sensitive(installed);
        if !installed {
            self.message
                .set_label("Night Light's settings, org.mate.Caminito, aren't installed.");
            self.message.set_visible(true);
        }
        self.rendering.set(false);
    }
}
