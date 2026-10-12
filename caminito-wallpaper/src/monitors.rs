//! The dialog's connection to the compositor (212), beside GTK's: a client of output
//! management that keeps each enabled output's connector, monitor, place and size, for the
//! sidebar's pictures. Nothing here blocks but [`Monitors::connect`]: the dialog calls
//! [`Monitors::dispatch`] when the connection's fd is readable.

use std::collections::HashMap;
use std::os::fd::{AsRawFd, RawFd};
use std::os::unix::net::UnixStream;

use wayland_client::backend::{ObjectId, WaylandError};
use wayland_client::protocol::wl_output::Transform;
use wayland_client::protocol::wl_registry::{self, WlRegistry};
use wayland_client::{
    Connection, Dispatch, DispatchError, EventQueue, Proxy, QueueHandle, WEnum, event_created_child,
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

use crate::page::Output;

/// Why the outputs can't be read.
#[derive(Debug)]
pub enum Error {
    /// There's no compositor to connect to.
    Connect(wayland_client::ConnectError),
    /// The compositor has no output management.
    NoOutputManagement,
    /// The connection to the compositor broke.
    Connection(WaylandError),
    /// The compositor sent what the protocol doesn't allow.
    Dispatch(DispatchError),
}

impl std::fmt::Display for Error {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Error::Connect(err) => write!(f, "can't connect to the compositor: {err}"),
            Error::NoOutputManagement => write!(f, "the compositor has no output management"),
            Error::Connection(err) => write!(f, "the connection to the compositor failed: {err}"),
            Error::Dispatch(err) => write!(f, "the compositor sent a bad event: {err}"),
        }
    }
}

impl std::error::Error for Error {}

/// A monitor as output management describes it.
#[derive(Debug, Default)]
struct Head {
    name: String,
    make: String,
    model: String,
    serial: String,
    enabled: bool,
    position: (i32, i32),
    mode: Option<ObjectId>,
    transform: Option<Transform>,
}

/// What the outputs are, kept up to date.
#[derive(Default)]
struct State {
    heads: HashMap<ObjectId, Head>,
    /// Each mode's size, by its object.
    modes: HashMap<ObjectId, (i32, i32)>,
    /// Whether the compositor has output management.
    managed: bool,
    /// Whether a `done` came since the last change was read.
    changed: bool,
    /// Whether the first `done` came.
    described: bool,
}

/// The connection, and the outputs it says there are.
pub struct Monitors {
    connection: Connection,
    queue: EventQueue<State>,
    state: State,
}

impl Monitors {
    /// Connects to the compositor the environment names, and waits for the outputs to be
    /// described.
    pub fn connect() -> Result<Self, Error> {
        let connection = Connection::connect_to_env().map_err(Error::Connect)?;
        let mut monitors = Self::new(connection);
        // The registry's globals, then the heads of output management's.
        for _ in 0..2 {
            monitors
                .queue
                .roundtrip(&mut monitors.state)
                .map_err(Error::Dispatch)?;
        }
        if !monitors.state.managed {
            return Err(Error::NoOutputManagement);
        }
        while !monitors.state.described {
            monitors
                .queue
                .blocking_dispatch(&mut monitors.state)
                .map_err(Error::Dispatch)?;
        }
        Ok(monitors)
    }

    /// Talks to the compositor over `stream`, without waiting: the outputs are there once
    /// [`Monitors::dispatch`] has said they changed. For the headless tests.
    pub fn over(stream: UnixStream) -> Result<Self, Error> {
        let connection = Connection::from_socket(stream).map_err(Error::Connect)?;
        Ok(Self::new(connection))
    }

    fn new(connection: Connection) -> Self {
        let queue = connection.new_event_queue();
        connection.display().get_registry(&queue.handle(), ());
        Monitors {
            connection,
            queue,
            state: State::default(),
        }
    }

    /// The connection's fd, to watch for reading.
    pub fn fd(&self) -> RawFd {
        self.connection.backend().poll_fd().as_raw_fd()
    }

    /// Sends what's queued, reads what came without blocking, and returns whether the
    /// outputs changed.
    pub fn dispatch(&mut self) -> Result<bool, Error> {
        self.queue.flush().map_err(Error::Connection)?;
        if let Some(guard) = self.queue.prepare_read() {
            match guard.read() {
                Ok(_) => {}
                Err(WaylandError::Io(err)) if err.kind() == std::io::ErrorKind::WouldBlock => {}
                Err(err) => return Err(Error::Connection(err)),
            }
        }
        self.queue
            .dispatch_pending(&mut self.state)
            .map_err(Error::Dispatch)?;
        self.queue.flush().map_err(Error::Connection)?;
        Ok(std::mem::take(&mut self.state.changed))
    }

    /// The enabled outputs, in no order.
    pub fn outputs(&self) -> Vec<Output> {
        let state = &self.state;
        state
            .heads
            .values()
            .filter(|head| head.enabled)
            .map(|head| {
                let size = head
                    .mode
                    .as_ref()
                    .and_then(|mode| state.modes.get(mode))
                    .copied()
                    .unwrap_or((1920, 1080));
                let turned = matches!(
                    head.transform,
                    Some(
                        Transform::_90
                            | Transform::_270
                            | Transform::Flipped90
                            | Transform::Flipped270
                    )
                );
                let size = if turned { (size.1, size.0) } else { size };
                Output::new(
                    &head.name,
                    &head.make,
                    &head.model,
                    &head.serial,
                    head.position,
                    size,
                )
            })
            .collect()
    }
}

impl Dispatch<WlRegistry, ()> for State {
    fn event(
        state: &mut Self,
        registry: &WlRegistry,
        event: wl_registry::Event,
        _: &(),
        _: &Connection,
        qh: &QueueHandle<Self>,
    ) {
        // Version 2 has the make, model and serial.
        if let wl_registry::Event::Global {
            name,
            interface,
            version,
        } = event
            && interface == ZwlrOutputManagerV1::interface().name
            && version >= 2
            && !state.managed
        {
            registry.bind::<ZwlrOutputManagerV1, _, _>(name, version.min(4), qh, ());
            state.managed = true;
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
            zwlr_output_manager_v1::Event::Head { head } => {
                state.heads.insert(head.id(), Head::default());
            }
            zwlr_output_manager_v1::Event::Done { .. } => {
                state.described = true;
                state.changed = true;
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
        head: &ZwlrOutputHeadV1,
        event: zwlr_output_head_v1::Event,
        _: &(),
        _: &Connection,
        _: &QueueHandle<Self>,
    ) {
        if let zwlr_output_head_v1::Event::Finished = event {
            state.heads.remove(&head.id());
            if head.version() >= 3 {
                head.release();
            }
            return;
        }
        let Some(described) = state.heads.get_mut(&head.id()) else {
            return;
        };
        match event {
            zwlr_output_head_v1::Event::Name { name } => described.name = name,
            zwlr_output_head_v1::Event::Make { make } => described.make = make,
            zwlr_output_head_v1::Event::Model { model } => described.model = model,
            zwlr_output_head_v1::Event::SerialNumber { serial_number } => {
                described.serial = serial_number;
            }
            zwlr_output_head_v1::Event::Enabled { enabled } => described.enabled = enabled != 0,
            zwlr_output_head_v1::Event::Position { x, y } => described.position = (x, y),
            zwlr_output_head_v1::Event::CurrentMode { mode } => described.mode = Some(mode.id()),
            zwlr_output_head_v1::Event::Transform {
                transform: WEnum::Value(transform),
            } => described.transform = Some(transform),
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
        mode: &ZwlrOutputModeV1,
        event: zwlr_output_mode_v1::Event,
        _: &(),
        _: &Connection,
        _: &QueueHandle<Self>,
    ) {
        match event {
            zwlr_output_mode_v1::Event::Size { width, height } => {
                state.modes.insert(mode.id(), (width, height));
            }
            zwlr_output_mode_v1::Event::Finished => {
                state.modes.remove(&mode.id());
                if mode.version() >= 3 {
                    mode.release();
                }
            }
            _ => {}
        }
    }
}
