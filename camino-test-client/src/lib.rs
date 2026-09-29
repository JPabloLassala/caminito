//! Test clients for Camino's integration tests (14, 15).
//!
//! A [`Client`] speaks Wayland over one end of a socket pair and never blocks: the test that
//! owns it runs the compositor in the same thread, and calls [`Client::dispatch`] between the
//! compositor's own dispatches. It records what the compositor sends, and the `Display` impls
//! turn those records into stable text for snapshots. The [`dbus`] proxies call Camino's
//! D-Bus names, and the [`x11`] client speaks X11 to Camino's Xwayland.

pub mod dbus;
pub mod selection;
pub mod x11;

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
use wayland_client::protocol::wl_data_device::WlDataDevice;
use wayland_client::protocol::wl_data_device_manager::WlDataDeviceManager;
use wayland_client::protocol::wl_keyboard::{self, WlKeyboard};
use wayland_client::protocol::wl_output::{self, WlOutput};
use wayland_client::protocol::wl_pointer::{self, WlPointer};
use wayland_client::protocol::wl_registry::{self, WlRegistry};
use wayland_client::protocol::wl_seat::{self, WlSeat};
use wayland_client::protocol::wl_shm::{self, WlShm};
use wayland_client::protocol::wl_shm_pool::{self, WlShmPool};
use wayland_client::protocol::wl_subcompositor::{self, WlSubcompositor};
use wayland_client::protocol::wl_subsurface::{self, WlSubsurface};
use wayland_client::protocol::wl_surface::{self, WlSurface};
use wayland_client::{Connection, Dispatch, EventQueue, Proxy, QueueHandle, WEnum};
use wayland_protocols::wp::keyboard_shortcuts_inhibit::zv1::client::zwp_keyboard_shortcuts_inhibit_manager_v1::{
    self, ZwpKeyboardShortcutsInhibitManagerV1,
};
use wayland_protocols::wp::keyboard_shortcuts_inhibit::zv1::client::zwp_keyboard_shortcuts_inhibitor_v1::{
    self, ZwpKeyboardShortcutsInhibitorV1,
};
use wayland_protocols::wp::pointer_constraints::zv1::client::zwp_locked_pointer_v1::{
    self, ZwpLockedPointerV1,
};
use wayland_protocols::wp::pointer_constraints::zv1::client::zwp_pointer_constraints_v1::{
    self, ZwpPointerConstraintsV1,
};
use wayland_protocols::wp::primary_selection::zv1::client::zwp_primary_selection_device_manager_v1::ZwpPrimarySelectionDeviceManagerV1;
use wayland_protocols::wp::primary_selection::zv1::client::zwp_primary_selection_device_v1::ZwpPrimarySelectionDeviceV1;
use wayland_protocols::xdg::decoration::zv1::client::zxdg_decoration_manager_v1::{
    self, ZxdgDecorationManagerV1,
};
use wayland_protocols::xdg::decoration::zv1::client::zxdg_toplevel_decoration_v1::{
    self, ZxdgToplevelDecorationV1,
};
use wayland_protocols::xdg::shell::client::xdg_popup::{self, XdgPopup};
use wayland_protocols::xdg::shell::client::xdg_positioner::{self, XdgPositioner};
use wayland_protocols::xdg::shell::client::xdg_surface::{self, XdgSurface};
use wayland_protocols::xdg::shell::client::xdg_toplevel::{self, XdgToplevel};
use wayland_protocols::xdg::shell::client::xdg_wm_base::{self, XdgWmBase};
use wayland_protocols_misc::server_decoration::client::org_kde_kwin_server_decoration::{
    self, OrgKdeKwinServerDecoration,
};
use wayland_protocols_misc::server_decoration::client::org_kde_kwin_server_decoration_manager::{
    self, OrgKdeKwinServerDecorationManager,
};

pub use wayland_protocols::xdg::decoration::zv1::client::zxdg_toplevel_decoration_v1::Mode as DecorationMode;
pub use wayland_protocols::xdg::shell::client::xdg_positioner::{
    Anchor, ConstraintAdjustment, Gravity,
};
pub use wayland_protocols::xdg::shell::client::xdg_toplevel::{ResizeEdge, State as ToplevelState};
pub use wayland_protocols_misc::server_decoration::client::org_kde_kwin_server_decoration::Mode as KdeMode;

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
    subcompositor: Option<WlSubcompositor>,
    wm_base: Option<XdgWmBase>,
    seat: Option<WlSeat>,
    decoration_manager: Option<ZxdgDecorationManagerV1>,
    shortcuts_inhibit_manager: Option<ZwpKeyboardShortcutsInhibitManagerV1>,
    pointer_constraints: Option<ZwpPointerConstraintsV1>,
    /// Whether the pointer lock the client asked for is active.
    pub pointer_locked: bool,
    kde_decoration_manager: Option<OrgKdeKwinServerDecorationManager>,
    /// The mode KDE server-decoration said is the default.
    pub kde_default_mode: Option<KdeMode>,
    windows: Vec<Window>,
    popups: Vec<Popup>,
    pointer: Option<WlPointer>,
    keyboard: Option<WlKeyboard>,
    /// Every pointer and keyboard event received, oldest first.
    pub input: Vec<InputEvent>,
    /// The serial of the last button press, which starts moves, resizes and popup grabs.
    pub last_press_serial: Option<u32>,
    /// The `wl_output`s bound so far, in the order the compositor advertised them.
    pub outputs: Vec<Output>,
    data_device_manager: Option<WlDataDeviceManager>,
    data_device: Option<WlDataDevice>,
    primary_manager: Option<ZwpPrimarySelectionDeviceManagerV1>,
    primary_device: Option<ZwpPrimarySelectionDeviceV1>,
    /// The selections the compositor last offered.
    offers: selection::Offers,
}

/// A bound `wl_output`.
pub struct Output {
    pub global_name: u32,
    pub proxy: WlOutput,
    /// The output's name, from `wl_output.name`.
    pub name: Option<String>,
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
    /// How many frame callbacks the compositor has answered.
    pub frames_done: usize,
    /// The window surface's `enter` and `leave` events, as `enter <output name>` and
    /// `leave <output name>`, oldest first.
    pub output_events: Vec<String>,
    /// Every mode KDE server-decoration has sent for the window, oldest first.
    pub kde_modes: Vec<KdeMode>,
    xdg_decoration: Option<ZxdgToplevelDecorationV1>,
    kde_decoration: Option<OrgKdeKwinServerDecoration>,
    subsurfaces: Vec<WlSubsurface>,
    /// The buffers attached so far, kept alive until the window is dropped.
    buffers: Vec<Buffer>,
}

/// One of the client's surfaces, as input events name it.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum SurfaceRef {
    Window(WindowId),
    Popup(PopupId),
    Unknown,
}

/// A pointer or keyboard event.
#[derive(Debug, Clone, PartialEq)]
pub enum InputEvent {
    PointerEnter {
        surface: SurfaceRef,
        x: f64,
        y: f64,
    },
    PointerLeave {
        surface: SurfaceRef,
    },
    Button {
        button: u32,
        pressed: bool,
    },
    KeyboardEnter {
        surface: SurfaceRef,
    },
    KeyboardLeave {
        surface: SurfaceRef,
    },
    /// An evdev key code.
    Key {
        key: u32,
        pressed: bool,
    },
}

/// The index of one of a client's popups.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct PopupId(usize);

/// An xdg popup.
pub struct Popup {
    pub surface: WlSurface,
    pub xdg_surface: XdgSurface,
    pub popup: XdgPopup,
    /// Every configure received, oldest first.
    pub configures: Vec<PopupConfigure>,
    pending: PopupConfigure,
    /// Whether the compositor dismissed the popup.
    pub done: bool,
    buffers: Vec<Buffer>,
}

/// One popup configure sequence: its place relative to the parent's window geometry, and the
/// reposition token it answers, if any.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct PopupConfigure {
    pub x: i32,
    pub y: i32,
    pub size: (i32, i32),
    pub repositioned: Option<u32>,
    pub serial: u32,
}

/// How a popup is positioned, as `xdg_positioner` takes it.
#[derive(Debug, Clone, Copy)]
pub struct Positioner {
    pub size: (i32, i32),
    /// x, y, width and height, relative to the parent's window geometry.
    pub anchor_rect: (i32, i32, i32, i32),
    pub anchor: Anchor,
    pub gravity: Gravity,
    pub constraint_adjustment: ConstraintAdjustment,
}

/// One configure sequence: the toplevel events, then the `xdg_surface.configure` ending it.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct Configure {
    pub size: (i32, i32),
    pub states: Vec<xdg_toplevel::State>,
    pub bounds: Option<(i32, i32)>,
    pub capabilities: Option<Vec<xdg_toplevel::WmCapabilities>>,
    /// The mode xdg-decoration configured, if it sent one.
    pub decoration: Option<DecorationMode>,
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
            frames_done: 0,
            output_events: Vec::new(),
            kde_modes: Vec::new(),
            xdg_decoration: None,
            kde_decoration: None,
            subsurfaces: Vec::new(),
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

    /// Creates a popup of `parent` placed by `positioner`, and commits it without a buffer,
    /// which asks for its initial configure.
    pub fn create_popup(&mut self, parent: WindowId, positioner: Positioner) -> PopupId {
        let id = PopupId(self.state.popups.len());
        let surface = self.compositor().create_surface(&self.qh, ());
        let xdg_surface = self.wm_base().get_xdg_surface(&surface, &self.qh, id);
        let xdg_positioner = self.positioner(positioner);
        let popup = xdg_surface.get_popup(
            Some(&self.window(parent).xdg_surface),
            &xdg_positioner,
            &self.qh,
            id,
        );
        xdg_positioner.destroy();
        surface.commit();
        self.state.popups.push(Popup {
            surface,
            xdg_surface,
            popup,
            configures: Vec::new(),
            pending: PopupConfigure::default(),
            done: false,
            buffers: Vec::new(),
        });
        id
    }

    pub fn popup(&self, id: PopupId) -> &Popup {
        &self.state.popups[id.0]
    }

    /// Acks the popup's latest configure and commits a buffer of its configured size.
    pub fn map_popup(&mut self, id: PopupId, argb: u32) {
        let configure = self
            .popup(id)
            .configures
            .last()
            .cloned()
            .expect("configured");
        let popup = &self.state.popups[id.0];
        popup.xdg_surface.ack_configure(configure.serial);
        let (pool, buffer) = self.shm_buffer(configure.size, argb);
        let popup = &mut self.state.popups[id.0];
        popup.surface.attach(Some(&buffer), 0, 0);
        popup
            .surface
            .damage_buffer(0, 0, configure.size.0, configure.size.1);
        popup.surface.commit();
        popup.buffers.push(Buffer { buffer, pool });
    }

    /// Asks for the popup to be placed again by `positioner`.
    pub fn reposition_popup(&mut self, id: PopupId, positioner: Positioner, token: u32) {
        let xdg_positioner = self.positioner(positioner);
        self.popup(id).popup.reposition(&xdg_positioner, token);
        xdg_positioner.destroy();
    }

    /// Asks for an explicit grab for the popup, as if `serial`'s button press opened it.
    pub fn grab_popup(&mut self, id: PopupId, serial: u32) {
        let seat = self.seat();
        self.popup(id).popup.grab(&seat, serial);
    }

    fn positioner(&mut self, positioner: Positioner) -> XdgPositioner {
        let xdg_positioner = self.wm_base().create_positioner(&self.qh, ());
        let (w, h) = positioner.size;
        let (x, y, aw, ah) = positioner.anchor_rect;
        xdg_positioner.set_size(w, h);
        xdg_positioner.set_anchor_rect(x, y, aw, ah);
        xdg_positioner.set_anchor(positioner.anchor);
        xdg_positioner.set_gravity(positioner.gravity);
        xdg_positioner.set_constraint_adjustment(positioner.constraint_adjustment);
        xdg_positioner
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
        let (pool, buffer) = self.shm_buffer(size, argb);
        let window = self.window_mut(id);
        window.surface.attach(Some(&buffer), 0, 0);
        window.surface.damage_buffer(0, 0, size.0, size.1);
        window.buffers.push(Buffer { buffer, pool });
    }

    /// A new shm buffer of `size` filled with `argb`, and its pool.
    fn shm_buffer(&mut self, size: (i32, i32), argb: u32) -> (WlShmPool, WlBuffer) {
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
        (pool, buffer)
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

    /// The window's `wl_surface` object id, which the compositor sees too.
    pub fn surface_protocol_id(&self, id: WindowId) -> u32 {
        self.window(id).surface.id().protocol_id()
    }

    /// Gives the window a desynchronised subsurface at `offset` with a committed `size`
    /// buffer of `argb`, and returns its surface. The parent must commit for it to show.
    pub fn add_subsurface(
        &mut self,
        id: WindowId,
        offset: (i32, i32),
        size: (i32, i32),
        argb: u32,
    ) -> WlSurface {
        let subcompositor = self.global(|state| &mut state.subcompositor, 1);
        let surface = self.compositor().create_surface(&self.qh, ());
        let subsurface =
            subcompositor.get_subsurface(&surface, &self.window(id).surface, &self.qh, ());
        subsurface.set_position(offset.0, offset.1);
        subsurface.set_desync();
        self.commit_subsurface_buffer(id, &surface, size, argb);
        self.window_mut(id).subsurfaces.push(subsurface);
        surface
    }

    /// Attaches and commits a new `size` buffer of `argb` to one of the window's subsurfaces.
    pub fn commit_subsurface_buffer(
        &mut self,
        id: WindowId,
        surface: &WlSurface,
        size: (i32, i32),
        argb: u32,
    ) {
        let (pool, buffer) = self.shm_buffer(size, argb);
        surface.attach(Some(&buffer), 0, 0);
        surface.damage_buffer(0, 0, size.0, size.1);
        surface.commit();
        self.window_mut(id).buffers.push(Buffer { buffer, pool });
    }

    /// Attaches no buffer and commits, which unmaps the window.
    pub fn commit_null_buffer(&self, id: WindowId) {
        let surface = &self.window(id).surface;
        surface.attach(None, 0, 0);
        surface.commit();
    }

    /// Asks for a frame callback on the next commit.
    pub fn request_frame(&self, id: WindowId) {
        self.window(id).surface.frame(&self.qh, FrameCallback(id));
    }

    /// Destroys the toplevel and its surfaces.
    pub fn destroy_window(&self, id: WindowId) {
        let window = self.window(id);
        window.toplevel.destroy();
        window.xdg_surface.destroy();
        window.surface.destroy();
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

    pub fn set_maximized(&self, id: WindowId) {
        self.window(id).toplevel.set_maximized();
    }

    pub fn unset_maximized(&self, id: WindowId) {
        self.window(id).toplevel.unset_maximized();
    }

    /// Asks for fullscreen, on the `output`th bound output if given.
    pub fn set_fullscreen(&self, id: WindowId, output: Option<usize>) {
        let output = output.map(|i| &self.state.outputs[i].proxy);
        self.window(id).toplevel.set_fullscreen(output);
    }

    pub fn unset_fullscreen(&self, id: WindowId) {
        self.window(id).toplevel.unset_fullscreen();
    }

    /// Gets the seat's pointer and keyboard, so input events are recorded.
    pub fn bind_input(&mut self) {
        let seat = self.seat();
        if self.state.pointer.is_none() {
            self.state.pointer = Some(seat.get_pointer(&self.qh, ()));
        }
        if self.state.keyboard.is_none() {
            self.state.keyboard = Some(seat.get_keyboard(&self.qh, ()));
        }
    }

    /// Asks for the pointer to be locked in place while it's over the window.
    pub fn lock_pointer(&mut self, id: WindowId) {
        let constraints = self.global(|state| &mut state.pointer_constraints, 1);
        let pointer = self.state.pointer.clone().expect("bind_input first");
        constraints.lock_pointer(
            &self.window(id).surface,
            &pointer,
            None,
            zwp_pointer_constraints_v1::Lifetime::Persistent,
            &self.qh,
            (),
        );
    }

    /// Asks for the compositor's shortcuts to be inhibited while the window has the keyboard.
    pub fn inhibit_shortcuts(&mut self, id: WindowId) {
        let manager = self.global(|state| &mut state.shortcuts_inhibit_manager, 1);
        let seat = self.seat();
        manager.inhibit_shortcuts(&self.window(id).surface, &seat, &self.qh, ());
    }

    /// Asks xdg-decoration for `mode`, or says it has no preference, making the window's
    /// decoration object first if it has none.
    pub fn request_decoration(&mut self, id: WindowId, mode: Option<DecorationMode>) {
        if self.window(id).xdg_decoration.is_none() {
            self.create_decoration(id);
        }
        let decoration = self.window(id).xdg_decoration.as_ref().unwrap();
        match mode {
            Some(mode) => decoration.set_mode(mode),
            None => decoration.unset_mode(),
        }
    }

    /// Makes the window's xdg-decoration object without asking for a mode.
    pub fn create_decoration(&mut self, id: WindowId) {
        let manager = self.global(|state| &mut state.decoration_manager, 1);
        let decoration = manager.get_toplevel_decoration(&self.window(id).toplevel, &self.qh, id);
        self.window_mut(id).xdg_decoration = Some(decoration);
    }

    /// Makes the window's KDE server-decoration object, and asks for `mode` if given.
    pub fn kde_request_decoration(&mut self, id: WindowId, mode: Option<KdeMode>) {
        if self.window(id).kde_decoration.is_none() {
            let manager = self.global(|state| &mut state.kde_decoration_manager, 1);
            let decoration = manager.create(&self.window(id).surface, &self.qh, id);
            self.window_mut(id).kde_decoration = Some(decoration);
        }
        if let Some(mode) = mode {
            self.window(id)
                .kde_decoration
                .as_ref()
                .unwrap()
                .request_mode(mode);
        }
    }

    /// Binds every `wl_output` not yet bound. Roundtrip before reading their names.
    pub fn bind_outputs(&mut self) {
        let unbound: Vec<Global> = self
            .state
            .globals
            .iter()
            .filter(|g| g.interface == WlOutput::interface().name)
            .filter(|g| !self.state.outputs.iter().any(|o| o.global_name == g.name))
            .cloned()
            .collect();
        for global in unbound {
            let proxy =
                self.registry
                    .bind(global.name, global.version.min(4), &self.qh, global.name);
            self.state.outputs.push(Output {
                global_name: global.name,
                proxy,
                name: None,
            });
        }
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

/// The popup configures as one line each, without serials.
pub fn format_popup_configures(configures: &[PopupConfigure]) -> String {
    configures
        .iter()
        .map(|c| {
            let mut line = format!("at {}, {}, size: {} × {}", c.x, c.y, c.size.0, c.size.1);
            if let Some(token) = c.repositioned {
                line += &format!(", repositioned: {token}");
            }
            line
        })
        .collect::<Vec<_>>()
        .join("\n")
}

/// The input events as one line each.
pub fn format_input(events: &[InputEvent]) -> String {
    let surface = |s: &SurfaceRef| match s {
        SurfaceRef::Window(w) => format!("window {}", w.0),
        SurfaceRef::Popup(p) => format!("popup {}", p.0),
        SurfaceRef::Unknown => "unknown surface".to_owned(),
    };
    let press = |pressed: &bool| if *pressed { "pressed" } else { "released" };
    events
        .iter()
        .map(|event| match event {
            InputEvent::PointerEnter { surface: s, x, y } => {
                format!("pointer enters {} at {x}, {y}", surface(s))
            }
            InputEvent::PointerLeave { surface: s } => format!("pointer leaves {}", surface(s)),
            InputEvent::Button { button, pressed } => {
                format!("button {button:#x} {}", press(pressed))
            }
            InputEvent::KeyboardEnter { surface: s } => format!("keyboard enters {}", surface(s)),
            InputEvent::KeyboardLeave { surface: s } => format!("keyboard leaves {}", surface(s)),
            InputEvent::Key { key, pressed } => format!("key {key} {}", press(pressed)),
        })
        .collect::<Vec<_>>()
        .join("\n")
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
        if let Some(mode) = self.decoration {
            write!(f, ", decoration: {mode:?}")?;
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

impl State {
    fn surface_ref(&self, surface: &WlSurface) -> SurfaceRef {
        if let Some(i) = self.windows.iter().position(|w| w.surface == *surface) {
            SurfaceRef::Window(WindowId(i))
        } else if let Some(i) = self.popups.iter().position(|p| p.surface == *surface) {
            SurfaceRef::Popup(PopupId(i))
        } else {
            SurfaceRef::Unknown
        }
    }
}

impl Dispatch<WlSurface, ()> for State {
    fn event(
        state: &mut Self,
        surface: &WlSurface,
        event: wl_surface::Event,
        _: &(),
        _: &Connection,
        _: &QueueHandle<Self>,
    ) {
        let (verb, output) = match event {
            wl_surface::Event::Enter { output } => ("enter", output),
            wl_surface::Event::Leave { output } => ("leave", output),
            _ => return,
        };
        let name = state
            .outputs
            .iter()
            .find(|o| o.proxy == output)
            .and_then(|o| o.name.clone())
            .unwrap_or_else(|| "unknown output".to_owned());
        if let SurfaceRef::Window(id) = state.surface_ref(surface) {
            state.windows[id.0]
                .output_events
                .push(format!("{verb} {name}"));
        }
    }
}

impl Dispatch<WlPointer, ()> for State {
    fn event(
        state: &mut Self,
        _: &WlPointer,
        event: wl_pointer::Event,
        _: &(),
        _: &Connection,
        _: &QueueHandle<Self>,
    ) {
        let input = match event {
            wl_pointer::Event::Enter {
                surface,
                surface_x,
                surface_y,
                ..
            } => InputEvent::PointerEnter {
                surface: state.surface_ref(&surface),
                x: surface_x,
                y: surface_y,
            },
            wl_pointer::Event::Leave { surface, .. } => InputEvent::PointerLeave {
                surface: state.surface_ref(&surface),
            },
            wl_pointer::Event::Button {
                serial,
                button,
                state: button_state,
                ..
            } => {
                let pressed = button_state == WEnum::Value(wl_pointer::ButtonState::Pressed);
                if pressed {
                    state.last_press_serial = Some(serial);
                }
                InputEvent::Button { button, pressed }
            }
            _ => return,
        };
        state.input.push(input);
    }
}

impl Dispatch<WlKeyboard, ()> for State {
    fn event(
        state: &mut Self,
        _: &WlKeyboard,
        event: wl_keyboard::Event,
        _: &(),
        _: &Connection,
        _: &QueueHandle<Self>,
    ) {
        let input = match event {
            wl_keyboard::Event::Enter { surface, .. } => InputEvent::KeyboardEnter {
                surface: state.surface_ref(&surface),
            },
            wl_keyboard::Event::Leave { surface, .. } => InputEvent::KeyboardLeave {
                surface: state.surface_ref(&surface),
            },
            wl_keyboard::Event::Key {
                key,
                state: key_state,
                ..
            } => InputEvent::Key {
                key,
                pressed: key_state == WEnum::Value(wl_keyboard::KeyState::Pressed),
            },
            _ => return,
        };
        state.input.push(input);
    }
}

impl Dispatch<ZwpLockedPointerV1, ()> for State {
    fn event(
        state: &mut Self,
        _: &ZwpLockedPointerV1,
        event: zwp_locked_pointer_v1::Event,
        _: &(),
        _: &Connection,
        _: &QueueHandle<Self>,
    ) {
        match event {
            zwp_locked_pointer_v1::Event::Locked => state.pointer_locked = true,
            zwp_locked_pointer_v1::Event::Unlocked => state.pointer_locked = false,
            _ => {}
        }
    }
}

impl Dispatch<ZxdgToplevelDecorationV1, WindowId> for State {
    fn event(
        state: &mut Self,
        _: &ZxdgToplevelDecorationV1,
        event: zxdg_toplevel_decoration_v1::Event,
        id: &WindowId,
        _: &Connection,
        _: &QueueHandle<Self>,
    ) {
        if let zxdg_toplevel_decoration_v1::Event::Configure {
            mode: WEnum::Value(mode),
        } = event
        {
            state.windows[id.0].pending.decoration = Some(mode);
        }
    }
}

impl Dispatch<OrgKdeKwinServerDecorationManager, ()> for State {
    fn event(
        state: &mut Self,
        _: &OrgKdeKwinServerDecorationManager,
        event: org_kde_kwin_server_decoration_manager::Event,
        _: &(),
        _: &Connection,
        _: &QueueHandle<Self>,
    ) {
        // The manager's mode enum has the same values as the decoration's.
        if let org_kde_kwin_server_decoration_manager::Event::DefaultMode {
            mode: WEnum::Value(mode),
        } = event
        {
            state.kde_default_mode = KdeMode::try_from(mode as u32).ok();
        }
    }
}

impl Dispatch<OrgKdeKwinServerDecoration, WindowId> for State {
    fn event(
        state: &mut Self,
        _: &OrgKdeKwinServerDecoration,
        event: org_kde_kwin_server_decoration::Event,
        id: &WindowId,
        _: &Connection,
        _: &QueueHandle<Self>,
    ) {
        if let org_kde_kwin_server_decoration::Event::Mode {
            mode: WEnum::Value(mode),
        } = event
        {
            state.windows[id.0].kde_modes.push(mode);
        }
    }
}

impl Dispatch<WlOutput, u32> for State {
    fn event(
        state: &mut Self,
        _: &WlOutput,
        event: wl_output::Event,
        global_name: &u32,
        _: &Connection,
        _: &QueueHandle<Self>,
    ) {
        if let wl_output::Event::Name { name } = event
            && let Some(output) = state
                .outputs
                .iter_mut()
                .find(|o| o.global_name == *global_name)
        {
            output.name = Some(name);
        }
    }
}

/// A frame callback's user data: the window that asked for it.
struct FrameCallback(WindowId);

impl Dispatch<WlCallback, FrameCallback> for State {
    fn event(
        state: &mut Self,
        _: &WlCallback,
        event: wl_callback::Event,
        frame: &FrameCallback,
        _: &Connection,
        _: &QueueHandle<Self>,
    ) {
        if let wl_callback::Event::Done { .. } = event {
            state.windows[frame.0.0].frames_done += 1;
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

impl Dispatch<XdgSurface, PopupId> for State {
    fn event(
        state: &mut Self,
        _: &XdgSurface,
        event: xdg_surface::Event,
        id: &PopupId,
        _: &Connection,
        _: &QueueHandle<Self>,
    ) {
        if let xdg_surface::Event::Configure { serial } = event {
            let popup = &mut state.popups[id.0];
            let mut configure = std::mem::take(&mut popup.pending);
            configure.serial = serial;
            popup.configures.push(configure);
        }
    }
}

impl Dispatch<XdgPopup, PopupId> for State {
    fn event(
        state: &mut Self,
        _: &XdgPopup,
        event: xdg_popup::Event,
        id: &PopupId,
        _: &Connection,
        _: &QueueHandle<Self>,
    ) {
        let popup = &mut state.popups[id.0];
        match event {
            xdg_popup::Event::Configure {
                x,
                y,
                width,
                height,
            } => {
                popup.pending.x = x;
                popup.pending.y = y;
                popup.pending.size = (width, height);
            }
            xdg_popup::Event::Repositioned { token } => popup.pending.repositioned = Some(token),
            xdg_popup::Event::PopupDone => popup.done = true,
            _ => {}
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
    WlShm => wl_shm::Event,
    WlShmPool => wl_shm_pool::Event,
    WlBuffer => wl_buffer::Event,
    WlSeat => wl_seat::Event,
    WlSubcompositor => wl_subcompositor::Event,
    WlSubsurface => wl_subsurface::Event,
    XdgPositioner => xdg_positioner::Event,
    ZxdgDecorationManagerV1 => zxdg_decoration_manager_v1::Event,
    ZwpPointerConstraintsV1 => zwp_pointer_constraints_v1::Event,
    ZwpKeyboardShortcutsInhibitManagerV1 => zwp_keyboard_shortcuts_inhibit_manager_v1::Event,
    ZwpKeyboardShortcutsInhibitorV1 => zwp_keyboard_shortcuts_inhibitor_v1::Event,
);
