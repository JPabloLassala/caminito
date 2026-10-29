//! The Notification bubbles (CMN-258), in GTK4: one layer surface in the namespace
//! `caminito-notification`, anchored to the corner the settings name (CMN-261), that holds the
//! bubbles on screen, newest nearest the corner. A new bubble moves them to the corner the
//! settings name now. Its exclusive zone is 0, so it sits under the panel, and
//! it names no output, so Caminito puts it on the output under the pointer as it maps. It's
//! hidden while there's no bubble, and maps again where the pointer is for the next one; a
//! bubble that opens while others show joins them on their output, as bubbles are on one
//! output at a time in v1 (the spec's "Not in v1").
//!
//! The server decides what's on screen; this draws it, from the GTK theme, and tells the server
//! what the user does through each bubble's [`Events`].

use std::cell::{Cell, RefCell};
use std::collections::HashMap;

use caminito_notifications::server::{Bubble, Bubbles, Events, Icon, Image, Pixels};
use caminito_notifications::settings::Corner;
use gtk::prelude::*;
use gtk::{gdk, gio, glib, pango};
use gtk4_layer_shell::{Edge, KeyboardMode, Layer, LayerShell};

/// The namespace of the bubbles' layer surface, which Caminito knows them by.
const NAMESPACE: &str = "caminito-notification";

/// The space between the corner and the bubbles, and between two bubbles.
const GAP: i32 = 10;
const WIDTH: i32 = 360;
const APP_ICON_SIZE: i32 = 16;
const IMAGE_SIZE: i32 = 48;

const CSS: &str = "
window.caminito-notifications { background: none; }
.caminito-notification {
  border-radius: 12px;
  border: 1px solid alpha(currentColor, 0.15);
  padding: 10px 12px;
}
.caminito-notification .app-name { font-size: smaller; }
.caminito-notification .close { min-width: 20px; min-height: 20px; padding: 2px; margin: 4px; }
";

/// The bubbles, in their layer surface.
pub(crate) struct GtkBubbles {
    window: gtk::Window,
    /// Whether the window is a layer surface, which only then has a corner.
    layer: bool,
    settings: gio::Settings,
    /// The corner the bubbles are in.
    corner: Cell<Corner>,
    /// The bubbles, newest first, which is nearest the corner.
    stack: gtk::Box,
    shown: RefCell<HashMap<u32, Shown>>,
}

/// A bubble on screen.
struct Shown {
    /// What's in the stack.
    root: gtk::Box,
    events: Events,
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
        window.set_child(Some(&stack));

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
            stack,
            shown: RefCell::default(),
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
        if !self.layer {
            return;
        }
        self.window.set_anchor(Edge::Top, corner.is_top());
        self.window.set_anchor(Edge::Bottom, !corner.is_top());
        self.window.set_anchor(Edge::Right, corner.is_right());
        self.window.set_anchor(Edge::Left, !corner.is_right());
    }

    /// Shows the window when it has a bubble, and hides it, shrunk, when it has none.
    fn present(&self) {
        if self.stack.first_child().is_some() {
            self.window.set_visible(true);
        } else {
            self.window.set_visible(false);
            // So that it maps again at the size of its next bubble.
            self.window.set_default_size(1, 1);
        }
    }
}

impl Bubbles for GtkBubbles {
    fn show(&self, id: u32, bubble: &Bubble, events: Events) {
        let root = gtk::Box::new(gtk::Orientation::Vertical, 0);
        fill(&root, bubble, &events);
        self.follow_corner();
        if self.corner.get().is_top() {
            self.stack.prepend(&root);
        } else {
            self.stack.append(&root);
        }
        self.shown.borrow_mut().insert(id, Shown { root, events });
        self.present();
    }

    fn update(&self, id: u32, bubble: &Bubble) {
        let shown = self.shown.borrow();
        let Some(shown) = shown.get(&id) else {
            return;
        };
        while let Some(child) = shown.root.first_child() {
            shown.root.remove(&child);
        }
        fill(&shown.root, bubble, &shown.events);
    }

    fn close(&self, id: u32) {
        let shown = self.shown.borrow_mut().remove(&id);
        if let Some(shown) = shown {
            self.stack.remove(&shown.root);
        }
        self.present();
    }
}

/// Puts what `bubble` shows into `root`: the app's icon and name on top, the summary in bold
/// and the body, the image on the right, the actions below, and a close button on hover.
fn fill(root: &gtk::Box, bubble: &Bubble, events: &Events) {
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

    let overlay = gtk::Overlay::new();
    overlay.set_child(Some(&frame));
    overlay.add_overlay(&close);

    let motion = gtk::EventControllerMotion::new();
    motion.connect_enter({
        let events = events.clone();
        let close = close.clone();
        move |_, _, _| {
            close.set_visible(true);
            events.hovered(true);
        }
    });
    motion.connect_leave({
        let events = events.clone();
        let close = close.clone();
        move |_| {
            close.set_visible(false);
            events.hovered(false);
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
