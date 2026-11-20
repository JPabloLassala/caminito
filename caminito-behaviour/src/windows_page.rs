//! The Windows page (ADR 0057, CMN-305): the switch that gives each fullscreen window a
//! Fullscreen workspace of its own. It's written to its key at once, as the Effects page's
//! are, and the page follows the key when it changes from outside. Caminito applies it to
//! windows already fullscreen too.

use std::rc::Rc;

use caminito_behaviour::windows::Windows;
use gtk::glib;
use gtk::prelude::*;

use crate::{DOMAIN, SPACING};

/// Makes the page over `windows`.
pub fn build(windows: Windows) -> gtk::Widget {
    let windows = Rc::new(windows);
    let switch = gtk::Switch::builder()
        .active(windows.fullscreen_workspaces())
        .halign(gtk::Align::End)
        .valign(gtk::Align::Center)
        .build();
    switch.connect_active_notify({
        let windows = windows.clone();
        move |switch| {
            // A change heard from the key sets the switch too, and needs no writing back.
            if windows.fullscreen_workspaces() == switch.is_active() {
                return;
            }
            if let Err(err) = windows.set_fullscreen_workspaces(switch.is_active()) {
                glib::g_warning!(DOMAIN, "can't set fullscreen-workspaces: {err}");
                switch.set_active(windows.fullscreen_workspaces());
            }
        }
    });
    windows.connect_changed({
        let switch = switch.downgrade();
        move |on| {
            if let Some(switch) = switch.upgrade() {
                switch.set_active(on);
            }
        }
    });

    let label = gtk::Label::builder()
        .label("_Fullscreen windows get a workspace of their own")
        .use_underline(true)
        .mnemonic_widget(&switch)
        .xalign(0.0)
        .hexpand(true)
        .build();
    let note = gtk::Label::builder()
        .label(
            "A fullscreen window is on a workspace right after the one it was on, and \
             switching left goes back there. Off, it stays on its workspace.",
        )
        .wrap(true)
        .xalign(0.0)
        .max_width_chars(50)
        .build();
    note.add_css_class("dim-label");

    let grid = gtk::Grid::builder()
        .row_spacing(SPACING / 2)
        .column_spacing(SPACING)
        .margin_top(SPACING)
        .margin_bottom(SPACING)
        .margin_start(SPACING)
        .margin_end(SPACING)
        .build();
    grid.attach(&label, 0, 0, 1, 1);
    grid.attach(&switch, 1, 0, 1, 1);
    grid.attach(&note, 0, 1, 2, 1);
    grid.upcast()
}

#[cfg(test)]
mod tests;
