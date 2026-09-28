//! Test clients for Camino's integration tests (14, 15).
//!
//! A [`Client`] speaks Wayland over one end of a socket pair and never blocks: the test that
//! owns it runs the compositor in the same thread, and calls [`Client::dispatch`] between the
//! compositor's own dispatches. It records what the compositor sends, and the `Display` impls
//! turn those records into stable text for snapshots.

use std::fmt;
use std::fs::File;
use std::io::{self, Write};
use std::os::fd::AsFd;
use std::os::unix::net::UnixStream;
use std::sync::Arc;
use std::sync::atomic::{AtomicBool, Ordering};

use rustix::fs::MemfdFlags;
use wayland_client::backend::WaylandError;
use wayland_client::protocol::wl_buffer::{self, WlBuffer};
use wayland_client::protocol::wl_callback::{self, WlCallback};
use wayland_client::protocol::wl_compositor::{self, WlCompositor};
use wayland_client::protocol::wl_registry::{self, WlRegistry};
use wayland_client::protocol::wl_seat::{self, WlSeat};
use wayland_client::protocol::wl_shm::{self, WlShm};
use wayland_client::protocol::wl_shm_pool::{self, WlShmPool};
use wayland_client::protocol::wl_surface::{self, WlSurface};
use wayland_client::{Connection, Dispatch, EventQueue, Proxy, QueueHandle, WEnum};
use wayland_protocols::xdg::shell::client::xdg_surface::{self, XdgSurface};
use wayland_protocols::xdg::shell::client::xdg_toplevel::{self, XdgToplevel};
use wayland_protocols::xdg::shell::client::xdg_wm_base::{self, XdgWmBase};

pub use wayland_protocols::xdg::shell::client::xdg_toplevel::ResizeEdge;

/// A Wayland client under a test's control.
pub struct Client {
    connection: Connection,
    queue: EventQueue<State>,
    qh: QueueHandle<State>,
    registry: WlRegistry,
    pub state: State,
}

/// Everything a [`Client`] has bound, made and been told.
#[derive(Default)]
pub struct State {
    /// The globals the compositor advertised, in the order it did.
    pub globals: Vec<Global>,
    compositor: Option<WlCompositor>,
    shm: Option<WlShm>,
    wm_base: Option<XdgWmBase>,
    seat: Option<WlSeat>,
    windows: Vec<Window>,
}

/// A global from `wl_registry`.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Global {
    pub name: u32,
    pub interface: String,
    pub version: u32,
}

/// The index of one of a client's windows.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct WindowId(usize);

/// An xdg toplevel.
pub struct Window {
    pub surface: WlSurface,
    pub xdg_surface: XdgSurface,
    pub toplevel: XdgToplevel,
    /// Every configure received, oldest first.
    pub configures: Vec<Configure>,
    /// The toplevel events since the last `xdg_surface.configure`.
    pending: Configure,
    /// The serial of the last configure acked.
    pub last_acked: Option<u32>,
    /// Whether the compositor asked the window to close.
    pub close_requested: bool,
    /// The buffers attached so far, kept alive until the window is dropped.
    buffers: Vec<Buffer>,
}

/// One configure sequence: the toplevel events, then the `xdg_surface.configure` ending it.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct Configure {
    pub size: (i32, i32),
    pub states: Vec<xdg_toplevel::State>,
    pub bounds: Option<(i32, i32)>,
    pub capabilities: Option<Vec<xdg_toplevel::WmCapabilities>>,
    pub serial: u32,
}

struct Buffer {
    buffer: WlBuffer,
    pool: WlShmPool,
}

impl Drop for Buffer {
    fn drop(&mut self) {
        self.buffer.destroy();
        self.pool.destroy();
    }
}

impl Client {
    /// Connects over `stream`, the client's end of a socket pair.
    pub fn new(stream: UnixStream) -> Self {
        stream.set_nonblocking(true).expect("a non-blocking socket");
        let connection = Connection::from_socket(stream).expect("a Wayland connection");
        let queue = connection.new_event_queue();
        let qh = queue.handle();
        let registry = connection.display().get_registry(&qh, ());
        Self {
            connection,
            queue,
            qh,
            registry,
            state: State::default(),
        }
    }

    /// Sends what's queued, then reads and handles whatever the compositor has sent, without
    /// blocking.
    pub fn dispatch(&mut self) {
        self.flush();
        if let Some(guard) = self.queue.prepare_read() {
            match guard.read() {
                Ok(_) => {}
                Err(WaylandError::Io(err)) if err.kind() == io::ErrorKind::WouldBlock => {}
                Err(err) => panic!("error reading events: {err}"),
            }
        }
        self.queue
            .dispatch_pending(&mut self.state)
            .expect("events dispatched");
    }

    pub fn flush(&self) {
        match self.connection.flush() {
            Ok(()) => {}
            Err(WaylandError::Io(err)) if err.kind() == io::ErrorKind::WouldBlock => {}
            Err(err) => panic!("error flushing requests: {err}"),
        }
    }

    /// Sends a `wl_display.sync`. The flag is set once the compositor has answered it, and so
    /// has handled every request sent before it.
    pub fn send_sync(&mut self) -> Arc<AtomicBool> {
        let done = Arc::new(AtomicBool::new(false));
        self.connection.display().sync(&self.qh, Arc::clone(&done));
        self.flush();
        done
    }

    /// Creates an xdg toplevel. Nothing is committed yet.
    pub fn create_window(&mut self) -> WindowId {
        let id = WindowId(self.state.windows.len());
        let surface = self.compositor().create_surface(&self.qh, ());
        let xdg_surface = self.wm_base().get_xdg_surface(&surface, &self.qh, id);
        let toplevel = xdg_surface.get_toplevel(&self.qh, id);
        self.state.windows.push(Window {
            surface,
            xdg_surface,
            toplevel,
            configures: Vec::new(),
            pending: Configure::default(),
            last_acked: None,
            close_requested: false,
            buffers: Vec::new(),
        });
        id
    }

    pub fn window(&self, id: WindowId) -> &Window {
        &self.state.windows[id.0]
    }

    pub fn window_mut(&mut self, id: WindowId) -> &mut Window {
        &mut self.state.windows[id.0]
    }

    /// Acks the latest configure the window has received, if there's one it hasn't acked.
    pub fn ack_last_configure(&mut self, id: WindowId) {
        let window = self.window_mut(id);
        let Some(serial) = window.configures.last().map(|c| c.serial) else {
            return;
        };
        if window.last_acked != Some(serial) {
            window.xdg_surface.ack_configure(serial);
            window.last_acked = Some(serial);
        }
    }

    /// Attaches a new shm buffer of `size` filled with `argb`, damaging all of it. Commit
    /// separately.
    pub fn attach_buffer(&mut self, id: WindowId, size: (i32, i32), argb: u32) {
        let (width, height) = size;
        let stride = width * 4;
        let len = stride * height;

        let fd =
            rustix::fs::memfd_create("camino-test-client", MemfdFlags::CLOEXEC).expect("a memfd");
        let mut file = File::from(fd);
        let pixels: Vec<u8> = std::iter::repeat_n(argb.to_le_bytes(), (width * height) as usize)
            .flatten()
            .collect();
        file.write_all(&pixels)
            .expect("pixels written to the memfd");

        let pool = self.shm().create_pool(file.as_fd(), len, &self.qh, ());
        let buffer = pool.create_buffer(
            0,
            width,
            height,
            stride,
            wl_shm::Format::Argb8888,
            &self.qh,
            (),
        );

        let window = self.window_mut(id);
        window.surface.attach(Some(&buffer), 0, 0);
        window.surface.damage_buffer(0, 0, width, height);
        window.buffers.push(Buffer { buffer, pool });
    }

    /// Acks the latest configure, then commits a `size` buffer of `argb`.
    pub fn ack_and_commit_buffer(&mut self, id: WindowId, size: (i32, i32), argb: u32) {
        self.ack_last_configure(id);
        self.attach_buffer(id, size, argb);
        self.window(id).surface.commit();
    }

    pub fn commit(&self, id: WindowId) {
        self.window(id).surface.commit();
    }

    pub fn set_title(&self, id: WindowId, title: &str) {
        self.window(id).toplevel.set_title(title.to_owned());
    }

    pub fn set_app_id(&self, id: WindowId, app_id: &str) {
        self.window(id).toplevel.set_app_id(app_id.to_owned());
    }

    pub fn set_parent(&self, id: WindowId, parent: Option<WindowId>) {
        let parent = parent.map(|p| &self.window(p).toplevel);
        self.window(id).toplevel.set_parent(parent);
    }

    /// Asks for an interactive move, as if `serial`'s button press started it.
    pub fn start_move(&mut self, id: WindowId, serial: u32) {
        let seat = self.seat();
        self.window(id).toplevel._move(&seat, serial);
    }

    /// Asks for an interactive resize from `edge`, as if `serial`'s button press started it.
    pub fn start_resize(&mut self, id: WindowId, serial: u32, edge: ResizeEdge) {
        let seat = self.seat();
        self.window(id).toplevel.resize(&seat, serial, edge);
    }

    pub fn set_minimized(&self, id: WindowId) {
        self.window(id).toplevel.set_minimized();
    }

    fn compositor(&mut self) -> WlCompositor {
        self.global(|state| &mut state.compositor, 6)
    }

    fn shm(&mut self) -> WlShm {
        self.global(|state| &mut state.shm, 2)
    }

    fn wm_base(&mut self) -> XdgWmBase {
        self.global(|state| &mut state.wm_base, 7)
    }

    fn seat(&mut self) -> WlSeat {
        self.global(|state| &mut state.seat, 9)
    }

    /// The proxy in `slot`, bound on first use.
    fn global<I>(&mut self, slot: fn(&mut State) -> &mut Option<I>, max_version: u32) -> I
    where
        I: Proxy + 'static,
        State: Dispatch<I, ()>,
    {
        if slot(&mut self.state).is_none() {
            let proxy = bind(&self.registry, &self.state.globals, &self.qh, max_version);
            *slot(&mut self.state) = Some(proxy);
        }
        slot(&mut self.state).clone().expect("bound above")
    }
}

/// Binds the global for `I`, at the lower of its advertised version and `max_version`.
fn bind<I>(
    registry: &WlRegistry,
    globals: &[Global],
    qh: &QueueHandle<State>,
    max_version: u32,
) -> I
where
    I: Proxy + 'static,
    State: Dispatch<I, ()>,
{
    let interface = I::interface().name;
    let global = globals
        .iter()
        .find(|g| g.interface == interface)
        .unwrap_or_else(|| panic!("the compositor has no {interface}; roundtrip first"));
    registry.bind(global.name, global.version.min(max_version), qh, ())
}

/// The globals as one line each, `interface vN`, sorted so the text doesn't depend on the
/// order the compositor made them in.
pub fn format_globals(globals: &[Global]) -> String {
    let mut lines: Vec<_> = globals
        .iter()
        .map(|g| format!("{} v{}", g.interface, g.version))
        .collect();
    lines.sort();
    lines.join("\n")
}

/// The configures as one line each, without serials, and with states and capabilities sorted.
pub fn format_configures(configures: &[Configure]) -> String {
    configures
        .iter()
        .map(|c| c.to_string())
        .collect::<Vec<_>>()
        .join("\n")
}

impl fmt::Display for Configure {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "size: {} × {}, states: [", self.size.0, self.size.1)?;
        write_list(f, &self.states)?;
        write!(f, "]")?;
        if let Some((w, h)) = self.bounds {
            write!(f, ", bounds: {w} × {h}")?;
        }
        if let Some(capabilities) = &self.capabilities {
            write!(f, ", capabilities: [")?;
            write_list(f, capabilities)?;
            write!(f, "]")?;
        }
        Ok(())
    }
}

/// Writes `items` sorted by name, since states and capabilities are sets whose order on the
/// wire isn't stable.
fn write_list(f: &mut fmt::Formatter<'_>, items: &[impl fmt::Debug]) -> fmt::Result {
    let mut names: Vec<_> = items.iter().map(|item| format!("{item:?}")).collect();
    names.sort();
    write!(f, "{}", names.join(", "))
}

impl Dispatch<WlRegistry, ()> for State {
    fn event(
        state: &mut Self,
        _: &WlRegistry,
        event: wl_registry::Event,
        _: &(),
        _: &Connection,
        _: &QueueHandle<Self>,
    ) {
        match event {
            wl_registry::Event::Global {
                name,
                interface,
                version,
            } => state.globals.push(Global {
                name,
                interface,
                version,
            }),
            wl_registry::Event::GlobalRemove { name } => state.globals.retain(|g| g.name != name),
            _ => {}
        }
    }
}

impl Dispatch<WlCallback, Arc<AtomicBool>> for State {
    fn event(
        _: &mut Self,
        _: &WlCallback,
        event: wl_callback::Event,
        done: &Arc<AtomicBool>,
        _: &Connection,
        _: &QueueHandle<Self>,
    ) {
        if let wl_callback::Event::Done { .. } = event {
            done.store(true, Ordering::SeqCst);
        }
    }
}

impl Dispatch<XdgWmBase, ()> for State {
    fn event(
        _: &mut Self,
        wm_base: &XdgWmBase,
        event: xdg_wm_base::Event,
        _: &(),
        _: &Connection,
        _: &QueueHandle<Self>,
    ) {
        if let xdg_wm_base::Event::Ping { serial } = event {
            wm_base.pong(serial);
        }
    }
}

impl Dispatch<XdgSurface, WindowId> for State {
    fn event(
        state: &mut Self,
        _: &XdgSurface,
        event: xdg_surface::Event,
        id: &WindowId,
        _: &Connection,
        _: &QueueHandle<Self>,
    ) {
        if let xdg_surface::Event::Configure { serial } = event {
            let window = &mut state.windows[id.0];
            let mut configure = std::mem::take(&mut window.pending);
            configure.serial = serial;
            window.configures.push(configure);
        }
    }
}

impl Dispatch<XdgToplevel, WindowId> for State {
    fn event(
        state: &mut Self,
        _: &XdgToplevel,
        event: xdg_toplevel::Event,
        id: &WindowId,
        _: &Connection,
        _: &QueueHandle<Self>,
    ) {
        let window = &mut state.windows[id.0];
        match event {
            xdg_toplevel::Event::Configure {
                width,
                height,
                states,
            } => {
                window.pending.size = (width, height);
                window.pending.states = decode_enums(&states);
            }
            xdg_toplevel::Event::ConfigureBounds { width, height } => {
                window.pending.bounds = Some((width, height));
            }
            xdg_toplevel::Event::WmCapabilities { capabilities } => {
                window.pending.capabilities = Some(decode_enums(&capabilities));
            }
            xdg_toplevel::Event::Close => window.close_requested = true,
            _ => {}
        }
    }
}

/// An array of 32-bit enum values, as `xdg_toplevel` sends states and capabilities.
fn decode_enums<E: TryFrom<u32>>(bytes: &[u8]) -> Vec<E> {
    bytes
        .as_chunks::<4>()
        .0
        .iter()
        .filter_map(|b| match WEnum::<E>::from(u32::from_ne_bytes(*b)) {
            WEnum::Value(value) => Some(value),
            WEnum::Unknown(_) => None,
        })
        .collect()
}

/// Proxies whose events the client ignores.
macro_rules! ignore_events {
    ($($proxy:ty => $event:ty),* $(,)?) => {
        $(
            impl Dispatch<$proxy, ()> for State {
                fn event(
                    _: &mut Self,
                    _: &$proxy,
                    _: $event,
                    _: &(),
                    _: &Connection,
                    _: &QueueHandle<Self>,
                ) {
                }
            }
        )*
    };
}

ignore_events!(
    WlCompositor => wl_compositor::Event,
    WlSurface => wl_surface::Event,
    WlShm => wl_shm::Event,
    WlShmPool => wl_shm_pool::Event,
    WlBuffer => wl_buffer::Event,
    WlSeat => wl_seat::Event,
);
