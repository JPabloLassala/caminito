//! The Screen locking page (CMN-337): the Blank delay's and the Lock delay's drop-downs, the
//! Idle view's two clock formats, a line on when Power has the outputs powered off, and the
//! programs that set the Lock wallpaper and the Lock screen's notifications. A choice is
//! written to its key at once, as the other pages' are, and the page follows the keys when
//! they change from outside. While the Blank delay is Never the Lock delay is greyed out with
//! its value kept, as the session doesn't lock by itself then.

use std::cell::{Cell, RefCell};
use std::rc::Rc;

use caminito_behaviour::screen_locking::{
    Clock, Delay, LOCALE, ScreenLocking, blank_choices, lock_choices, preview,
};
use gtk::prelude::*;
use gtk::{gdk, gio, glib};

use crate::{DOMAIN, SPACING};

/// The drop-down's last entry for a clock, after its presets.
const CUSTOM: &str = "Custom";

/// Makes the page over `screen_locking`.
pub fn build(screen_locking: ScreenLocking) -> gtk::Widget {
    let screen_locking = Rc::new(screen_locking);
    let grid = gtk::Grid::builder()
        .row_spacing(SPACING / 2)
        .column_spacing(SPACING)
        .build();

    delay_row(
        &grid,
        0,
        "Blank the screen after",
        DelayRow {
            read: {
                let screen_locking = screen_locking.clone();
                Box::new(move || screen_locking.blank_delay())
            },
            write: {
                let screen_locking = screen_locking.clone();
                Box::new(move |delay| screen_locking.set_blank_delay(delay))
            },
            choices: blank_choices,
            label: Delay::blank_label,
        },
        |f| screen_locking.connect_blank_delay_changed(f),
    );
    let lock = delay_row(
        &grid,
        1,
        "Lock the screen",
        DelayRow {
            read: {
                let screen_locking = screen_locking.clone();
                Box::new(move || screen_locking.lock_delay())
            },
            write: {
                let screen_locking = screen_locking.clone();
                Box::new(move |delay| screen_locking.set_lock_delay(delay))
            },
            choices: lock_choices,
            label: Delay::lock_label,
        },
        |f| screen_locking.connect_lock_delay_changed(f),
    );
    let never = gtk::Label::builder()
        .label("The screen never blanks, so it never locks by itself.")
        .wrap(true)
        .xalign(0.0)
        .css_classes(["dim-label"])
        .build();
    grid.attach(&never, 0, 2, 2, 1);
    let show_blank = {
        let (lock, never) = (lock.clone(), never.clone());
        move |blank: Delay| {
            lock.set_sensitive(blank != Delay::Never);
            never.set_visible(blank == Delay::Never);
        }
    };
    show_blank(screen_locking.blank_delay());
    screen_locking.connect_blank_delay_changed(show_blank);

    for (index, clock) in Clock::ALL.into_iter().enumerate() {
        clock_row(&grid, 3 + 2 * index as i32, &screen_locking, clock);
    }

    let page = gtk::Box::builder()
        .orientation(gtk::Orientation::Vertical)
        .spacing(SPACING)
        .margin_top(SPACING)
        .margin_bottom(SPACING)
        .margin_start(SPACING)
        .margin_end(SPACING)
        .build();
    page.append(&grid);
    if let Some(power) = power_line(&screen_locking) {
        page.append(&power);
    }
    if let Some(elsewhere) = elsewhere() {
        page.append(&elsewhere);
    }
    page.upcast()
}

/// What a delay's row reads, writes and offers.
struct DelayRow {
    read: Box<dyn Fn() -> Delay>,
    write: Box<dyn Fn(Delay) -> Result<(), caminito_behaviour::screen_locking::Error>>,
    choices: fn(Delay) -> Vec<Delay>,
    label: fn(Delay) -> String,
}

/// A delay's row of the grid: a drop-down showing its keys and writing the one chosen. Its
/// entries are rebuilt from the keys each time they change, so that a value set outside the
/// list shows as itself. `connect` hears the keys' changes.
fn delay_row(
    grid: &gtk::Grid,
    row: i32,
    text: &str,
    delay: DelayRow,
    connect: impl FnOnce(Box<dyn Fn(Delay)>),
) -> gtk::DropDown {
    let delay = Rc::new(delay);
    let dropdown = gtk::DropDown::builder().build();
    let offered = Rc::new(RefCell::new(Vec::new()));
    // Set while the entries are rebuilt, when the drop-down's selection moves on its own.
    let rebuilding = Rc::new(Cell::new(false));
    let select = {
        let (dropdown, rebuilding) = (dropdown.clone(), rebuilding.clone());
        move |choices: &[Delay], current: Delay| {
            let place = choices
                .iter()
                .position(|each| *each == current)
                .expect("the choices hold the current delay") as u32;
            if dropdown.selected() != place {
                rebuilding.set(true);
                dropdown.set_selected(place);
                rebuilding.set(false);
            }
        }
    };
    let rebuild = {
        let (delay, dropdown, offered, rebuilding) = (
            delay.clone(),
            dropdown.clone(),
            offered.clone(),
            rebuilding.clone(),
        );
        let select = select.clone();
        move || {
            let current = (delay.read)();
            let choices = (delay.choices)(current);
            let labels: Vec<String> = choices.iter().map(|each| (delay.label)(*each)).collect();
            let labels: Vec<&str> = labels.iter().map(String::as_str).collect();
            rebuilding.set(true);
            dropdown.set_model(Some(&gtk::StringList::new(&labels)));
            rebuilding.set(false);
            select(&choices, current);
            *offered.borrow_mut() = choices;
        }
    };
    rebuild();
    let show = {
        let (delay, offered) = (delay.clone(), offered.clone());
        move |current: Delay| {
            let choices = (delay.choices)(current);
            if *offered.borrow() == choices {
                select(&choices, current);
                return;
            }
            // The keys can change while the drop-down tells of the choice that changed them,
            // and its model can't be swapped under it then: the entries are rebuilt after.
            glib::idle_add_local_once(rebuild.clone());
        }
    };
    dropdown.connect_selected_notify({
        let (delay, offered, rebuilding) = (delay.clone(), offered.clone(), rebuilding.clone());
        let show = show.clone();
        move |dropdown| {
            if rebuilding.get() {
                return;
            }
            let Some(&chosen) = offered.borrow().get(dropdown.selected() as usize) else {
                return;
            };
            // A change heard from the keys sets the drop-down too, and needs no writing back.
            if (delay.read)() == chosen {
                return;
            }
            if let Err(err) = (delay.write)(chosen) {
                glib::g_warning!(DOMAIN, "can't set the delay: {err}");
                show((delay.read)());
            }
        }
    });
    connect(Box::new(show));
    grid.attach(&row_label(text, &dropdown), 0, row, 1, 1);
    grid.attach(&dropdown, 1, row, 1, 1);
    dropdown
}

/// `clock`'s two rows of the grid, from `row`: a drop-down of its presets and Custom, and
/// under it, for Custom, the format to type and what it gives now.
fn clock_row(grid: &gtk::Grid, row: i32, screen_locking: &Rc<ScreenLocking>, clock: Clock) {
    let presets = clock.presets();
    let labels: Vec<&str> = presets
        .iter()
        .map(|preset| preset.label)
        .chain([CUSTOM])
        .collect();
    let custom = presets.len() as u32;
    let dropdown = gtk::DropDown::builder()
        .model(&gtk::StringList::new(&labels))
        .build();
    let entry = gtk::Entry::builder()
        .placeholder_text("A GDateTime format, such as %H:%M")
        .hexpand(true)
        .build();
    let preview_label = gtk::Label::builder()
        .xalign(0.0)
        .css_classes(["dim-label"])
        .build();
    let custom_box = gtk::Box::builder()
        .orientation(gtk::Orientation::Horizontal)
        .spacing(SPACING / 2)
        .build();
    custom_box.append(&entry);
    custom_box.append(&preview_label);

    let show_preview = {
        let preview_label = preview_label.clone();
        move |format: &str| {
            let now = glib::DateTime::now_local().expect("the local time can be had");
            let text = match preview(format, &now) {
                Some(text) => text,
                None => "Not a format GLib can read".into(),
            };
            preview_label.set_label(&text);
        }
    };
    let show = {
        let (dropdown, entry, custom_box) = (dropdown.clone(), entry.clone(), custom_box.clone());
        let show_preview = show_preview.clone();
        move |format: &str| {
            // A format typed in Custom that happens to be a preset stays in Custom, so that
            // the field doesn't leave while it's typed in.
            let typed = dropdown.selected() == custom && entry.text() == format;
            let place = if typed { None } else { clock.preset(format) };
            dropdown.set_selected(place.map_or(custom, |place| place as u32));
            custom_box.set_visible(place.is_none());
            if place.is_none() && entry.text() != format {
                entry.set_text(format);
            }
            show_preview(format);
        }
    };
    show(&screen_locking.format(clock));

    dropdown.connect_selected_notify({
        let (screen_locking, entry, custom_box) =
            (screen_locking.clone(), entry.clone(), custom_box.clone());
        let show_preview = show_preview.clone();
        move |dropdown| {
            let current = screen_locking.format(clock);
            let Some(preset) = presets.get(dropdown.selected() as usize) else {
                // Custom: the field starts from the format in use, which is written only
                // once it's changed. The locale's is no GDateTime format to start from.
                custom_box.set_visible(true);
                entry.set_text(if current == LOCALE { "" } else { &current });
                show_preview(&entry.text());
                return;
            };
            custom_box.set_visible(false);
            if current == preset.format {
                return;
            }
            if let Err(err) = screen_locking.set_format(clock, preset.format) {
                glib::g_warning!(DOMAIN, "can't set the clock's format: {err}");
            }
        }
    });
    entry.connect_changed({
        let screen_locking = screen_locking.clone();
        move |entry| {
            let format = entry.text();
            show_preview(&format);
            // Nothing is written that the clock couldn't show.
            let now = glib::DateTime::now_local().expect("the local time can be had");
            if format.is_empty() || preview(&format, &now).is_none() {
                return;
            }
            if screen_locking.format(clock) == format.as_str() {
                return;
            }
            if let Err(err) = screen_locking.set_format(clock, &format) {
                glib::g_warning!(DOMAIN, "can't set the clock's format: {err}");
            }
        }
    });
    screen_locking.connect_format_changed(move |changed, format| {
        if changed == clock {
            show(&format);
        }
    });

    grid.attach(&row_label(clock.label(), &dropdown), 0, row, 1, 1);
    grid.attach(&dropdown, 1, row, 1, 1);
    grid.attach(&custom_box, 0, row + 1, 2, 1);
}

/// The line on when Power has the outputs powered off, with a button that opens Power where
/// it's installed, or `None` without mate-power-manager's schema.
fn power_line(screen_locking: &Rc<ScreenLocking>) -> Option<gtk::Widget> {
    let text = screen_locking.power_line()?;
    let label = gtk::Label::builder()
        .label(&text)
        .wrap(true)
        .xalign(0.0)
        .hexpand(true)
        .build();
    screen_locking.connect_power_line_changed({
        let label = label.clone();
        move |text| label.set_label(&text)
    });
    let line = gtk::Box::builder()
        .orientation(gtk::Orientation::Horizontal)
        .spacing(SPACING)
        .build();
    line.append(&label);
    if let Some(button) = opener("Power Settings…", "mate-power-preferences.desktop") {
        line.append(&button);
    }
    Some(line.upcast())
}

/// The line that sends the user to the programs that set the rest of the Lock screen, or
/// `None` if neither is installed.
fn elsewhere() -> Option<gtk::Widget> {
    let line = gtk::Box::builder()
        .orientation(gtk::Orientation::Horizontal)
        .spacing(SPACING / 2)
        .build();
    line.append(
        &gtk::Label::builder()
            .label("Wallpaper and notifications on the Lock screen are set in")
            .wrap(true)
            .xalign(0.0)
            .hexpand(true)
            .build(),
    );
    let buttons: Vec<gtk::Button> = [
        ("Wallpaper…", "caminito-wallpaper.desktop"),
        ("Notifications…", "caminito-notifications.desktop"),
    ]
    .into_iter()
    .filter_map(|(text, id)| opener(text, id))
    .collect();
    if buttons.is_empty() {
        return None;
    }
    for button in &buttons {
        line.append(button);
    }
    Some(line.upcast())
}

/// A button that starts the program of the desktop entry `id`, or `None` if it isn't
/// installed.
fn opener(text: &str, id: &str) -> Option<gtk::Button> {
    let app = gio::AppInfo::all()
        .into_iter()
        .find(|app| app.id().as_deref() == Some(id))?;
    let button = gtk::Button::builder()
        .label(text)
        .valign(gtk::Align::Center)
        .build();
    button.connect_clicked(move |_| {
        let context = gdk::Display::default().map(|display| display.app_launch_context());
        if let Err(err) = app.launch(&[], context.as_ref()) {
            glib::g_warning!(DOMAIN, "can't start {}: {err}", app.name());
        }
    });
    Some(button)
}

/// A row's label, which fills the width the control doesn't.
fn row_label(text: &str, control: &impl IsA<gtk::Widget>) -> gtk::Label {
    gtk::Label::builder()
        .label(text)
        .mnemonic_widget(control)
        .xalign(0.0)
        .hexpand(true)
        .build()
}

#[cfg(test)]
mod tests;
