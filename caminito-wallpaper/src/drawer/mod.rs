//! The drawer, `caminito-wallpaperd` (210, ADR 0031): a Wayland client with one Layer
//! surface per output in the background layer, under the namespace Caminito's scene puts
//! behind caja's desktop. It runs on GLib's loop, which hears GSettings' changes, with the
//! Wayland connection's fd as one of its sources, and links no GTK.
//!
//! Each output's surface is made once Caminito has said what monitor it shows, through
//! output management's heads, and goes with the output's global, so a hot-plug leaves none
//! behind (88). An output's buffer is its pixels at its fractional scale, shown at its
//! logical size through a viewport. A change of what's drawn cross-fades over [`FADE`]; a
//! change of size draws at once.
//!
//! A picture of the day's source is fetched on GLib's loop as [`daily`] says, and the output
//! shows the source's latest picture, or its colour before the first (214).
//!
//! It keeps `org.mate.background` bridged (211): the primary output's wallpaper goes out to
//! it after every refresh, and a picture another program writes to it comes in first.

mod buffer;
mod daily;
mod paint;

use std::cell::{Cell, OnceCell, RefCell};
use std::collections::{HashMap, HashSet};
use std::os::fd::AsRawFd;
use std::path::PathBuf;
use std::rc::{Rc, Weak};
use std::time::{Duration, Instant};

use gio::glib;
use gio::prelude::*;
use wayland_client::backend::ObjectId;
use wayland_client::globals::{GlobalListContents, registry_queue_init};
use wayland_client::protocol::wl_callback::{self, WlCallback};
use wayland_client::protocol::wl_compositor::WlCompositor;
use wayland_client::protocol::wl_output::{self, WlOutput};
use wayland_client::protocol::wl_registry::{self, WlRegistry};
use wayland_client::protocol::wl_shm::WlShm;
use wayland_client::protocol::wl_shm_pool::WlShmPool;
use wayland_client::protocol::wl_surface::WlSurface;
use wayland_client::{
    Connection, Dispatch, EventQueue, Proxy, QueueHandle, delegate_noop, event_created_child,
};
use wayland_protocols::wp::fractional_scale::v1::client::wp_fractional_scale_manager_v1::WpFractionalScaleManagerV1;
use wayland_protocols::wp::fractional_scale::v1::client::wp_fractional_scale_v1::{
    self, WpFractionalScaleV1,
};
use wayland_protocols::wp::viewporter::client::wp_viewport::WpViewport;
use wayland_protocols::wp::viewporter::client::wp_viewporter::WpViewporter;
use wayland_protocols_wlr::layer_shell::v1::client::zwlr_layer_shell_v1::{self, ZwlrLayerShellV1};
use wayland_protocols_wlr::layer_shell::v1::client::zwlr_layer_surface_v1::{
    self, ZwlrLayerSurfaceV1,
};
use wayland_protocols_wlr::output_management::v1::client::zwlr_output_head_v1::{
    self, ZwlrOutputHeadV1,
};
use wayland_protocols_wlr::output_management::v1::client::zwlr_output_manager_v1::{
    self, ZwlrOutputManagerV1,
};
use wayland_protocols_wlr::output_management::v1::client::zwlr_output_mode_v1::{
    self, ZwlrOutputModeV1,
};

use crate::DOMAIN;
use crate::bridge::Bridge;
use crate::daily::Source;
use crate::dynamic::{Dynamic, Moment, Sky};
pub use crate::gsettings::Error as StoreError;
use crate::gsettings::{GioStore, Mate};
use crate::heic::{self, Heic};
use crate::identity::Identity;
use crate::settings::{Drawn, Kind, OutputSettings, settings_for, take_mate_background};
use crate::sun::sky_now;
use buffer::Buffers;
use daily::{Daily, fetch_if_due};
use paint::Pictures;

/// How long a change of wallpaper takes to cross-fade.
const FADE: Duration = Duration::from_secs(1);

/// How often a dynamic wallpaper is drawn for the moment again (216), besides each time the
/// sun moves, and a timed XML's picture ending sooner: its blend shows in whole percent. It's
/// also how soon after local midnight a picture of the day is fetched (214).
const DYNAMIC_TICK: u32 = 60;

/// The namespace Caminito's scene puts behind every other surface of the background layer,
/// caja's desktop among them (63). It's msd's background plugin's, whose place the drawer
/// takes.
const NAMESPACE: &str = "mate-background";

/// Fractional scale's denominator.
const SCALE_DENOMINATOR: u32 = 120;

/// Why the drawer can't run.
#[derive(Debug)]
pub enum Error {
    /// There's no Wayland display to connect to.
    Connect(wayland_client::ConnectError),
    /// The compositor's globals can't be listed.
    Globals(wayland_client::globals::GlobalError),
    /// The compositor lacks a global the drawer can't do without.
    Missing(&'static str, wayland_client::globals::BindError),
    /// The connection failed.
    Connection(wayland_client::backend::WaylandError),
    /// The store can't be opened.
    Store(StoreError),
}

impl std::fmt::Display for Error {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Error::Connect(err) => write!(f, "can't connect to the compositor: {err}"),
            Error::Globals(err) => write!(f, "can't list the compositor's globals: {err}"),
            Error::Missing(name, err) => write!(f, "the compositor has no {name}: {err}"),
            Error::Connection(err) => write!(f, "the connection to the compositor failed: {err}"),
            Error::Store(err) => write!(f, "can't open the settings: {err}"),
        }
    }
}

impl std::error::Error for Error {}

/// A monitor as output management describes it.
#[derive(Default)]
struct Head {
    name: String,
    make: String,
    model: String,
    serial: String,
    /// Whether a `done` came since it was made, so it's whole: a monitor plugged in is
    /// named before its make, model and serial come.
    described: bool,
}

impl Head {
    fn identity(&self) -> Identity {
        Identity::of(&self.make, &self.model, &self.serial, &self.name)
    }
}

/// An output, from its `wl_output` global.
struct Output {
    /// Its registry name, which its surface's objects carry.
    global: u32,
    wl_output: WlOutput,
    /// Its connector, from `wl_output.name`.
    name: Option<String>,
    /// Its scale from `wl_output`, for a compositor without fractional scale.
    integer_scale: i32,
    surface: Option<Surface>,
}

/// An output's Layer surface, and what it shows.
struct Surface {
    wl_surface: WlSurface,
    layer: ZwlrLayerSurfaceV1,
    viewport: Option<WpViewport>,
    fractional: Option<WpFractionalScaleV1>,
    /// Its logical size, from the last configure.
    size: Option<(i32, i32)>,
    /// The fractional scale, in 120ths, if the compositor said.
    scale: Option<u32>,
    buffers: Buffers,
    /// What it shows, or will once a fade ends, and that frame.
    shown: Option<(Drawn, cairo::ImageSurface)>,
    /// The frame a fade started from, and when.
    fade: Option<(cairo::ImageSurface, Instant)>,
    /// Whether a frame callback is awaited.
    waiting_for_frame: bool,
    /// Whether a frame couldn't be shown, for want of a callback or a buffer.
    behind: bool,
}

impl Drop for Surface {
    fn drop(&mut self) {
        if let Some(fractional) = &self.fractional {
            fractional.destroy();
        }
        if let Some(viewport) = &self.viewport {
            viewport.destroy();
        }
        self.layer.destroy();
        self.wl_surface.destroy();
    }
}

impl Surface {
    /// The size of its buffer: its logical size at its scale.
    fn buffer_size(&self, integer_scale: i32) -> Option<(i32, i32)> {
        let (width, height) = self.size?;
        if width <= 0 || height <= 0 {
            return None;
        }
        let scaled = |length: i32| match self.scale {
            Some(scale) if self.viewport.is_some() => ((f64::from(length) * f64::from(scale))
                / f64::from(SCALE_DENOMINATOR))
            .round() as i32,
            _ => length * integer_scale,
        };
        Some((scaled(width), scaled(height)))
    }
}

/// The drawer's state, which the Wayland events and the settings' changes act on.
pub(crate) struct App {
    qh: QueueHandle<App>,
    compositor: WlCompositor,
    shm: WlShm,
    layer_shell: ZwlrLayerShellV1,
    viewporter: Option<WpViewporter>,
    fractional: Option<WpFractionalScaleManagerV1>,
    outputs: Vec<Output>,
    heads: HashMap<ObjectId, Head>,
    /// Whether output management describes the monitors.
    managed: bool,
    store: GioStore,
    /// Caminito's D-Bus interface, for the primary output.
    caminito: Option<gio::DBusProxy>,
    /// MATE's background, if its schema is installed.
    mate: Option<Mate>,
    /// Shared with `mate`'s signal, which hears the changes the bridge brings in.
    bridge: Rc<RefCell<Bridge>>,
    pictures: Pictures,
    /// The dynamic wallpapers shown, by their path, read once each, or `None` for one that
    /// can't be, logged once.
    dynamics: HashMap<String, Option<Dynamic>>,
    /// The HEICs whose pictures are being decoded (217), and those that couldn't be, logged
    /// once and not tried again in this session.
    decoding: Rc<RefCell<HashSet<PathBuf>>>,
    undecodable: Rc<RefCell<HashSet<PathBuf>>>,
    /// The pictures of the day (214).
    daily: Rc<RefCell<Daily>>,
    /// For a wake before the next tick, when a timed XML's picture ends sooner.
    refresh: Refresh,
    /// Whether such a wake is set.
    waking: Rc<Cell<bool>>,
}

/// Runs the drawer until the connection fails.
pub fn run() -> Result<(), Error> {
    let connection = Connection::connect_to_env().map_err(Error::Connect)?;
    let (globals, queue) = registry_queue_init::<App>(&connection).map_err(Error::Globals)?;
    let qh = queue.handle();

    let refresh = Refresh::default();
    let store = {
        let refresh = refresh.clone();
        GioStore::open(DOMAIN, move |settings| {
            let refresh = refresh.clone();
            settings.connect_changed(None, move |_, _| refresh.schedule());
        })
        .map_err(Error::Store)?
    };
    let bridge = Rc::new(RefCell::new(Bridge::default()));
    let mate = {
        let (bridge, refresh) = (bridge.clone(), refresh.clone());
        Mate::open(move |file| {
            if bridge.borrow_mut().heard(file) {
                refresh.schedule();
            }
        })
    };
    let caminito = caminito_proxy();
    if let Some(proxy) = &caminito {
        // The primary output's wallpaper is the one that goes out.
        let refresh = refresh.clone();
        proxy.connect_local("g-properties-changed", false, move |_| {
            refresh.schedule();
            None
        });
    }
    let mut app = App {
        compositor: globals
            .bind(&qh, 4..=6, ())
            .map_err(|err| Error::Missing("wl_compositor", err))?,
        shm: globals
            .bind(&qh, 1..=1, ())
            .map_err(|err| Error::Missing("wl_shm", err))?,
        layer_shell: globals
            .bind(&qh, 1..=4, ())
            .map_err(|err| Error::Missing("zwlr_layer_shell_v1", err))?,
        viewporter: globals.bind(&qh, 1..=1, ()).ok(),
        fractional: globals.bind(&qh, 1..=1, ()).ok(),
        outputs: Vec::new(),
        heads: HashMap::new(),
        managed: false,
        store,
        caminito,
        mate,
        bridge,
        pictures: Pictures::default(),
        dynamics: HashMap::new(),
        decoding: Rc::default(),
        undecodable: Rc::default(),
        daily: Daily::open(&refresh),
        refresh: refresh.clone(),
        waking: Rc::default(),
        qh: qh.clone(),
    };
    // Version 2 has the make, model and serial.
    app.managed = globals
        .bind::<ZwlrOutputManagerV1, _, _>(&qh, 2..=4, ())
        .is_ok();
    if let Some(mate) = app.mate.as_ref().map(Mate::read)
        && take_mate_background(&mut app.store, &mate)
    {
        glib::g_debug!(
            DOMAIN,
            "took org.mate.background's picture, {}",
            mate.picture
        );
    }
    let outputs = globals.contents().clone_list();
    for global in outputs
        .iter()
        .filter(|global| global.interface == "wl_output")
    {
        app.add_output(globals.registry(), global.name, global.version);
    }

    let main_loop = glib::MainLoop::new(None, false);
    let failure = Rc::new(Cell::new(None));
    let fd = connection.backend().poll_fd().as_raw_fd();
    queue.flush().map_err(Error::Connection)?;
    let runner = Rc::new(RefCell::new(Runner { queue, app }));
    refresh
        .runner
        .set(Rc::downgrade(&runner))
        .unwrap_or_else(|_| unreachable!("the runner is set once"));
    {
        let runner = runner.clone();
        let main_loop = main_loop.clone();
        let failure = failure.clone();
        glib_unix::unix_fd_add_local(
            fd,
            glib::IOCondition::IN | glib::IOCondition::ERR | glib::IOCondition::HUP,
            move |_, _| match runner.borrow_mut().read() {
                Ok(()) => glib::ControlFlow::Continue,
                Err(err) => {
                    failure.set(Some(err));
                    main_loop.quit();
                    glib::ControlFlow::Break
                }
            },
        );
    }
    {
        let refresh = refresh.clone();
        glib::timeout_add_seconds_local(DYNAMIC_TICK, move || {
            refresh.schedule();
            glib::ControlFlow::Continue
        });
    }
    main_loop.run();
    match failure.take() {
        Some(err) => Err(Error::Connection(err)),
        None => Ok(()),
    }
}

/// `org.mate.Caminito`, or `None`, said once, without a session bus.
fn caminito_proxy() -> Option<gio::DBusProxy> {
    let proxy = gio::DBusProxy::for_bus_sync(
        gio::BusType::Session,
        gio::DBusProxyFlags::DO_NOT_AUTO_START,
        None,
        "org.mate.Caminito",
        "/org/mate/Caminito",
        "org.mate.Caminito",
        None::<&gio::Cancellable>,
    );
    proxy
        .inspect_err(|err| {
            glib::g_debug!(DOMAIN, "can't ask Caminito for the primary output: {err}");
        })
        .ok()
}

/// The connection's queue and the state it dispatches to.
struct Runner {
    queue: EventQueue<App>,
    app: App,
}

impl Runner {
    /// Reads what the compositor sent, dispatches it and sends the requests made.
    fn read(&mut self) -> Result<(), wayland_client::backend::WaylandError> {
        if let Some(guard) = self.queue.prepare_read() {
            match guard.read() {
                Ok(_) => {}
                Err(wayland_client::backend::WaylandError::Io(err))
                    if err.kind() == std::io::ErrorKind::WouldBlock => {}
                Err(err) => return Err(err),
            }
        }
        self.queue
            .dispatch_pending(&mut self.app)
            .map_err(|err| match err {
                wayland_client::DispatchError::Backend(err) => err,
                wayland_client::DispatchError::BadMessage { .. } => {
                    wayland_client::backend::WaylandError::Io(std::io::Error::other(
                        err.to_string(),
                    ))
                }
            })?;
        self.queue.flush()
    }

    /// Draws every output again, after a change of the settings.
    fn refresh(&mut self) {
        self.app.refresh();
        if let Err(err) = self.queue.flush() {
            glib::g_warning!(DOMAIN, "can't send the new frames: {err}");
        }
    }
}

/// A refresh after a change of the settings, scheduled once however many keys change
/// together. The settings' signals come while the runner may be borrowed, so they only
/// schedule.
#[derive(Clone, Default)]
struct Refresh {
    scheduled: Rc<Cell<bool>>,
    runner: Rc<OnceCell<Weak<RefCell<Runner>>>>,
}

impl Refresh {
    fn schedule(&self) {
        if self.scheduled.replace(true) {
            return;
        }
        let this = self.clone();
        glib::idle_add_local_once(move || {
            this.scheduled.set(false);
            if let Some(runner) = this.runner.get().and_then(Weak::upgrade) {
                runner.borrow_mut().refresh();
            }
        });
    }
}

impl App {
    fn add_output(&mut self, registry: &WlRegistry, global: u32, version: u32) {
        if version < 4 {
            glib::g_warning!(DOMAIN, "wl_output {global} has no name before version 4");
            return;
        }
        let wl_output = registry.bind::<WlOutput, _, _>(global, 4, &self.qh, global);
        self.outputs.push(Output {
            global,
            wl_output,
            name: None,
            integer_scale: 1,
            surface: None,
        });
    }

    /// Takes away an output whose global went, with its surface.
    fn remove_output(&mut self, global: u32) {
        let Some(index) = self.outputs.iter().position(|o| o.global == global) else {
            return;
        };
        let output = self.outputs.remove(index);
        drop(output.surface);
        output.wl_output.release();
    }

    fn output_mut(&mut self, global: u32) -> Option<&mut Output> {
        self.outputs
            .iter_mut()
            .find(|output| output.global == global)
    }

    /// Whether the monitor at `connector` is described, so that its identity is known. A
    /// compositor without output management knows monitors by their connectors alone.
    fn described(&self, connector: &str) -> bool {
        !self.managed
            || self
                .heads
                .values()
                .any(|head| head.described && head.name == connector)
    }

    /// Gives a surface to each output whose monitor is described, so that the settings it
    /// gets are its monitor's from its first frame.
    fn make_surfaces(&mut self) {
        for index in 0..self.outputs.len() {
            let output = &self.outputs[index];
            let described = output
                .name
                .as_deref()
                .is_some_and(|name| self.described(name));
            if output.surface.is_some() || !described {
                continue;
            }
            let surface = self.make_surface(output);
            self.outputs[index].surface = Some(surface);
        }
    }

    fn make_surface(&self, output: &Output) -> Surface {
        let global = output.global;
        let wl_surface = self.compositor.create_surface(&self.qh, ());
        let layer = self.layer_shell.get_layer_surface(
            &wl_surface,
            Some(&output.wl_output),
            zwlr_layer_shell_v1::Layer::Background,
            NAMESPACE.to_owned(),
            &self.qh,
            global,
        );
        use zwlr_layer_surface_v1::Anchor;
        layer.set_anchor(Anchor::Top | Anchor::Bottom | Anchor::Left | Anchor::Right);
        // The whole output, whatever the panel reserves.
        layer.set_exclusive_zone(-1);
        layer.set_keyboard_interactivity(zwlr_layer_surface_v1::KeyboardInteractivity::None);
        let viewport = self
            .viewporter
            .as_ref()
            .map(|viewporter| viewporter.get_viewport(&wl_surface, &self.qh, ()));
        let fractional = self
            .fractional
            .as_ref()
            .map(|manager| manager.get_fractional_scale(&wl_surface, &self.qh, global));
        wl_surface.commit();
        Surface {
            wl_surface,
            layer,
            viewport,
            fractional,
            size: None,
            scale: None,
            buffers: Buffers::default(),
            shown: None,
            fade: None,
            waiting_for_frame: false,
            behind: false,
        }
    }

    /// The identity of the monitor at `connector`.
    fn identity(&self, connector: &str) -> Identity {
        let head = self.heads.values().find(|head| head.name == connector);
        match head {
            Some(head) => head.identity(),
            None => Identity::Connector(connector.to_owned()),
        }
    }

    /// The primary output's identity: Caminito's, or the first output's without it.
    fn primary(&self) -> Option<Identity> {
        let named = self
            .caminito
            .as_ref()
            .and_then(|proxy| proxy.cached_property("PrimaryOutput"))
            .and_then(|value| value.get::<String>())
            .filter(|name| !name.is_empty());
        let name = named.or_else(|| self.outputs.iter().find_map(|o| o.name.clone()))?;
        Some(self.identity(&name))
    }

    /// Draws every output whose wallpaper or size changed, after bringing in a picture
    /// another program gave `org.mate.background`, and sends the primary output's out.
    fn refresh(&mut self) {
        if self.bridge.borrow_mut().bring_in(&mut self.store) {
            glib::g_debug!(DOMAIN, "took org.mate.background's new picture");
        }
        let primary = self.primary();
        let mut shown = Vec::new();
        for output in &self.outputs {
            let (Some(name), Some(_)) = (&output.name, &output.surface) else {
                continue;
            };
            let identity = self.identity(name);
            let settings = settings_for(&mut self.store, &identity, primary.as_ref());
            shown.push((output.global, primary.as_ref() == Some(&identity), settings));
        }
        let dynamics: Vec<&str> = shown
            .iter()
            .filter(|(_, _, settings)| settings.kind == Kind::Dynamic)
            .map(|(_, _, settings)| settings.dynamic.as_str())
            .collect();
        self.dynamics
            .retain(|path, _| dynamics.contains(&path.as_str()));
        let sky = sky_now(self.caminito.as_ref());
        let mut sources: Vec<Source> = Vec::new();
        for (_, _, settings) in &shown {
            if settings.kind == Kind::PictureOfTheDay && !sources.contains(&settings.source) {
                sources.push(settings.source);
            }
        }
        self.daily.borrow_mut().keep_only(&sources);
        for &source in &sources {
            fetch_if_due(&self.daily, source, sky.local, &self.refresh);
        }
        let mut drawn = Vec::new();
        let mut primary_drawn = None;
        for (global, is_primary, settings) in shown {
            let moment = self.moment(&settings, &sky);
            let daily = (settings.kind == Kind::PictureOfTheDay)
                .then(|| self.daily.borrow_mut().latest(settings.source))
                .flatten();
            let output_drawn = settings.drawn(
                moment.as_ref(),
                daily.as_ref().map(|kept| kept.picture.as_path()),
            );
            if is_primary {
                primary_drawn = Some(output_drawn.clone());
            }
            drawn.push((global, output_drawn));
        }
        self.wake_for_timed(&sky);
        if let Some(primary) = primary_drawn {
            self.bridge_out(&primary);
        }
        let files: Vec<String> = drawn
            .iter()
            .flat_map(|(_, drawn)| drawn.files())
            .map(str::to_owned)
            .collect();
        self.pictures.keep_only(files.iter().map(String::as_str));
        for (global, drawn) in drawn {
            self.update(global, drawn);
        }
    }

    /// Wakes the drawer when the soonest timed XML shown next changes, if before the next
    /// tick: at its next picture, or each second of its transition.
    fn wake_for_timed(&mut self, sky: &Sky) {
        let soonest = self
            .dynamics
            .values()
            .flatten()
            .filter_map(|dynamic| dynamic.next_change(sky))
            .min();
        let Some(seconds) = soonest.filter(|&seconds| seconds < u64::from(DYNAMIC_TICK)) else {
            return;
        };
        if self.waking.replace(true) {
            return;
        }
        let (refresh, waking) = (self.refresh.clone(), self.waking.clone());
        glib::timeout_add_local_once(Duration::from_secs(seconds), move || {
            waking.set(false);
            refresh.schedule();
        });
    }

    /// What the Dynamic of `settings` shows at `sky`, or `None` for another kind, a
    /// dynamic wallpaper that can't be read, or a HEIC whose pictures aren't decoded yet,
    /// which starts their decoding.
    fn moment(&mut self, settings: &OutputSettings, sky: &Sky) -> Option<Moment> {
        if settings.kind != Kind::Dynamic || settings.dynamic.is_empty() {
            return None;
        }
        let path = &settings.dynamic;
        let dynamic = self.dynamics.entry(path.clone()).or_insert_with(|| {
            Dynamic::read(std::path::Path::new(path))
                .inspect_err(|err| {
                    glib::g_warning!(DOMAIN, "can't show the dynamic wallpaper {path}: {err}");
                })
                .ok()
        });
        let dynamic = dynamic.as_ref()?;
        if let Some(heic) = dynamic.heic().filter(|heic| !heic.decoded()).cloned() {
            self.decode(&heic);
            return None;
        }
        Some(dynamic.at(sky))
    }

    /// Decodes `heic`'s pictures, unless they're being decoded or couldn't be, and draws
    /// again once they're in.
    fn decode(&self, heic: &Heic) {
        let file = heic.file().to_owned();
        if self.undecodable.borrow().contains(&file)
            || !self.decoding.borrow_mut().insert(file.clone())
        {
            return;
        }
        glib::g_debug!(DOMAIN, "decoding {}", file.display());
        let (decoding, undecodable) = (self.decoding.clone(), self.undecodable.clone());
        let refresh = self.refresh.clone();
        heic::decode(heic, move |outcome| {
            decoding.borrow_mut().remove(&file);
            match outcome {
                Ok(()) => glib::g_debug!(DOMAIN, "decoded {}", file.display()),
                Err(err) => {
                    glib::g_warning!(
                        DOMAIN,
                        "can't show the dynamic wallpaper {}: {err}",
                        file.display()
                    );
                    undecodable.borrow_mut().insert(file);
                }
            }
            refresh.schedule();
        });
    }

    /// Writes what the primary output shows to `org.mate.background`, where it differs.
    fn bridge_out(&self, primary: &Drawn) {
        let Some(mate) = &self.mate else {
            return;
        };
        // Not borrowed while writing: the write is heard at once.
        let writes = self.bridge.borrow_mut().out(&mate.read(), primary);
        if writes.is_empty() {
            return;
        }
        if let Err(err) = mate.write(&writes) {
            glib::g_warning!(DOMAIN, "{err}");
            self.bridge.borrow_mut().not_written();
        }
    }

    /// Draws `drawn` on the output `global`: at once on a new or resized surface, and
    /// with a fade on one that showed something else.
    fn update(&mut self, global: u32, drawn: Drawn) {
        let Some(index) = self.outputs.iter().position(|o| o.global == global) else {
            return;
        };
        let output = &mut self.outputs[index];
        let integer_scale = output.integer_scale;
        let Some(surface) = &mut output.surface else {
            return;
        };
        let Some(size) = surface.buffer_size(integer_scale) else {
            return;
        };
        let same_size = surface
            .shown
            .as_ref()
            .is_some_and(|(_, frame)| (frame.width(), frame.height()) == size);
        if same_size
            && surface
                .shown
                .as_ref()
                .is_some_and(|(shown, _)| *shown == drawn)
        {
            return;
        }
        let frame = match paint::render(&drawn, size, &mut self.pictures) {
            Ok(frame) => frame,
            Err(err) => {
                glib::g_warning!(DOMAIN, "can't draw the wallpaper: {err}");
                return;
            }
        };
        if same_size {
            // A fade that's under way goes on from where it is.
            match current(surface) {
                Some(Ok(from)) => surface.fade = Some((from, Instant::now())),
                Some(Err(err)) => {
                    glib::g_warning!(DOMAIN, "can't draw the wallpaper: {err}");
                    return;
                }
                None => surface.fade = None,
            }
        } else {
            surface.fade = None;
        }
        surface.shown = Some((drawn, frame));
        self.present(global);
    }

    /// Shows the output's current frame, unless it waits for a frame callback or a buffer.
    fn present(&mut self, global: u32) {
        let Some(output) = self.outputs.iter_mut().find(|o| o.global == global) else {
            return;
        };
        let integer_scale = output.integer_scale;
        let Some(surface) = &mut output.surface else {
            return;
        };
        if surface.waiting_for_frame {
            surface.behind = true;
            return;
        }
        let Some((logical_width, logical_height)) = surface.size else {
            return;
        };
        // Ended before the frame is drawn, not after: a fade ended after would show its
        // last blend, a frame short of the picture, and ask for no frame to follow it (211).
        if surface
            .fade
            .as_ref()
            .is_some_and(|(_, start)| start.elapsed() >= FADE)
        {
            surface.fade = None;
        }
        // Every frame of a fade comes here, so a failure is logged at debug alone; the
        // frame stays as it was until the next.
        let frame = match current(surface) {
            Some(Ok(frame)) => frame,
            Some(Err(err)) => {
                glib::g_debug!(DOMAIN, "can't draw the wallpaper: {err}");
                return;
            }
            None => return,
        };
        let buffer = match surface.buffers.write(&frame, &self.shm, &self.qh, global) {
            Ok(Some(buffer)) => buffer,
            Ok(None) => {
                surface.behind = true;
                return;
            }
            Err(err) => {
                glib::g_debug!(DOMAIN, "can't show the wallpaper: {err}");
                return;
            }
        };
        surface.behind = false;
        let wl_surface = &surface.wl_surface;
        wl_surface.attach(Some(buffer), 0, 0);
        wl_surface.damage_buffer(0, 0, i32::MAX, i32::MAX);
        match &surface.viewport {
            Some(viewport) => viewport.set_destination(logical_width, logical_height),
            None => wl_surface.set_buffer_scale(integer_scale),
        }
        if surface.fade.is_some() {
            wl_surface.frame(&self.qh, global);
            surface.waiting_for_frame = true;
        }
        wl_surface.commit();
    }

    /// A frame callback came: the next frame of a fade, or one held back.
    fn frame_done(&mut self, global: u32) {
        let Some(surface) = self.output_mut(global).and_then(|o| o.surface.as_mut()) else {
            return;
        };
        surface.waiting_for_frame = false;
        if surface.fade.is_some() || surface.behind {
            self.present(global);
        }
    }

    /// A buffer was released: a frame held back for want of one can be shown.
    fn buffer_released(&mut self, global: u32) {
        let behind = self
            .output_mut(global)
            .and_then(|o| o.surface.as_ref())
            .is_some_and(|surface| surface.behind);
        if behind {
            self.present(global);
        }
    }
}

/// What the surface shows now: its frame, or a fade's frame for this moment, or `None` for
/// a surface never drawn.
fn current(surface: &Surface) -> Option<Result<cairo::ImageSurface, cairo::Error>> {
    let (_, to) = surface.shown.as_ref()?;
    Some(match &surface.fade {
        Some((from, start)) => {
            let progress = start.elapsed().as_secs_f64() / FADE.as_secs_f64();
            paint::blend(from, to, progress)
        }
        None => Ok(to.clone()),
    })
}

impl Dispatch<WlRegistry, GlobalListContents> for App {
    fn event(
        app: &mut Self,
        registry: &WlRegistry,
        event: wl_registry::Event,
        _: &GlobalListContents,
        _: &Connection,
        _: &QueueHandle<Self>,
    ) {
        match event {
            wl_registry::Event::Global {
                name,
                interface,
                version,
            } if interface == WlOutput::interface().name => {
                app.add_output(registry, name, version);
            }
            wl_registry::Event::GlobalRemove { name } => app.remove_output(name),
            _ => {}
        }
    }
}

impl Dispatch<WlOutput, u32> for App {
    fn event(
        app: &mut Self,
        _: &WlOutput,
        event: wl_output::Event,
        global: &u32,
        _: &Connection,
        _: &QueueHandle<Self>,
    ) {
        let Some(output) = app.output_mut(*global) else {
            return;
        };
        match event {
            wl_output::Event::Name { name } => output.name = Some(name),
            wl_output::Event::Scale { factor } => output.integer_scale = factor.max(1),
            wl_output::Event::Done => {
                app.make_surfaces();
                app.refresh();
            }
            _ => {}
        }
    }
}

impl Dispatch<ZwlrLayerSurfaceV1, u32> for App {
    fn event(
        app: &mut Self,
        layer: &ZwlrLayerSurfaceV1,
        event: zwlr_layer_surface_v1::Event,
        global: &u32,
        _: &Connection,
        _: &QueueHandle<Self>,
    ) {
        let Some(output) = app.output_mut(*global) else {
            return;
        };
        match event {
            zwlr_layer_surface_v1::Event::Configure {
                serial,
                width,
                height,
            } => {
                layer.ack_configure(serial);
                if let Some(surface) = &mut output.surface {
                    surface.size = Some((width as i32, height as i32));
                }
                app.refresh();
            }
            // The compositor took it away: the output goes, or will.
            zwlr_layer_surface_v1::Event::Closed => output.surface = None,
            _ => {}
        }
    }
}

impl Dispatch<WpFractionalScaleV1, u32> for App {
    fn event(
        app: &mut Self,
        _: &WpFractionalScaleV1,
        event: wp_fractional_scale_v1::Event,
        global: &u32,
        _: &Connection,
        _: &QueueHandle<Self>,
    ) {
        let wp_fractional_scale_v1::Event::PreferredScale { scale } = event else {
            return;
        };
        if let Some(surface) = app.output_mut(*global).and_then(|o| o.surface.as_mut()) {
            surface.scale = Some(scale);
        }
        app.refresh();
    }
}

impl Dispatch<WlCallback, u32> for App {
    fn event(
        app: &mut Self,
        _: &WlCallback,
        event: wl_callback::Event,
        global: &u32,
        _: &Connection,
        _: &QueueHandle<Self>,
    ) {
        if let wl_callback::Event::Done { .. } = event {
            app.frame_done(*global);
        }
    }
}

impl Dispatch<ZwlrOutputManagerV1, ()> for App {
    fn event(
        app: &mut Self,
        _: &ZwlrOutputManagerV1,
        event: zwlr_output_manager_v1::Event,
        _: &(),
        _: &Connection,
        _: &QueueHandle<Self>,
    ) {
        match event {
            zwlr_output_manager_v1::Event::Head { head } => {
                app.heads.insert(head.id(), Head::default());
            }
            zwlr_output_manager_v1::Event::Done { .. } => {
                for head in app.heads.values_mut() {
                    head.described = true;
                }
                app.make_surfaces();
                app.refresh();
            }
            _ => {}
        }
    }

    event_created_child!(App, ZwlrOutputManagerV1, [
        zwlr_output_manager_v1::EVT_HEAD_OPCODE => (ZwlrOutputHeadV1, ()),
    ]);
}

impl Dispatch<ZwlrOutputHeadV1, ()> for App {
    fn event(
        app: &mut Self,
        head: &ZwlrOutputHeadV1,
        event: zwlr_output_head_v1::Event,
        _: &(),
        _: &Connection,
        _: &QueueHandle<Self>,
    ) {
        if let zwlr_output_head_v1::Event::Finished = event {
            app.heads.remove(&head.id());
            if head.version() >= 3 {
                head.release();
            }
            return;
        }
        let Some(described) = app.heads.get_mut(&head.id()) else {
            return;
        };
        match event {
            zwlr_output_head_v1::Event::Name { name } => described.name = name,
            zwlr_output_head_v1::Event::Make { make } => described.make = make,
            zwlr_output_head_v1::Event::Model { model } => described.model = model,
            zwlr_output_head_v1::Event::SerialNumber { serial_number } => {
                described.serial = serial_number;
            }
            _ => {}
        }
    }

    event_created_child!(App, ZwlrOutputHeadV1, [
        zwlr_output_head_v1::EVT_MODE_OPCODE => (ZwlrOutputModeV1, ()),
    ]);
}

impl Dispatch<ZwlrOutputModeV1, ()> for App {
    fn event(
        _: &mut Self,
        mode: &ZwlrOutputModeV1,
        event: zwlr_output_mode_v1::Event,
        _: &(),
        _: &Connection,
        _: &QueueHandle<Self>,
    ) {
        if let zwlr_output_mode_v1::Event::Finished = event
            && mode.version() >= 3
        {
            mode.release();
        }
    }
}

delegate_noop!(App: WlCompositor);
delegate_noop!(App: ignore WlShm);
delegate_noop!(App: WlShmPool);
delegate_noop!(App: ignore WlSurface);
delegate_noop!(App: ZwlrLayerShellV1);
delegate_noop!(App: WpViewporter);
delegate_noop!(App: WpViewport);
delegate_noop!(App: WpFractionalScaleManagerV1);
