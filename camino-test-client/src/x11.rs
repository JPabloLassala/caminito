//! An x11rb client for the X11 tier (14, 15), on a display Camino's Xwayland serves.
//!
//! Its calls block, and Xwayland may need Camino's event loop to answer them, so a test
//! makes them on another thread while it dispatches the compositor, as it does D-Bus calls.
//! [`Names`] turns what it reads into stable text for snapshots: X ids differ from run to
//! run, so windows are named in the order the text first mentions them.

use std::collections::HashMap;
use std::fmt::Write;
use std::sync::Mutex;

use x11rb::connection::Connection;
use x11rb::errors::{ConnectError, ReplyError, ReplyOrIdError};
use x11rb::protocol::Event;
use x11rb::protocol::xproto::{
    Atom, AtomEnum, ChangeWindowAttributesAux, ClientMessageEvent, ConnectionExt, CreateWindowAux,
    EventMask, PropMode, Window, WindowClass,
};
use x11rb::rust_connection::RustConnection;
use x11rb::wrapper::ConnectionExt as _;

/// `WM_HINTS`'s input flag (ICCCM 4.1.2.4).
const INPUT_HINT: u32 = 1;

/// A window for [`X11Client::create_window`] to make.
#[derive(Debug, Clone, Default)]
pub struct WindowSpec {
    pub size: (u16, u16),
    /// `_NET_WM_WINDOW_TYPE`'s atoms, such as `_NET_WM_WINDOW_TYPE_DIALOG`, most preferred
    /// first. Empty leaves the property unset.
    pub types: Vec<&'static str>,
    /// `WM_HINTS`'s input flag; `None` leaves `WM_HINTS` unset.
    pub input: Option<bool>,
    /// `WM_TRANSIENT_FOR`.
    pub transient_for: Option<Window>,
    pub override_redirect: bool,
}

/// One connection to Xwayland.
pub struct X11Client {
    conn: RustConnection,
    root: Window,
    atoms: Mutex<HashMap<String, Atom>>,
}

impl X11Client {
    /// Connects to `:display`.
    pub fn connect(display: u32) -> Result<Self, ConnectError> {
        let (conn, screen) = x11rb::connect(Some(&format!(":{display}")))?;
        let root = conn.setup().roots[screen].root;
        Ok(X11Client {
            conn,
            root,
            atoms: Mutex::new(HashMap::new()),
        })
    }

    pub fn root(&self) -> Window {
        self.root
    }

    pub fn atom(&self, name: &str) -> Result<Atom, ReplyError> {
        if let Some(atom) = self.atoms.lock().unwrap().get(name) {
            return Ok(*atom);
        }
        let atom = self.conn.intern_atom(false, name.as_bytes())?.reply()?.atom;
        self.atoms.lock().unwrap().insert(name.to_owned(), atom);
        Ok(atom)
    }

    pub fn atom_name(&self, atom: Atom) -> Result<String, ReplyError> {
        let reply = self.conn.get_atom_name(atom)?.reply()?;
        Ok(String::from_utf8_lossy(&reply.name).into_owned())
    }

    /// A property of `window`, or `None` if it isn't set.
    pub fn property(&self, window: Window, name: &str) -> Result<Option<Property>, ReplyError> {
        let reply = self
            .conn
            .get_property(false, window, self.atom(name)?, AtomEnum::ANY, 0, u32::MAX)?
            .reply()?;
        if reply.type_ == u32::from(AtomEnum::NONE) {
            return Ok(None);
        }
        let value = match reply.format {
            32 => PropertyValue::Cardinals(reply.value32().unwrap().collect()),
            _ => PropertyValue::Bytes(reply.value),
        };
        Ok(Some(Property {
            type_: self.atom_name(reply.type_)?,
            value,
        }))
    }

    /// Creates a window as `spec` says, with the properties set before it's mapped, and
    /// selects its structure and property events.
    pub fn create_window(&self, spec: &WindowSpec) -> Result<Window, ReplyOrIdError> {
        let window = self.conn.generate_id()?;
        let aux = CreateWindowAux::new()
            .override_redirect(u32::from(spec.override_redirect))
            .event_mask(EventMask::STRUCTURE_NOTIFY | EventMask::PROPERTY_CHANGE);
        self.conn.create_window(
            x11rb::COPY_DEPTH_FROM_PARENT,
            window,
            self.root,
            0,
            0,
            spec.size.0.max(1),
            spec.size.1.max(1),
            0,
            WindowClass::INPUT_OUTPUT,
            x11rb::COPY_FROM_PARENT,
            &aux,
        )?;
        if !spec.types.is_empty() {
            let types = spec
                .types
                .iter()
                .map(|name| self.atom(name))
                .collect::<Result<Vec<_>, _>>()?;
            let property = self.atom("_NET_WM_WINDOW_TYPE")?;
            self.conn.change_property32(
                PropMode::REPLACE,
                window,
                property,
                AtomEnum::ATOM,
                &types,
            )?;
        }
        if let Some(input) = spec.input {
            // flags, input, then the unused initial state, icon and group fields.
            let hints = [INPUT_HINT, u32::from(input), 0, 0, 0, 0, 0, 0, 0];
            self.conn.change_property32(
                PropMode::REPLACE,
                window,
                AtomEnum::WM_HINTS,
                AtomEnum::WM_HINTS,
                &hints,
            )?;
        }
        if let Some(parent) = spec.transient_for {
            self.conn.change_property32(
                PropMode::REPLACE,
                window,
                AtomEnum::WM_TRANSIENT_FOR,
                AtomEnum::WINDOW,
                &[parent],
            )?;
        }
        self.conn.sync()?;
        Ok(window)
    }

    pub fn map(&self, window: Window) -> Result<(), ReplyError> {
        self.conn.map_window(window)?;
        self.conn.sync()
    }

    /// Sends a client message about `window` to the root, as EWMH's requests to the window
    /// manager go.
    pub fn send_client_message(
        &self,
        window: Window,
        type_: &str,
        data: [u32; 5],
    ) -> Result<(), ReplyError> {
        let event = ClientMessageEvent::new(32, window, self.atom(type_)?, data);
        self.conn.send_event(
            false,
            self.root,
            EventMask::SUBSTRUCTURE_REDIRECT | EventMask::SUBSTRUCTURE_NOTIFY,
            event,
        )?;
        self.conn.sync()
    }

    /// Selects property changes on the root, so [`events`](Self::events) reports them.
    pub fn watch_root(&self) -> Result<(), ReplyError> {
        let aux = ChangeWindowAttributesAux::new().event_mask(EventMask::PROPERTY_CHANGE);
        self.conn.change_window_attributes(self.root, &aux)?;
        self.conn.sync()
    }

    /// The events that have arrived since the last call, after a roundtrip.
    pub fn events(&self) -> Result<Vec<Event>, ReplyError> {
        self.conn.sync()?;
        let mut events = Vec::new();
        while let Some(event) = self.conn.poll_for_event()? {
            events.push(event);
        }
        Ok(events)
    }
}

/// A property as read.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Property {
    /// The name of its type, such as `WINDOW` or `UTF8_STRING`.
    pub type_: String,
    pub value: PropertyValue,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum PropertyValue {
    /// Format 32.
    Cardinals(Vec<u32>),
    /// Format 8, or 16 as raw bytes.
    Bytes(Vec<u8>),
}

/// Names windows in the order text first mentions them, `w1`, `w2` and on, and the root
/// `root`, so snapshots don't depend on X ids.
#[derive(Debug)]
pub struct Names {
    root: Window,
    windows: HashMap<Window, usize>,
}

impl Names {
    pub fn new(client: &X11Client) -> Self {
        Names {
            root: client.root,
            windows: HashMap::new(),
        }
    }

    pub fn window(&mut self, window: Window) -> String {
        if window == self.root {
            return "root".to_owned();
        }
        if window == x11rb::NONE {
            return "None".to_owned();
        }
        let next = self.windows.len() + 1;
        format!("w{}", self.windows.entry(window).or_insert(next))
    }

    /// Reads a property of `window`, as [`format_property`](Self::format_property) writes it.
    pub fn property(
        &mut self,
        client: &X11Client,
        window: Window,
        name: &str,
    ) -> Result<String, ReplyError> {
        let property = client.property(window, name)?;
        self.format_property(client, name, property.as_ref())
    }

    /// `name(TYPE) = value`, with windows named, atoms by name and strings quoted, or
    /// `name unset`.
    pub fn format_property(
        &mut self,
        client: &X11Client,
        name: &str,
        property: Option<&Property>,
    ) -> Result<String, ReplyError> {
        let Some(property) = property else {
            return Ok(format!("{name} unset"));
        };
        let value = match (&property.value, property.type_.as_str()) {
            (PropertyValue::Cardinals(values), "WINDOW") => {
                let names: Vec<_> = values.iter().map(|w| self.window(*w)).collect();
                names.join(", ")
            }
            (PropertyValue::Cardinals(values), "ATOM") => values
                .iter()
                .map(|atom| client.atom_name(*atom))
                .collect::<Result<Vec<_>, _>>()?
                .join(", "),
            (PropertyValue::Cardinals(values), _) => values
                .iter()
                .map(u32::to_string)
                .collect::<Vec<_>>()
                .join(", "),
            (PropertyValue::Bytes(bytes), "STRING" | "UTF8_STRING") => bytes
                .split(|&b| b == 0)
                .map(|s| format!("{:?}", String::from_utf8_lossy(s)))
                .collect::<Vec<_>>()
                .join(", "),
            (PropertyValue::Bytes(bytes), _) => format!("{bytes:?}"),
        };
        Ok(format!("{name}({}) = {value}", property.type_))
    }

    /// One line per event, with windows named and atoms by name.
    pub fn events(&mut self, client: &X11Client, events: &[Event]) -> Result<String, ReplyError> {
        let mut text = String::new();
        for event in events {
            let line = match event {
                Event::CreateNotify(e) => format!("CreateNotify {}", self.window(e.window)),
                Event::DestroyNotify(e) => format!("DestroyNotify {}", self.window(e.window)),
                Event::MapNotify(e) => format!("MapNotify {}", self.window(e.window)),
                Event::UnmapNotify(e) => format!("UnmapNotify {}", self.window(e.window)),
                Event::ConfigureNotify(e) => format!(
                    "ConfigureNotify {} {}x{}+{}+{}",
                    self.window(e.window),
                    e.width,
                    e.height,
                    e.x,
                    e.y
                ),
                Event::PropertyNotify(e) => format!(
                    "PropertyNotify {} {} {:?}",
                    self.window(e.window),
                    client.atom_name(e.atom)?,
                    e.state
                ),
                Event::ClientMessage(e) => format!(
                    "ClientMessage {} {} {:?}",
                    self.window(e.window),
                    client.atom_name(e.type_)?,
                    e.data.as_data32()
                ),
                other => format!("{other:?}"),
            };
            writeln!(text, "{line}").unwrap();
        }
        Ok(text)
    }
}
