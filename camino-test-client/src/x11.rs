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
    Atom, AtomEnum, ChangeWindowAttributesAux, ClientMessageEvent, ConfigureWindowAux,
    ConnectionExt, CreateWindowAux, EventMask, PropMode, Window, WindowClass,
};
use x11rb::rust_connection::RustConnection;
use x11rb::wrapper::ConnectionExt as _;

/// `WM_HINTS`'s input flag (ICCCM 4.1.2.4).
const INPUT_HINT: u32 = 1;
/// `WM_NORMAL_HINTS`'s `USPosition` flag (ICCCM 4.1.2.3).
const US_POSITION: u32 = 1;
/// `_MOTIF_WM_HINTS`'s decorations flag.
const MWM_HINTS_DECORATIONS: u32 = 1 << 1;

/// The colour windows are filled with, so Xwayland commits a buffer when they map.
pub const BACKGROUND: u32 = 0x00ff00;

/// An X window for [`X11Client::create_window`] to make, with everything X lets a client
/// say about it.
#[derive(Debug, Clone, Default)]
pub struct XWindowSpec {
    pub size: (u16, u16),
    /// Where it's created, and whether `WM_NORMAL_HINTS` says the user asked for it
    /// (`USPosition`).
    pub position: Option<(i16, i16)>,
    /// `_NET_WM_WINDOW_TYPE`'s atoms, such as `_NET_WM_WINDOW_TYPE_DIALOG`, most preferred
    /// first. Empty leaves the property unset.
    pub types: Vec<&'static str>,
    /// `_NET_WM_STATE`'s atoms before it's mapped, such as `_NET_WM_STATE_MAXIMIZED_VERT`.
    pub states: Vec<&'static str>,
    /// `WM_HINTS`'s input flag; `None` leaves `WM_HINTS` unset.
    pub input: Option<bool>,
    /// `WM_TRANSIENT_FOR`.
    pub transient_for: Option<Window>,
    /// `_MOTIF_WM_HINTS`'s decorations: `Some(false)` asks for none.
    pub decorations: Option<bool>,
    /// `_NET_WM_NAME`.
    pub title: Option<&'static str>,
    /// `WM_CLASS`'s class, with the instance the same.
    pub class: Option<&'static str>,
    /// Whether `WM_PROTOCOLS` has `WM_DELETE_WINDOW`, so closing it sends a message.
    pub delete_window: bool,
    /// Leaves out the background, so the window has no content and Xwayland commits none.
    pub empty: bool,
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
    pub fn create_window(&self, spec: &XWindowSpec) -> Result<Window, ReplyOrIdError> {
        let window = self.conn.generate_id()?;
        let mut aux = CreateWindowAux::new()
            .override_redirect(u32::from(spec.override_redirect))
            .event_mask(
                EventMask::STRUCTURE_NOTIFY | EventMask::PROPERTY_CHANGE | EventMask::FOCUS_CHANGE,
            );
        if !spec.empty {
            aux = aux.background_pixel(BACKGROUND);
        }
        let (x, y) = spec.position.unwrap_or_default();
        self.conn.create_window(
            x11rb::COPY_DEPTH_FROM_PARENT,
            window,
            self.root,
            x,
            y,
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
        if let Some((x, y)) = spec.position {
            // flags, then the obsolete position and size, which ICCCM says to leave be.
            let mut hints = [0u32; 18];
            hints[0] = US_POSITION;
            (hints[1], hints[2]) = (x as u32, y as u32);
            self.conn.change_property32(
                PropMode::REPLACE,
                window,
                AtomEnum::WM_NORMAL_HINTS,
                AtomEnum::WM_SIZE_HINTS,
                &hints,
            )?;
        }
        if !spec.states.is_empty() {
            let states = spec
                .states
                .iter()
                .map(|name| self.atom(name))
                .collect::<Result<Vec<_>, _>>()?;
            let property = self.atom("_NET_WM_STATE")?;
            self.conn.change_property32(
                PropMode::REPLACE,
                window,
                property,
                AtomEnum::ATOM,
                &states,
            )?;
        }
        if let Some(decorations) = spec.decorations {
            // flags, functions, decorations, input mode, status.
            let hints = [MWM_HINTS_DECORATIONS, 0, u32::from(decorations), 0, 0];
            let property = self.atom("_MOTIF_WM_HINTS")?;
            self.conn
                .change_property32(PropMode::REPLACE, window, property, property, &hints)?;
        }
        if let Some(title) = spec.title {
            let property = self.atom("_NET_WM_NAME")?;
            let utf8 = self.atom("UTF8_STRING")?;
            self.conn.change_property8(
                PropMode::REPLACE,
                window,
                property,
                utf8,
                title.as_bytes(),
            )?;
        }
        if let Some(class) = spec.class {
            let value = format!("{class}\0{class}\0");
            self.conn.change_property8(
                PropMode::REPLACE,
                window,
                AtomEnum::WM_CLASS,
                AtomEnum::STRING,
                value.as_bytes(),
            )?;
        }
        if spec.delete_window {
            let protocols = self.atom("WM_PROTOCOLS")?;
            let delete = self.atom("WM_DELETE_WINDOW")?;
            self.conn.change_property32(
                PropMode::REPLACE,
                window,
                protocols,
                AtomEnum::ATOM,
                &[delete],
            )?;
        }
        self.conn.sync()?;
        Ok(window)
    }

    pub fn unmap(&self, window: Window) -> Result<(), ReplyError> {
        self.conn.unmap_window(window)?;
        self.conn.sync()
    }

    pub fn destroy(&self, window: Window) -> Result<(), ReplyError> {
        self.conn.destroy_window(window)?;
        self.conn.sync()
    }

    /// Asks to move and resize `window`, as a client does with `ConfigureWindow`.
    pub fn configure(
        &self,
        window: Window,
        (x, y): (i32, i32),
        (w, h): (u32, u32),
    ) -> Result<(), ReplyError> {
        let aux = ConfigureWindowAux::new().x(x).y(y).width(w).height(h);
        self.conn.configure_window(window, &aux)?;
        self.conn.sync()
    }

    /// Sets a string property, as a client does to change its title.
    pub fn set_string(&self, window: Window, name: &str, value: &str) -> Result<(), ReplyError> {
        let property = self.atom(name)?;
        let utf8 = self.atom("UTF8_STRING")?;
        self.conn
            .change_property8(PropMode::REPLACE, window, property, utf8, value.as_bytes())?;
        self.conn.sync()
    }

    /// Where `window`'s origin is on the root, as its client sees it.
    pub fn position(&self, window: Window) -> Result<(i32, i32), ReplyError> {
        let reply = self
            .conn
            .translate_coordinates(window, self.root, 0, 0)?
            .reply()?;
        Ok((reply.dst_x.into(), reply.dst_y.into()))
    }

    /// Waits until Xwayland has handled every request this client sent, without reading
    /// its events.
    pub fn sync(&self) -> Result<(), ReplyError> {
        self.conn.sync()
    }

    /// The window with the X input focus.
    pub fn input_focus(&self) -> Result<Window, ReplyError> {
        Ok(self.conn.get_input_focus()?.reply()?.focus)
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
