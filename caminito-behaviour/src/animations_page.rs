//! The Animations page (ADR 0055): the Popup animation's drop-down and a switch for each of
//! the other animations. A choice is written to its key at once, as the Hot corners page's
//! are, and the page follows the keys when they change from outside. While one of MATE's keys
//! turns every animation off, the controls are greyed out with their values kept, under a
//! line that says why.

use std::rc::Rc;

use caminito_behaviour::animations::{Animations, PopupAnimation, Switch};
use gtk::glib;
use gtk::prelude::*;

use crate::{DOMAIN, SPACING};

/// Makes the page over `animations`.
pub fn build(animations: Animations) -> gtk::Widget {
    let animations = Rc::new(animations);
    let grid = gtk::Grid::builder()
        .row_spacing(SPACING / 2)
        .column_spacing(SPACING)
        .build();
    popup_row(&grid, &animations);
    let switches: Vec<(Switch, gtk::Switch)> = Switch::ALL
        .into_iter()
        .enumerate()
        .map(|(index, each)| (each, switch_row(&grid, index as i32 + 1, &animations, each)))
        .collect();
    animations.connect_changed(move |changed, on| {
        if let Some((_, switch)) = switches.iter().find(|(each, _)| *each == changed) {
            switch.set_active(on);
        }
    });

    let blocked = gtk::Label::builder().wrap(true).xalign(0.0).build();
    let show_blocker = {
        let (blocked, grid) = (blocked.clone(), grid.clone());
        move |blocker: Option<caminito_behaviour::animations::Blocker>| {
            blocked.set_label(blocker.map_or("", |blocker| blocker.reason()));
            blocked.set_visible(blocker.is_some());
            grid.set_sensitive(blocker.is_none());
        }
    };
    show_blocker(animations.blocker());
    animations.connect_blocker_changed(show_blocker);

    let page = gtk::Box::builder()
        .orientation(gtk::Orientation::Vertical)
        .spacing(SPACING)
        .margin_top(SPACING)
        .margin_bottom(SPACING)
        .margin_start(SPACING)
        .margin_end(SPACING)
        .build();
    page.append(&blocked);
    page.append(&grid);
    page.upcast()
}

/// The Popup animation's row, the grid's first: a drop-down showing its key and writing the
/// one chosen.
fn popup_row(grid: &gtk::Grid, animations: &Rc<Animations>) {
    let labels = PopupAnimation::ALL.map(PopupAnimation::label);
    let dropdown = gtk::DropDown::builder()
        .model(&gtk::StringList::new(&labels))
        .selected(index(animations.popup_animation()))
        .build();
    dropdown.connect_selected_notify({
        let animations = animations.clone();
        move |dropdown| {
            let Some(&animation) = PopupAnimation::ALL.get(dropdown.selected() as usize) else {
                return;
            };
            // A change heard from the key sets the drop-down too, and needs no writing back.
            if animations.popup_animation() == animation {
                return;
            }
            if let Err(err) = animations.set_popup_animation(animation) {
                glib::g_warning!(DOMAIN, "can't set the popup animation: {err}");
                dropdown.set_selected(index(animations.popup_animation()));
            }
        }
    });
    animations.connect_popup_animation_changed({
        let dropdown = dropdown.clone();
        move |animation| dropdown.set_selected(index(animation))
    });
    grid.attach(&row_label("Menus and popups", &dropdown), 0, 0, 1, 1);
    grid.attach(&dropdown, 1, 0, 1, 1);
}

/// `each`'s row of the grid: a switch showing its key and writing what it's set to.
fn switch_row(
    grid: &gtk::Grid,
    row: i32,
    animations: &Rc<Animations>,
    each: Switch,
) -> gtk::Switch {
    let switch = gtk::Switch::builder()
        .active(animations.is_on(each))
        .halign(gtk::Align::End)
        .valign(gtk::Align::Center)
        .build();
    switch.connect_active_notify({
        let animations = animations.clone();
        move |switch| {
            // A change heard from the key sets the switch too, and needs no writing back.
            if animations.is_on(each) == switch.is_active() {
                return;
            }
            if let Err(err) = animations.set(each, switch.is_active()) {
                glib::g_warning!(DOMAIN, "can't set the animation: {err}");
                switch.set_active(animations.is_on(each));
            }
        }
    });
    grid.attach(&row_label(each.label(), &switch), 0, row, 1, 1);
    grid.attach(&switch, 1, row, 1, 1);
    switch
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

/// `animation`'s place in the drop-down.
fn index(animation: PopupAnimation) -> u32 {
    PopupAnimation::ALL
        .iter()
        .position(|each| *each == animation)
        .expect("every animation is in the drop-down") as u32
}
