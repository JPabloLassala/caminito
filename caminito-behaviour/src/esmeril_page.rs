//! The Effects page (ADR 0048): Esmeril's switch, its strength and the applications that get
//! it. A choice is written to its key at once, as the Animations page's are, and the page
//! follows the keys when they change from outside. The strength is greyed out while the
//! switch is off. While `reduced-resources` is on Esmeril isn't drawn, and a line under the
//! switch says so; the controls keep working, as they set what applies once it's off again.

use std::cell::RefCell;
use std::rc::Rc;

use caminito_behaviour::esmeril::{Esmeril, Key, STRENGTHS};
use caminito_behaviour::open_windows;
use gtk::glib;
use gtk::prelude::*;

use crate::dialogs::{ask_text, choose};
use crate::{DOMAIN, SPACING};

/// The title of the page's dialogs.
const TITLE: &str = "Effects";
/// The applications' list's size, in pixels.
const APPS: (i32, i32) = (360, 200);

/// The page's state and the widgets it follows the keys with.
struct Page {
    esmeril: Esmeril,
    /// The app ids of the windows open when the page opened.
    open: Vec<String>,
    /// Every app id the list has shown since the page opened, sorted, so that one unchecked
    /// stays to be checked again.
    known: RefCell<Vec<String>>,
    /// The list's check buttons, in `known`'s order.
    checks: RefCell<Vec<gtk::CheckButton>>,
    switch: gtk::Switch,
    strength: gtk::Scale,
    reduced: gtk::Label,
    apps: gtk::Box,
}

/// Makes the page over `esmeril`.
pub fn build(esmeril: Esmeril) -> gtk::Widget {
    let open = open_windows::app_ids().unwrap_or_else(|err| {
        glib::g_warning!(DOMAIN, "can't list the open windows: {err}");
        Vec::new()
    });
    let page = Rc::new(Page {
        esmeril,
        open,
        known: RefCell::default(),
        checks: RefCell::default(),
        switch: gtk::Switch::builder()
            .halign(gtk::Align::End)
            .valign(gtk::Align::Center)
            .build(),
        strength: gtk::Scale::with_range(
            gtk::Orientation::Horizontal,
            f64::from(*STRENGTHS.start()),
            f64::from(*STRENGTHS.end()),
            1.0,
        ),
        reduced: gtk::Label::builder().wrap(true).xalign(0.0).build(),
        apps: gtk::Box::builder()
            .orientation(gtk::Orientation::Vertical)
            .build(),
    });
    page.strength.set_digits(0);
    page.strength.set_value_pos(gtk::PositionType::Right);
    page.strength.set_draw_value(true);
    page.strength.set_hexpand(true);
    page.reduced.add_css_class("dim-label");
    page.show_keys();

    page.switch.connect_active_notify({
        let page = Rc::downgrade(&page);
        move |switch| {
            let Some(page) = page.upgrade() else {
                return;
            };
            // A change heard from the key sets the switch too, and needs no writing back.
            if page.esmeril.is_on() == switch.is_active() {
                return;
            }
            if let Err(err) = page.esmeril.set_on(switch.is_active()) {
                glib::g_warning!(DOMAIN, "can't set esmeril: {err}");
                switch.set_active(page.esmeril.is_on());
            }
            page.strength.set_sensitive(switch.is_active());
        }
    });
    page.strength.connect_value_changed({
        let page = Rc::downgrade(&page);
        move |scale| {
            let Some(page) = page.upgrade() else {
                return;
            };
            let strength = scale.value() as u32;
            if page.esmeril.strength() == strength {
                return;
            }
            if let Err(err) = page.esmeril.set_strength(strength) {
                glib::g_warning!(DOMAIN, "can't set the strength of esmeril: {err}");
                scale.set_value(f64::from(page.esmeril.strength()));
            }
        }
    });
    page.esmeril.connect_changed({
        let page = Rc::downgrade(&page);
        move |key| {
            let Some(page) = page.upgrade() else {
                return;
            };
            match key {
                Key::On | Key::Strength => page.show_keys(),
                Key::Applications => page.redraw_apps(),
            }
        }
    });
    page.esmeril.connect_reduced_resources_changed({
        let page = Rc::downgrade(&page);
        move |_| {
            if let Some(page) = page.upgrade() {
                page.show_reduced_resources();
            }
        }
    });
    page.show_reduced_resources();
    page.redraw_apps();

    let grid = gtk::Grid::builder()
        .row_spacing(SPACING / 2)
        .column_spacing(SPACING)
        .build();
    grid.attach(&row_label("_Esmeril", &page.switch), 0, 0, 1, 1);
    grid.attach(&page.switch, 1, 0, 1, 1);
    grid.attach(&page.reduced, 0, 1, 2, 1);
    grid.attach(&row_label("_Strength", &page.strength), 0, 2, 1, 1);
    grid.attach(&page.strength, 1, 2, 1, 1);

    let root = gtk::Box::builder()
        .orientation(gtk::Orientation::Vertical)
        .spacing(SPACING)
        .margin_top(SPACING)
        .margin_bottom(SPACING)
        .margin_start(SPACING)
        .margin_end(SPACING)
        .build();
    root.append(&grid);
    root.append(
        &gtk::Label::builder()
            .label("<b>Applications</b>")
            .use_markup(true)
            .xalign(0.0)
            .build(),
    );
    let note = gtk::Label::builder()
        .label(
            "The windows of the checked applications show what is beneath them blurred, \
             wherever they are see-through.",
        )
        .wrap(true)
        .xalign(0.0)
        .max_width_chars(50)
        .build();
    note.add_css_class("dim-label");
    root.append(&note);
    root.append(
        &gtk::ScrolledWindow::builder()
            .child(&page.apps)
            .width_request(APPS.0)
            .height_request(APPS.1)
            .hscrollbar_policy(gtk::PolicyType::Never)
            .has_frame(true)
            .build(),
    );
    let another = gtk::Button::with_label("Another app id…");
    another.set_halign(gtk::Align::Start);
    another.connect_clicked({
        let page = Rc::downgrade(&page);
        move |_| {
            if let Some(page) = page.upgrade() {
                glib::spawn_future_local(add_app_id(page));
            }
        }
    });
    root.append(&another);

    // The widgets' handlers hold the page weakly, so that it doesn't hold itself through
    // them: its root holds it, until the root is destroyed.
    let kept = RefCell::new(Some(page));
    root.connect_destroy(move |_| {
        kept.borrow_mut().take();
    });
    root.upcast()
}

impl Page {
    /// The switch and the strength, as the keys have them.
    fn show_keys(&self) {
        let on = self.esmeril.is_on();
        self.switch.set_active(on);
        self.strength.set_sensitive(on);
        self.strength.set_value(f64::from(self.esmeril.strength()));
    }

    /// The line that says Esmeril is off while reduced resources are on.
    fn show_reduced_resources(&self) {
        let reduced = self.esmeril.reduced_resources();
        self.reduced.set_label(if reduced {
            "Esmeril is off while reduced resources are turned on."
        } else {
            ""
        });
        self.reduced.set_visible(reduced);
    }

    /// The list: the app ids listed, open or seen, each a check button that lists the app id
    /// or takes it off the list. The buttons are made again only for a new app id, so that a
    /// toggle keeps the focus and the scroll where they are.
    fn redraw_apps(self: &Rc<Self>) {
        let listed = self.esmeril.applications();
        let mut known = self.known.borrow_mut();
        let before = known.len();
        known.extend(listed.iter().chain(&self.open).cloned());
        known.sort_by_key(|app_id| app_id.to_lowercase());
        known.dedup();
        if known.len() != before {
            self.rebuild_checks(&known);
        }
        for (check, app_id) in self.checks.borrow().iter().zip(known.iter()) {
            check.set_active(listed.contains(app_id));
        }
    }

    /// A check button for each of `app_ids`.
    fn rebuild_checks(self: &Rc<Self>, app_ids: &[String]) {
        while let Some(child) = self.apps.first_child() {
            self.apps.remove(&child);
        }
        let mut checks = self.checks.borrow_mut();
        checks.clear();
        for app_id in app_ids {
            let check = gtk::CheckButton::builder()
                .label(app_id)
                .margin_top(SPACING / 6)
                .margin_bottom(SPACING / 6)
                .margin_start(SPACING / 3)
                .build();
            check.connect_toggled({
                let page = Rc::downgrade(self);
                move |check| {
                    let Some(page) = page.upgrade() else {
                        return;
                    };
                    let app_id = check.label().unwrap_or_default();
                    // A change heard from the key sets the button too, and needs no writing
                    // back.
                    if page.esmeril.is_listed(&app_id) == check.is_active() {
                        return;
                    }
                    if let Err(err) = page.esmeril.set_listed(&app_id, check.is_active()) {
                        glib::g_warning!(DOMAIN, "can't set the applications of esmeril: {err}");
                        check.set_active(page.esmeril.is_listed(&app_id));
                    }
                }
            });
            self.apps.append(&check);
            checks.push(check);
        }
    }
}

/// A row's label, which fills the width the control doesn't.
fn row_label(text: &str, control: &impl IsA<gtk::Widget>) -> gtk::Label {
    gtk::Label::builder()
        .label(text)
        .use_underline(true)
        .mnemonic_widget(control)
        .xalign(0.0)
        .hexpand(true)
        .build()
}

/// Asks for an app id, as the Key remaps page's "Another app id…" does, and lists it.
async fn add_app_id(page: Rc<Page>) {
    let window: Option<gtk::Window> = page.apps.root().and_downcast();
    let asked = ask_text(window.as_ref(), TITLE, "Another app id:", "").await;
    match asked.map(|app_id| app_id.trim().to_owned()) {
        Some(app_id) if app_id.contains(char::is_whitespace) => {
            let detail = "An app id has no spaces.";
            let message = format!("“{app_id}” isn't an app id");
            choose(window.as_ref(), &message, detail, &["OK"]).await;
        }
        Some(app_id) if !app_id.is_empty() => {
            if let Err(err) = page.esmeril.set_listed(&app_id, true) {
                glib::g_warning!(DOMAIN, "can't set the applications of esmeril: {err}");
            }
        }
        _ => {}
    }
}

#[cfg(test)]
mod tests;
