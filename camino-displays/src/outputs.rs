//! camino-displays's own connection to the compositor, beside GTK's: a client of
//! `zwlr_output_management_unstable_v1` that keeps what Camino shows of each monitor and
//! applies a configuration, and the layer surfaces Identify shows a label on per output.
//!
//! Nothing here blocks but `connect`: the window calls `dispatch` when the connection's
//! file descriptor is readable.

use std::fs::File;
use std::io::{self, Write};
use std::os::fd::{AsFd, BorrowedFd};
use std::os::unix::net::UnixStream;

use rustix::fs::{MemfdFlags, memfd_create};
use wayland_client::backend::WaylandError;
use wayland_client::protocol::wl_buffer::WlBuffer;
use wayland_client::protocol::wl_compositor::WlCompositor;
use wayland_client::protocol::wl_output::{self, WlOutput};
use wayland_client::protocol::wl_region::WlRegion;
use wayland_client::protocol::wl_registry::{self, WlRegistry};
use wayland_client::protocol::wl_shm::{self, WlShm};
use wayland_client::protocol::wl_shm_pool::WlShmPool;
use wayland_client::protocol::wl_surface::WlSurface;
use wayland_client::{
    ConnectError, Connection, Dispatch, DispatchError, EventQueue, Proxy, QueueHandle,
    delegate_noop, event_created_child,
};
use wayland_protocols_wlr::layer_shell::v1::client::zwlr_layer_shell_v1::{
    Layer, ZwlrLayerShellV1,
};
use wayland_protocols_wlr::layer_shell::v1::client::zwlr_layer_surface_v1::{
    self, KeyboardInteractivity, ZwlrLayerSurfaceV1,
};
use wayland_protocols_wlr::output_management::v1::client::zwlr_output_configuration_head_v1::ZwlrOutputConfigurationHeadV1;
use wayland_protocols_wlr::output_management::v1::client::zwlr_output_configuration_v1::{
    self, ZwlrOutputConfigurationV1,
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

use crate::page::{Mode, Monitor, Scale, Shows, Wanted};

/// Why the connection can't do what's asked of it.
#[derive(Debug)]
pub enum Error {
    /// There's no compositor to connect to.
    Connect(ConnectError),
    /// The connection to the compositor broke.
    Connection(WaylandError),
    /// The compositor sent what the protocol doesn't allow.
    Dispatch(DispatchError),
    /// The compositor has no output management, or hasn't said what it shows yet.
    NoOutputManagement,
    /// The compositor has no layer shell, or no shared memory, to show a label with.
    NoLayerShell,
    /// A label's buffer couldn't be made.
    Buffer(io::Error),
}

impl std::fmt::Display for Error {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Error::Connect(err) => write!(f, "can't connect to the compositor: {err}"),
            Error::Connection(err) => write!(f, "the connection to the compositor broke: {err}"),
            Error::Dispatch(err) => write!(f, "can't follow the compositor: {err}"),
            Error::NoOutputManagement => write!(f, "the compositor has no output management"),
            Error::NoLayerShell => write!(f, "the compositor has no layer shell"),
            Error::Buffer(err) => write!(f, "can't make a label's buffer: {err}"),
        }
    }
}

impl std::error::Error for Error {}

/// What `dispatch` found the compositor to have said.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Event {
    /// What Camino shows changed, or is known for the first time: read `monitors`.
    Changed,
    /// The answer to the configuration `apply` sent.
    Applied(Outcome),
}

/// The compositor's answer to a configuration.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Outcome {
    /// It shows the configuration, healed if it had to be (82).
    Succeeded,
    /// It can't show it, and shows what it did.
    Failed,
    /// The monitors changed while the configuration was made.
    Cancelled,
}

/// One label for Identify: a picture to show in the middle of an output.
pub struct Label {
    /// The output's name.
    pub output: String,
    /// The picture's width in its own pixels.
    pub width: i32,
    /// The picture's height in its own pixels.
    pub height: i32,
    /// How many of the picture's pixels make a logical one, along each axis.
    pub scale: i32,
    /// The picture: ARGB, premultiplied, in the machine's byte order, row by row with no
    /// padding, as cairo's `ARgb32` has it and `wl_shm`'s `argb8888` wants it.
    pub pixels: Vec<u8>,
}

/// The connection.
pub struct Outputs {
    connection: Connection,
    queue: EventQueue<State>,
    state: State,
}

#[derive(Default)]
struct State {
    manager: Option<ZwlrOutputManagerV1>,
    /// The heads there are, in the order the compositor announced them.
    heads: Vec<Head>,
    /// The serial of the last `done`.
    serial: Option<u32>,
    events: Vec<Event>,
    compositor: Option<WlCompositor>,
    shm: Option<WlShm>,
    layer_shell: Option<ZwlrLayerShellV1>,
    /// The `wl_output`s, with the registry's name for each and the name it says it has.
    wl_outputs: Vec<(u32, WlOutput, Option<String>)>,
    /// The labels Identify shows now.
    labels: Vec<ShownLabel>,
}

struct Head {
    proxy: ZwlrOutputHeadV1,
    name: String,
    make: Option<String>,
    model: Option<String>,
    modes: Vec<(ZwlrOutputModeV1, Mode)>,
    enabled: bool,
    current_mode: Option<ZwlrOutputModeV1>,
    position: (i32, i32),
    scale: f64,
}

struct ShownLabel {
    surface: WlSurface,
    layer_surface: ZwlrLayerSurfaceV1,
    buffer: WlBuffer,
    scale: i32,
}

impl ShownLabel {
    fn destroy(&self) {
        self.layer_surface.destroy();
        self.surface.destroy();
        self.buffer.destroy();
    }
}

impl Outputs {
    /// Connects to the compositor `WAYLAND_DISPLAY` names, and waits for what it shows.
    pub fn connect() -> Result<Self, Error> {
        let connection = Connection::connect_to_env().map_err(Error::Connect)?;
        let mut outputs = Self::new(connection);
        // The globals, then the heads of the manager bound for them.
        for _ in 0..2 {
            let state = &mut outputs.state;
            outputs.queue.roundtrip(state).map_err(Error::Dispatch)?;
        }
        if outputs.state.serial.is_none() {
            return Err(Error::NoOutputManagement);
        }
        outputs.state.events.clear();
        Ok(outputs)
    }

    /// A connection over `stream`, which knows nothing until `dispatch` has read what the
    /// compositor says.
    pub fn over(stream: UnixStream) -> Result<Self, Error> {
        let connection = Connection::from_socket(stream).map_err(Error::Connect)?;
        Ok(Self::new(connection))
    }

    fn new(connection: Connection) -> Self {
        let queue = connection.new_event_queue();
        connection.display().get_registry(&queue.handle(), ());
        Outputs {
            connection,
            queue,
            state: State::default(),
        }
    }

    /// The connection's file descriptor, to wait on before `dispatch`.
    pub fn fd(&self) -> BorrowedFd<'_> {
        self.connection.as_fd()
    }

    /// Sends what's queued, then reads and handles what the compositor has sent, without
    /// blocking. Returns what that amounts to, oldest first.
    pub fn dispatch(&mut self) -> Result<Vec<Event>, Error> {
        self.flush()?;
        if let Some(guard) = self.queue.prepare_read() {
            match guard.read() {
                Ok(_) => {}
                Err(WaylandError::Io(err)) if err.kind() == io::ErrorKind::WouldBlock => {}
                Err(err) => return Err(Error::Connection(err)),
            }
        }
        self.queue
            .dispatch_pending(&mut self.state)
            .map_err(Error::Dispatch)?;
        self.flush()?;
        Ok(std::mem::take(&mut self.state.events))
    }

    /// Waits until the compositor has handled everything sent so far, and returns what it
    /// said meanwhile. For when the window is closing and there's no later turn of the
    /// main loop.
    pub fn roundtrip(&mut self) -> Result<Vec<Event>, Error> {
        self.queue
            .roundtrip(&mut self.state)
            .map_err(Error::Dispatch)?;
        Ok(std::mem::take(&mut self.state.events))
    }

    fn flush(&self) -> Result<(), Error> {
        match self.connection.flush() {
            Ok(()) => Ok(()),
            Err(WaylandError::Io(err)) if err.kind() == io::ErrorKind::WouldBlock => Ok(()),
            Err(err) => Err(Error::Connection(err)),
        }
    }

    /// What Camino shows of each connected monitor, as of the last `Event::Changed`.
    pub fn monitors(&self) -> Vec<Monitor> {
        let monitor = |head: &Head| {
            let current = head.current_mode.as_ref();
            let listed = current.and_then(|c| head.modes.iter().position(|(proxy, _)| proxy == c));
            // A custom mode, which `wlr-randr` can set, is none of the listed ones. The
            // display is on all the same, and the page has it at its native mode.
            let native = head.modes.iter().position(|(_, mode)| mode.preferred);
            let fallback = native.or((!head.modes.is_empty()).then_some(0));
            let mode = listed.or(fallback);
            let shows = |mode| Shows {
                mode,
                x: head.position.0,
                y: head.position.1,
                scale: Scale::from_factor(head.scale),
            };
            Monitor {
                name: head.name.clone(),
                make: head.make.clone(),
                model: head.model.clone(),
                modes: head.modes.iter().map(|(_, mode)| *mode).collect(),
                shows: mode.filter(|_| head.enabled).map(shows),
            }
        };
        self.state.heads.iter().map(monitor).collect()
    }

    /// Sends a configuration that says what every monitor is to be: `wanted`'s, and off
    /// for one `wanted` doesn't name. The answer comes as `Event::Applied`.
    pub fn apply(&mut self, wanted: &[Wanted]) -> Result<(), Error> {
        let state = &self.state;
        let (Some(manager), Some(serial)) = (&state.manager, state.serial) else {
            return Err(Error::NoOutputManagement);
        };
        let handle = self.queue.handle();
        let configuration = manager.create_configuration(serial, &handle, ());
        for head in &state.heads {
            let shows = wanted
                .iter()
                .find(|wanted| wanted.name == head.name)
                .and_then(|wanted| wanted.shows);
            let Some(shows) = shows else {
                configuration.disable_head(&head.proxy);
                continue;
            };
            let enabled = configuration.enable_head(&head.proxy, &handle, ());
            // The `preferred` mark is the page's own, and the rest names the mode.
            let same = |(_, mode): &&(ZwlrOutputModeV1, Mode)| {
                (mode.width, mode.height, mode.refresh)
                    == (shows.mode.width, shows.mode.height, shows.mode.refresh)
            };
            if let Some((mode, _)) = head.modes.iter().find(same) {
                enabled.set_mode(mode);
            }
            enabled.set_position(shows.x, shows.y);
            if let Some(scale) = shows.scale {
                enabled.set_scale(scale.factor());
            }
        }
        configuration.apply();
        self.flush()
    }

    /// Whether the compositor has what `identify` needs.
    pub fn can_identify(&self) -> bool {
        let state = &self.state;
        state.compositor.is_some() && state.shm.is_some() && state.layer_shell.is_some()
    }

    /// Shows each label in the middle of its output, over everything, until
    /// `end_identify`. The labels take no input. A label whose output isn't there is
    /// left out.
    pub fn identify(&mut self, labels: &[Label]) -> Result<(), Error> {
        self.end_identify()?;
        let state = &mut self.state;
        let (Some(compositor), Some(shm), Some(layer_shell)) =
            (&state.compositor, &state.shm, &state.layer_shell)
        else {
            return Err(Error::NoLayerShell);
        };
        let handle = self.queue.handle();
        for label in labels {
            let named = |(_, _, name): &&(u32, WlOutput, Option<String>)| {
                name.as_deref() == Some(label.output.as_str())
            };
            let Some((_, output, _)) = state.wl_outputs.iter().find(named) else {
                continue;
            };
            let buffer = buffer(shm, label, &handle).map_err(Error::Buffer)?;
            let surface = compositor.create_surface(&handle, ());
            // Clicks and the pointer go to what's under the label.
            let nowhere = compositor.create_region(&handle, ());
            surface.set_input_region(Some(&nowhere));
            nowhere.destroy();
            let layer_surface = layer_shell.get_layer_surface(
                &surface,
                Some(output),
                Layer::Overlay,
                "camino-displays-identify".to_owned(),
                &handle,
                (),
            );
            let logical = |pixels: i32| (pixels / label.scale.max(1)).max(1) as u32;
            layer_surface.set_size(logical(label.width), logical(label.height));
            layer_surface.set_keyboard_interactivity(KeyboardInteractivity::None);
            // Out of the panel's way too: the label is in the middle of the monitor.
            layer_surface.set_exclusive_zone(-1);
            surface.commit();
            state.labels.push(ShownLabel {
                surface,
                layer_surface,
                buffer,
                scale: label.scale.max(1),
            });
        }
        self.flush()
    }

    /// Takes away the labels `identify` shows.
    pub fn end_identify(&mut self) -> Result<(), Error> {
        for label in self.state.labels.drain(..) {
            label.destroy();
        }
        self.flush()
    }
}

/// A `wl_buffer` of `label`'s picture, in shared memory nothing else has a name for.
fn buffer(shm: &WlShm, label: &Label, handle: &QueueHandle<State>) -> io::Result<WlBuffer> {
    let stride = label.width * 4;
    let size = stride * label.height;
    if label.width <= 0 || label.height <= 0 || label.pixels.len() != size as usize {
        return Err(io::Error::new(
            io::ErrorKind::InvalidInput,
            "the picture isn't of the size it says",
        ));
    }
    let memory = memfd_create("camino-displays-label", MemfdFlags::CLOEXEC)?;
    let mut memory = File::from(memory);
    memory.write_all(&label.pixels)?;
    let pool = shm.create_pool(memory.as_fd(), size, handle, ());
    let buffer = pool.create_buffer(
        0,
        label.width,
        label.height,
        stride,
        wl_shm::Format::Argb8888,
        handle,
        (),
    );
    // The buffer keeps the memory.
    pool.destroy();
    Ok(buffer)
}

impl Dispatch<WlRegistry, ()> for State {
    fn event(
        state: &mut Self,
        registry: &WlRegistry,
        event: wl_registry::Event,
        _: &(),
        _: &Connection,
        handle: &QueueHandle<Self>,
    ) {
        match event {
            wl_registry::Event::Global {
                name,
                interface,
                version,
            } => match interface.as_str() {
                "zwlr_output_manager_v1" => {
                    state.manager = Some(registry.bind(name, version.min(4), handle, ()));
                }
                "wl_compositor" => {
                    state.compositor = Some(registry.bind(name, version.min(4), handle, ()));
                }
                "wl_shm" => state.shm = Some(registry.bind(name, 1, handle, ())),
                "zwlr_layer_shell_v1" => {
                    state.layer_shell = Some(registry.bind(name, version.min(4), handle, ()));
                }
                // Version 4 has the output's name, which is all that's wanted of it.
                "wl_output" if version >= 4 => {
                    let output = registry.bind(name, 4, handle, ());
                    state.wl_outputs.push((name, output, None));
                }
                _ => {}
            },
            wl_registry::Event::GlobalRemove { name } => {
                state.wl_outputs.retain(|(global, output, _)| {
                    if *global == name {
                        output.release();
                    }
                    *global != name
                });
            }
            _ => {}
        }
    }
}

impl Dispatch<WlOutput, ()> for State {
    fn event(
        state: &mut Self,
        output: &WlOutput,
        event: wl_output::Event,
        _: &(),
        _: &Connection,
        _: &QueueHandle<Self>,
    ) {
        if let wl_output::Event::Name { name } = event
            && let Some(known) = state.wl_outputs.iter_mut().find(|(_, o, _)| o == output)
        {
            known.2 = Some(name);
        }
    }
}

impl Dispatch<ZwlrOutputManagerV1, ()> for State {
    fn event(
        state: &mut Self,
        _: &ZwlrOutputManagerV1,
        event: zwlr_output_manager_v1::Event,
        _: &(),
        _: &Connection,
        _: &QueueHandle<Self>,
    ) {
        match event {
            zwlr_output_manager_v1::Event::Head { head } => state.heads.push(Head {
                proxy: head,
                name: String::new(),
                make: None,
                model: None,
                modes: Vec::new(),
                enabled: false,
                current_mode: None,
                position: (0, 0),
                scale: 1.0,
            }),
            zwlr_output_manager_v1::Event::Done { serial } => {
                state.serial = Some(serial);
                state.events.push(Event::Changed);
            }
            zwlr_output_manager_v1::Event::Finished => {
                state.manager = None;
                state.serial = None;
            }
            _ => {}
        }
    }

    event_created_child!(State, ZwlrOutputManagerV1, [
        zwlr_output_manager_v1::EVT_HEAD_OPCODE => (ZwlrOutputHeadV1, ()),
    ]);
}

impl Dispatch<ZwlrOutputHeadV1, ()> for State {
    fn event(
        state: &mut Self,
        proxy: &ZwlrOutputHeadV1,
        event: zwlr_output_head_v1::Event,
        _: &(),
        _: &Connection,
        _: &QueueHandle<Self>,
    ) {
        use zwlr_output_head_v1::Event;
        let Some(index) = state.heads.iter().position(|head| head.proxy == *proxy) else {
            return;
        };
        let head = &mut state.heads[index];
        match event {
            Event::Name { name } => head.name = name,
            Event::Make { make } => head.make = Some(make),
            Event::Model { model } => head.model = Some(model),
            Event::Mode { mode } => {
                let unknown = Mode {
                    width: 0,
                    height: 0,
                    refresh: 0,
                    preferred: false,
                };
                head.modes.push((mode, unknown));
            }
            Event::Enabled { enabled } => {
                head.enabled = enabled != 0;
                if !head.enabled {
                    head.current_mode = None;
                }
            }
            Event::CurrentMode { mode } => head.current_mode = Some(mode),
            Event::Position { x, y } => head.position = (x, y),
            Event::Scale { scale } => head.scale = scale,
            Event::Finished => {
                let head = state.heads.remove(index);
                if head.proxy.version() >= 3 {
                    for (mode, _) in &head.modes {
                        mode.release();
                    }
                    head.proxy.release();
                }
            }
            _ => {}
        }
    }

    event_created_child!(State, ZwlrOutputHeadV1, [
        zwlr_output_head_v1::EVT_MODE_OPCODE => (ZwlrOutputModeV1, ()),
    ]);
}

impl Dispatch<ZwlrOutputModeV1, ()> for State {
    fn event(
        state: &mut Self,
        proxy: &ZwlrOutputModeV1,
        event: zwlr_output_mode_v1::Event,
        _: &(),
        _: &Connection,
        _: &QueueHandle<Self>,
    ) {
        use zwlr_output_mode_v1::Event;
        if let Event::Finished = event {
            for head in &mut state.heads {
                head.modes.retain(|(mode, _)| mode != proxy);
                if head.current_mode.as_ref() == Some(proxy) {
                    head.current_mode = None;
                }
            }
            if proxy.version() >= 3 {
                proxy.release();
            }
            return;
        }
        let mut modes = state.heads.iter_mut().flat_map(|head| &mut head.modes);
        let Some((_, mode)) = modes.find(|(mode, _)| mode == proxy) else {
            return;
        };
        match event {
            Event::Size { width, height } => (mode.width, mode.height) = (width, height),
            Event::Refresh { refresh } => mode.refresh = refresh,
            Event::Preferred => mode.preferred = true,
            _ => {}
        }
    }
}

impl Dispatch<ZwlrOutputConfigurationV1, ()> for State {
    fn event(
        state: &mut Self,
        configuration: &ZwlrOutputConfigurationV1,
        event: zwlr_output_configuration_v1::Event,
        _: &(),
        _: &Connection,
        _: &QueueHandle<Self>,
    ) {
        use zwlr_output_configuration_v1::Event as Answer;
        let outcome = match event {
            Answer::Succeeded => Outcome::Succeeded,
            Answer::Failed => Outcome::Failed,
            Answer::Cancelled => Outcome::Cancelled,
            _ => return,
        };
        configuration.destroy();
        state.events.push(Event::Applied(outcome));
    }
}

impl Dispatch<ZwlrLayerSurfaceV1, ()> for State {
    fn event(
        state: &mut Self,
        layer_surface: &ZwlrLayerSurfaceV1,
        event: zwlr_layer_surface_v1::Event,
        _: &(),
        _: &Connection,
        _: &QueueHandle<Self>,
    ) {
        let shown = |label: &ShownLabel| label.layer_surface == *layer_surface;
        let Some(index) = state.labels.iter().position(shown) else {
            return;
        };
        match event {
            zwlr_layer_surface_v1::Event::Configure { serial, .. } => {
                let label = &state.labels[index];
                layer_surface.ack_configure(serial);
                label.surface.set_buffer_scale(label.scale);
                label.surface.attach(Some(&label.buffer), 0, 0);
                label.surface.damage_buffer(0, 0, i32::MAX, i32::MAX);
                label.surface.commit();
            }
            // Its output went away.
            zwlr_layer_surface_v1::Event::Closed => state.labels.remove(index).destroy(),
            _ => {}
        }
    }
}

delegate_noop!(State: WlCompositor);
delegate_noop!(State: WlRegion);
delegate_noop!(State: WlShmPool);
delegate_noop!(State: ZwlrLayerShellV1);
delegate_noop!(State: ZwlrOutputConfigurationHeadV1);
delegate_noop!(State: ignore WlShm);
delegate_noop!(State: ignore WlBuffer);
delegate_noop!(State: ignore WlSurface);
