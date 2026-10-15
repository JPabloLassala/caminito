//! The window: variant B of the prototype (spec). A sidebar with a small picture of each
//! output, "Same on all outputs" and the five kinds, and beside it the selected kind's
//! controls for the selected output. A click writes the store at once, and the drawer
//! draws it.

use std::cell::{Cell, RefCell};
use std::path::{Path, PathBuf};
use std::rc::{Rc, Weak};

use caminito_wallpaper::daily::{self, Cache, Kept, Source};
use caminito_wallpaper::dynamic::Dynamic;
use caminito_wallpaper::grid::{self, DynamicGroup, Group, Installed, Roots};
use caminito_wallpaper::heic::{self, DecodeError, Schedule};
use caminito_wallpaper::monitors::Monitors;
use caminito_wallpaper::page::{Output, Page};
use caminito_wallpaper::slideshow::Interval;
use caminito_wallpaper::sun::sky_now;
use caminito_wallpaper::{
    Colour, GioStore, Kind, Memory, OutputSettings, Positioning, Set, Store, place,
};
use gtk::prelude::*;
use gtk::{gdk, gio, glib};

use crate::decoder::{Decoded, Decoder};

/// The log domain of everything the dialog logs.
const DOMAIN: &str = "caminito-wallpaper";

/// The width of every output's picture in the sidebar.
const PREVIEW_WIDTH: i32 = 200;
/// A thumbnail of the grid.
const THUMBNAIL: (i32, i32) = (160, 100);
/// The space between the window's parts and around them.
const SPACING: i32 = 12;

/// The swatches of the Colour pane: MATE's blue, which is the schema's default, then
/// greys and a few quiet colours.
const SWATCHES: [&str; 10] = [
    "#5891bc", "#000000", "#2e3436", "#555753", "#888a85", "#3c6e47", "#7a5230", "#6b3a5b",
    "#a33b3b", "#c4a000",
];

/// What a kind is called in the list, and its icon.
fn kind_label(kind: Kind) -> (&'static str, &'static str) {
    match kind {
        Kind::Picture => ("Picture", "image-x-generic"),
        Kind::Colour => ("Colour", "applications-graphics"),
        Kind::Slideshow => ("Slideshow", "media-playlist-shuffle"),
        Kind::PictureOfTheDay => ("Picture of the day", "web-browser"),
        Kind::Dynamic => ("Dynamic", "weather-few-clouds"),
    }
}

/// What the positionings are called, in [`Positioning::ALL`]'s order.
const POSITIONINGS: [&str; 5] = ["Fill", "Fit", "Stretch", "Centre", "Tile"];

/// What the Slideshow's intervals are called, in [`Interval::ALL`]'s order.
const INTERVALS: [&str; 4] = ["5 minutes", "15 minutes", "1 hour", "1 day"];

/// How often the window is rendered again without a change of the settings, so that the
/// sidebar's Slideshow or Dynamic follows the desktop's, and the Slideshow's count its
/// folder, within this.
const RENDER_TICK: u32 = 60;

const CSS: &str = "
.caminito-wallpaper-output { padding: 4px; border-radius: 6px; }
.caminito-wallpaper-output.selected { box-shadow: inset 0 0 0 2px @theme_selected_bg_color; }
.caminito-wallpaper-thumbnail { padding: 3px; border-radius: 4px; }
.caminito-wallpaper-thumbnail.current { box-shadow: inset 0 0 0 3px @theme_selected_bg_color; }
";

struct Ui {
    /// The state itself, for the handlers, which hold it weakly since it holds them.
    me: Weak<Ui>,
    store: RefCell<Box<dyn Store>>,
    page: RefCell<Page>,
    /// Set while the widgets are given the page's values, so that their handlers don't
    /// take them for the user's.
    rendering: Cell<bool>,
    decoder: Rc<Decoder>,
    /// MATE's and Plasma's pictures, scanned as the window opens.
    installed: Installed,
    /// The groups the grid shows now.
    shown_groups: RefCell<Vec<Group>>,
    /// The grid's thumbnails, by their picture.
    thumbnails: RefCell<Vec<(PathBuf, gtk::FlowBoxChild)>>,
    /// The groups the Dynamic grid shows now.
    shown_dynamic_groups: RefCell<Vec<DynamicGroup>>,
    /// The Dynamic grid's thumbnails, by their wallpaper's path.
    dynamic_thumbnails: RefCell<Vec<(PathBuf, gtk::FlowBoxChild)>>,
    /// Why the last file given to the Dynamic pane's "Add…" wasn't added, until another is.
    refused: RefCell<Option<String>>,
    /// The HEICs whose pictures are being decoded for their thumbnails (217).
    decodes: heic::Decodes,
    /// Caminito's D-Bus interface, for the primary output.
    caminito: Option<gio::DBusProxy>,
    /// Where the drawer keeps each source's picture of the day (214).
    cache: Cache,
    /// Tells of the drawer's new pictures of the day, while the window is open.
    _cache_monitor: Option<gio::FileMonitor>,
    /// What "Keep this picture" did last, until the picture or the source changes.
    kept: RefCell<Option<(PathBuf, String)>>,
    /// Whether the window is the demo, which keeps no picture outside it.
    demo: bool,

    window: gtk::ApplicationWindow,
    previews: gtk::Box,
    same: gtk::Switch,
    same_row: gtk::Box,
    kinds: gtk::ListBox,
    panes: gtk::Stack,
    grid: gtk::Box,
    scrolled: gtk::ScrolledWindow,
    positioning: gtk::DropDown,
    colour: gtk::ColorDialogButton,
    dynamic_grid: gtk::Box,
    dynamic_scrolled: gtk::ScrolledWindow,
    /// Whether the selected dynamic wallpaper follows the sun or fixed times.
    dynamic_line: gtk::Label,
    dynamic_positioning: gtk::DropDown,
    /// The Slideshow's folder, named on the button that chooses it.
    slideshow_folder: gtk::Button,
    interval: gtk::DropDown,
    in_order: gtk::CheckButton,
    /// How many pictures the Slideshow's folder has.
    slideshow_count: gtk::Label,
    slideshow_positioning: gtk::DropDown,
    sources: gtk::ListBox,
    daily_title: gtk::Label,
    daily_credit: gtk::Label,
    keep: gtk::Button,
    keep_line: gtk::Label,
    daily_positioning: gtk::DropDown,
}

/// Opens the window, on the store and the compositor's outputs or, for `demo`, on made-up
/// outputs and a store in memory.
pub(crate) fn open(application: &gtk::Application, demo: bool) {
    let window = gtk::ApplicationWindow::builder()
        .application(application)
        .title("Wallpaper")
        .icon_name("preferences-desktop-wallpaper")
        .default_width(960)
        .default_height(640)
        .build();
    close_on_escape(&window);
    if let Some(display) = gdk::Display::default() {
        let provider = gtk::CssProvider::new();
        provider.load_from_string(CSS);
        gtk::style_context_add_provider_for_display(
            &display,
            &provider,
            gtk::STYLE_PROVIDER_PRIORITY_APPLICATION,
        );
    }

    // The window the store's changes render again, once it's made.
    let changed: Rc<RefCell<Weak<Ui>>> = Rc::default();
    let (store, monitors, caminito): (Box<dyn Store>, _, _) = if demo {
        (Box::new(demo_store()), None, None)
    } else {
        let store = match GioStore::open(DOMAIN, watch(&changed)) {
            Ok(store) => store,
            Err(err) => {
                glib::g_critical!(DOMAIN, "can't open the settings: {err}");
                window.set_child(Some(&message(&format!(
                    "The wallpaper's settings can't be opened: {err}."
                ))));
                window.present();
                return;
            }
        };
        let monitors = match Monitors::connect() {
            Ok(monitors) => Some(monitors),
            Err(err) => {
                glib::g_warning!(
                    DOMAIN,
                    "can't read the outputs, so every output is set at once: {err}"
                );
                None
            }
        };
        (Box::new(store), monitors, caminito_proxy())
    };

    let ui = build(window, store, caminito, demo, &changed);
    *changed.borrow_mut() = Rc::downgrade(&ui);
    let outputs = match &monitors {
        Some(monitors) => monitors.outputs(),
        None if demo => demo_outputs(),
        None => Vec::new(),
    };
    ui.set_outputs(outputs);
    if let Some(monitors) = monitors {
        follow(&ui, monitors);
    }
    {
        let ui = Rc::downgrade(&ui);
        glib::timeout_add_seconds_local(RENDER_TICK, move || {
            let Some(ui) = ui.upgrade() else {
                return glib::ControlFlow::Break;
            };
            ui.render();
            glib::ControlFlow::Continue
        });
    }
    // The handlers hold the state weakly, so the window holds it, until it's disposed of with
    // its handlers.
    ui.window.connect_close_request({
        let ui = ui.clone();
        move |_| {
            let _ = &ui;
            glib::Propagation::Proceed
        }
    });
    ui.render();
    ui.window.present();
}

/// A label in the middle of the window, for when there's nothing else to show.
fn message(text: &str) -> gtk::Label {
    gtk::Label::builder()
        .label(text)
        .wrap(true)
        .margin_top(SPACING * 2)
        .margin_bottom(SPACING * 2)
        .margin_start(SPACING * 2)
        .margin_end(SPACING * 2)
        .build()
}

fn close_on_escape(window: &gtk::ApplicationWindow) {
    let keys = gtk::EventControllerKey::new();
    keys.connect_key_pressed({
        let window = window.downgrade();
        move |_, key, _, _| {
            if key != gdk::Key::Escape {
                return glib::Propagation::Proceed;
            }
            if let Some(window) = window.upgrade() {
                window.close();
            }
            glib::Propagation::Stop
        }
    });
    window.add_controller(keys);
}

/// The demo's store: every output's wallpaper the schema's colour.
fn demo_store() -> Memory {
    let mut store = Memory::default();
    let settings = OutputSettings {
        colour: Colour::parse(SWATCHES[0]).unwrap_or_default(),
        ..OutputSettings::default()
    };
    store.write(&Set::All, &settings);
    store
}

/// The demo's outputs: a 4K monitor, and a laptop's panel to its right.
fn demo_outputs() -> Vec<Output> {
    vec![
        Output::new("DP-1", "DEL", "U2720Q", "DEMO1", (0, 0), (3840, 2160)),
        Output::new("eDP-1", "BOE", "0x0BCA", "", (3840, 300), (2560, 1600)),
    ]
}

/// `org.mate.Caminito`, or `None` without a session bus or a Caminito.
fn caminito_proxy() -> Option<gio::DBusProxy> {
    gio::DBusProxy::for_bus_sync(
        gio::BusType::Session,
        gio::DBusProxyFlags::DO_NOT_AUTO_START,
        None,
        "org.mate.Caminito",
        "/org/mate/Caminito",
        "org.mate.Caminito",
        None::<&gio::Cancellable>,
    )
    .ok()
}

/// Rebuilds the page as the outputs change, keeping the output selected.
fn follow(ui: &Rc<Ui>, monitors: Monitors) {
    let fd = monitors.fd();
    let monitors = RefCell::new(monitors);
    let ui = Rc::downgrade(ui);
    glib_unix::unix_fd_add_local(
        fd,
        glib::IOCondition::IN | glib::IOCondition::ERR | glib::IOCondition::HUP,
        move |_, _| {
            let Some(ui) = ui.upgrade() else {
                return glib::ControlFlow::Break;
            };
            match monitors.borrow_mut().dispatch() {
                Ok(false) => glib::ControlFlow::Continue,
                Ok(true) => {
                    ui.set_outputs(monitors.borrow().outputs());
                    ui.render();
                    glib::ControlFlow::Continue
                }
                Err(err) => {
                    glib::g_warning!(DOMAIN, "can't follow the outputs: {err}");
                    glib::ControlFlow::Break
                }
            }
        },
    );
}

/// What the store is given for each `Settings` it reads: renders the window again when one
/// changes, as by `gsettings` or by the window itself, once per turn of the loop.
fn watch(ui: &Rc<RefCell<Weak<Ui>>>) -> impl Fn(&gio::Settings) + 'static {
    let pending = Rc::new(Cell::new(false));
    let ui = ui.clone();
    move |settings| {
        let pending = pending.clone();
        let ui = ui.clone();
        settings.connect_changed(None, move |_, _| {
            if pending.replace(true) {
                return;
            }
            let pending = pending.clone();
            let ui = ui.clone();
            glib::idle_add_local_once(move || {
                pending.set(false);
                if let Some(ui) = ui.borrow().upgrade() {
                    ui.render();
                }
            });
        });
    }
}

/// Makes the window's widgets, and connects them.
fn build(
    window: gtk::ApplicationWindow,
    store: Box<dyn Store>,
    caminito: Option<gio::DBusProxy>,
    demo: bool,
    changed: &Rc<RefCell<Weak<Ui>>>,
) -> Rc<Ui> {
    let body = gtk::Box::new(gtk::Orientation::Horizontal, 0);
    window.set_child(Some(&body));

    let sidebar = gtk::Box::builder()
        .orientation(gtk::Orientation::Vertical)
        .spacing(SPACING)
        .margin_top(SPACING)
        .margin_bottom(SPACING)
        .margin_start(SPACING)
        .margin_end(SPACING)
        .width_request(PREVIEW_WIDTH + 2 * SPACING)
        .build();
    let previews = gtk::Box::new(gtk::Orientation::Vertical, SPACING / 2);
    let sidebar_scroll = gtk::ScrolledWindow::builder()
        .hscrollbar_policy(gtk::PolicyType::Never)
        .propagate_natural_height(true)
        .vexpand(false)
        .child(&previews)
        .build();
    sidebar.append(&sidebar_scroll);
    let same_row = gtk::Box::new(gtk::Orientation::Horizontal, SPACING);
    let same_label = gtk::Label::builder()
        .label("Same on all outputs")
        .xalign(0.0)
        .hexpand(true)
        .build();
    let same = gtk::Switch::builder().valign(gtk::Align::Center).build();
    same_row.append(&same_label);
    same_row.append(&same);
    sidebar.append(&same_row);
    let kinds = gtk::ListBox::builder()
        .selection_mode(gtk::SelectionMode::Single)
        .css_classes(["navigation-sidebar"])
        .build();
    for kind in Kind::ALL {
        let (label, icon) = kind_label(kind);
        let row = gtk::Box::new(gtk::Orientation::Horizontal, SPACING);
        row.append(&gtk::Image::from_icon_name(icon));
        row.append(&gtk::Label::builder().label(label).xalign(0.0).build());
        kinds.append(&row);
    }
    sidebar.append(&kinds);
    body.append(&sidebar);
    body.append(&gtk::Separator::new(gtk::Orientation::Vertical));

    let panes = gtk::Stack::builder().hexpand(true).vexpand(true).build();
    body.append(&panes);

    let picture_pane = gtk::Box::new(gtk::Orientation::Vertical, 0);
    let grid = gtk::Box::builder()
        .orientation(gtk::Orientation::Vertical)
        .spacing(SPACING / 2)
        .margin_top(SPACING)
        .margin_bottom(SPACING)
        .margin_start(SPACING)
        .margin_end(SPACING)
        .build();
    let scrolled = gtk::ScrolledWindow::builder()
        .hscrollbar_policy(gtk::PolicyType::Never)
        .vexpand(true)
        .child(&grid)
        .build();
    picture_pane.append(&scrolled);
    picture_pane.append(&gtk::Separator::new(gtk::Orientation::Horizontal));
    let bar = gtk::Box::builder()
        .spacing(SPACING / 2)
        .margin_top(SPACING)
        .margin_bottom(SPACING)
        .margin_start(SPACING)
        .margin_end(SPACING)
        .build();
    let add_picture = gtk::Button::with_label("Add picture…");
    let add_folder = gtk::Button::with_label("Add folder…");
    bar.append(&add_picture);
    bar.append(&add_folder);
    bar.append(
        &gtk::Label::builder()
            .label("Positioning")
            .hexpand(true)
            .xalign(1.0)
            .build(),
    );
    let positioning = gtk::DropDown::from_strings(&POSITIONINGS);
    bar.append(&positioning);
    picture_pane.append(&bar);
    panes.add_named(&picture_pane, Some("picture"));

    let colour_pane = gtk::Box::builder()
        .orientation(gtk::Orientation::Vertical)
        .spacing(SPACING)
        .margin_top(SPACING)
        .margin_bottom(SPACING)
        .margin_start(SPACING)
        .margin_end(SPACING)
        .build();
    colour_pane.append(&heading("Colours"));
    let swatches = gtk::FlowBox::builder()
        .selection_mode(gtk::SelectionMode::None)
        .max_children_per_line(10)
        .column_spacing(SPACING as u32 / 2)
        .row_spacing(SPACING as u32 / 2)
        .homogeneous(true)
        .build();
    colour_pane.append(&swatches);
    let custom = gtk::Box::new(gtk::Orientation::Horizontal, SPACING);
    custom.append(&gtk::Label::new(Some("Another colour")));
    let dialog = gtk::ColorDialog::builder().with_alpha(false).build();
    let colour = gtk::ColorDialogButton::new(Some(dialog));
    custom.append(&colour);
    colour_pane.append(&custom);
    panes.add_named(&colour_pane, Some("colour"));

    let dynamic_pane = gtk::Box::new(gtk::Orientation::Vertical, 0);
    let dynamic_grid = gtk::Box::builder()
        .orientation(gtk::Orientation::Vertical)
        .spacing(SPACING / 2)
        .margin_top(SPACING)
        .margin_bottom(SPACING)
        .margin_start(SPACING)
        .margin_end(SPACING)
        .build();
    let dynamic_scrolled = gtk::ScrolledWindow::builder()
        .hscrollbar_policy(gtk::PolicyType::Never)
        .vexpand(true)
        .child(&dynamic_grid)
        .build();
    dynamic_pane.append(&dynamic_scrolled);
    dynamic_pane.append(&gtk::Separator::new(gtk::Orientation::Horizontal));
    let dynamic_bar = gtk::Box::builder()
        .spacing(SPACING / 2)
        .margin_top(SPACING)
        .margin_bottom(SPACING)
        .margin_start(SPACING)
        .margin_end(SPACING)
        .build();
    let add_dynamic = gtk::Button::with_label("Add…");
    dynamic_bar.append(&add_dynamic);
    let dynamic_line = gtk::Label::builder()
        .hexpand(true)
        .xalign(0.0)
        .wrap(true)
        .margin_start(SPACING / 2)
        .css_classes(["dim-label"])
        .build();
    dynamic_bar.append(&dynamic_line);
    dynamic_bar.append(&gtk::Label::new(Some("Positioning")));
    let dynamic_positioning = gtk::DropDown::from_strings(&POSITIONINGS);
    dynamic_bar.append(&dynamic_positioning);
    dynamic_pane.append(&dynamic_bar);
    panes.add_named(&dynamic_pane, Some("dynamic"));

    let slideshow_pane = gtk::Box::builder()
        .orientation(gtk::Orientation::Vertical)
        .spacing(SPACING)
        .margin_top(SPACING)
        .margin_bottom(SPACING)
        .margin_start(SPACING)
        .margin_end(SPACING)
        .build();
    slideshow_pane.append(&heading("Folder"));
    let slideshow_folder = gtk::Button::builder()
        .halign(gtk::Align::Start)
        .tooltip_text("Choose the folder of pictures to show")
        .build();
    slideshow_pane.append(&slideshow_folder);
    let slideshow_count = gtk::Label::builder()
        .xalign(0.0)
        .wrap(true)
        .css_classes(["dim-label"])
        .build();
    slideshow_pane.append(&slideshow_count);
    let interval_row = gtk::Box::new(gtk::Orientation::Horizontal, SPACING);
    interval_row.append(&gtk::Label::new(Some("Change every")));
    let interval = gtk::DropDown::from_strings(&INTERVALS);
    interval_row.append(&interval);
    slideshow_pane.append(&interval_row);
    let in_order = gtk::CheckButton::with_label("In order, not at random");
    slideshow_pane.append(&in_order);
    let slideshow_positioning_row = gtk::Box::new(gtk::Orientation::Horizontal, SPACING);
    slideshow_positioning_row.append(&gtk::Label::new(Some("Positioning")));
    let slideshow_positioning = gtk::DropDown::from_strings(&POSITIONINGS);
    slideshow_positioning_row.append(&slideshow_positioning);
    slideshow_pane.append(&slideshow_positioning_row);
    panes.add_named(&slideshow_pane, Some("slideshow"));

    let daily_pane = gtk::Box::new(gtk::Orientation::Vertical, 0);
    let daily_body = gtk::Box::builder()
        .orientation(gtk::Orientation::Vertical)
        .spacing(SPACING)
        .margin_top(SPACING)
        .margin_bottom(SPACING)
        .margin_start(SPACING)
        .margin_end(SPACING)
        .vexpand(true)
        .build();
    daily_body.append(&heading("Source"));
    let sources = gtk::ListBox::builder()
        .selection_mode(gtk::SelectionMode::Single)
        .css_classes(["boxed-list"])
        .build();
    for source in Source::ALL {
        let row = gtk::Box::builder()
            .orientation(gtk::Orientation::Vertical)
            .spacing(2)
            .margin_top(SPACING / 2)
            .margin_bottom(SPACING / 2)
            .margin_start(SPACING / 2)
            .margin_end(SPACING / 2)
            .build();
        row.append(
            &gtk::Label::builder()
                .label(source.name())
                .xalign(0.0)
                .build(),
        );
        row.append(
            &gtk::Label::builder()
                .label(source.line())
                .xalign(0.0)
                .wrap(true)
                .css_classes(["dim-label"])
                .build(),
        );
        sources.append(&row);
    }
    daily_body.append(&sources);
    daily_body.append(&heading("Today"));
    let daily_title = gtk::Label::builder()
        .xalign(0.0)
        .wrap(true)
        .selectable(true)
        .build();
    daily_body.append(&daily_title);
    let daily_credit = gtk::Label::builder()
        .xalign(0.0)
        .wrap(true)
        .selectable(true)
        .css_classes(["dim-label"])
        .build();
    daily_body.append(&daily_credit);
    daily_pane.append(&daily_body);
    daily_pane.append(&gtk::Separator::new(gtk::Orientation::Horizontal));
    let daily_bar = gtk::Box::builder()
        .spacing(SPACING / 2)
        .margin_top(SPACING)
        .margin_bottom(SPACING)
        .margin_start(SPACING)
        .margin_end(SPACING)
        .build();
    let keep = gtk::Button::with_label("Keep this picture");
    daily_bar.append(&keep);
    let keep_line = gtk::Label::builder()
        .hexpand(true)
        .xalign(0.0)
        .wrap(true)
        .margin_start(SPACING / 2)
        .css_classes(["dim-label"])
        .build();
    daily_bar.append(&keep_line);
    daily_bar.append(&gtk::Label::new(Some("Positioning")));
    let daily_positioning = gtk::DropDown::from_strings(&POSITIONINGS);
    daily_bar.append(&daily_positioning);
    daily_pane.append(&daily_bar);
    panes.add_named(&daily_pane, Some("picture-of-the-day"));

    let cache = Cache::user();
    let cache_monitor = watch_cache(&cache, changed);

    let ui = Rc::new_cyclic(|me| Ui {
        me: me.clone(),
        store: RefCell::new(store),
        page: RefCell::new(Page::new(&mut Memory::default(), Vec::new(), None)),
        rendering: Cell::new(false),
        decoder: Rc::new(Decoder::default()),
        installed: Installed::scan(&Roots::default()),
        shown_groups: RefCell::new(Vec::new()),
        thumbnails: RefCell::new(Vec::new()),
        shown_dynamic_groups: RefCell::new(Vec::new()),
        dynamic_thumbnails: RefCell::new(Vec::new()),
        refused: RefCell::new(None),
        decodes: heic::Decodes::new(DOMAIN),
        caminito,
        cache,
        _cache_monitor: cache_monitor,
        kept: RefCell::new(None),
        demo,
        window,
        previews,
        same,
        same_row,
        kinds,
        panes,
        grid,
        scrolled,
        positioning,
        colour,
        dynamic_grid,
        dynamic_scrolled,
        dynamic_line,
        dynamic_positioning,
        slideshow_folder,
        interval,
        in_order,
        slideshow_count,
        slideshow_positioning,
        sources,
        daily_title,
        daily_credit,
        keep,
        keep_line,
        daily_positioning,
    });
    ui.connect(&add_picture, &add_folder, &add_dynamic, &swatches);
    ui
}

/// Renders the window again when the drawer keeps a new picture of the day, once per turn
/// of the loop. The cache's folder is made if it isn't there, so there's a folder to watch.
fn watch_cache(cache: &Cache, ui: &Rc<RefCell<Weak<Ui>>>) -> Option<gio::FileMonitor> {
    if let Err(err) = std::fs::create_dir_all(cache.root()) {
        glib::g_debug!(DOMAIN, "can't make {}: {err}", cache.root().display());
    }
    let monitor = gio::File::for_path(cache.root())
        .monitor_directory(
            gio::FileMonitorFlags::WATCH_MOVES,
            None::<&gio::Cancellable>,
        )
        .inspect_err(|err| {
            glib::g_debug!(DOMAIN, "can't watch the pictures of the day: {err}");
        })
        .ok()?;
    let pending = Rc::new(Cell::new(false));
    let ui = ui.clone();
    monitor.connect_changed(move |_, _, _, _| {
        if pending.replace(true) {
            return;
        }
        let pending = pending.clone();
        let ui = ui.clone();
        glib::idle_add_local_once(move || {
            pending.set(false);
            if let Some(ui) = ui.borrow().upgrade() {
                ui.render();
            }
        });
    });
    Some(monitor)
}

/// Where "Keep this picture" copies a picture of the day: `~/Pictures/Wallpapers`, the
/// pictures folder being the user's own if they named one.
fn wallpapers_folder() -> PathBuf {
    glib::user_special_dir(glib::UserDirectory::Pictures)
        .unwrap_or_else(|| glib::home_dir().join("Pictures"))
        .join("Wallpapers")
}

/// A heading of the grid or a pane.
fn heading(text: &str) -> gtk::Label {
    gtk::Label::builder()
        .label(text)
        .xalign(0.0)
        .css_classes(["heading"])
        .attributes(&{
            let attributes = gtk::pango::AttrList::new();
            attributes.insert(gtk::pango::AttrInt::new_weight(gtk::pango::Weight::Bold));
            attributes
        })
        .build()
}

/// Whether `dynamic` is a HEIC that can't be shown, its pictures not decoded and libheif
/// not installed to decode them: greyed out in the grid.
fn unshowable(dynamic: &Dynamic) -> bool {
    !dynamic.ready() && heic::decoder().is_none()
}

/// What a dynamic wallpaper is called in its grid: a Plasma package's folder, or a timed
/// XML's file without `.xml`, or its folder's name for a file named only `background`, as
/// `cosmos/background-1.xml` is.
fn dynamic_name(path: &Path) -> String {
    let name = |path: Option<&Path>| {
        path.and_then(Path::file_stem)
            .unwrap_or_default()
            .to_string_lossy()
            .into_owned()
    };
    let own = name(Some(path));
    if own.starts_with("background") {
        name(path.parent())
    } else {
        own
    }
}

/// A colour as GDK has it.
fn rgba(colour: Colour) -> gdk::RGBA {
    gdk::RGBA::new(
        f32::from(colour.red) / 255.0,
        f32::from(colour.green) / 255.0,
        f32::from(colour.blue) / 255.0,
        1.0,
    )
}

/// A colour of GDK's, as the store has it.
fn colour_of(rgba: &gdk::RGBA) -> Colour {
    let channel = |value: f32| (value.clamp(0.0, 1.0) * 255.0).round() as u8;
    Colour {
        red: channel(rgba.red()),
        green: channel(rgba.green()),
        blue: channel(rgba.blue()),
    }
}

/// Fills a rectangle with `colour`.
fn fill(cr: &gtk::cairo::Context, colour: Colour, width: f64, height: f64) {
    cr.set_source_rgb(
        f64::from(colour.red) / 255.0,
        f64::from(colour.green) / 255.0,
        f64::from(colour.blue) / 255.0,
    );
    cr.rectangle(0.0, 0.0, width, height);
    let _ = cr.fill();
}

impl Ui {
    /// Calls `act` with the state, if the window is still there.
    fn handler<A: 'static>(&self, act: impl Fn(&Ui, A) + 'static) -> impl Fn(A) + 'static {
        let me = self.me.clone();
        move |argument| {
            if let Some(ui) = me.upgrade() {
                act(&ui, argument);
            }
        }
    }

    /// Connects the widgets' handlers.
    fn connect(
        &self,
        add_picture: &gtk::Button,
        add_folder: &gtk::Button,
        add_dynamic: &gtk::Button,
        swatches: &gtk::FlowBox,
    ) {
        let on = self.handler(|ui, same: bool| {
            ui.act(|page, store| page.set_same_on_all_outputs(store, same));
        });
        self.same
            .connect_active_notify(move |switch| on(switch.is_active()));

        let on = self.handler(|ui, index: usize| {
            if let Some(&kind) = Kind::ALL.get(index) {
                ui.act(|page, store| page.show_kind(store, kind));
            }
        });
        self.kinds.connect_row_selected(move |_, row| {
            if let Some(row) = row {
                on(row.index() as usize);
            }
        });

        let on = self.handler(|ui, index: u32| {
            if let Some(&positioning) = Positioning::ALL.get(index as usize) {
                ui.act(|page, store| page.choose_positioning(store, positioning));
            }
        });
        self.positioning
            .connect_selected_notify(move |dropdown| on(dropdown.selected()));
        let on = self.handler(|ui, index: u32| {
            if let Some(&positioning) = Positioning::ALL.get(index as usize) {
                ui.act(|page, store| page.choose_positioning(store, positioning));
            }
        });
        self.dynamic_positioning
            .connect_selected_notify(move |dropdown| on(dropdown.selected()));
        let on = self.handler(|ui, index: u32| {
            if let Some(&positioning) = Positioning::ALL.get(index as usize) {
                ui.act(|page, store| page.choose_positioning(store, positioning));
            }
        });
        self.slideshow_positioning
            .connect_selected_notify(move |dropdown| on(dropdown.selected()));

        let on = self.handler(|ui, ()| ui.choose_slideshow());
        self.slideshow_folder.connect_clicked(move |_| on(()));
        let on = self.handler(|ui, index: u32| {
            if let Some(&interval) = Interval::ALL.get(index as usize) {
                ui.act(|page, store| page.choose_interval(store, interval));
            }
        });
        self.interval
            .connect_selected_notify(move |dropdown| on(dropdown.selected()));
        let on = self.handler(|ui, in_order: bool| {
            ui.act(|page, store| page.choose_in_order(store, in_order));
        });
        self.in_order
            .connect_active_notify(move |check| on(check.is_active()));

        let on = self.handler(|ui, index: u32| {
            if let Some(&positioning) = Positioning::ALL.get(index as usize) {
                ui.act(|page, store| page.choose_positioning(store, positioning));
            }
        });
        self.daily_positioning
            .connect_selected_notify(move |dropdown| on(dropdown.selected()));

        let on = self.handler(|ui, index: usize| {
            if let Some(&source) = Source::ALL.get(index) {
                ui.act(|page, store| page.choose_source(store, source));
            }
        });
        self.sources.connect_row_selected(move |_, row| {
            if let Some(row) = row {
                on(row.index() as usize);
            }
        });
        let on = self.handler(|ui, ()| ui.keep_picture());
        self.keep.connect_clicked(move |_| on(()));

        let on = self.handler(|ui, colour: Colour| {
            ui.act(|page, store| page.choose_colour(store, colour));
        });
        self.colour
            .connect_rgba_notify(move |button| on(colour_of(&button.rgba())));

        for text in SWATCHES {
            let Some(colour) = Colour::parse(text) else {
                continue;
            };
            let swatch = gtk::DrawingArea::builder()
                .content_width(40)
                .content_height(28)
                .build();
            swatch.set_draw_func(move |_, cr, w, h| fill(cr, colour, f64::from(w), f64::from(h)));
            let button = gtk::Button::builder()
                .child(&swatch)
                .tooltip_text(text)
                .build();
            let on = self.handler(move |ui, ()| {
                ui.act(|page, store| page.choose_colour(store, colour));
            });
            button.connect_clicked(move |_| on(()));
            swatches.append(&button);
        }

        let on = self.handler(|ui, ()| ui.add_picture());
        add_picture.connect_clicked(move |_| on(()));
        let on = self.handler(|ui, ()| ui.add_folder());
        add_folder.connect_clicked(move |_| on(()));
        let on = self.handler(|ui, ()| ui.add_dynamic());
        add_dynamic.connect_clicked(move |_| on(()));
    }

    /// Copies the selected output's picture of the day to `~/Pictures/Wallpapers`, and adds
    /// it to the Picture grid, or says why not.
    fn keep_picture(&self) {
        let current = self.page.borrow().current(self.store.borrow().as_ref());
        let Some(kept) = self.cache.latest(current.source) else {
            return;
        };
        let line = if self.demo {
            "The demo keeps no picture.".to_owned()
        } else {
            match daily::keep_copy(&kept, &wallpapers_folder()) {
                Ok(copy) => {
                    let file = copy.to_string_lossy();
                    {
                        let mut store = self.store.borrow_mut();
                        self.page.borrow_mut().keep_picture(store.as_mut(), &file);
                    }
                    format!(
                        "Kept as {}, under Your pictures.",
                        copy.file_name().unwrap_or_default().to_string_lossy()
                    )
                }
                Err(err) => {
                    glib::g_warning!(DOMAIN, "can't keep {}: {err}", kept.picture.display());
                    format!("It can't be kept: {err}.")
                }
            }
        };
        *self.kept.borrow_mut() = Some((kept.picture, line));
        self.render();
    }

    /// Asks for a timed XML or a HEIC, and adds it and makes it the selected output's if it
    /// is one, or says why not. A HEIC added without libheif is added, greyed out, and not
    /// made the output's.
    fn add_dynamic(&self) {
        let filter = gtk::FileFilter::new();
        filter.set_name(Some("Dynamic wallpapers"));
        for suffix in ["xml", "heic", "heif"] {
            filter.add_suffix(suffix);
        }
        let filters = gio::ListStore::new::<gtk::FileFilter>();
        filters.append(&filter);
        let dialog = gtk::FileDialog::builder()
            .title("Add Dynamic Wallpaper")
            .modal(true)
            .filters(&filters)
            .default_filter(&filter)
            .build();
        let on = self.handler(|ui, path: PathBuf| match Dynamic::read(&path) {
            Ok(dynamic @ (Dynamic::Timed(_) | Dynamic::Heic(_))) => {
                *ui.refused.borrow_mut() = None;
                let file = path.to_string_lossy();
                if unshowable(&dynamic) {
                    ui.act(|page, store| page.keep_dynamic(store, &file));
                } else {
                    ui.act(|page, store| page.add_dynamic(store, &file));
                }
            }
            Ok(Dynamic::Pair { .. }) => {}
            Err(err) => {
                glib::g_debug!(DOMAIN, "can't add {}: {err}", path.display());
                *ui.refused.borrow_mut() = Some(format!(
                    "{} wasn't added: {err}.",
                    path.file_name().unwrap_or_default().to_string_lossy()
                ));
                ui.render();
            }
        });
        dialog.open(
            Some(&self.window),
            None::<&gio::Cancellable>,
            move |result| {
                if let Some(path) = result.ok().and_then(|file| file.path()) {
                    on(path);
                }
            },
        );
    }

    /// Asks for a folder, and makes its pictures the selected output's Slideshow.
    fn choose_slideshow(&self) {
        let dialog = gtk::FileDialog::builder()
            .title("Choose Slideshow Folder")
            .modal(true)
            .build();
        let current = self.page.borrow().current(self.store.borrow().as_ref());
        if !current.slideshow.is_empty() {
            dialog.set_initial_folder(Some(&gio::File::for_path(&current.slideshow)));
        }
        let on = self.handler(|ui, folder: String| {
            ui.act(|page, store| page.choose_slideshow(store, &folder));
        });
        dialog.select_folder(
            Some(&self.window),
            None::<&gio::Cancellable>,
            move |result| {
                if let Some(path) = result.ok().and_then(|file| file.path()) {
                    on(path.to_string_lossy().into_owned());
                }
            },
        );
    }

    /// Asks for a picture, adds it and makes it the selected output's.
    fn add_picture(&self) {
        let filter = gtk::FileFilter::new();
        filter.set_name(Some("Pictures"));
        filter.add_pixbuf_formats();
        let filters = gio::ListStore::new::<gtk::FileFilter>();
        filters.append(&filter);
        let dialog = gtk::FileDialog::builder()
            .title("Add Picture")
            .modal(true)
            .filters(&filters)
            .default_filter(&filter)
            .build();
        let on = self.handler(|ui, file: String| {
            ui.act(|page, store| page.add_picture(store, &file));
        });
        dialog.open(
            Some(&self.window),
            None::<&gio::Cancellable>,
            move |result| {
                if let Some(path) = result.ok().and_then(|file| file.path()) {
                    on(path.to_string_lossy().into_owned());
                }
            },
        );
    }

    /// Asks for a folder, and adds a heading of its pictures to the grid.
    fn add_folder(&self) {
        let dialog = gtk::FileDialog::builder()
            .title("Add Folder")
            .modal(true)
            .build();
        let on = self.handler(|ui, folder: String| {
            ui.act(|page, store| page.add_folder(store, &folder));
        });
        dialog.select_folder(
            Some(&self.window),
            None::<&gio::Cancellable>,
            move |result| {
                if let Some(path) = result.ok().and_then(|file| file.path()) {
                    on(path.to_string_lossy().into_owned());
                }
            },
        );
    }

    /// Changes the page and the store with `change`, and renders what changed.
    fn act(&self, change: impl FnOnce(&mut Page, &mut dyn Store)) {
        if self.rendering.get() {
            return;
        }
        {
            let mut store = self.store.borrow_mut();
            change(&mut self.page.borrow_mut(), store.as_mut());
        }
        self.render();
    }

    /// Makes the page of `outputs`, keeping the output selected if it's still there.
    fn set_outputs(&self, outputs: Vec<Output>) {
        let selected = {
            let page = self.page.borrow();
            page.outputs()
                .get(page.selected())
                .map(|output| output.connector.clone())
        };
        let primary = self.primary();
        let mut store = self.store.borrow_mut();
        let mut page = Page::new(store.as_mut(), outputs, primary.as_deref());
        if let Some(index) = selected.and_then(|connector| {
            page.outputs()
                .iter()
                .position(|output| output.connector == connector)
        }) {
            page.select(store.as_ref(), index);
        }
        *self.page.borrow_mut() = page;
    }

    /// The primary output's connector, as Caminito says.
    fn primary(&self) -> Option<String> {
        let property = self.caminito.as_ref()?.cached_property("PrimaryOutput")?;
        property.get::<String>().filter(|name| !name.is_empty())
    }

    /// Gives every widget the page's values.
    fn render(&self) {
        self.rendering.set(true);
        let page = self.page.borrow().clone();
        let store = self.store.borrow();
        let current = page.current(store.as_ref());

        self.same_row.set_visible(page.has_switch());
        self.same.set_active(store.same_on_all_outputs());
        let kind_index = Kind::ALL
            .iter()
            .position(|&k| k == page.kind())
            .unwrap_or(0);
        if let Some(row) = self.kinds.row_at_index(kind_index as i32) {
            self.kinds.select_row(Some(&row));
        }
        match page.kind() {
            Kind::Picture => self.panes.set_visible_child_name("picture"),
            Kind::Colour => self.panes.set_visible_child_name("colour"),
            Kind::Slideshow => self.panes.set_visible_child_name("slideshow"),
            Kind::Dynamic => self.panes.set_visible_child_name("dynamic"),
            Kind::PictureOfTheDay => self.panes.set_visible_child_name("picture-of-the-day"),
        }
        let positioning = Positioning::ALL
            .iter()
            .position(|&p| p == current.positioning)
            .unwrap_or(0);
        self.positioning.set_selected(positioning as u32);
        self.dynamic_positioning.set_selected(positioning as u32);
        self.slideshow_positioning.set_selected(positioning as u32);
        self.daily_positioning.set_selected(positioning as u32);
        self.colour.set_rgba(&rgba(current.colour));
        self.render_slideshow(&current);

        let groups = grid::groups(
            (current.kind == Kind::Picture).then_some(current.picture.as_str()),
            &store.pictures(),
            &store.folders(),
            &self.installed,
        );
        let dynamic_groups = grid::dynamic_groups(&store.dynamics(), &self.installed);
        drop(store);
        self.render_previews(&page);
        if *self.shown_groups.borrow() != groups {
            self.render_grid(&groups);
            *self.shown_groups.borrow_mut() = groups;
        }
        if *self.shown_dynamic_groups.borrow() != dynamic_groups {
            self.render_dynamic_grid(&dynamic_groups);
            *self.shown_dynamic_groups.borrow_mut() = dynamic_groups;
        }
        self.dynamic_line
            .set_label(&self.dynamic_line_text(&current));
        self.render_daily(&current);
        self.mark_current(&current);
        self.rendering.set(false);
    }

    /// Outlines the thumbnails of the current picture and dynamic wallpaper.
    fn mark_current(&self, current: &OutputSettings) {
        let mark = |child: &gtk::FlowBoxChild, is_current: bool| {
            if is_current {
                child.add_css_class("current");
            } else {
                child.remove_css_class("current");
            }
        };
        for (path, child) in self.thumbnails.borrow().iter() {
            mark(
                child,
                current.kind == Kind::Picture && path == Path::new(&current.picture),
            );
        }
        for (path, child) in self.dynamic_thumbnails.borrow().iter() {
            mark(
                child,
                current.kind == Kind::Dynamic && path == Path::new(&current.dynamic),
            );
        }
    }

    /// The Slideshow pane's values: the folder, how many pictures it has, the interval and
    /// the order.
    fn render_slideshow(&self, current: &OutputSettings) {
        let interval = Interval::ALL
            .iter()
            .position(|&i| i == current.interval)
            .unwrap_or(0);
        self.interval.set_selected(interval as u32);
        self.in_order.set_active(current.in_order);
        if current.slideshow.is_empty() {
            self.slideshow_folder.set_label("Choose a folder…");
            self.slideshow_folder
                .set_tooltip_text(Some("Choose the folder of pictures to show"));
            self.slideshow_count
                .set_label("Choose a folder of pictures to show one after another.");
            return;
        }
        let folder = Path::new(&current.slideshow);
        let name = folder.file_name().map_or_else(
            || current.slideshow.clone(),
            |name| name.to_string_lossy().into_owned(),
        );
        self.slideshow_folder.set_label(&name);
        self.slideshow_folder
            .set_tooltip_text(Some(&current.slideshow));
        if !folder.is_dir() {
            self.slideshow_count
                .set_label("The folder isn't there, so the output shows its colour.");
            return;
        }
        let count = current
            .read_slideshow()
            .map_or(0, |slideshow| slideshow.len());
        self.slideshow_count.set_label(&match count {
            0 => "The folder has no pictures, so the output shows its colour.".to_owned(),
            1 => "1 picture in the folder.".to_owned(),
            count => format!("{count} pictures in the folder."),
        });
    }

    /// The Picture of the day pane: the source selected, and its picture's title and credit.
    fn render_daily(&self, current: &OutputSettings) {
        let index = Source::ALL
            .iter()
            .position(|&source| source == current.source)
            .unwrap_or(0);
        if let Some(row) = self.sources.row_at_index(index as i32) {
            self.sources.select_row(Some(&row));
        }
        let latest = self.cache.latest(current.source);
        let (title, credit) = match &latest {
            Some(Kept {
                title, credit, day, ..
            }) => (title.clone(), format!("{credit} · {day}")),
            None => (
                "There's no picture from this source yet.".to_owned(),
                "It's fetched as soon as the network is there, and stays when it isn't.".to_owned(),
            ),
        };
        self.daily_title.set_label(&title);
        self.daily_credit.set_label(&credit);
        self.keep.set_sensitive(latest.is_some());
        let line = match (&*self.kept.borrow(), &latest) {
            (Some((picture, line)), Some(latest)) if *picture == latest.picture => line.clone(),
            _ => String::new(),
        };
        self.keep_line.set_label(&line);
    }

    /// The Dynamic pane's line: why a file wasn't added, or whether the selected output's
    /// dynamic wallpaper follows the sun or fixed times.
    fn dynamic_line_text(&self, current: &OutputSettings) -> String {
        if let Some(refused) = self.refused.borrow().clone() {
            return refused;
        }
        let greyed = self
            .shown_dynamic_groups
            .borrow()
            .iter()
            .flat_map(|group| group.wallpapers.iter())
            .any(|offered| unshowable(&offered.dynamic));
        // Said after the line about the selected one, whichever it is.
        let and_greyed = |line: String| {
            if greyed {
                format!(
                    "{line} HEIC files are greyed out: {}.",
                    DecodeError::NoDecoder
                )
            } else {
                line
            }
        };
        if current.dynamic.is_empty() {
            return and_greyed("Choose a dynamic wallpaper.".into());
        }
        let offered = self
            .shown_dynamic_groups
            .borrow()
            .iter()
            .flat_map(|group| group.wallpapers.iter())
            .find(|offered| offered.path == Path::new(&current.dynamic))
            .map(|offered| Ok(offered.dynamic.clone()));
        let line = match offered.unwrap_or_else(|| Dynamic::read(Path::new(&current.dynamic))) {
            Ok(dynamic) if unshowable(&dynamic) => {
                return format!("It can't be shown: {}.", DecodeError::NoDecoder);
            }
            Ok(Dynamic::Heic(heic)) if matches!(heic.schedule(), Schedule::Solar { .. }) => {
                "Follows the sun: each of its pictures shows where the sun is in it.".into()
            }
            Ok(dynamic) if dynamic.follows_sun() => {
                "Follows the sun: light by day and dark by night, as Night Light warms.".into()
            }
            Ok(_) => "Follows the clock: its pictures change at the times its file gives.".into(),
            Err(err) => format!("It can't be shown: {err}."),
        };
        and_greyed(line)
    }

    /// The sidebar's pictures of the outputs, left to right.
    fn render_previews(&self, page: &Page) {
        while let Some(child) = self.previews.first_child() {
            self.previews.remove(&child);
        }
        let outputs: Vec<Output> = if page.outputs().is_empty() {
            vec![Output::new("", "", "All outputs", "", (0, 0), (1920, 1080))]
        } else {
            page.outputs().to_vec()
        };
        let several = outputs.len() > 1;
        let ranks = page.slideshow_ranks(self.store.borrow().as_ref());
        for (index, output) in outputs.iter().enumerate() {
            let rank = ranks.get(index).copied().flatten().unwrap_or_default();
            let preview = self.preview(index, rank, output);
            let label = gtk::Label::builder()
                .label(&output.title)
                .ellipsize(gtk::pango::EllipsizeMode::End)
                .build();
            let column = gtk::Box::new(gtk::Orientation::Vertical, 4);
            column.append(&preview);
            column.append(&label);
            if several {
                let button = gtk::Button::builder()
                    .child(&column)
                    .css_classes(["flat", "caminito-wallpaper-output"])
                    .tooltip_text(&output.connector)
                    .build();
                if index == page.selected() {
                    button.add_css_class("selected");
                }
                let on = self.handler(move |ui, ()| {
                    if ui.rendering.get() {
                        return;
                    }
                    ui.page
                        .borrow_mut()
                        .select(ui.store.borrow().as_ref(), index);
                    ui.render();
                });
                button.connect_clicked(move |_| on(()));
                self.previews.append(&button);
            } else {
                column.add_css_class("caminito-wallpaper-output");
                self.previews.append(&column);
            }
        }
    }

    /// An output's picture, with its wallpaper drawn as the drawer draws it, `rank` its rank
    /// among the outputs on its Slideshow's folder.
    fn preview(&self, index: usize, rank: usize, output: &Output) -> gtk::DrawingArea {
        let (width, height) = (output.size.0.max(1), output.size.1.max(1));
        // A monitor on its side is as wide as the others, and taller: its shape is kept.
        let preview_height = ((PREVIEW_WIDTH * height) / width).max(1);
        let area = gtk::DrawingArea::builder()
            .content_width(PREVIEW_WIDTH)
            .content_height(preview_height)
            .halign(gtk::Align::Center)
            .build();
        let settings = self
            .page
            .borrow()
            .wallpaper(self.store.borrow().as_ref(), index);
        let decoder = self.decoder.clone();
        let picture = match settings.kind {
            Kind::Picture if !settings.picture.is_empty() => Some(PathBuf::from(&settings.picture)),
            // The picture the drawer shows on this output now.
            Kind::Slideshow => settings.read_slideshow().and_then(|slideshow| {
                let local = sky_now(self.caminito.as_ref()).local;
                slideshow.picture(local, rank).map(Path::to_owned)
            }),
            // The picture that shows most now, as the drawer draws it.
            Kind::Dynamic if !settings.dynamic.is_empty() => {
                Dynamic::read(Path::new(&settings.dynamic))
                    .ok()
                    // A HEIC's pictures once decoded, not before, or the decoder would keep
                    // that they can't be.
                    .filter(Dynamic::ready)
                    .map(|dynamic| {
                        let sky = sky_now(self.caminito.as_ref());
                        dynamic.at(&sky).nearer().to_owned()
                    })
            }
            Kind::PictureOfTheDay => self.cache.latest(settings.source).map(|kept| kept.picture),
            _ => None,
        };
        // Decoded to fit a square twice the preview's longer side, so that a picture of any
        // shape still covers it at a scale of 2 once Fill crops it.
        let side = PREVIEW_WIDTH.max(preview_height) * 2;
        let size = (side, side);
        if let Some(picture) = &picture
            && decoder.get(picture, size.0, size.1).is_none()
        {
            let area = area.downgrade();
            decoder.request(picture, size.0, size.1, move |_| {
                if let Some(area) = area.upgrade() {
                    area.queue_draw();
                }
            });
        }
        let output_size = (width, height);
        area.set_draw_func(move |_, cr, w, h| {
            let (w, h) = (f64::from(w), f64::from(h));
            fill(cr, settings.colour, w, h);
            let Some(picture) = &picture else {
                return;
            };
            let Some(Some(decoded)) = decoder.get(picture, size.0, size.1) else {
                return;
            };
            let rect = place(decoded.original, output_size, settings.positioning);
            cr.save().ok();
            cr.rectangle(0.0, 0.0, w, h);
            cr.clip();
            cr.scale(w / f64::from(output_size.0), h / f64::from(output_size.1));
            cr.translate(f64::from(rect.x), f64::from(rect.y));
            cr.scale(
                f64::from(rect.width) / f64::from(decoded.pixbuf.width().max(1)),
                f64::from(rect.height) / f64::from(decoded.pixbuf.height().max(1)),
            );
            cr.set_source_pixbuf(&decoded.pixbuf, 0.0, 0.0);
            if settings.positioning == Positioning::Tile {
                cr.source().set_extend(gtk::cairo::Extend::Repeat);
                let _ = cr.paint();
            } else {
                cr.rectangle(
                    0.0,
                    0.0,
                    f64::from(decoded.pixbuf.width()),
                    f64::from(decoded.pixbuf.height()),
                );
                let _ = cr.fill();
            }
            cr.restore().ok();
        });
        area
    }

    /// The grid: a heading and a box of thumbnails per group. The scroll stays where it was.
    fn render_grid(&self, groups: &[Group]) {
        let scroll = self.scrolled.vadjustment().value();
        while let Some(child) = self.grid.first_child() {
            self.grid.remove(&child);
        }
        let mut thumbnails = Vec::new();
        if groups.is_empty() {
            self.grid.append(&message(
                "There are no pictures to show. Add a picture or a folder of yours.",
            ));
        }
        for group in groups {
            self.grid.append(&heading(&group.heading));
            let flow = gtk::FlowBox::builder()
                .selection_mode(gtk::SelectionMode::None)
                .activate_on_single_click(true)
                .homogeneous(true)
                .max_children_per_line(12)
                .halign(gtk::Align::Start)
                .column_spacing(SPACING as u32 / 2)
                .row_spacing(SPACING as u32 / 2)
                .build();
            for path in &group.pictures {
                let thumbnail = self.thumbnail(path);
                let child = gtk::FlowBoxChild::builder()
                    .child(&thumbnail)
                    .css_classes(["caminito-wallpaper-thumbnail"])
                    .tooltip_text(path.to_string_lossy().as_ref())
                    .build();
                flow.append(&child);
                thumbnails.push((path.clone(), child));
            }
            let pictures = group.pictures.clone();
            let on = self.handler(move |ui, index: usize| {
                if let Some(path) = pictures.get(index) {
                    let file = path.to_string_lossy();
                    ui.act(|page, store| page.choose_picture(store, &file));
                }
            });
            flow.connect_child_activated(move |_, child| on(child.index() as usize));
            self.grid.append(&flow);
        }
        *self.thumbnails.borrow_mut() = thumbnails;
        let adjustment = self.scrolled.vadjustment();
        glib::idle_add_local_once(move || adjustment.set_value(scroll));
    }

    /// The Dynamic grid: a heading and a box of thumbnails per group, each wallpaper shown by
    /// day on its left and by night on its right. The scroll stays where it was.
    fn render_dynamic_grid(&self, groups: &[DynamicGroup]) {
        let scroll = self.dynamic_scrolled.vadjustment().value();
        while let Some(child) = self.dynamic_grid.first_child() {
            self.dynamic_grid.remove(&child);
        }
        let mut thumbnails = Vec::new();
        if groups.is_empty() {
            self.dynamic_grid.append(&message(
                "There are no dynamic wallpapers installed. Add a timed XML or a HEIC of yours.",
            ));
        }
        for group in groups {
            self.dynamic_grid.append(&heading(&group.heading));
            let flow = gtk::FlowBox::builder()
                .selection_mode(gtk::SelectionMode::None)
                .activate_on_single_click(true)
                .homogeneous(true)
                .max_children_per_line(12)
                .halign(gtk::Align::Start)
                .column_spacing(SPACING as u32 / 2)
                .row_spacing(SPACING as u32 / 2)
                .build();
            for offered in &group.wallpapers {
                let (day, night) = offered.dynamic.day_and_night();
                // A HEIC's pictures are decoded first, then the grid is drawn again.
                let ready = offered.dynamic.ready();
                if let Some(heic) = offered.dynamic.heic().filter(|_| !ready) {
                    self.decode(heic);
                }
                let thumbnail = self.day_and_night_thumbnail(&day, &night, ready);
                let name = dynamic_name(&offered.path);
                let column = gtk::Box::new(gtk::Orientation::Vertical, 2);
                column.append(&thumbnail);
                column.append(
                    &gtk::Label::builder()
                        .label(&name)
                        .ellipsize(gtk::pango::EllipsizeMode::End)
                        .max_width_chars(20)
                        .build(),
                );
                let child = gtk::FlowBoxChild::builder()
                    .child(&column)
                    .css_classes(["caminito-wallpaper-thumbnail"])
                    .tooltip_text(offered.path.to_string_lossy().as_ref())
                    .build();
                if unshowable(&offered.dynamic) {
                    child.set_sensitive(false);
                    child.set_tooltip_text(Some(&format!(
                        "{}\n{}.",
                        offered.path.display(),
                        DecodeError::NoDecoder
                    )));
                }
                flow.append(&child);
                thumbnails.push((offered.path.clone(), child));
            }
            let paths: Vec<PathBuf> = group.wallpapers.iter().map(|w| w.path.clone()).collect();
            let on = self.handler(move |ui, index: usize| {
                if let Some(path) = paths.get(index) {
                    *ui.refused.borrow_mut() = None;
                    let path = path.to_string_lossy();
                    ui.act(|page, store| page.choose_dynamic(store, &path));
                }
            });
            flow.connect_child_activated(move |_, child| on(child.index() as usize));
            self.dynamic_grid.append(&flow);
        }
        *self.dynamic_thumbnails.borrow_mut() = thumbnails;
        let adjustment = self.dynamic_scrolled.vadjustment();
        glib::idle_add_local_once(move || adjustment.set_value(scroll));
    }

    /// Decodes a HEIC's pictures, unless they're being decoded or couldn't be, and draws
    /// the Dynamic grid and the sidebar again once they're in.
    fn decode(&self, file: &heic::Heic) {
        let on = self.handler(|ui, ()| {
            let groups = ui.shown_dynamic_groups.borrow().clone();
            ui.render_dynamic_grid(&groups);
            ui.render();
        });
        self.decodes.start(file, move || on(()));
    }

    /// A thumbnail of a dynamic wallpaper: its day picture's left half and its night
    /// picture's right half, each filled in once decoded, if they're `ready` to be.
    fn day_and_night_thumbnail(&self, day: &Path, night: &Path, ready: bool) -> gtk::DrawingArea {
        let area = gtk::DrawingArea::builder()
            .content_width(THUMBNAIL.0)
            .content_height(THUMBNAIL.1)
            .build();
        let halves: Rc<RefCell<[Option<Rc<Decoded>>; 2]>> = Rc::default();
        area.set_draw_func({
            let halves = halves.clone();
            move |_, cr, w, h| {
                let (w, h) = (f64::from(w), f64::from(h));
                for (index, decoded) in halves.borrow().iter().enumerate() {
                    let left = index as f64 * w / 2.0;
                    cr.save().ok();
                    cr.rectangle(left, 0.0, w / 2.0, h);
                    cr.clip();
                    match decoded {
                        // Each covers the whole thumbnail, cropped, so the two halves meet
                        // as one scene would.
                        Some(decoded) => {
                            let (pw, ph) = (
                                f64::from(decoded.pixbuf.width().max(1)),
                                f64::from(decoded.pixbuf.height().max(1)),
                            );
                            let scale = (w / pw).max(h / ph);
                            cr.translate((w - pw * scale) / 2.0, (h - ph * scale) / 2.0);
                            cr.scale(scale, scale);
                            cr.set_source_pixbuf(&decoded.pixbuf, 0.0, 0.0);
                            let _ = cr.paint();
                        }
                        None => {
                            cr.set_source_rgba(0.5, 0.5, 0.5, 0.2);
                            let _ = cr.paint();
                        }
                    }
                    cr.restore().ok();
                }
            }
        });
        for (index, path) in [day, night].into_iter().enumerate().filter(|_| ready) {
            let weak = area.downgrade();
            let halves = halves.clone();
            self.decoder
                .request(path, THUMBNAIL.0 * 2, THUMBNAIL.0 * 2, move |decoded| {
                    if let Some(area) = weak.upgrade() {
                        halves.borrow_mut()[index] = decoded;
                        area.queue_draw();
                    }
                });
        }
        area
    }

    /// A thumbnail of the grid, filled in once its picture is decoded.
    fn thumbnail(&self, path: &Path) -> gtk::DrawingArea {
        let area = gtk::DrawingArea::builder()
            .content_width(THUMBNAIL.0)
            .content_height(THUMBNAIL.1)
            .build();
        let shown: Rc<RefCell<Option<Rc<Decoded>>>> = Rc::default();
        area.set_draw_func({
            let shown = shown.clone();
            move |_, cr, w, h| {
                let (w, h) = (f64::from(w), f64::from(h));
                let Some(decoded) = shown.borrow().clone() else {
                    cr.set_source_rgba(0.5, 0.5, 0.5, 0.2);
                    cr.rectangle(0.0, 0.0, w, h);
                    let _ = cr.fill();
                    return;
                };
                // Covers the thumbnail, cropped, as Fill covers an output.
                let (pw, ph) = (
                    f64::from(decoded.pixbuf.width().max(1)),
                    f64::from(decoded.pixbuf.height().max(1)),
                );
                let scale = (w / pw).max(h / ph);
                cr.translate((w - pw * scale) / 2.0, (h - ph * scale) / 2.0);
                cr.scale(scale, scale);
                cr.set_source_pixbuf(&decoded.pixbuf, 0.0, 0.0);
                let _ = cr.paint();
            }
        });
        let weak = area.downgrade();
        // Decoded to fit a square twice the thumbnail's width, so that a picture of any shape
        // still covers it at a scale of 2 once cropped.
        self.decoder
            .request(path, THUMBNAIL.0 * 2, THUMBNAIL.0 * 2, move |decoded| {
                if let Some(area) = weak.upgrade() {
                    *shown.borrow_mut() = decoded;
                    area.queue_draw();
                }
            });
        area
    }
}
