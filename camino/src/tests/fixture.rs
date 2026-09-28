//! `Fixture`: Camino on the headless backend and any number of test clients in one calloop
//! (14, 15). This is niri's `tests/fixture.rs` shape.

use std::os::unix::net::UnixStream;
use std::sync::Arc;
use std::sync::atomic::Ordering;
use std::time::Duration;

use camino_policy::PolicyWindow;
use camino_test_client::{Client, WindowId};
use smithay::backend::input::{ButtonState, InputTime, KeyState, Keycode};
use smithay::output::Output;
use smithay::reexports::calloop::EventLoop;
use smithay::reexports::wayland_server::Display;
use smithay::reexports::wayland_server::Resource;
use smithay::utils::{Physical, Size};

use crate::settings::{self, Value};
use crate::state::{ClientState, State};

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
pub(crate) use crate::input::{BTN_LEFT, BTN_MIDDLE};
pub const KEY_TAB: u32 = 15;
pub const KEY_LEFTCTRL: u32 = 29;
pub const KEY_A: u32 = 30;
pub const KEY_LEFTALT: u32 = 56;
pub const KEY_F4: u32 = 62;
pub const KEY_F10: u32 = 68;
pub const KEY_LEFT: u32 = 105;
pub const KEY_RIGHT: u32 = 106;

/// The colour test windows are filled with.
pub const WINDOW_COLOR: u32 = 0xff00ff00;

/// How many dispatches a roundtrip may take before the test fails rather than hangs.
const MAX_ROUNDTRIP_DISPATCHES: usize = 100;

pub struct Fixture {
    event_loop: EventLoop<'static, State>,
    pub state: State,
    clients: Vec<Client>,
    /// Each client as the compositor knows it.
    server_clients: Vec<smithay::reexports::wayland_server::Client>,
    input_millis: u32,
}

/// The index of one of a fixture's clients.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct ClientId(usize);

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
            server_clients: Vec::new(),
            input_millis: 0,
        }
    }

    pub fn add_output(&mut self, size: impl Into<Size<i32, Physical>>, scale: f64) -> Output {
        self.state.add_headless_output(size.into(), scale)
    }

    /// Connects a new client over a socket pair. It has seen nothing until a roundtrip.
    pub fn add_client(&mut self) -> ClientId {
        let (server, client) = UnixStream::pair().unwrap();
        let server_client = self
            .state
            .camino
            .display_handle
            .insert_client(server, Arc::new(ClientState::default()))
            .unwrap();
        self.server_clients.push(server_client);
        self.clients.push(Client::new(client));
        ClientId(self.clients.len() - 1)
    }

    /// One 1920 × 1080 output and a client that has seen the globals.
    pub fn connected() -> (Self, ClientId) {
        let mut f = Self::new();
        f.add_output((1920, 1080), 1.0);
        let id = f.add_client();
        f.roundtrip(id);
        (f, id)
    }

    /// Creates a window, lets it have its initial configure, then acks it and commits a
    /// `size` buffer, which maps it.
    pub fn map_window(&mut self, id: ClientId, size: (i32, i32)) -> WindowId {
        let window = self.client(id).create_window();
        self.client(id).commit(window);
        self.roundtrip(id);
        self.client(id)
            .ack_and_commit_buffer(window, size, WINDOW_COLOR);
        self.roundtrip(id);
        window
    }

    /// Policy's id for a client's window, if it's mapped.
    pub fn policy_id(&mut self, id: ClientId, window: WindowId) -> Option<camino_policy::WindowId> {
        let protocol_id = self.client(id).surface_protocol_id(window);
        let client = &self.server_clients[id.0];
        self.state.camino.policy.windows().find_map(|mapped| {
            let surface = mapped.wl_surface()?;
            (surface.client().as_ref() == Some(client) && surface.id().protocol_id() == protocol_id)
                .then(|| mapped.id())
        })
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

    pub fn client(&mut self, id: ClientId) -> &mut Client {
        &mut self.clients[id.0]
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
    pub fn dispatch_until(&mut self, done: impl Fn(&Self) -> bool) {
        let deadline = std::time::Instant::now() + Duration::from_secs(5);
        while !done(self) {
            assert!(
                std::time::Instant::now() < deadline,
                "the condition didn't hold within 5 s"
            );
            for client in &mut self.clients {
                client.flush();
            }
            self.event_loop
                .dispatch(Duration::from_millis(10), &mut self.state)
                .unwrap();
            self.state.refresh_and_flush_clients();
            for client in &mut self.clients {
                client.dispatch();
            }
        }
    }

    /// Every client sends its requests, the compositor handles them, then every client handles
    /// the compositor's answers.
    pub fn dispatch(&mut self) {
        for client in &mut self.clients {
            client.flush();
        }
        self.dispatch_server();
        for client in &mut self.clients {
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
