//! The Notification bubbles (CMN-258), in GTK4: one layer surface in the namespace
//! `caminito-notification`, on the side of the corner the settings name (CMN-261), that holds
//! the bubbles on screen, newest nearest the corner, under their header (CMN-285). A new bubble
//! moves them to the corner the settings name now. Its exclusive zone is 0, so it sits under
//! the panel. It spans the output's height between the panel and the far edge, which is the
//! room the bubbles have, and takes the pointer only where they and their header are. It
//! names no output, so Caminito puts it on the output under the pointer as it maps. It's
//! hidden while there's no bubble, and maps again where the pointer is for the next one; a
//! bubble that opens while others show joins them on their output, as bubbles are on one
//! output at a time in v1 (the spec's "Not in v1").
//!
//! The server decides what's on screen; this draws it, from the GTK theme, and tells the server
//! what the user does through each bubble's [`Events`]. A grouped bubble (CMN-265) is the
//! prototype's "Digest": its newest notification in full with the count in its header, the two
//! before it on a line each, and "+*N* more" or "Show all *N*", which opens them all inside the
//! bubble, scrolling past half the output's height.
//!
//! A bubble with a timeout has a line along its bottom edge, in the theme's accent, that
//! shrinks over what's left of it (CMN-286). It draws what the server tells it, whose timer is
//! the one that closes the bubble.

use std::cell::{Cell, RefCell};
use std::collections::HashMap;
use std::rc::Rc;
use std::time::Instant;

use caminito_notifications::server::{
    Bubble, Bubbles, Earlier, Events, Header, HeaderEvents, Icon, Image, Pixels, TimeLeft,
};
use caminito_notifications::settings::Corner;
use gtk::prelude::*;
use gtk::{cairo, gdk, gio, glib, pango};
use gtk4_layer_shell::{Edge, KeyboardMode, Layer, LayerShell};

/// The namespace of the bubbles' layer surface, which Caminito knows them by.
const NAMESPACE: &str = "caminito-notification";

/// The space between the corner and the bubbles, and between two bubbles.
const GAP: i32 = 10;
const WIDTH: i32 = 360;
const APP_ICON_SIZE: i32 = 16;
const IMAGE_SIZE: i32 = 48;
/// The notifications before the newest that a closed grouped bubble has a line of.
const DIGEST_LINES: usize = 2;
/// The time line's thickness, and its distance from the bubble's sides and bottom edge, inside
/// the bubble's padding, so that it's clear of what's above it and of the round corners.
const TIME_LINE: i32 = 2;
const TIME_LINE_INSET: i32 = 12;
const TIME_LINE_BOTTOM: i32 = 4;

const CSS: &str = "
window.caminito-notifications { background: none; }
.caminito-notification {
  border-radius: 12px;
  border: 1px solid alpha(currentColor, 0.15);
  padding: 10px 12px;
}
.caminito-notification .app-name { font-size: smaller; }
.caminito-notification .close { min-width: 20px; min-height: 20px; padding: 2px; margin: 4px; }
.caminito-notifications-header button { border-radius: 12px; min-height: 24px; padding: 0 10px; }
.caminito-notifications-header .more.critical { color: @error_color; }
.caminito-notification progressbar trough, .caminito-notification progressbar progress {
  min-height: 6px;
}
.caminito-notification .count {
  font-size: smaller; font-weight: bold; padding: 0 7px; margin-right: 22px;
  border-radius: 9px; background: alpha(currentColor, 0.16);
}
.caminito-notification .digest { border-top: 1px solid alpha(currentColor, 0.1); padding-top: 6px; }
.caminito-notification .more { font-size: smaller; padding: 2px 9px; min-height: 0; }
";

/// The bubbles, in their layer surface.
pub(crate) struct GtkBubbles {
    window: gtk::Window,
    /// Whether the window is a layer surface, which only then has a corner.
    layer: bool,
    settings: gio::Settings,
    /// The corner the bubbles are in.
    corner: Cell<Corner>,
    /// The header and the bubbles under it.
    column: gtk::Box,
    /// The bubbles, newest first, which is nearest the corner.
    stack: gtk::Box,
    shown: RefCell<HashMap<u32, Shown>>,
    /// "*N* more notifications", and the icon of a critical one among them.
    more: gtk::Button,
    more_label: gtk::Label,
    more_critical: gtk::Image,
    /// What the header's buttons tell the server.
    header_events: Rc<RefCell<Option<HeaderEvents>>>,
}

/// A bubble on screen.
struct Shown {
    /// What's in the stack.
    root: gtk::Box,
    events: Events,
    /// Whether it has a reply field, which needs the keyboard.
    reply: bool,
    /// Whether a grouped bubble shows all its notifications, which an update keeps.
    open: Rc<Cell<bool>>,
    /// The line of its time left, which an update keeps.
    time_line: TimeLine,
}

/// The line along a bubble's bottom edge that shrinks over what's left of its timeout.
struct TimeLine {
    area: gtk::DrawingArea,
    /// What the bubble was last told of its timeout, and when, while it has one.
    told: Rc<Cell<Option<(TimeLeft, Instant)>>>,
    /// What redraws it on each frame while the time runs.
    tick: RefCell<Option<gtk::TickCallbackId>>,
}

impl GtkBubbles {
    pub(crate) fn new(application: &gtk::Application, settings: gio::Settings) -> Self {
        let window = gtk::Window::builder()
            .application(application)
            .decorated(false)
            .resizable(false)
            .css_classes(["caminito-notifications"])
            .build();
        // The library asks GDK's Wayland display, which an X11 one isn't.
        let on_wayland = gdk::Display::default()
            .is_some_and(|display| display.type_().name() == "GdkWaylandDisplay");
        let layer = on_wayland && gtk4_layer_shell::is_supported();
        if layer {
            window.init_layer_shell();
            window.set_namespace(Some(NAMESPACE));
            window.set_layer(Layer::Overlay);
            window.set_keyboard_mode(KeyboardMode::None);
            for edge in [Edge::Top, Edge::Bottom, Edge::Left, Edge::Right] {
                window.set_margin(edge, GAP);
            }
        } else {
            glib::g_warning!(
                caminito_notifications::DOMAIN,
                "there's no layer shell, so the bubbles are plain windows"
            );
        }
        let stack = gtk::Box::new(gtk::Orientation::Vertical, GAP);
        let header_events: Rc<RefCell<Option<HeaderEvents>>> = Rc::default();
        let (header, more, more_label, more_critical) = header(&header_events);
        let column = gtk::Box::new(gtk::Orientation::Vertical, GAP);
        column.append(&header);
        column.append(&stack);
        // The window is as tall as the room, which the area's size says, and as wide as the
        // column.
        let room = gtk::DrawingArea::new();
        let overlay = gtk::Overlay::new();
        overlay.set_child(Some(&room));
        overlay.add_overlay(&column);
        overlay.set_measure_overlay(&column, true);
        window.set_child(Some(&overlay));
        room.connect_resize({
            let window = window.clone();
            let column = column.clone();
            move |_, _, _| reshape(&window, &column, layer)
        });

        if let Some(display) = gdk::Display::default() {
            let provider = gtk::CssProvider::new();
            provider.load_from_string(CSS);
            gtk::style_context_add_provider_for_display(
                &display,
                &provider,
                gtk::STYLE_PROVIDER_PRIORITY_APPLICATION,
            );
        }
        let corner = Corner::of(&settings);
        let bubbles = GtkBubbles {
            window,
            layer,
            settings,
            corner: Cell::new(corner),
            column,
            stack,
            shown: RefCell::default(),
            more,
            more_label,
            more_critical,
            header_events,
        };
        bubbles.anchor(corner);
        bubbles
    }

    /// Moves the bubbles to the corner the settings name, if they're elsewhere.
    fn follow_corner(&self) {
        let corner = Corner::of(&self.settings);
        let was = self.corner.replace(corner);
        if corner == was {
            return;
        }
        // The newest is nearest the corner, so the stack turns over between top and bottom.
        if corner.is_top() != was.is_top() {
            let mut child = self.stack.first_child();
            while let Some(current) = child {
                child = current.next_sibling();
                self.stack
                    .reorder_child_after(&current, None::<&gtk::Widget>);
            }
        }
        self.anchor(corner);
    }

    fn anchor(&self, corner: Corner) {
        self.column.set_valign(if corner.is_top() {
            gtk::Align::Start
        } else {
            gtk::Align::End
        });
        if !self.layer {
            return;
        }
        self.window.set_anchor(Edge::Top, true);
        self.window.set_anchor(Edge::Bottom, true);
        self.window.set_anchor(Edge::Right, corner.is_right());
        self.window.set_anchor(Edge::Left, !corner.is_right());
    }

    /// The height the header and the bubbles have: the window's, between the panel and the
    /// output's far edge, or the output's without a layer shell, or before the compositor has
    /// sized the window that just mapped. `None` until the window has an output.
    fn room(&self) -> Option<i32> {
        if self.layer && self.window.height() > 0 {
            return Some(self.window.height());
        }
        let surface = self.window.surface()?;
        let monitor = surface.display().monitor_at_surface(&surface)?;
        Some(monitor.geometry().height() - 2 * GAP)
    }

    /// Shows the window when it has a bubble, and hides it, shrunk, when it has none. It takes
    /// the keyboard on a click only while a bubble has a reply field to type in, so that a
    /// click on any other leaves the keyboard where it was.
    fn present(&self) {
        if self.layer {
            let typing = self.shown.borrow().values().any(|shown| shown.reply);
            self.window.set_keyboard_mode(if typing {
                KeyboardMode::OnDemand
            } else {
                KeyboardMode::None
            });
        }
        if self.stack.first_child().is_some() {
            self.window.set_visible(true);
            reshape(&self.window, &self.column, self.layer);
        } else {
            self.window.set_visible(false);
            // So that it maps again at the size of its next bubble.
            self.window.set_default_size(1, 1);
        }
    }
}

/// The header: "*N* more notifications" on the left while some got no bubble, which opens
/// the Notification center, and on the right the close button that shows "Clear all" on
/// hover. Returns it with its "more" button, that button's label and its critical icon.
fn header(
    events: &Rc<RefCell<Option<HeaderEvents>>>,
) -> (gtk::Box, gtk::Button, gtk::Label, gtk::Image) {
    let header = gtk::Box::new(gtk::Orientation::Horizontal, 6);
    header.add_css_class("caminito-notifications-header");

    let more_label = gtk::Label::new(None);
    let more_critical = gtk::Image::from_icon_name("dialog-warning-symbolic");
    let more_content = gtk::Box::new(gtk::Orientation::Horizontal, 4);
    more_content.append(&more_critical);
    more_content.append(&more_label);
    let more = gtk::Button::builder()
        .child(&more_content)
        .css_classes(["more"])
        .focusable(false)
        .visible(false)
        .build();
    more.connect_clicked({
        let events = events.clone();
        move |_| {
            if let Some(events) = events.borrow().clone() {
                events.open_center();
            }
        }
    });
    header.append(&more);

    let clear_label = gtk::Label::new(Some("Clear all"));
    clear_label.set_visible(false);
    let clear_content = gtk::Box::new(gtk::Orientation::Horizontal, 6);
    clear_content.append(&clear_label);
    clear_content.append(&gtk::Image::from_icon_name("window-close-symbolic"));
    let clear = gtk::Button::builder()
        .child(&clear_content)
        .css_classes(["clear"])
        .focusable(false)
        .halign(gtk::Align::End)
        .hexpand(true)
        .build();
    clear.update_property(&[gtk::accessible::Property::Label("Clear all")]);
    clear.connect_clicked({
        let events = events.clone();
        move |_| {
            if let Some(events) = events.borrow().clone() {
                events.clear_all();
            }
        }
    });
    let motion = gtk::EventControllerMotion::new();
    motion.connect_enter({
        let label = clear_label.clone();
        move |_, _, _| label.set_visible(true)
    });
    motion.connect_leave(move |_| clear_label.set_visible(false));
    clear.add_controller(motion);
    header.append(&clear);

    (header, more, more_label, more_critical)
}

/// The header and the bubbles' natural size: width, and height at that width.
fn natural_size(column: &gtk::Box) -> (i32, i32) {
    let (_, width, _, _) = column.measure(gtk::Orientation::Horizontal, -1);
    let (_, height, _, _) = column.measure(gtk::Orientation::Vertical, width);
    (width, height)
}

/// Lets the pointer through the window, which spans the output's height, but where the header
/// and the bubbles are, at its top or its bottom as the corner is.
fn reshape(window: &gtk::Window, column: &gtk::Box, layer: bool) {
    let Some(surface) = window.surface().filter(|_| layer) else {
        return;
    };
    let (width, height) = natural_size(column);
    let y = if column.valign() == gtk::Align::End {
        (window.height() - height).max(0)
    } else {
        0
    };
    let area = cairo::RectangleInt::new(0, y, width, height);
    surface.set_input_region(Some(&cairo::Region::create_rectangle(&area)));
}

impl Bubbles for GtkBubbles {
    fn show(&self, id: u32, bubble: &Bubble, events: Events) -> bool {
        let root = gtk::Box::new(gtk::Orientation::Vertical, 0);
        let open = Rc::new(Cell::new(false));
        let time_line = TimeLine::new();
        fill(&root, bubble, &events, &open, &time_line.area);
        self.follow_corner();
        let first = self.stack.first_child().is_none();
        if self.corner.get().is_top() {
            self.stack.prepend(&root);
        } else {
            self.stack.append(&root);
        }
        // Measured in the stack, where the theme styles it.
        if !first
            && let Some(room) = self.room()
            && natural_size(&self.column).1 > room
        {
            self.stack.remove(&root);
            return false;
        }
        let reply = bubble.reply.is_some();
        self.shown.borrow_mut().insert(
            id,
            Shown {
                root,
                events,
                reply,
                open,
                time_line,
            },
        );
        self.present();
        true
    }

    fn header(&self, header: Header, events: HeaderEvents) {
        *self.header_events.borrow_mut() = Some(events);
        let more = match header.more {
            1 => "1 more notification".to_owned(),
            more => format!("{more} more notifications"),
        };
        self.more_label.set_text(&more);
        self.more.set_visible(header.more > 0);
        self.more_critical.set_visible(header.critical);
        if header.critical {
            self.more.add_css_class("critical");
        } else {
            self.more.remove_css_class("critical");
        }
        reshape(&self.window, &self.column, self.layer);
    }

    fn update(&self, id: u32, bubble: &Bubble) {
        {
            let mut all = self.shown.borrow_mut();
            let Some(shown) = all.get_mut(&id) else {
                return;
            };
            while let Some(child) = shown.root.first_child() {
                shown.root.remove(&child);
            }
            fill(
                &shown.root,
                bubble,
                &shown.events,
                &shown.open,
                &shown.time_line.area,
            );
            shown.reply = bubble.reply.is_some();
        }
        self.present();
    }

    fn close(&self, id: u32) {
        let shown = self.shown.borrow_mut().remove(&id);
        if let Some(shown) = shown {
            self.stack.remove(&shown.root);
        }
        self.present();
    }

    fn time_left(&self, id: u32, time_left: Option<TimeLeft>) {
        if let Some(shown) = self.shown.borrow().get(&id) {
            shown.time_line.tell(time_left);
        }
    }
}

impl TimeLine {
    fn new() -> Self {
        let area = gtk::DrawingArea::builder()
            .content_height(TIME_LINE)
            .valign(gtk::Align::End)
            .margin_start(TIME_LINE_INSET)
            .margin_end(TIME_LINE_INSET)
            .margin_bottom(TIME_LINE_BOTTOM)
            .can_target(false)
            .visible(false)
            .build();
        let told: Rc<Cell<Option<(TimeLeft, Instant)>>> = Rc::default();
        area.set_draw_func({
            let told = told.clone();
            move |area, cr, width, height| {
                let Some((time_left, at)) = told.get() else {
                    return;
                };
                let left = if time_left.running {
                    time_left.left.saturating_sub(at.elapsed())
                } else {
                    time_left.left
                };
                let fraction = (left.as_secs_f64() / time_left.timeout.as_secs_f64()).min(1.0);
                // A line with round ends, which are half its thickness past its ends.
                let thickness = f64::from(height);
                let length = (f64::from(width) - thickness) * fraction;
                if length <= 0.0 {
                    return;
                }
                let accent = accent(area);
                cr.set_source_rgba(
                    f64::from(accent.red()),
                    f64::from(accent.green()),
                    f64::from(accent.blue()),
                    f64::from(accent.alpha()),
                );
                cr.set_line_width(thickness);
                cr.set_line_cap(cairo::LineCap::Round);
                cr.move_to(thickness / 2.0, thickness / 2.0);
                cr.line_to(thickness / 2.0 + length, thickness / 2.0);
                let _ = cr.stroke();
            }
        });
        TimeLine {
            area,
            told,
            tick: RefCell::default(),
        }
    }

    /// Draws `time_left` from now on, redrawing on each frame while it runs, or nothing.
    fn tell(&self, time_left: Option<TimeLeft>) {
        self.told
            .set(time_left.map(|time_left| (time_left, Instant::now())));
        self.area.set_visible(time_left.is_some());
        let running = time_left.is_some_and(|time_left| time_left.running);
        let mut tick = self.tick.borrow_mut();
        if running && tick.is_none() {
            *tick = Some(self.area.add_tick_callback(|area, _| {
                area.queue_draw();
                glib::ControlFlow::Continue
            }));
        } else if !running && let Some(tick) = tick.take() {
            tick.remove();
        }
        self.area.queue_draw();
    }
}

/// The theme's accent colour.
fn accent(widget: &gtk::DrawingArea) -> gdk::RGBA {
    #[expect(
        deprecated,
        reason = "GTK 4 has no other way to read a theme's named colour, and a widget's \
                  own colour isn't the accent"
    )]
    let named = |name| widget.style_context().lookup_color(name);
    named("accent_bg_color")
        .or_else(|| named("theme_selected_bg_color"))
        .unwrap_or_else(|| widget.color())
}

/// Puts what `bubble` shows into `root`: the app's icon and name on top, the summary in bold
/// and the body, the image on the right, the progress bar, the actions and the reply field
/// below, and a close button on hover. A grouped bubble has its count in the header and the
/// notifications before the newest below, all of them while `open`. `time_line` goes along
/// its bottom edge, out of what it showed before.
fn fill(
    root: &gtk::Box,
    bubble: &Bubble,
    events: &Events,
    open: &Rc<Cell<bool>>,
    time_line: &gtk::DrawingArea,
) {
    // The timeout waits while the pointer is on the bubble, or a reply is being typed in it.
    let pointer = Rc::new(Cell::new(false));
    let typing = Rc::new(Cell::new(false));
    let hold = {
        let events = events.clone();
        let (pointer, typing) = (pointer.clone(), typing.clone());
        move || events.hovered(pointer.get() || typing.get())
    };

    let frame = gtk::Box::new(gtk::Orientation::Vertical, 6);
    frame.add_css_class("background");
    frame.add_css_class("caminito-notification");
    frame.set_size_request(WIDTH, -1);

    let header = gtk::Box::new(gtk::Orientation::Horizontal, 6);
    if let Some(icon) = &bubble.app_icon {
        let icon = icon_image(icon);
        icon.set_pixel_size(APP_ICON_SIZE);
        header.append(&icon);
    }
    let app_name = gtk::Label::new(Some(&bubble.app_name));
    app_name.add_css_class("app-name");
    app_name.add_css_class("dim-label");
    app_name.set_xalign(0.0);
    app_name.set_hexpand(true);
    app_name.set_ellipsize(pango::EllipsizeMode::End);
    header.append(&app_name);
    if !bubble.earlier.is_empty() {
        let count = gtk::Label::new(Some(&(bubble.earlier.len() + 1).to_string()));
        count.add_css_class("count");
        header.append(&count);
    }
    frame.append(&header);

    let content = gtk::Box::new(gtk::Orientation::Horizontal, 10);
    let text = gtk::Box::new(gtk::Orientation::Vertical, 2);
    text.set_hexpand(true);
    let summary = text_label();
    summary.set_markup(&format!(
        "<b>{}</b>",
        glib::markup_escape_text(&bubble.summary)
    ));
    text.append(&summary);
    if !bubble.body.is_empty() {
        // The server made it markup that GtkLabel reads, its links with it.
        let body = text_label();
        body.set_markup(&bubble.body);
        text.append(&body);
    }
    content.append(&text);
    if let Some(image) = bubble.image.as_ref().and_then(image) {
        image.set_pixel_size(IMAGE_SIZE);
        image.set_valign(gtk::Align::Start);
        content.append(&image);
    }
    frame.append(&content);

    if let Some(progress) = bubble.progress {
        let bar = gtk::ProgressBar::new();
        bar.set_fraction(f64::from(progress) / 100.0);
        bar.set_margin_top(4);
        frame.append(&bar);
    }

    if !bubble.actions.is_empty() {
        let actions = gtk::Box::new(gtk::Orientation::Horizontal, 6);
        actions.set_homogeneous(true);
        actions.set_margin_top(4);
        for action in &bubble.actions {
            let button = gtk::Button::with_label(&action.label);
            let events = events.clone();
            let key = action.key.clone();
            button.connect_clicked(move |button| events.invoke(&key, token(button)));
            actions.append(&button);
        }
        frame.append(&actions);
    }

    if let Some(placeholder) = &bubble.reply {
        frame.append(&reply_field(placeholder, events, &typing, hold.clone()));
    }
    if !bubble.earlier.is_empty() {
        frame.append(&earlier_part(&bubble.earlier, open));
    }

    let close = gtk::Button::from_icon_name("window-close-symbolic");
    close.add_css_class("flat");
    close.add_css_class("circular");
    close.add_css_class("close");
    close.set_halign(gtk::Align::End);
    close.set_valign(gtk::Align::Start);
    close.set_visible(false);
    close.set_tooltip_text(Some("Close"));
    close.connect_clicked({
        let events = events.clone();
        move |_| events.dismiss()
    });

    if let Some(before) = time_line.parent().and_downcast::<gtk::Overlay>() {
        before.remove_overlay(time_line);
    }
    let overlay = gtk::Overlay::new();
    overlay.set_child(Some(&frame));
    overlay.add_overlay(time_line);
    overlay.add_overlay(&close);

    let motion = gtk::EventControllerMotion::new();
    motion.connect_enter({
        let close = close.clone();
        let pointer = pointer.clone();
        let hold = hold.clone();
        move |_, _, _| {
            close.set_visible(true);
            pointer.set(true);
            hold();
        }
    });
    motion.connect_leave({
        let close = close.clone();
        move |_| {
            close.set_visible(false);
            pointer.set(false);
            hold();
        }
    });
    overlay.add_controller(motion);

    // A click on the bubble, which its buttons and links claim for themselves.
    let click = gtk::GestureClick::new();
    click.set_button(gdk::BUTTON_PRIMARY);
    click.connect_released({
        let events = events.clone();
        move |gesture, _, _, _| {
            let token = gesture.widget().and_then(|widget| token(&widget));
            events.click(token);
        }
    });
    frame.add_controller(click);

    root.append(&overlay);
}

/// KDE's inline reply: a field with the app's placeholder and a send button, which Enter
/// presses too. An empty reply isn't sent. While the field has the keyboard, `typing` is set
/// and `hold` keeps the bubble's timeout waiting.
fn reply_field(
    placeholder: &str,
    events: &Events,
    typing: &Rc<Cell<bool>>,
    hold: impl Fn() + Clone + 'static,
) -> gtk::Box {
    let field = gtk::Box::new(gtk::Orientation::Horizontal, 6);
    field.set_margin_top(4);
    let entry = gtk::Entry::new();
    entry.set_placeholder_text(Some(placeholder));
    entry.set_hexpand(true);
    let send = gtk::Button::from_icon_name("document-send-symbolic");
    send.set_tooltip_text(Some("Send"));
    send.set_sensitive(false);
    entry.connect_changed({
        let send = send.clone();
        move |entry| send.set_sensitive(!entry.text().trim().is_empty())
    });
    let reply = {
        let events = events.clone();
        let entry = entry.clone();
        move || {
            let text = entry.text();
            if !text.trim().is_empty() {
                events.reply(&text);
            }
        }
    };
    entry.connect_activate({
        let reply = reply.clone();
        move |_| reply()
    });
    send.connect_clicked(move |_| reply());

    let focus = gtk::EventControllerFocus::new();
    focus.connect_enter({
        let typing = typing.clone();
        let hold = hold.clone();
        move |_| {
            typing.set(true);
            hold();
        }
    });
    focus.connect_leave({
        let typing = typing.clone();
        move |_| {
            typing.set(false);
            hold();
        }
    });
    entry.add_controller(focus);

    field.append(&entry);
    field.append(&send);
    field
}

/// A grouped bubble's notifications before its newest: a line each of the latest
/// [`DIGEST_LINES`] and a button that opens them all, or all of them and a button that closes
/// them again.
fn earlier_part(earlier: &[Earlier], open: &Rc<Cell<bool>>) -> gtk::Box {
    let part = gtk::Box::new(gtk::Orientation::Vertical, 4);
    part.add_css_class("digest");

    let digest = gtk::Box::new(gtk::Orientation::Vertical, 2);
    for notification in earlier.iter().take(DIGEST_LINES) {
        let line = gtk::Label::new(None);
        let summary = glib::markup_escape_text(&notification.summary);
        if notification.body.is_empty() {
            line.set_markup(&summary);
        } else {
            line.set_markup(&format!("{summary} · {}", one_line(&notification.body)));
        }
        line.set_xalign(0.0);
        line.set_ellipsize(pango::EllipsizeMode::End);
        line.set_single_line_mode(true);
        line.add_css_class("dim-label");
        digest.append(&line);
    }

    let all = gtk::Box::new(gtk::Orientation::Vertical, 6);
    for notification in earlier {
        let summary = text_label();
        summary.set_markup(&format!(
            "<b>{}</b>",
            glib::markup_escape_text(&notification.summary)
        ));
        all.append(&summary);
        if !notification.body.is_empty() {
            let body = text_label();
            body.set_markup(&notification.body);
            body.add_css_class("dim-label");
            all.append(&body);
        }
    }
    let scrolled = gtk::ScrolledWindow::builder()
        .hscrollbar_policy(gtk::PolicyType::Never)
        .propagate_natural_height(true)
        .child(&all)
        .build();
    let all = all.clone();

    let count = earlier.len() + 1;
    let more = if earlier.len() > DIGEST_LINES {
        format!("+{} more", earlier.len() - DIGEST_LINES)
    } else {
        format!("Show all {count}")
    };
    let toggle = gtk::Button::new();
    toggle.add_css_class("more");
    toggle.set_halign(gtk::Align::Start);
    part.append(&digest);
    part.append(&scrolled);
    part.append(&toggle);

    let show = {
        let (digest, scrolled, toggle) = (digest.clone(), scrolled.clone(), toggle.clone());
        move |opened: bool| {
            digest.set_visible(!opened);
            scrolled.set_visible(opened);
            toggle.set_label(if opened { "Show less" } else { &more });
            if opened {
                // A scrolled window asks only for its minimum, which is what it needs up to
                // half the output, so that the bubble grows to fit them until it scrolls.
                let (_, natural, _, _) = all.measure(gtk::Orientation::Vertical, WIDTH);
                let height = half_output(&scrolled).map_or(natural, |half| natural.min(half));
                scrolled.set_max_content_height(height);
                scrolled.set_min_content_height(height);
            }
        }
    };
    show(open.get());
    toggle.connect_clicked({
        let open = open.clone();
        move |_| {
            open.set(!open.get());
            show(open.get());
        }
    });
    part
}

/// `body`, markup, on one line: its line breaks as spaces.
fn one_line(body: &str) -> String {
    body.replace(['\n', '\r'], " ")
}

/// Half the height of the output `widget` is on, which an open grouped bubble scrolls past.
fn half_output(widget: &impl IsA<gtk::Widget>) -> Option<i32> {
    let surface = widget.native()?.surface()?;
    let monitor = widget.display().monitor_at_surface(&surface)?;
    Some(monitor.geometry().height() / 2)
}

fn text_label() -> gtk::Label {
    let label = gtk::Label::new(None);
    label.set_xalign(0.0);
    label.set_wrap(true);
    label.set_wrap_mode(pango::WrapMode::WordChar);
    label.set_max_width_chars(40);
    label
}

fn icon_image(icon: &Icon) -> gtk::Image {
    match icon {
        Icon::Name(name) => gtk::Image::from_icon_name(name),
        Icon::File(path) => gtk::Image::from_file(path),
    }
}

fn image(image: &Image) -> Option<gtk::Image> {
    match image {
        Image::Icon(icon) => Some(icon_image(icon)),
        Image::Pixels(pixels) => {
            let texture = texture(pixels)?;
            Some(gtk::Image::from_paintable(Some(&texture)))
        }
    }
}

/// The pixels as a texture. The server took only 8-bit RGB or RGBA with all their rows.
fn texture(pixels: &Pixels) -> Option<gdk::MemoryTexture> {
    let format = if pixels.has_alpha {
        gdk::MemoryFormat::R8g8b8a8
    } else {
        gdk::MemoryFormat::R8g8b8
    };
    Some(gdk::MemoryTexture::new(
        pixels.width,
        pixels.height,
        format,
        &pixels.data,
        usize::try_from(pixels.rowstride).ok()?,
    ))
}

/// An xdg-activation token for the app to bring its window forward with, which GDK asks
/// Caminito for with the click's serial.
fn token(widget: &impl IsA<gtk::Widget>) -> Option<String> {
    let context = widget.display().app_launch_context();
    context
        .startup_notify_id(None::<&gio::AppInfo>, &[])
        .map(String::from)
}
