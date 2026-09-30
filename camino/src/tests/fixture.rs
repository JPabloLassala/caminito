//! `Fixture`: Camino on the headless backend and any number of test clients in one calloop
//! (14, 15). This is niri's `tests/fixture.rs` shape.
//!
//! Tests map windows of either kind with [`Fixture::map`], and ask about them through the
//! fixture's queries rather than `state.camino`, so a test of behaviour both kinds share can
//! run on both through [`on_both_kinds!`] (45).

use std::io::Read;
use std::os::fd::OwnedFd;
use std::os::unix::net::UnixStream;
use std::sync::Arc;
use std::sync::atomic::Ordering;
use std::time::Duration;

use camino_policy::PolicyWindow;
use camino_test_client::x11::{X11Client, XWindowSpec};
use camino_test_client::{Client, DecorationMode, WindowId};
use camino_theme::{ButtonFunction, ButtonState as FrameButtonState};
use smithay::backend::input::{ButtonState, InputTime, KeyState, Keycode};
use smithay::backend::renderer::utils::with_renderer_surface_state;
use smithay::output::Output;
use smithay::reexports::calloop::EventLoop;
use smithay::reexports::wayland_server::Display;
use smithay::reexports::wayland_server::Resource;
use smithay::reexports::wayland_server::protocol::wl_surface::WlSurface;
use smithay::utils::{FrameExtents, Logical, Physical, Point, Rectangle, Size};
use smithay::wayland::seat::WaylandFocus;
use smithay::xwayland::X11Surface;
use x11rb::protocol::Event;

use crate::decoration::control::FrameControl;
use crate::settings::{self, Value};
use crate::state::{ClientState, State};
use crate::window::Mapped;

/// The vendored schemas under `tests/schemas/` (14), compiled once per test run beside the
/// test binary.
fn test_schemas() -> &'static std::path::Path {
    static DIR: std::sync::OnceLock<std::path::PathBuf> = std::sync::OnceLock::new();
    DIR.get_or_init(|| {
        let exe = std::env::current_exe().unwrap();
        let dir = exe.parent().unwrap().join("camino-test-schemas");
        std::fs::create_dir_all(&dir).unwrap();
        let sources = concat!(env!("CARGO_MANIFEST_DIR"), "/../tests/schemas");
        let status = std::process::Command::new("glib-compile-schemas")
            .arg(format!("--targetdir={}", dir.display()))
            .arg(sources)
            .status()
            .expect("glib-compile-schemas runs");
        assert!(status.success(), "the test schemas compile");
        dir
    })
}

// Linux input event codes.
pub(crate) use crate::input::{BTN_LEFT, BTN_MIDDLE, BTN_RIGHT};
pub const KEY_TAB: u32 = 15;
pub const KEY_LEFTCTRL: u32 = 29;
pub const KEY_A: u32 = 30;
pub const KEY_LEFTALT: u32 = 56;
pub const KEY_F4: u32 = 62;
pub const KEY_F10: u32 = 68;
pub const KEY_NUMLOCK: u32 = 69;
pub const KEY_LEFT: u32 = 105;
pub const KEY_RIGHT: u32 = 106;

/// The colour test windows are filled with.
pub const WINDOW_COLOR: u32 = 0xff00ff00;

/// How many dispatches a roundtrip may take before the test fails rather than hangs.
const MAX_ROUNDTRIP_DISPATCHES: usize = 100;

type PolicyId = camino_policy::WindowId;

/// Expands one test body, which takes the `Kind` of client it runs with, into a module of
/// two tests, `<name>::xdg` and `<name>::x11`, so each tier runs, filters and fails on its
/// own.
macro_rules! on_both_kinds {
    ($(#[$meta:meta])* fn $name:ident($kind:ident: Kind) $body:block) => {
        mod $name {
            #[allow(unused_imports)]
            use super::*;

            $(#[$meta])*
            fn body($kind: $crate::tests::fixture::Kind) $body

            #[test]
            fn xdg() {
                body($crate::tests::fixture::Kind::Xdg);
            }

            #[test]
            fn x11() {
                body($crate::tests::fixture::Kind::X11);
            }
        }
    };
}
pub(crate) use on_both_kinds;

/// Which protocol a client speaks.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Kind {
    /// A Wayland client, whose windows are xdg toplevels.
    Xdg,
    /// An X client on Xwayland.
    X11,
}

pub struct Fixture {
    event_loop: EventLoop<'static, State>,
    pub state: State,
    clients: Vec<TestClient>,
    input_millis: u32,
}

enum TestClient {
    /// A Wayland client, and itself as the compositor knows it.
    Wayland {
        client: Box<Client>,
        server: smithay::reexports::wayland_server::Client,
    },
    X11(Arc<X11Client>),
    /// An X client the test disconnected.
    Disconnected,
}

/// The index of one of a fixture's clients, of either kind.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct ClientId(usize);

/// A test's window, whichever protocol its client speaks.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ClientWindow {
    /// An xdg toplevel of a Wayland client.
    Xdg(ClientId, WindowId),
    /// An X window of an X client, by its id.
    X11(ClientId, u32),
}

impl ClientWindow {
    pub fn client(self) -> ClientId {
        match self {
            ClientWindow::Xdg(client, _) | ClientWindow::X11(client, _) => client,
        }
    }

    /// The xdg toplevel, for requests only an xdg client makes.
    pub fn xdg(self) -> WindowId {
        match self {
            ClientWindow::Xdg(_, window) => window,
            ClientWindow::X11(..) => panic!("{self:?} is an X window, not an xdg toplevel"),
        }
    }

    /// The X window's id, for requests only an X client makes.
    pub fn x11(self) -> u32 {
        match self {
            ClientWindow::X11(_, window) => window,
            ClientWindow::Xdg(..) => panic!("{self:?} is an xdg toplevel, not an X window"),
        }
    }
}

/// What a window asks about its decoration.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Decoration {
    /// A frame: xdg-decoration's server-side mode, or no `_MOTIF_WM_HINTS`.
    Server,
    /// Its own: xdg-decoration's client-side mode, or `_MOTIF_WM_HINTS` without decorations.
    Client,
    /// xdg only: an xdg-decoration object without a mode.
    NoPreference,
    /// Nothing: no xdg-decoration object, and no `_MOTIF_WM_HINTS`.
    Unspoken,
}

/// A window for [`Fixture::map`] to map, as either kind of client asks for it. A field only
/// one kind can ask for panics on the other.
#[derive(Debug, Clone)]
pub struct WindowSpec {
    /// The size its client draws it at.
    pub size: (i32, i32),
    pub decoration: Decoration,
    /// The xdg title, or `_NET_WM_NAME`.
    pub title: Option<&'static str>,
    /// The xdg app id, or `WM_CLASS`'s class.
    pub app_id: Option<&'static str>,
    /// A window of the same client it's transient for: the xdg parent, or
    /// `WM_TRANSIENT_FOR`.
    pub parent: Option<ClientWindow>,
    /// X11 only: where it's created, with `USPosition`.
    pub position: Option<(i16, i16)>,
    /// X11 only: `_NET_WM_WINDOW_TYPE`'s atoms, most preferred first.
    pub types: Vec<&'static str>,
    /// X11 only: `_NET_WM_STATE`'s atoms before it's mapped.
    pub states: Vec<&'static str>,
    /// X11 only: `WM_HINTS`'s input flag; `None` leaves `WM_HINTS` unset.
    pub input: Option<bool>,
    /// X11 only: `CARDINAL` properties set before it's mapped, such as `_NET_WM_DESKTOP`.
    pub cardinals: Vec<(&'static str, Vec<u32>)>,
    /// Whether it can be asked to close: `WM_DELETE_WINDOW` in `WM_PROTOCOLS`, as toolkits
    /// set it. An xdg toplevel always can.
    pub delete_window: bool,
    /// xdg only: whether it makes a `wp_fractional_scale_v1` before its first commit, as
    /// GTK4 and Qt 6 do.
    pub fractional_scale: bool,
}

impl Default for WindowSpec {
    /// A 400 × 300 window that asks for a frame, as a MATE app does.
    fn default() -> Self {
        WindowSpec {
            size: (400, 300),
            decoration: Decoration::Server,
            title: None,
            app_id: None,
            parent: None,
            position: None,
            types: Vec::new(),
            states: Vec::new(),
            input: None,
            cardinals: Vec::new(),
            delete_window: true,
            fractional_scale: false,
        }
    }
}

impl WindowSpec {
    /// A window of `size` that says nothing about its decoration, so Camino draws no frame
    /// for it if it's an xdg toplevel.
    pub fn plain(size: (i32, i32)) -> Self {
        WindowSpec {
            size,
            decoration: Decoration::Unspoken,
            ..WindowSpec::default()
        }
    }
}

impl Fixture {
    /// Camino with no outputs and no clients, and the settings at their defaults.
    pub fn new() -> Self {
        Self::with_settings(Vec::new())
    }

    /// Camino with no outputs and no clients, on settings of its own with `initial` set
    /// before the first snapshot.
    pub fn with_settings(initial: Vec<(&'static str, String, Value)>) -> Self {
        let event_loop = EventLoop::try_new().unwrap();
        let settings = settings::Source::Test {
            schema_dir: test_schemas().to_owned(),
            initial,
        };
        let state = State::new_headless(&event_loop, Display::new().unwrap(), settings);
        Self {
            event_loop,
            state,
            clients: Vec::new(),
            input_millis: 0,
        }
    }

    /// Adds an output of `size` pixels at `scale`, as output management would set it, at
    /// the default place.
    pub fn add_output(&mut self, size: impl Into<Size<i32, Physical>>, scale: f64) -> Output {
        let output = self.state.plug_headless_output((0, 0).into(), size.into());
        self.state.camino.set_output_scale(&output, scale);
        output
    }

    /// Plugs in a monitor whose EDID says it's `physical_size` millimetres, with one mode
    /// of `mode` pixels, which it prefers. Camino picks its scale and place.
    pub fn plug(&mut self, physical_size: (i32, i32), mode: (i32, i32)) -> Output {
        self.state
            .plug_headless_output(physical_size.into(), mode.into())
    }

    /// Unplugs a monitor, and dispatches.
    pub fn unplug(&mut self, output: &Output) {
        self.state.unplug_headless_output(output);
        self.dispatch();
    }

    /// Switches a monitor to a mode of `mode` pixels, as the backend does, and dispatches.
    pub fn change_mode(&mut self, output: &Output, mode: (i32, i32)) {
        self.state.change_headless_mode(output, mode.into());
        self.dispatch();
    }

    /// Changes an output's scale, as output management will, and dispatches.
    pub fn set_output_scale(&mut self, output: &Output, scale: f64) {
        self.state.camino.set_output_scale(output, scale);
        self.dispatch();
    }

    /// Moves an output, as output management will, and dispatches.
    pub fn set_output_position(&mut self, output: &Output, position: (i32, i32)) {
        self.state
            .camino
            .set_output_position(output, position.into());
        self.dispatch();
    }

    /// Connects a new Wayland client over a socket pair. It has seen nothing until a
    /// roundtrip.
    pub fn add_client(&mut self) -> ClientId {
        let (server, client) = UnixStream::pair().unwrap();
        let server = self
            .state
            .camino
            .display_handle
            .insert_client(server, Arc::new(ClientState::default()))
            .unwrap();
        self.clients.push(TestClient::Wayland {
            client: Box::new(Client::new(client)),
            server,
        });
        ClientId(self.clients.len() - 1)
    }

    /// Connects a client of `kind` that has seen the globals, or the root, starting
    /// Xwayland first if it isn't running.
    pub fn connect(&mut self, kind: Kind) -> ClientId {
        match kind {
            Kind::Xdg => {
                let id = self.add_client();
                self.roundtrip(id);
                id
            }
            Kind::X11 => {
                if self.state.camino.xwayland_display().is_none() {
                    self.start_xwayland();
                }
                self.connect_x11()
            }
        }
    }

    /// One 1920 × 1080 output and a connected client of `kind`.
    pub fn connected(kind: Kind) -> (Self, ClientId) {
        let mut f = Self::new();
        f.add_output((1920, 1080), 1.0);
        let id = f.connect(kind);
        (f, id)
    }

    /// Maps a window of `client` as `spec` says. It returns once policy manages the window,
    /// its frame is drawn, and the client has seen everything Camino sent it.
    #[track_caller]
    pub fn map(&mut self, client: ClientId, spec: WindowSpec) -> ClientWindow {
        let window = match &self.clients[client.0] {
            TestClient::Wayland { .. } => self.map_xdg(client, spec),
            TestClient::X11(_) => self.map_x11(client, spec),
            TestClient::Disconnected => panic!("{client:?} is disconnected"),
        };
        let managed = self.mapped(window);
        assert!(
            !managed.shows_frame() || managed.frame().flags().is_some(),
            "{window:?}'s frame is drawn"
        );
        window
    }

    /// Creates an xdg toplevel, lets it have its initial configure, then acks it and commits
    /// a buffer, which maps it.
    fn map_xdg(&mut self, id: ClientId, spec: WindowSpec) -> ClientWindow {
        assert!(
            spec.position.is_none()
                && spec.types.is_empty()
                && spec.states.is_empty()
                && spec.input.is_none()
                && spec.cardinals.is_empty(),
            "an xdg toplevel can't ask for X properties: {spec:?}"
        );
        assert!(spec.delete_window, "an xdg toplevel can always be closed");
        let client = self.client(id);
        let window = client.create_window();
        if spec.fractional_scale {
            client.request_fractional_scale(window);
        }
        if let Some(title) = spec.title {
            client.set_title(window, title);
        }
        if let Some(app_id) = spec.app_id {
            client.set_app_id(window, app_id);
        }
        if let Some(parent) = spec.parent {
            assert_eq!(parent.client(), id, "a parent of the same client");
            client.set_parent(window, Some(parent.xdg()));
        }
        match spec.decoration {
            Decoration::Server => {
                client.request_decoration(window, Some(DecorationMode::ServerSide));
            }
            Decoration::Client => {
                client.request_decoration(window, Some(DecorationMode::ClientSide));
            }
            Decoration::NoPreference => client.create_decoration(window),
            Decoration::Unspoken => {}
        }
        client.commit(window);
        self.roundtrip(id);
        self.client(id)
            .ack_and_commit_buffer(window, spec.size, WINDOW_COLOR);
        self.roundtrip(id);
        let window = ClientWindow::Xdg(id, window);
        assert!(self.policy_id(window).is_some(), "{window:?} is managed");
        window
    }

    /// Creates and maps an X window, dispatches until Camino manages it and has drawn its
    /// frame, then until the client has seen Camino's answer.
    fn map_x11(&mut self, id: ClientId, spec: WindowSpec) -> ClientWindow {
        assert!(
            !spec.fractional_scale,
            "Xwayland makes an X window's surface: {spec:?}"
        );
        let decorations = match spec.decoration {
            Decoration::Server | Decoration::Unspoken => None,
            Decoration::Client => Some(false),
            Decoration::NoPreference => panic!("only xdg-decoration has no preference"),
        };
        let x_spec = XWindowSpec {
            size: (
                u16::try_from(spec.size.0).unwrap(),
                u16::try_from(spec.size.1).unwrap(),
            ),
            position: spec.position,
            types: spec.types,
            states: spec.states,
            input: spec.input,
            transient_for: spec.parent.map(|parent| {
                assert_eq!(parent.client(), id, "a parent of the same client");
                parent.x11()
            }),
            decorations,
            title: spec.title,
            class: spec.app_id,
            delete_window: spec.delete_window,
            empty: false,
            background: None,
            override_redirect: false,
            cardinals: spec.cardinals,
        };
        let window = self
            .x11_call(id, move |x| {
                let window = x.create_window(&x_spec)?;
                x.map(window)?;
                Ok::<_, x11rb::errors::ReplyOrIdError>(window)
            })
            .expect("the X window maps");
        let window = ClientWindow::X11(id, window);
        self.dispatch_until(|f| {
            f.policy_id(window).is_some_and(|pid| {
                let mapped = f.state.camino.policy.window(pid).unwrap();
                !mapped.shows_frame() || mapped.frame().flags().is_some()
            })
        });
        self.sync(window);
        window
    }

    /// Creates and maps an override-redirect window of the X client `id` at `position`,
    /// filled with `color`. It returns once Camino shows it, with content, in its
    /// override-redirect layer. Such windows aren't managed, so they're X ids, not
    /// `ClientWindow`s.
    #[track_caller]
    pub fn map_override_redirect(
        &mut self,
        id: ClientId,
        position: (i16, i16),
        size: (u16, u16),
        color: u32,
    ) -> u32 {
        let spec = XWindowSpec {
            size,
            position: Some(position),
            background: Some(color),
            override_redirect: true,
            ..XWindowSpec::default()
        };
        let window = self
            .x11_call(id, move |x| {
                let window = x.create_window(&spec)?;
                x.map(window)?;
                Ok::<_, x11rb::errors::ReplyOrIdError>(window)
            })
            .expect("the override-redirect window maps");
        self.dispatch_until(|f| {
            f.state
                .camino
                .override_redirect_windows()
                .any(|surface| surface.window_id() == window && has_buffer(surface))
        });
        window
    }

    /// The override-redirect window whose surface has pointer focus, by X id.
    pub fn override_redirect_under_pointer(&self) -> Option<u32> {
        let (focus, _) = self.state.camino.pointer_focus.as_ref()?;
        self.override_redirect_for(&*focus.wl_surface()?)
    }

    /// The override-redirect window whose surface has keyboard focus, by X id.
    pub fn override_redirect_with_keyboard(&self) -> Option<u32> {
        let keyboard = self.state.camino.seat.get_keyboard().unwrap();
        let focus = keyboard.current_focus()?;
        self.override_redirect_for(&*focus.wl_surface()?)
    }

    /// Whether a managed window has the keyboard.
    pub fn has_keyboard(&self, window: ClientWindow) -> bool {
        let keyboard = self.state.camino.seat.get_keyboard().unwrap();
        keyboard.current_focus() == Some(self.mapped(window).keyboard_target())
    }

    fn override_redirect_for(&self, surface: &WlSurface) -> Option<u32> {
        self.state
            .camino
            .override_redirect_for_surface(surface)
            .map(X11Surface::window_id)
    }

    /// Dispatches until the first output shows `color` (`0xRRGGBB`, opaque) at the logical
    /// point `(x, y)`, and fails rather than hangs.
    #[track_caller]
    pub fn wait_for_pixel(&mut self, x: i32, y: i32, color: u32) {
        let output = self.state.camino.outputs[0].clone();
        let scale = output.current_scale().fractional_scale();
        let at = Point::<i32, Logical>::from((x, y)).to_physical_precise_round(scale);
        let expected = 0xff00_0000 | color;
        let deadline = std::time::Instant::now() + Duration::from_secs(5);
        loop {
            let pixel = self.state.backend.headless_mut().pixel(&output, at);
            if pixel == Some(expected) {
                return;
            }
            assert!(
                std::time::Instant::now() < deadline,
                "({x}, {y}) is {pixel:08x?}, not {expected:08x}, after 5 s"
            );
            self.dispatch_waiting();
        }
    }

    /// Policy's id for a window, if it's managed.
    pub fn policy_id(&self, window: ClientWindow) -> Option<PolicyId> {
        match window {
            ClientWindow::Xdg(id, window) => {
                let TestClient::Wayland { client, server } = &self.clients[id.0] else {
                    unreachable!("an xdg toplevel's client is a Wayland one");
                };
                let protocol_id = client.surface_protocol_id(window);
                self.state.camino.policy.windows().find_map(|mapped| {
                    let surface = mapped.wl_surface()?;
                    (surface.client().as_ref() == Some(server)
                        && surface.id().protocol_id() == protocol_id)
                        .then(|| mapped.id())
                })
            }
            ClientWindow::X11(_, window) => self.state.camino.policy.windows().find_map(|mapped| {
                let surface = mapped.window().x11_surface()?;
                (surface.window_id() == window).then(|| mapped.id())
            }),
        }
    }

    /// Policy's id for a window it manages.
    pub fn pid(&self, window: ClientWindow) -> PolicyId {
        self.policy_id(window)
            .unwrap_or_else(|| panic!("{window:?} is managed"))
    }

    /// A window policy manages.
    pub fn mapped(&self, window: ClientWindow) -> &Mapped {
        self.state.camino.policy.window(self.pid(window)).unwrap()
    }

    /// Where policy lays a window out, globally.
    pub fn geometry(&self, window: ClientWindow) -> Rectangle<i32, Logical> {
        self.state.camino.policy.geometry(self.pid(window)).unwrap()
    }

    /// The frame extents policy lays a window out with.
    pub fn frame_extents(&self, window: ClientWindow) -> FrameExtents<i32, Logical> {
        self.state.camino.policy.frame_extents(self.pid(window))
    }

    /// The visible frame around a window, globally: its geometry grown by its extents.
    pub fn visible_frame(&self, window: ClientWindow) -> Rectangle<i32, Logical> {
        let geometry = self.geometry(window);
        let e = self.frame_extents(window);
        Rectangle::new(
            geometry.loc - Point::from((e.left, e.top)),
            (
                geometry.size.w + e.left + e.right,
                geometry.size.h + e.top + e.bottom,
            )
                .into(),
        )
    }

    /// Where a frame's button is drawn, globally.
    pub fn button_rect(
        &self,
        window: ClientWindow,
        function: ButtonFunction,
    ) -> Rectangle<i32, Logical> {
        let mut rect = self
            .mapped(window)
            .frame()
            .button_rect(function)
            .expect("the frame shows the button");
        rect.loc += self.geometry(window).loc;
        rect
    }

    /// How a frame's button is drawn.
    pub fn button_state(&self, window: ClientWindow, function: ButtonFunction) -> FrameButtonState {
        self.mapped(window).frame().button_states().get(function)
    }

    /// The colour at the middle of the icon a window's frame draws, as cairo's ARGB32.
    pub fn frame_icon_color(&self, window: ClientWindow) -> Option<u32> {
        self.mapped(window).frame().mini_icon_center()
    }

    /// The frame control under the pointer, if it's over a frame.
    pub fn control_under_pointer(&self) -> Option<FrameControl> {
        self.state
            .camino
            .frame_pointer
            .hover
            .map(|(_, control)| control)
    }

    /// Lets the window's client see what Camino sent it, and Camino what the client answered.
    pub fn sync(&mut self, window: ClientWindow) {
        match window {
            ClientWindow::Xdg(id, _) => self.roundtrip(id),
            ClientWindow::X11(id, _) => {
                // Xwayland has handled Camino's requests before it answers the client's, so
                // its answers to Camino are waiting on Camino's connection by then.
                self.x11_call(id, |x| x.sync()).unwrap();
                self.dispatch();
            }
        }
    }

    /// Whether Camino has asked the window's client to close it since this was last asked.
    pub fn close_requested(&mut self, window: ClientWindow) -> bool {
        match window {
            ClientWindow::Xdg(id, window) => self.client(id).window(window).close_requested,
            ClientWindow::X11(id, window) => self
                .x11_call(id, move |x| {
                    let protocols = x.atom("WM_PROTOCOLS")?;
                    let delete = x.atom("WM_DELETE_WINDOW")?;
                    Ok::<_, x11rb::errors::ReplyError>(x.events()?.iter().any(|event| {
                        matches!(event, Event::ClientMessage(message)
                            if message.window == window
                                && message.type_ == protocols
                                && message.data.as_data32()[0] == delete)
                    }))
                })
                .unwrap(),
        }
    }

    /// Has the client draw at the size Camino last asked of it, `size`: an xdg client acks
    /// and commits, and Xwayland redraws the window once X has resized it.
    #[track_caller]
    pub fn draw_at_asked_size(&mut self, window: ClientWindow, size: (i32, i32)) {
        match window {
            ClientWindow::Xdg(id, window) => {
                self.roundtrip(id);
                let asked = self
                    .client(id)
                    .window(window)
                    .configures
                    .last()
                    .unwrap()
                    .size;
                assert_eq!(asked, size);
                self.client(id)
                    .ack_and_commit_buffer(window, size, WINDOW_COLOR);
                self.roundtrip(id);
            }
            ClientWindow::X11(..) => {
                self.dispatch_until(|f| f.geometry(window).size == size.into());
            }
        }
    }

    /// A made-up input timestamp, later than the last.
    fn input_time(&mut self) -> InputTime {
        self.input_millis += 10;
        InputTime::from_millis(self.input_millis)
    }

    /// Moves the pointer to `(x, y)` in global logical coordinates.
    pub fn pointer_move(&mut self, x: f64, y: f64) {
        let time = self.input_time();
        self.state.on_pointer_motion((x, y).into(), time);
        self.dispatch();
    }

    /// Moves the pointer by `(dx, dy)`, as a mouse does.
    pub fn pointer_move_by(&mut self, dx: f64, dy: f64) {
        let time = self.input_time();
        self.state
            .on_pointer_motion_relative((dx, dy).into(), (dx, dy).into(), time);
        self.dispatch();
    }

    /// Presses or releases a pointer button, such as `BTN_LEFT`.
    pub fn pointer_button(&mut self, button: u32, pressed: bool) {
        let time = self.input_time();
        let state = if pressed {
            ButtonState::Pressed
        } else {
            ButtonState::Released
        };
        self.state.on_pointer_button(button, state, time);
        self.dispatch();
    }

    /// Moves the pointer to `(x, y)`, then presses and releases `BTN_LEFT` there.
    pub fn click(&mut self, x: f64, y: f64) {
        self.pointer_move(x, y);
        self.pointer_button(BTN_LEFT, true);
        self.pointer_button(BTN_LEFT, false);
    }

    /// Presses or releases a key by its evdev code, such as `KEY_A`.
    pub fn key(&mut self, key: u32, pressed: bool) {
        let time = self.input_time();
        let state = if pressed {
            KeyState::Pressed
        } else {
            KeyState::Released
        };
        // xkb key codes are evdev's plus 8.
        self.state.on_key(Keycode::new(key + 8), state, time);
        self.dispatch();
    }

    /// Presses `keys` in order, then releases them in reverse.
    pub fn chord(&mut self, keys: &[u32]) {
        for key in keys {
            self.key(*key, true);
        }
        for key in keys.iter().rev() {
            self.key(*key, false);
        }
    }

    /// A Wayland client.
    pub fn client(&mut self, id: ClientId) -> &mut Client {
        match &mut self.clients[id.0] {
            TestClient::Wayland { client, .. } => client,
            _ => panic!("{id:?} isn't a Wayland client"),
        }
    }

    /// A Wayland client, to read what it was told.
    pub fn client_state(&self, id: ClientId) -> &camino_test_client::State {
        match &self.clients[id.0] {
            TestClient::Wayland { client, .. } => &client.state,
            _ => panic!("{id:?} isn't a Wayland client"),
        }
    }

    /// Reads a pipe to its end, dispatching while its writer, which may need the loop,
    /// writes it, and fails rather than hangs.
    pub fn read_to_end(&mut self, fd: OwnedFd) -> Vec<u8> {
        rustix::io::ioctl_fionbio(&fd, true).expect("a non-blocking pipe");
        let mut file = std::fs::File::from(fd);
        let mut read = Vec::new();
        let deadline = std::time::Instant::now() + Duration::from_secs(5);
        loop {
            match file.read_to_end(&mut read) {
                Ok(_) => return read,
                Err(err) if err.kind() == std::io::ErrorKind::WouldBlock => {}
                Err(err) => panic!("can't read the pipe: {err}"),
            }
            assert!(
                std::time::Instant::now() < deadline,
                "the pipe didn't close within 5 s"
            );
            self.dispatch_waiting();
        }
    }

    fn wayland_clients(&mut self) -> impl Iterator<Item = &mut Client> {
        self.clients.iter_mut().filter_map(|client| match client {
            TestClient::Wayland { client, .. } => Some(&mut **client),
            _ => None,
        })
    }

    /// One pass of the compositor's loop and its refresh, without waiting.
    fn dispatch_server(&mut self) {
        self.event_loop
            .dispatch(Duration::ZERO, &mut self.state)
            .unwrap();
        self.state.refresh_and_flush_clients();
    }

    /// Writes a setting, as `gsettings set` would. It takes effect once the settings thread
    /// reports it; see `dispatch_until`.
    pub fn set_setting(&mut self, schema: &'static str, key: &str, value: Value) {
        self.state.camino.settings_thread.set(schema, key, value);
    }

    /// Dispatches until `done` holds, waiting for other threads such as the settings
    /// thread, and fails rather than hangs.
    #[track_caller]
    pub fn dispatch_until(&mut self, done: impl Fn(&Self) -> bool) {
        let deadline = std::time::Instant::now() + Duration::from_secs(5);
        while !done(self) {
            assert!(
                std::time::Instant::now() < deadline,
                "the condition didn't hold within 5 s"
            );
            self.dispatch_waiting();
        }
    }

    /// Dispatches until `done` holds, as [`dispatch_until`](Self::dispatch_until) does, for
    /// a condition that asks a client, which dispatches too.
    #[track_caller]
    pub fn dispatch_until_mut(&mut self, mut done: impl FnMut(&mut Self) -> bool) {
        let deadline = std::time::Instant::now() + Duration::from_secs(5);
        while !done(self) {
            assert!(
                std::time::Instant::now() < deadline,
                "the condition didn't hold within 5 s"
            );
            self.dispatch_waiting();
        }
    }

    /// One `dispatch` that waits up to 10 ms for the compositor's sources.
    fn dispatch_waiting(&mut self) {
        for client in self.wayland_clients() {
            client.flush();
        }
        self.event_loop
            .dispatch(Duration::from_millis(10), &mut self.state)
            .unwrap();
        self.state.refresh_and_flush_clients();
        for client in self.wayland_clients() {
            client.dispatch();
        }
    }

    /// Serves Camino's D-Bus interfaces on the session bus without its well-known names, and
    /// returns the unique name they're at.
    pub fn start_dbus(&mut self) -> String {
        let camino = &mut self.state.camino;
        camino
            .start_dbus(crate::dbus::Names::UniqueOnly)
            .expect("the test binary runs under dbus-run-session (14)");
        camino.dbus.as_ref().unwrap().unique_name().unwrap()
    }

    /// Makes a blocking D-Bus call on another thread, dispatching until it returns, since
    /// Camino answers from its event loop.
    #[track_caller]
    pub fn dbus_call<T: Send + 'static>(&mut self, call: impl FnOnce() -> T + Send + 'static) -> T {
        self.off_loop(call)
    }

    /// Runs a blocking call on another thread, dispatching until it returns.
    #[track_caller]
    fn off_loop<T: Send + 'static>(&mut self, call: impl FnOnce() -> T + Send + 'static) -> T {
        let call = std::thread::spawn(call);
        self.dispatch_until(|_| call.is_finished());
        call.join().expect("the blocking call returns")
    }

    /// One pass of the compositor's loop that waits up to 10 ms for its sources, without the
    /// refresh pass after it, to see what a source's callback alone leaves. It still flushes
    /// the clients, which Xwayland waits on while it starts.
    pub fn dispatch_sources_only(&mut self) {
        self.event_loop
            .dispatch(Duration::from_millis(10), &mut self.state)
            .unwrap();
        self.state.camino.display_handle.flush_clients().unwrap();
    }

    /// Dispatches for `duration`, as time passes in a running Camino.
    pub fn dispatch_for(&mut self, duration: Duration) {
        let end = std::time::Instant::now() + duration;
        while std::time::Instant::now() < end {
            self.dispatch_waiting();
        }
    }

    /// Spawns Xwayland as Camino does at start-up, and dispatches until `X11Wm` is attached.
    /// Returns its display number.
    #[track_caller]
    pub fn start_xwayland(&mut self) -> u32 {
        self.state.camino.start_xwayland();
        self.dispatch_until(|f| {
            assert!(f.state.camino.xwayland.is_some(), "Xwayland starts");
            f.state.camino.xwayland_display().is_some()
        });
        self.state.camino.xwayland_display().unwrap()
    }

    /// Kills Xwayland as a crash would, and dispatches until Camino has let it go.
    #[track_caller]
    pub fn crash_xwayland(&mut self) {
        let display = self
            .state
            .camino
            .xwayland_display()
            .expect("Xwayland is ready");
        let pid = xwayland_child(display).expect("Xwayland runs");
        rustix::process::kill_process(pid, rustix::process::Signal::KILL).unwrap();
        self.dispatch_until(|f| f.state.camino.xwayland.is_none());
    }

    /// Kills Xwayland as it restarts, before it's ready, and dispatches until Camino has let
    /// it go.
    #[track_caller]
    pub fn crash_xwayland_starting(&mut self) {
        self.dispatch_until(|f| f.state.camino.xwayland.is_some());
        let display = self.state.camino.xwayland.as_ref().unwrap().display;
        assert_eq!(
            self.state.camino.xwayland_display(),
            None,
            "Xwayland isn't ready"
        );
        let pid = xwayland_child(display).expect("Xwayland runs");
        rustix::process::kill_process(pid, rustix::process::Signal::KILL).unwrap();
        self.dispatch_until(|f| f.state.camino.xwayland.is_none());
    }

    /// Dispatches until Xwayland is ready again after a crash.
    #[track_caller]
    pub fn wait_for_xwayland(&mut self) {
        self.dispatch_until(|f| f.state.camino.xwayland_display().is_some());
    }

    /// Connects an x11rb client to the running Xwayland.
    #[track_caller]
    pub fn connect_x11(&mut self) -> ClientId {
        let display = self
            .state
            .camino
            .xwayland_display()
            .expect("Xwayland is ready");
        let client = self.off_loop(move || X11Client::connect(display));
        let client = client.expect("the X11 client connects");
        self.clients.push(TestClient::X11(Arc::new(client)));
        ClientId(self.clients.len() - 1)
    }

    /// Closes an X client's connection.
    pub fn disconnect(&mut self, id: ClientId) {
        assert!(matches!(self.clients[id.0], TestClient::X11(_)));
        self.clients[id.0] = TestClient::Disconnected;
    }

    /// Makes blocking X11 calls as the X client `id` on another thread, dispatching until
    /// they return, since Xwayland may need Camino to answer.
    #[track_caller]
    pub fn x11_call<T: Send + 'static>(
        &mut self,
        id: ClientId,
        call: impl FnOnce(&X11Client) -> T + Send + 'static,
    ) -> T {
        let TestClient::X11(client) = &self.clients[id.0] else {
            panic!("{id:?} isn't a connected X client");
        };
        let client = Arc::clone(client);
        // Xwayland orders requests per connection only, so the call sees what Camino's X
        // connection has written only once Xwayland has handled it.
        self.state.camino.sync_x_connection_in_test();
        self.off_loop(move || call(&client))
    }

    /// Every client sends its requests, the compositor handles them, then every client handles
    /// the compositor's answers.
    pub fn dispatch(&mut self) {
        for client in self.wayland_clients() {
            client.flush();
        }
        self.dispatch_server();
        for client in self.wayland_clients() {
            client.dispatch();
        }
    }

    /// Dispatches until the compositor has handled every request `id` has sent, and `id` has
    /// handled every event sent in reply.
    pub fn roundtrip(&mut self, id: ClientId) {
        let done = self.client(id).send_sync();
        for _ in 0..MAX_ROUNDTRIP_DISPATCHES {
            self.dispatch();
            if done.load(Ordering::SeqCst) {
                return;
            }
        }
        panic!("roundtrip didn't finish in {MAX_ROUNDTRIP_DISPATCHES} dispatches");
    }
}

/// Whether an X window's surface has committed content.
fn has_buffer(window: &X11Surface) -> bool {
    window.wl_surface().is_some_and(|surface| {
        with_renderer_surface_state(&surface, |state| state.buffer().is_some()).unwrap_or(false)
    })
}

impl Drop for Fixture {
    /// Ends Xwayland, since Smithay only tries displays `:0` to `:32`, fewer than the tests
    /// that start it. Stopping it gives the display's lock back, but not the display:
    /// `X11Wm` leaves a source in the event loop that holds the loop's handle, so the loop
    /// and Xwayland's Wayland connection outlive the fixture, and Xwayland with them. So
    /// it's signalled too.
    fn drop(&mut self) {
        let Some(display) = self.state.camino.xwayland.as_ref().map(|x| x.display) else {
            return;
        };
        self.state.camino.stop_xwayland_in_test();
        if let Some(pid) = xwayland_child(display) {
            let _ = rustix::process::kill_process(pid, rustix::process::Signal::TERM);
        }
    }
}

/// The Xwayland this test binary spawned for `:display`.
fn xwayland_child(display: u32) -> Option<rustix::process::Pid> {
    let parent = std::process::id().to_string();
    let display = format!(":{display}");
    std::fs::read_dir("/proc").ok()?.find_map(|entry| {
        let dir = entry.ok()?.path();
        // `pid (comm) state ppid …`
        let stat = std::fs::read_to_string(dir.join("stat")).ok()?;
        let (comm, rest) = stat.split_once(") ")?;
        let mut fields = rest.split(' ');
        let (state, ppid) = (fields.next()?, fields.next()?);
        // A crashed one may not be reaped yet.
        if !comm.ends_with("(Xwayland") || ppid != parent || state == "Z" {
            return None;
        }
        let cmdline = std::fs::read(dir.join("cmdline")).ok()?;
        cmdline
            .split(|&b| b == 0)
            .any(|arg| arg == display.as_bytes())
            .then(|| rustix::process::Pid::from_raw(dir.file_name()?.to_str()?.parse().ok()?))?
    })
}
