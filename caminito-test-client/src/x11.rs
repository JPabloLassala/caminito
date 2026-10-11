//! An x11rb client for the X11 tier (14, 15), on a display Caminito's Xwayland serves.
//!
//! Its calls block, and Xwayland may need Caminito's event loop to answer them, so a test
//! makes them on another thread while it dispatches the compositor, as it does D-Bus calls.
//! [`Names`] turns what it reads into stable text for snapshots: X ids differ from run to
//! run, so windows are named in the order the text first mentions them.

use std::collections::HashMap;
use std::fmt::Write;
use std::sync::Mutex;

use x11rb::connection::Connection;
use x11rb::errors::{ConnectError, ReplyError, ReplyOrIdError};
use x11rb::protocol::Event;
use x11rb::protocol::xkb::{self, ConnectionExt as _};
use x11rb::protocol::xproto::{
    Atom, AtomEnum, ChangeWindowAttributesAux, ClientMessageEvent, ClipOrdering,
    ConfigureWindowAux, ConnectionExt, CreateWindowAux, EventMask, GrabMode, GrabStatus, PropMode,
    Rectangle, SELECTION_NOTIFY_EVENT, SelectionNotifyEvent, StackMode, Window, WindowClass,
};
use x11rb::rust_connection::RustConnection;
use x11rb::wrapper::ConnectionExt as _;

/// `WM_HINTS`'s input flag (ICCCM 4.1.2.4).
const INPUT_HINT: u32 = 1;
/// Its urgency flag.
const URGENCY_HINT: u32 = 1 << 8;
/// `WM_NORMAL_HINTS`'s `USPosition` flag (ICCCM 4.1.2.3).
const US_POSITION: u32 = 1;
/// Its `PMinSize` and `PMaxSize` flags.
const P_MIN_SIZE: u32 = 1 << 4;
const P_MAX_SIZE: u32 = 1 << 5;
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
    /// Whether `WM_NORMAL_HINTS` fixes its size, with its least and greatest sizes both
    /// `size`.
    pub fixed_size: bool,
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
    /// Whether `WM_PROTOCOLS` has `_NET_WM_PING`, as a toolkit's window does. Nothing
    /// answers the ping but [`answer_pings`](X11Client::answer_pings).
    pub ping: bool,
    /// Leaves out the background, so the window has no content and Xwayland commits none.
    pub empty: bool,
    /// The colour it's filled with, if not [`BACKGROUND`].
    pub background: Option<u32>,
    pub override_redirect: bool,
    /// `CARDINAL` properties set before it's mapped, such as `_NET_WM_DESKTOP`.
    pub cardinals: Vec<(&'static str, Vec<u32>)>,
    /// Whether it takes `_NET_WM_SYNC_REQUEST`, as toolkits do: a resize waits for
    /// [`X11Client::ack_sync_requests`].
    pub sync_request: bool,
}

/// One connection to Xwayland.
pub struct X11Client {
    conn: RustConnection,
    root: Window,
    atoms: Mutex<HashMap<String, Atom>>,
    /// Events read while waiting for another, which [`events`](Self::events) reports first.
    backlog: Mutex<Vec<Event>>,
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
            backlog: Mutex::new(Vec::new()),
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
            aux = aux.background_pixel(spec.background.unwrap_or(BACKGROUND));
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
        if spec.position.is_some() || spec.fixed_size {
            // flags, then the obsolete position and size, which ICCCM says to leave be, then
            // the least and greatest sizes.
            let mut hints = [0u32; 18];
            if let Some((x, y)) = spec.position {
                hints[0] |= US_POSITION;
                (hints[1], hints[2]) = (x as u32, y as u32);
            }
            if spec.fixed_size {
                let (width, height) = (u32::from(spec.size.0), u32::from(spec.size.1));
                hints[0] |= P_MIN_SIZE | P_MAX_SIZE;
                (hints[5], hints[6]) = (width, height);
                (hints[7], hints[8]) = (width, height);
            }
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
        for (name, value) in &spec.cardinals {
            let property = self.atom(name)?;
            self.conn.change_property32(
                PropMode::REPLACE,
                window,
                property,
                AtomEnum::CARDINAL,
                value,
            )?;
        }
        let mut protocols = Vec::new();
        if spec.delete_window {
            protocols.push(self.atom("WM_DELETE_WINDOW")?);
        }
        if spec.ping {
            protocols.push(self.atom("_NET_WM_PING")?);
        }
        if spec.sync_request {
            use x11rb::protocol::sync::{ConnectionExt as _, Int64};
            let counter = self.conn.generate_id()?;
            self.conn
                .sync_create_counter(counter, Int64 { hi: 0, lo: 0 })?;
            let property = self.atom("_NET_WM_SYNC_REQUEST_COUNTER")?;
            self.conn.change_property32(
                PropMode::REPLACE,
                window,
                property,
                AtomEnum::CARDINAL,
                &[counter],
            )?;
            protocols.push(self.atom("_NET_WM_SYNC_REQUEST")?);
        }
        if !protocols.is_empty() {
            let property = self.atom("WM_PROTOCOLS")?;
            self.conn.change_property32(
                PropMode::REPLACE,
                window,
                property,
                AtomEnum::ATOM,
                &protocols,
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

    /// Restacks `window` as `ConfigureWindow` does: `Above` with no sibling raises it to the
    /// top, and with one puts it just above that sibling.
    pub fn restack(
        &self,
        window: Window,
        mode: StackMode,
        sibling: Option<Window>,
    ) -> Result<(), ReplyError> {
        let mut aux = ConfigureWindowAux::new().stack_mode(mode);
        if let Some(sibling) = sibling {
            aux = aux.sibling(sibling);
        }
        self.conn.configure_window(window, &aux)?;
        self.conn.sync()
    }

    /// Grabs the keyboard for `window`, as a menu does when it opens.
    pub fn grab_keyboard(&self, window: Window) -> Result<GrabStatus, ReplyError> {
        let reply = self
            .conn
            .grab_keyboard(
                false,
                window,
                x11rb::CURRENT_TIME,
                GrabMode::ASYNC,
                GrabMode::ASYNC,
            )?
            .reply()?;
        Ok(reply.status)
    }

    /// Lets go of the keyboard, as a menu does when it closes.
    pub fn ungrab_keyboard(&self) -> Result<(), ReplyError> {
        self.conn.ungrab_keyboard(x11rb::CURRENT_TIME)?;
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

    /// Sets a `CARDINAL` property, as a client does to change its icon or struts.
    pub fn set_cardinals(
        &self,
        window: Window,
        name: &str,
        value: &[u32],
    ) -> Result<(), ReplyError> {
        let property = self.atom(name)?;
        self.conn.change_property32(
            PropMode::REPLACE,
            window,
            property,
            AtomEnum::CARDINAL,
            value,
        )?;
        self.conn.sync()
    }

    /// Sets `WM_HINTS` with its urgency flag or without it, and nothing else, as
    /// `XSetWMHints` does for a window that wants attention.
    pub fn set_urgency_hint(&self, window: Window, urgent: bool) -> Result<(), ReplyError> {
        let flags = if urgent { URGENCY_HINT } else { 0 };
        self.conn.change_property32(
            PropMode::REPLACE,
            window,
            AtomEnum::WM_HINTS,
            AtomEnum::WM_HINTS,
            &[flags, 0, 0, 0, 0, 0, 0, 0, 0],
        )?;
        self.conn.sync()
    }

    /// Deletes a property, as a client does when it withdraws a hint.
    pub fn delete_property(&self, window: Window, name: &str) -> Result<(), ReplyError> {
        let property = self.atom(name)?;
        self.conn.delete_property(window, property)?;
        self.conn.sync()
    }

    /// Sets the window's input shape (SHAPE's `Input` kind) to `rects`, each x, y, width,
    /// height from its origin, as GTK does to leave a CSD window's shadow out.
    pub fn set_input_shape(
        &self,
        window: Window,
        rects: &[(i16, i16, u16, u16)],
    ) -> Result<(), ReplyError> {
        use x11rb::protocol::shape::{self, ConnectionExt as _};
        let rects: Vec<Rectangle> = rects
            .iter()
            .map(|&(x, y, width, height)| Rectangle {
                x,
                y,
                width,
                height,
            })
            .collect();
        self.conn.shape_rectangles(
            shape::SO::SET,
            shape::SK::INPUT,
            ClipOrdering::UNSORTED,
            window,
            0,
            0,
            &rects,
        )?;
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

    /// `window`'s width and height, as its client sees them.
    pub fn size(&self, window: Window) -> Result<(u16, u16), ReplyError> {
        let reply = self.conn.get_geometry(window)?.reply()?;
        Ok((reply.width, reply.height))
    }

    /// Waits until Xwayland has handled every request this client sent, without reading
    /// its events.
    pub fn sync(&self) -> Result<(), ReplyError> {
        self.conn.sync()
    }

    /// Rings the bell for `window` with `XkbBell`, as GDK3's `gdk_window_beep` does, or
    /// for none with the core `Bell`, as `XBell` does.
    pub fn bell(&self, window: Option<Window>) -> Result<(), ReplyError> {
        match window {
            Some(window) => {
                // XKB refuses a client's requests until it has said which version it knows.
                self.conn.xkb_use_extension(1, 0)?.reply()?;
                self.conn.xkb_bell(
                    xkb::ID::USE_CORE_KBD.into(),
                    xkb::BellClass::DFLT_XI_CLASS.into(),
                    xkb::ID::DFLT_XI_ID.into(),
                    0,
                    false,
                    false,
                    0,
                    0,
                    x11rb::NONE,
                    window,
                )?;
            }
            None => {
                self.conn.bell(0)?;
            }
        }
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
        let mut events = std::mem::take(&mut *self.backlog.lock().unwrap());
        while let Some(event) = self.conn.poll_for_event()? {
            events.push(event);
        }
        Ok(events)
    }

    /// Answers the `_NET_WM_SYNC_REQUEST`s `window` has had since the last call, as a
    /// toolkit does once it has drawn at the size it was configured with: sets the window's
    /// counter to the last one's value. Returns how many there were; the other events are
    /// kept for [`events`](Self::events).
    pub fn ack_sync_requests(&self, window: Window) -> Result<usize, ReplyError> {
        use x11rb::protocol::sync::{ConnectionExt as _, Int64};
        let protocols = self.atom("WM_PROTOCOLS")?;
        let sync_request = self.atom("_NET_WM_SYNC_REQUEST")?;
        let mut values = Vec::new();
        let mut others = Vec::new();
        for event in self.events()? {
            match &event {
                Event::ClientMessage(message)
                    if message.window == window
                        && message.type_ == protocols
                        && message.data.as_data32()[0] == sync_request =>
                {
                    let data = message.data.as_data32();
                    values.push(Int64 {
                        lo: data[2],
                        hi: data[3] as i32,
                    });
                }
                _ => others.push(event),
            }
        }
        self.backlog.lock().unwrap().extend(others);
        if let Some(&value) = values.last() {
            let counter = match self.property(window, "_NET_WM_SYNC_REQUEST_COUNTER")? {
                Some(Property {
                    value: PropertyValue::Cardinals(counters),
                    ..
                }) => counters[0],
                _ => panic!("the window takes no sync requests"),
            };
            self.conn.sync_set_counter(counter, value)?;
            self.conn.sync()?;
        }
        Ok(values.len())
    }

    /// Answers the `_NET_WM_PING`s `window` has had since the last call, as a toolkit's
    /// loop does: sends each back to the root. Returns how many there were; the other
    /// events are kept for [`events`](Self::events).
    pub fn answer_pings(&self, window: Window) -> Result<usize, ReplyError> {
        let protocols = self.atom("WM_PROTOCOLS")?;
        let ping = self.atom("_NET_WM_PING")?;
        let mut pings = 0;
        let mut others = Vec::new();
        for event in self.events()? {
            match &event {
                Event::ClientMessage(message)
                    if message.window == window
                        && message.type_ == protocols
                        && message.data.as_data32()[0] == ping =>
                {
                    let answer = ClientMessageEvent::new(32, self.root, protocols, message.data);
                    self.conn.send_event(
                        false,
                        self.root,
                        EventMask::SUBSTRUCTURE_REDIRECT | EventMask::SUBSTRUCTURE_NOTIFY,
                        answer,
                    )?;
                    pings += 1;
                }
                _ => others.push(event),
            }
        }
        self.backlog.lock().unwrap().extend(others);
        self.conn.sync()?;
        Ok(pings)
    }

    /// Blocks until an event `pick` takes arrives, keeping the others for
    /// [`events`](Self::events).
    fn wait_for<T>(&self, pick: impl Fn(&Event) -> Option<T>) -> Result<T, ReplyError> {
        loop {
            let event = self.conn.wait_for_event()?;
            match pick(&event) {
                Some(found) => return Ok(found),
                None => self.backlog.lock().unwrap().push(event),
            }
        }
    }

    /// The window that owns the selection `name`, such as `CLIPBOARD`, or `NONE` (0).
    pub fn selection_owner(&self, name: &str) -> Result<Window, ReplyError> {
        let selection = self.atom(name)?;
        Ok(self.conn.get_selection_owner(selection)?.reply()?.owner)
    }

    /// The XSETTINGS table, as a GTK client reads it: `_XSETTINGS_SETTINGS` on the owner of
    /// `_XSETTINGS_S0`, by name. `None` if there's no owner or it has no table.
    pub fn xsettings(&self) -> Result<Option<Vec<(String, XSetting)>>, ReplyError> {
        let owner = self.selection_owner("_XSETTINGS_S0")?;
        if owner == x11rb::NONE {
            return Ok(None);
        }
        let Some(property) = self.property(owner, "_XSETTINGS_SETTINGS")? else {
            return Ok(None);
        };
        let PropertyValue::Bytes(bytes) = property.value else {
            panic!("_XSETTINGS_SETTINGS has format 8");
        };
        Ok(Some(parse_xsettings(&bytes)))
    }

    /// The name of RandR's primary output, as `xrandr` shows it, or `None` if RandR has
    /// none.
    pub fn randr_primary(&self) -> Result<Option<String>, ReplyError> {
        use x11rb::protocol::randr::ConnectionExt as _;
        let primary = self
            .conn
            .randr_get_output_primary(self.root)?
            .reply()?
            .output;
        if primary == x11rb::NONE {
            return Ok(None);
        }
        let info = self
            .conn
            .randr_get_output_info(primary, x11rb::CURRENT_TIME)?
            .reply()?;
        Ok(Some(String::from_utf8_lossy(&info.name).into_owned()))
    }

    /// Makes `window` the selection `name`'s owner, as a client does when the user copies.
    pub fn set_selection_owner(&self, window: Window, name: &str) -> Result<(), ReplyError> {
        let selection = self.atom(name)?;
        self.conn
            .set_selection_owner(window, selection, x11rb::CURRENT_TIME)?;
        self.conn.sync()
    }

    /// Asks for the selection `name` as `target`, such as `UTF8_STRING`, into a property on
    /// `window`, as a client does when the user pastes. `None` is the owner's refusal, or
    /// no owner at all.
    pub fn convert_selection(
        &self,
        window: Window,
        name: &str,
        target: &str,
    ) -> Result<Option<Vec<u8>>, ReplyError> {
        let selection = self.atom(name)?;
        let property = self.atom("CAMINITO_TEST_SELECTION")?;
        self.conn.convert_selection(
            window,
            selection,
            self.atom(target)?,
            property,
            x11rb::CURRENT_TIME,
        )?;
        self.conn.flush()?;
        let answered = self.wait_for(|event| match event {
            Event::SelectionNotify(notify)
                if notify.requestor == window && notify.selection == selection =>
            {
                Some(notify.property)
            }
            _ => None,
        })?;
        if answered == u32::from(AtomEnum::NONE) {
            return Ok(None);
        }
        let reply = self
            .conn
            .get_property(true, window, property, AtomEnum::ANY, 0, u32::MAX)?
            .reply()?;
        Ok(Some(reply.value))
    }

    /// Answers requests for the selection this client owns with `text` as `UTF8_STRING`, and
    /// its `TARGETS`, until one asks for `last_target`. Others are refused.
    pub fn serve_selection(&self, text: &str, last_target: &str) -> Result<(), ReplyError> {
        let targets = self.atom("TARGETS")?;
        let utf8 = self.atom("UTF8_STRING")?;
        let last = self.atom(last_target)?;
        loop {
            let request = self.wait_for(|event| match event {
                Event::SelectionRequest(request) => Some(*request),
                _ => None,
            })?;
            let mut property = request.property;
            if request.target == targets {
                self.conn.change_property32(
                    PropMode::REPLACE,
                    request.requestor,
                    request.property,
                    AtomEnum::ATOM,
                    &[targets, utf8],
                )?;
            } else if request.target == utf8 {
                self.conn.change_property8(
                    PropMode::REPLACE,
                    request.requestor,
                    request.property,
                    utf8,
                    text.as_bytes(),
                )?;
            } else {
                property = x11rb::NONE;
            }
            let notify = SelectionNotifyEvent {
                response_type: SELECTION_NOTIFY_EVENT,
                sequence: 0,
                time: request.time,
                requestor: request.requestor,
                selection: request.selection,
                target: request.target,
                property,
            };
            self.conn
                .send_event(false, request.requestor, EventMask::NO_EVENT, notify)?;
            self.conn.sync()?;
            if request.target == last {
                return Ok(());
            }
        }
    }
}

/// An XSETTINGS value.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum XSetting {
    Int(i32),
    Str(String),
    Color([u16; 4]),
}

/// The settings in an `_XSETTINGS_SETTINGS` property, sorted by name (the XSETTINGS
/// specification 0.5, "Format of the _XSETTINGS_SETTINGS property").
fn parse_xsettings(bytes: &[u8]) -> Vec<(String, XSetting)> {
    let big_endian = bytes[0] == 1;
    let mut at = 4;
    let mut take = |len: usize| {
        let taken = &bytes[at..at + len];
        at += len;
        taken
    };
    let card16 = |b: &[u8]| {
        let b = [b[0], b[1]];
        if big_endian {
            u16::from_be_bytes(b)
        } else {
            u16::from_le_bytes(b)
        }
    };
    let card32 = |b: &[u8]| {
        let b = [b[0], b[1], b[2], b[3]];
        if big_endian {
            u32::from_be_bytes(b)
        } else {
            u32::from_le_bytes(b)
        }
    };
    let padded = |len: usize| len.div_ceil(4) * 4;

    let _serial = card32(take(4));
    let count = card32(take(4));
    let mut settings = Vec::new();
    for _ in 0..count {
        let kind = take(2)[0];
        let name_len = usize::from(card16(take(2)));
        let name = String::from_utf8_lossy(&take(padded(name_len))[..name_len]).into_owned();
        let _last_change_serial = card32(take(4));
        let value = match kind {
            0 => XSetting::Int(card32(take(4)) as i32),
            1 => {
                let len = card32(take(4)) as usize;
                XSetting::Str(String::from_utf8_lossy(&take(padded(len))[..len]).into_owned())
            }
            2 => {
                let color = take(8);
                XSetting::Color([0, 2, 4, 6].map(|i| card16(&color[i..])))
            }
            other => panic!("an XSETTINGS setting has an unknown type {other}"),
        };
        settings.push((name, value));
    }
    settings.sort_by(|a, b| a.0.cmp(&b.0));
    settings
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
