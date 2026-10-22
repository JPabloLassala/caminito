//! The Hot corners page (CMN-184): a picture of a screen with a drop-down in each of its
//! corners. A choice is written to the corner's key at once, as the Wallpaper dialog's are,
//! and the drop-downs follow the keys when they change from outside, as by `gsettings set`.

use std::rc::Rc;

use caminito_behaviour::hot_corners::{Action, Corner, HotCorners};
use gtk::glib;
use gtk::prelude::*;

use crate::{DOMAIN, SPACING};

/// The screen's size in the picture, in pixels: a 16:10 monitor.
const SCREEN: (i32, i32) = (480, 300);
/// The width of the screen's bezel, in pixels.
const BEZEL: i32 = 8;
/// The space between a drop-down and its corner of the screen, in pixels.
const INSET: i32 = 10;

/// The screen's look: a monitor's bezel around a desktop of MATE's blue, the wallpaper's
/// default (212).
fn css() -> String {
    format!(
        ".hot-corners-screen {{
            background-color: #3a6ea5;
            border: {BEZEL}px solid #2e3436;
            border-radius: 8px;
        }}"
    )
}

/// Makes the page over `corners`.
pub fn build(corners: HotCorners) -> gtk::Widget {
    if let Some(display) = gtk::gdk::Display::default() {
        let provider = gtk::CssProvider::new();
        provider.load_from_string(&css());
        gtk::style_context_add_provider_for_display(
            &display,
            &provider,
            gtk::STYLE_PROVIDER_PRIORITY_APPLICATION,
        );
    }

    let corners = Rc::new(corners);
    let screen = gtk::Overlay::builder()
        .child(
            &gtk::Box::builder()
                .width_request(SCREEN.0)
                .height_request(SCREEN.1)
                .css_classes(["hot-corners-screen"])
                .build(),
        )
        .halign(gtk::Align::Center)
        .build();
    let dropdowns: Vec<(Corner, gtk::DropDown)> = Corner::ALL
        .into_iter()
        .map(|corner| {
            let dropdown = dropdown(&corners, corner);
            screen.add_overlay(&dropdown);
            (corner, dropdown)
        })
        .collect();
    corners.connect_changed(move |corner, action| {
        if let Some((_, dropdown)) = dropdowns.iter().find(|(each, _)| *each == corner) {
            dropdown.set_selected(index(action));
        }
    });

    let page = gtk::Box::builder()
        .orientation(gtk::Orientation::Vertical)
        .spacing(SPACING)
        .margin_top(SPACING)
        .margin_bottom(SPACING)
        .margin_start(SPACING)
        .margin_end(SPACING)
        .build();
    page.append(
        &gtk::Label::builder()
            .label("Rest the pointer in a corner of any monitor to run what its corner is set to.")
            .wrap(true)
            .xalign(0.0)
            .build(),
    );
    page.append(&screen);
    page.upcast()
}

/// The drop-down of `corner`, in its corner of the screen, showing its key's action and
/// writing the one chosen.
fn dropdown(corners: &Rc<HotCorners>, corner: Corner) -> gtk::DropDown {
    let labels = Action::ALL.map(Action::label);
    let (halign, valign) = match corner {
        Corner::TopLeft => (gtk::Align::Start, gtk::Align::Start),
        Corner::TopRight => (gtk::Align::End, gtk::Align::Start),
        Corner::BottomLeft => (gtk::Align::Start, gtk::Align::End),
        Corner::BottomRight => (gtk::Align::End, gtk::Align::End),
    };
    // Inside the bezel.
    let margin = BEZEL + INSET;
    let dropdown = gtk::DropDown::builder()
        .model(&gtk::StringList::new(&labels))
        .halign(halign)
        .valign(valign)
        .margin_top(margin)
        .margin_bottom(margin)
        .margin_start(margin)
        .margin_end(margin)
        .build();
    dropdown.set_selected(index(corners.action(corner)));
    let corners = corners.clone();
    dropdown.connect_selected_notify(move |dropdown| {
        let Some(&action) = Action::ALL.get(dropdown.selected() as usize) else {
            return;
        };
        // A change heard from the key sets the drop-down too, and needs no writing back.
        if corners.action(corner) == action {
            return;
        }
        if let Err(err) = corners.set(corner, action) {
            glib::g_warning!(DOMAIN, "can't set the corner: {err}");
            dropdown.set_selected(index(corners.action(corner)));
        }
    });
    dropdown
}

/// `action`'s place in a drop-down.
fn index(action: Action) -> u32 {
    Action::ALL
        .iter()
        .position(|each| *each == action)
        .expect("every action is in the drop-downs") as u32
}
