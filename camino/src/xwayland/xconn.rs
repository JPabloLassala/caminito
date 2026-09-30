//! Camino's X connection (11, 15): an ordinary x11rb client of Xwayland, beside Smithay's
//! window-manager connection, for the X work Smithay doesn't do. It claims Camino's
//! identity on the root once per `start_wm`, and publishes the root properties Smithay
//! doesn't: the desktops, from policy, whenever the refresh pass finds them changed, and
//! the Xft resources in `RESOURCE_MANAGER`, from the settings (47). Per
//! window it publishes each managed window's desktop, and reads what Smithay doesn't
//! (39). It's a calloop fd source on the main thread (15), which hands on the root and
//! window messages Smithay drops as [`Request`]s, and hears XKB's bell (06).
//!
//! Ownership rule (11): it never writes a property Smithay maintains. Every write goes
//! through `replace32`, `replace8` or `delete`, which assert it in debug builds.

use std::collections::{HashMap, HashSet};
use std::fmt;
use std::os::fd::AsFd;

use camino_policy::MAX_WORKSPACES;
use smithay::reexports::calloop::generic::Generic;
use smithay::reexports::calloop::{
    self, Interest, LoopHandle, Mode, PostAction, RegistrationToken,
};
use smithay::utils::{FrameExtents, Logical, Rectangle};
use smithay::xwayland::xwm::WmWindowType;
use x11rb::connection::{Connection, RequestConnection};
use x11rb::errors::{ConnectError, ConnectionError, ReplyError};
use x11rb::protocol::Event;
use x11rb::protocol::shape::{self, ConnectionExt as _};
use x11rb::protocol::xkb::{self, ConnectionExt as _};
use x11rb::protocol::xproto::{
    Atom, AtomEnum, ChangeWindowAttributesAux, ConnectionExt, EventMask, GetPropertyReply, Gravity,
    PropMode, Window,
};
use x11rb::rust_connection::RustConnection;
use x11rb::wrapper::ConnectionExt as _;

use super::xsettings::{Resources, merge_resources};
use crate::state::State;
use crate::window::{Strut, struts};

x11rb::atom_manager! {
    Atoms: AtomsCookie {
        UTF8_STRING,
        _NET_SUPPORTED,
        _NET_SUPPORTING_WM_CHECK,
        _NET_WM_NAME,
        _NET_NUMBER_OF_DESKTOPS,
        _NET_DESKTOP_NAMES,
        _NET_CURRENT_DESKTOP,
        _NET_WORKAREA,
        _GTK_WORKAREAS,
        // Per window (39).
        _NET_WM_DESKTOP,
        _NET_FRAME_EXTENTS,
        _NET_REQUEST_FRAME_EXTENTS,
        _NET_WM_WINDOW_TYPE,
        _NET_WM_WINDOW_TYPE_COMBO,
        _NET_WM_WINDOW_TYPE_DESKTOP,
        _NET_WM_WINDOW_TYPE_DND,
        _NET_WM_WINDOW_TYPE_DOCK,
        _NET_WM_WINDOW_TYPE_DROPDOWN_MENU,
        _NET_WM_WINDOW_TYPE_DIALOG,
        _NET_WM_WINDOW_TYPE_MENU,
        _NET_WM_WINDOW_TYPE_NOTIFICATION,
        _NET_WM_WINDOW_TYPE_NORMAL,
        _NET_WM_WINDOW_TYPE_POPUP_MENU,
        _NET_WM_WINDOW_TYPE_SPLASH,
        _NET_WM_WINDOW_TYPE_TOOLBAR,
        _NET_WM_WINDOW_TYPE_TOOLTIP,
        _NET_WM_WINDOW_TYPE_UTILITY,
        _MOTIF_WM_HINTS,
        _NET_CLOSE_WINDOW,
        _NET_MOVERESIZE_WINDOW,
        _NET_WM_MOVERESIZE,
        _NET_WM_ICON,
        _NET_WM_STRUT,
        _NET_WM_STRUT_PARTIAL,
        _NET_WM_STATE_SKIP_TASKBAR,
        _NET_WM_STATE_SKIP_PAGER,
        _GTK_SHOW_WINDOW_MENU,
        _NET_WM_STATE_FULLSCREEN,
        _NET_WM_STATE_MAXIMIZED_VERT,
        _NET_WM_STATE_MAXIMIZED_HORZ,
        // Smithay's (11), which are never written here.
        _NET_CLIENT_LIST,
        _NET_CLIENT_LIST_STACKING,
        _NET_ACTIVE_WINDOW,
        _NET_WM_STATE,
        WM_STATE,
        _NET_SHOWING_DESKTOP,
        _XSETTINGS_SETTINGS,
    }
}

impl Atoms {
    /// The first of `types` Smithay knows, as `X11Surface::window_type` reads them.
    fn window_type(&self, types: &[Atom]) -> Option<WmWindowType> {
        let known = [
            (self._NET_WM_WINDOW_TYPE_COMBO, WmWindowType::Combo),
            (self._NET_WM_WINDOW_TYPE_DESKTOP, WmWindowType::Desktop),
            (self._NET_WM_WINDOW_TYPE_DND, WmWindowType::Dnd),
            (self._NET_WM_WINDOW_TYPE_DOCK, WmWindowType::Dock),
            (
                self._NET_WM_WINDOW_TYPE_DROPDOWN_MENU,
                WmWindowType::DropdownMenu,
            ),
            (self._NET_WM_WINDOW_TYPE_DIALOG, WmWindowType::Dialog),
            (self._NET_WM_WINDOW_TYPE_MENU, WmWindowType::Menu),
            (
                self._NET_WM_WINDOW_TYPE_NOTIFICATION,
                WmWindowType::Notification,
            ),
            (self._NET_WM_WINDOW_TYPE_NORMAL, WmWindowType::Normal),
            (self._NET_WM_WINDOW_TYPE_POPUP_MENU, WmWindowType::PopupMenu),
            (self._NET_WM_WINDOW_TYPE_SPLASH, WmWindowType::Splash),
            (self._NET_WM_WINDOW_TYPE_TOOLBAR, WmWindowType::Toolbar),
            (self._NET_WM_WINDOW_TYPE_TOOLTIP, WmWindowType::Tooltip),
            (self._NET_WM_WINDOW_TYPE_UTILITY, WmWindowType::Utility),
        ];
        types.iter().find_map(|atom| {
            known
                .iter()
                .find(|(known, _)| known == atom)
                .map(|(_, window_type)| *window_type)
        })
    }

    /// What Camino handles beyond Smithay's `_NET_SUPPORTED`. `_GTK_SHOW_WINDOW_MENU` waits
    /// for M8's window menu: GTK sends it only if it's listed, and shows its own menu
    /// otherwise.
    fn supported(&self) -> [Atom; 15] {
        [
            self._NET_SUPPORTING_WM_CHECK,
            self._NET_WM_NAME,
            self._NET_NUMBER_OF_DESKTOPS,
            self._NET_DESKTOP_NAMES,
            self._NET_CURRENT_DESKTOP,
            self._NET_WORKAREA,
            // GTK3 reads `_GTK_WORKAREAS_D<n>` only if this is listed.
            self._GTK_WORKAREAS,
            // Per window (39).
            self._NET_WM_DESKTOP,
            self._NET_FRAME_EXTENTS,
            self._NET_REQUEST_FRAME_EXTENTS,
            self._NET_CLOSE_WINDOW,
            self._NET_MOVERESIZE_WINDOW,
            self._NET_WM_ICON,
            self._NET_WM_STRUT,
            self._NET_WM_STRUT_PARTIAL,
        ]
    }

    /// The properties Smithay maintains (11): `_NET_SHOWING_DESKTOP` and XSETTINGS only
    /// through its API, the rest never.
    fn smithay_maintained(&self) -> [Atom; 7] {
        [
            self._NET_CLIENT_LIST,
            self._NET_CLIENT_LIST_STACKING,
            self._NET_ACTIVE_WINDOW,
            self._NET_WM_STATE,
            self.WM_STATE,
            self._NET_SHOWING_DESKTOP,
            self._XSETTINGS_SETTINGS,
        ]
    }
}

/// Why Camino's X connection failed.
#[derive(Debug)]
pub(super) enum Error {
    /// It couldn't connect to Xwayland's display.
    Connect(ConnectError),
    /// The connection broke while talking to Xwayland.
    Connection(ConnectionError),
    /// Xwayland refused a request.
    Reply(ReplyError),
    /// The event loop couldn't watch the connection.
    Watch(calloop::Error),
}

impl fmt::Display for Error {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Error::Connect(err) => write!(f, "can't connect to Xwayland: {err}"),
            Error::Connection(err) => write!(f, "lost the connection to Xwayland: {err}"),
            Error::Reply(err) => write!(f, "Xwayland refused a request: {err}"),
            Error::Watch(err) => write!(f, "can't watch the connection to Xwayland: {err}"),
        }
    }
}

impl std::error::Error for Error {}

impl From<ConnectError> for Error {
    fn from(err: ConnectError) -> Self {
        Error::Connect(err)
    }
}

impl From<ConnectionError> for Error {
    fn from(err: ConnectionError) -> Self {
        Error::Connection(err)
    }
}

impl From<ReplyError> for Error {
    fn from(err: ReplyError) -> Self {
        Error::Reply(err)
    }
}

/// The root properties Camino publishes, as policy has them.
#[derive(Debug, Clone, PartialEq)]
pub(super) struct RootProperties {
    /// One per workspace, so also `_NET_NUMBER_OF_DESKTOPS`.
    pub(super) desktop_names: Vec<String>,
    /// The active workspace.
    pub(super) current_desktop: usize,
    /// Each monitor's, in X client space.
    pub(super) work_areas: Vec<Rectangle<i32, Logical>>,
}

impl RootProperties {
    /// `_NET_WORKAREA`'s one rectangle: Marco's is the X screen's, from the origin to the
    /// far edges of every monitor, less the struts (`screen.c:2354-2435`). There are none
    /// yet.
    fn screen_work_area(&self) -> Rectangle<i32, Logical> {
        let far = self.work_areas.iter().fold((0, 0), |(w, h), area| {
            let edge = area.loc + area.size;
            (w.max(edge.x), h.max(edge.y))
        });
        Rectangle::new((0, 0).into(), far.into())
    }
}

/// `_MOTIF_WM_HINTS`'s flag for its decorations field.
const MWM_HINTS_DECORATIONS: u32 = 1 << 1;

/// A reply, or `None` if X refused the request, as it does for a window that has gone. Only
/// a broken connection is an error.
fn gone_is_none<T>(reply: Result<T, ReplyError>) -> Result<Option<T>, Error> {
    match reply {
        Ok(reply) => Ok(Some(reply)),
        Err(ReplyError::X11Error(_)) => Ok(None),
        Err(ReplyError::ConnectionError(err)) => Err(Error::Connection(err)),
    }
}

/// A rectangle as EWMH writes one: x, y, width, height.
fn cardinals(rect: Rectangle<i32, Logical>) -> [u32; 4] {
    [rect.loc.x, rect.loc.y, rect.size.w, rect.size.h].map(|n| n as u32)
}

/// A message Smithay drops, for Camino to run as its own bindings would (11, 39).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(super) enum Request {
    /// `_NET_CURRENT_DESKTOP`: switch to this workspace.
    CurrentDesktop(usize),
    /// `_NET_NUMBER_OF_DESKTOPS`: have this many workspaces.
    NumberOfDesktops(usize),
    /// `_NET_WM_DESKTOP`: move the window to this desktop, or `0xFFFFFFFF` for every one.
    Desktop { window: Window, desktop: u32 },
    /// `_NET_REQUEST_FRAME_EXTENTS`: publish the window's frame extents, before it maps.
    FrameExtents { window: Window },
    /// `_NET_CLOSE_WINDOW`.
    Close { window: Window },
    /// `_NET_WM_MOVERESIZE`'s keyboard move (10) or resize (9), which Smithay drops.
    KeyboardMoveResize { window: Window, resize: bool },
    /// `_NET_WM_MOVERESIZE`'s cancel (11).
    CancelMoveResize { window: Window },
    /// `_NET_WM_STATE`'s skip-taskbar or skip-pager, which Smithay drops after map: `action`
    /// 0 removes, 1 adds and 2 toggles.
    Skips {
        window: Window,
        action: u32,
        taskbar: bool,
        pager: bool,
    },
    /// SHAPE's `ShapeNotify` for a window's input region: read it again, or it has none if
    /// it's no longer shaped.
    InputShape { window: Window, shaped: bool },
    /// `_NET_MOVERESIZE_WINDOW`: a `ConfigureRequest` by message, with its own gravity if
    /// it names one.
    MoveResize {
        window: Window,
        gravity: Option<Gravity>,
        loc: (Option<i32>, Option<i32>),
        size: (Option<u32>, Option<u32>),
    },
}

/// A window property Camino reads over its own connection, which Smithay reports as
/// `WmWindowProperty::Other` when it changes.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(super) enum WindowProperty {
    Icon,
    /// `_NET_WM_STRUT` or `_NET_WM_STRUT_PARTIAL`.
    Struts,
}

/// What a window that may not have mapped says about its frame, read for
/// `_NET_REQUEST_FRAME_EXTENTS`. Smithay may not have read it yet.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(super) struct FrameRequest {
    /// `_NET_WM_WINDOW_TYPE`'s first type Smithay knows.
    pub(super) window_type: Option<WmWindowType>,
    /// `WM_TRANSIENT_FOR`.
    pub(super) transient_for: Option<Window>,
    /// Whether `_MOTIF_WM_HINTS` leaves decorations on.
    pub(super) decorated: bool,
    /// `_NET_WM_STATE_FULLSCREEN`, asked for before map.
    pub(super) fullscreen: bool,
    /// `_NET_WM_STATE_MAXIMIZED_VERT` and `_HORZ`, both asked for before map.
    pub(super) maximized: bool,
}

/// The properties Camino publishes on a managed window, as policy has them (39).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(super) struct WindowProperties {
    /// `_NET_WM_DESKTOP`: its workspace, or `0xFFFFFFFF` while it's sticky.
    pub(super) desktop: u32,
    /// `_NET_FRAME_EXTENTS`: the frame policy lays it out with, in X client space.
    pub(super) frame_extents: FrameExtents<i32, Logical>,
}

/// Camino's connection to Xwayland. Dropping it closes it.
pub(super) struct XConnection {
    conn: RustConnection,
    root: Window,
    atoms: Atoms,
    /// `_GTK_WORKAREAS_D<n>`, for each workspace there can be.
    gtk_workareas: Vec<Atom>,
    /// What was last written, so only what changed is written again.
    published: Option<RootProperties>,
    /// What was last written on each managed window.
    published_windows: HashMap<Window, WindowProperties>,
    /// The X resources last merged into `RESOURCE_MANAGER`.
    published_resources: Option<Resources>,
    /// Managed windows whose `_NET_FRAME_EXTENTS` is written again, changed or not.
    frame_extents_owed: HashSet<Window>,
    /// Whether Xwayland has SHAPE, which input regions are read with.
    has_shape: bool,
    loop_handle: LoopHandle<'static, State>,
    source: RegistrationToken,
}

impl XConnection {
    /// Connects to `:display` once `start_wm` has returned, and claims Camino's identity:
    /// Camino's superset of `_NET_SUPPORTED`, and "Camino" as the check window's name,
    /// for mcc's `wm-common` (11). From then on, `handle` runs `State::x_requests`
    /// when X sends something.
    pub(super) fn connect(
        display: u32,
        handle: &LoopHandle<'static, State>,
    ) -> Result<Self, Error> {
        let (conn, screen) = x11rb::connect(Some(&format!(":{display}")))?;
        let root = conn.setup().roots[screen].root;
        let atoms = Atoms::new(&conn)?;
        let gtk_workareas = (0..MAX_WORKSPACES)
            .map(|n| conn.intern_atom(false, format!("_GTK_WORKAREAS_D{n}").as_bytes()))
            .collect::<Result<Vec<_>, _>>()?;
        let atoms = atoms.reply()?;
        let gtk_workareas = gtk_workareas
            .into_iter()
            .map(|cookie| Ok(cookie.reply()?.atom))
            .collect::<Result<Vec<_>, Error>>()?;
        // Root messages are sent with both masks, so `SubstructureNotify` gets them without
        // taking redirection from Smithay (11).
        let events = EventMask::SUBSTRUCTURE_NOTIFY | EventMask::PROPERTY_CHANGE;
        conn.change_window_attributes(root, &ChangeWindowAttributesAux::new().event_mask(events))?
            .check()?;
        let xkb = conn.xkb_use_extension(1, 0)?.reply()?;
        if xkb.supported {
            conn.xkb_select_events(
                xkb::ID::USE_CORE_KBD.into(),
                xkb::EventType::from(0u16),
                xkb::EventType::BELL_NOTIFY,
                xkb::MapPart::from(0u16),
                xkb::MapPart::from(0u16),
                &xkb::SelectEventsAux::new(),
            )?;
        } else {
            tracing::debug!("Xwayland has no XKB, so X bells aren't heard");
        }
        let has_shape = conn
            .extension_information(shape::X11_EXTENSION_NAME)?
            .is_some();
        if !has_shape {
            tracing::debug!("Xwayland has no SHAPE, so X windows take the pointer everywhere");
        }
        let fd = conn
            .stream()
            .as_fd()
            .try_clone_to_owned()
            .map_err(|err| Error::Watch(calloop::Error::IoError(err)))?;
        // Level-triggered, and every event x11rb has queued is handled each time. x11rb can
        // queue events while it waits for a reply or flushes, which leaves the socket quiet,
        // but each publish then writes a root property, and its `PropertyNotify` wakes this.
        let source = handle
            .insert_source(
                Generic::new(fd, Interest::READ, Mode::Level),
                |_, _, state| {
                    state.x_requests();
                    Ok(PostAction::Continue)
                },
            )
            .map_err(|err| Error::Watch(err.error))?;
        let connection = XConnection {
            conn,
            root,
            atoms,
            gtk_workareas,
            published: None,
            published_windows: HashMap::new(),
            published_resources: None,
            frame_extents_owed: HashSet::new(),
            has_shape,
            loop_handle: handle.clone(),
            source,
        };
        connection.claim_identity()?;
        Ok(connection)
    }

    /// The messages X has sent since this was last asked. Bells are only logged until M6
    /// joins them to the single bell event (06).
    pub(super) fn requests(&mut self) -> Result<Vec<Request>, Error> {
        let mut requests = Vec::new();
        while let Some(event) = self.conn.poll_for_event()? {
            let message = match event {
                Event::ClientMessage(message) => message,
                Event::ShapeNotify(notify) => {
                    if notify.shape_kind == shape::SK::INPUT {
                        requests.push(Request::InputShape {
                            window: notify.affected_window,
                            shaped: notify.shaped,
                        });
                    }
                    continue;
                }
                Event::XkbBellNotify(bell) => {
                    tracing::debug!("an X client rang the bell at {}%", bell.percent);
                    continue;
                }
                _ => continue,
            };
            if message.format != 32 {
                continue;
            }
            let data = message.data.as_data32();
            let (window, type_) = (message.window, message.type_);
            let atoms = &self.atoms;
            if window == self.root {
                if type_ == atoms._NET_CURRENT_DESKTOP {
                    requests.push(Request::CurrentDesktop(data[0] as usize));
                } else if type_ == atoms._NET_NUMBER_OF_DESKTOPS {
                    requests.push(Request::NumberOfDesktops(data[0] as usize));
                }
            } else if type_ == atoms._NET_REQUEST_FRAME_EXTENTS {
                requests.push(Request::FrameExtents { window });
            } else if type_ == atoms._NET_WM_MOVERESIZE {
                // Smithay handles the pointer's directions, 0 to 8.
                match data[2] {
                    9 | 10 => requests.push(Request::KeyboardMoveResize {
                        window,
                        resize: data[2] == 9,
                    }),
                    11 => requests.push(Request::CancelMoveResize { window }),
                    _ => {}
                }
            } else if type_ == atoms._NET_WM_STATE {
                let names = |atom: Atom| data[1] == atom || data[2] == atom;
                let (taskbar, pager) = (
                    names(atoms._NET_WM_STATE_SKIP_TASKBAR),
                    names(atoms._NET_WM_STATE_SKIP_PAGER),
                );
                if taskbar || pager {
                    requests.push(Request::Skips {
                        window,
                        action: data[0],
                        taskbar,
                        pager,
                    });
                }
            } else if type_ == atoms._GTK_SHOW_WINDOW_MENU {
                // M8's window menu (05) opens here.
                tracing::debug!(
                    "an X client asked for the window menu of {window:#x} at {}, {}",
                    data[1] as i32,
                    data[2] as i32
                );
            } else if type_ == atoms._NET_CLOSE_WINDOW {
                requests.push(Request::Close { window });
            } else if type_ == atoms._NET_MOVERESIZE_WINDOW {
                // EWMH: the gravity in the low byte, 0 for the window's own; then which of
                // x, y, width and height are given, in bits 8 to 11.
                let flags = data[0];
                let given = |bit: u32| flags & 1 << bit != 0;
                let gravity = (flags & 0xff != 0).then(|| Gravity::from((flags & 0xff) as u8));
                requests.push(Request::MoveResize {
                    window,
                    gravity,
                    loc: (
                        given(8).then_some(data[1] as i32),
                        given(9).then_some(data[2] as i32),
                    ),
                    size: (given(10).then_some(data[3]), given(11).then_some(data[4])),
                });
            } else if type_ == atoms._NET_WM_DESKTOP {
                requests.push(Request::Desktop {
                    window,
                    desktop: data[0],
                });
            }
        }
        Ok(requests)
    }

    /// A window's `CARDINAL` property, or `None` if it's unset or the window has gone.
    fn cardinals(&self, window: Window, property: Atom) -> Result<Option<Vec<u32>>, Error> {
        let cookie =
            self.conn
                .get_property(false, window, property, AtomEnum::CARDINAL, 0, u32::MAX)?;
        let Some(reply) = gone_is_none(cookie.reply())? else {
            return Ok(None);
        };
        Ok(reply.value32().map(Iterator::collect))
    }

    /// `_NET_WM_DESKTOP`, as a window asks for it before it maps.
    pub(super) fn desktop(&self, window: Window) -> Result<Option<u32>, Error> {
        let desktop = self.cardinals(window, self.atoms._NET_WM_DESKTOP)?;
        Ok(desktop.and_then(|value| value.first().copied()))
    }

    /// `_NET_WM_ICON`, unparsed.
    pub(super) fn icon(&self, window: Window) -> Result<Option<Vec<u32>>, Error> {
        self.cardinals(window, self.atoms._NET_WM_ICON)
    }

    /// The window's struts, from `_NET_WM_STRUT_PARTIAL` or else `_NET_WM_STRUT`.
    pub(super) fn struts(&self, window: Window) -> Result<Vec<Strut>, Error> {
        let partial = self.cardinals(window, self.atoms._NET_WM_STRUT_PARTIAL)?;
        let strut = self.cardinals(window, self.atoms._NET_WM_STRUT)?;
        Ok(struts(partial.as_deref(), strut.as_deref()))
    }

    /// Asks for `ShapeNotify` on a window, so its input region is kept up to date.
    pub(super) fn watch_shape(&self, window: Window) -> Result<(), Error> {
        if self.has_shape {
            self.conn.shape_select_input(window, true)?;
        }
        Ok(())
    }

    /// A window's input region (SHAPE's `Input` kind), from its origin, or `None` if it
    /// takes input everywhere: X reports one rectangle covering an unshaped window, which
    /// is taken as no shape, so that it follows the window's size.
    pub(super) fn input_shape(
        &self,
        window: Window,
    ) -> Result<Option<Vec<Rectangle<i32, Logical>>>, Error> {
        if !self.has_shape {
            return Ok(None);
        }
        let rectangles = self.conn.shape_get_rectangles(window, shape::SK::INPUT)?;
        let geometry = self.conn.get_geometry(window)?;
        let (Some(rectangles), Some(geometry)) = (
            gone_is_none(rectangles.reply())?,
            gone_is_none(geometry.reply())?,
        ) else {
            return Ok(None);
        };
        let rects: Vec<Rectangle<i32, Logical>> = rectangles
            .rectangles
            .iter()
            .map(|r| {
                Rectangle::new(
                    (i32::from(r.x), i32::from(r.y)).into(),
                    (i32::from(r.width), i32::from(r.height)).into(),
                )
            })
            .collect();
        let border = i32::from(geometry.border_width);
        let whole = Rectangle::new(
            (-border, -border).into(),
            (
                i32::from(geometry.width) + 2 * border,
                i32::from(geometry.height) + 2 * border,
            )
                .into(),
        );
        Ok((rects != [whole]).then_some(rects))
    }

    /// Which of the window properties Camino reads `atom` is, if it's one.
    pub(super) fn window_property(&self, atom: Atom) -> Option<WindowProperty> {
        let atoms = &self.atoms;
        if atom == atoms._NET_WM_ICON {
            Some(WindowProperty::Icon)
        } else if atom == atoms._NET_WM_STRUT || atom == atoms._NET_WM_STRUT_PARTIAL {
            Some(WindowProperty::Struts)
        } else {
            None
        }
    }

    /// The frame extents last published on a managed window.
    pub(super) fn published_frame_extents(
        &self,
        window: Window,
    ) -> Option<FrameExtents<i32, Logical>> {
        Some(self.published_windows.get(&window)?.frame_extents)
    }

    /// What a window says about its frame, or `None` if it has gone.
    pub(super) fn frame_request(&self, window: Window) -> Result<Option<FrameRequest>, Error> {
        let atoms = &self.atoms;
        let get = |property: Atom| {
            self.conn
                .get_property(false, window, property, AtomEnum::ANY, 0, u32::MAX)
        };
        let types = get(atoms._NET_WM_WINDOW_TYPE)?;
        let transient_for = get(AtomEnum::WM_TRANSIENT_FOR.into())?;
        let motif = get(atoms._MOTIF_WM_HINTS)?;
        let states = get(atoms._NET_WM_STATE)?;
        let values = |reply: Option<GetPropertyReply>| -> Vec<u32> {
            reply
                .and_then(|reply| reply.value32().map(Iterator::collect))
                .unwrap_or_default()
        };
        let (Some(types), Some(transient_for), Some(motif), Some(states)) = (
            gone_is_none(types.reply())?,
            gone_is_none(transient_for.reply())?,
            gone_is_none(motif.reply())?,
            gone_is_none(states.reply())?,
        ) else {
            return Ok(None);
        };
        let states = values(Some(states));
        let motif = values(Some(motif));
        // flags, functions, decorations: decorations count only if the flags say so.
        let decorated =
            !(motif.len() >= 3 && motif[0] & MWM_HINTS_DECORATIONS != 0 && motif[2] == 0);
        Ok(Some(FrameRequest {
            window_type: atoms.window_type(&values(Some(types))),
            transient_for: values(Some(transient_for)).first().copied(),
            decorated,
            fullscreen: states.contains(&atoms._NET_WM_STATE_FULLSCREEN),
            maximized: states.contains(&atoms._NET_WM_STATE_MAXIMIZED_VERT)
                && states.contains(&atoms._NET_WM_STATE_MAXIMIZED_HORZ),
        }))
    }

    /// Answers `_NET_REQUEST_FRAME_EXTENTS` for a window that isn't managed with
    /// `extents`.
    pub(super) fn answer_frame_extents(
        &self,
        window: Window,
        extents: FrameExtents<i32, Logical>,
    ) -> Result<(), Error> {
        self.write_frame_extents(window, extents)?;
        self.conn.flush()?;
        Ok(())
    }

    /// Has the next publish write a managed window's `_NET_FRAME_EXTENTS` again, as an
    /// answer to `_NET_REQUEST_FRAME_EXTENTS`.
    pub(super) fn republish_frame_extents(&mut self, window: Window) {
        self.frame_extents_owed.insert(window);
    }

    /// Smithay's writes are done by now: `start_wm` waits for replies after them.
    fn claim_identity(&self) -> Result<(), Error> {
        let atoms = &self.atoms;
        let mut supported: Vec<Atom> = self
            .conn
            .get_property(
                false,
                self.root,
                atoms._NET_SUPPORTED,
                AtomEnum::ATOM,
                0,
                u32::MAX,
            )?
            .reply()?
            .value32()
            .into_iter()
            .flatten()
            .collect();
        for atom in atoms.supported() {
            if !supported.contains(&atom) {
                supported.push(atom);
            }
        }
        self.replace32(
            self.root,
            atoms._NET_SUPPORTED,
            AtomEnum::ATOM.into(),
            &supported,
        )?;

        let check = self
            .conn
            .get_property(
                false,
                self.root,
                atoms._NET_SUPPORTING_WM_CHECK,
                AtomEnum::WINDOW,
                0,
                1,
            )?
            .reply()?
            .value32()
            .and_then(|mut value| value.next());
        if let Some(check) = check {
            self.replace8(check, atoms._NET_WM_NAME, atoms.UTF8_STRING, b"Camino")?;
        }
        self.conn.flush()?;
        Ok(())
    }

    /// Writes the root properties that differ from those last written.
    pub(super) fn publish(&mut self, properties: RootProperties) -> Result<(), Error> {
        let last = self.published.as_ref();
        if last == Some(&properties) {
            return Ok(());
        }
        let atoms = &self.atoms;
        let root = self.root;
        if last.map(|last| &last.desktop_names) != Some(&properties.desktop_names) {
            let count = properties.desktop_names.len() as u32;
            self.replace32(
                root,
                atoms._NET_NUMBER_OF_DESKTOPS,
                AtomEnum::CARDINAL.into(),
                &[count],
            )?;
            // EWMH: each name ends in a NUL.
            let names: Vec<u8> = properties
                .desktop_names
                .iter()
                .flat_map(|name| name.bytes().chain([0]))
                .collect();
            self.replace8(root, atoms._NET_DESKTOP_NAMES, atoms.UTF8_STRING, &names)?;
        }
        if last.map(|last| last.current_desktop) != Some(properties.current_desktop) {
            let current = properties.current_desktop as u32;
            self.replace32(
                root,
                atoms._NET_CURRENT_DESKTOP,
                AtomEnum::CARDINAL.into(),
                &[current],
            )?;
        }
        let count_changed =
            last.map(|last| last.desktop_names.len()) != Some(properties.desktop_names.len());
        if count_changed || last.map(|last| &last.work_areas) != Some(&properties.work_areas) {
            let count = properties.desktop_names.len();
            let screen = cardinals(properties.screen_work_area());
            self.replace32(
                root,
                atoms._NET_WORKAREA,
                AtomEnum::CARDINAL.into(),
                &screen.repeat(count),
            )?;
            let monitors: Vec<u32> = properties
                .work_areas
                .iter()
                .copied()
                .flat_map(cardinals)
                .collect();
            for atom in &self.gtk_workareas[..count] {
                self.replace32(root, *atom, AtomEnum::CARDINAL.into(), &monitors)?;
            }
            let last_count = last.map_or(0, |last| last.desktop_names.len());
            for atom in self
                .gtk_workareas
                .get(count..last_count)
                .unwrap_or_default()
            {
                self.delete(root, *atom)?;
            }
        }
        self.conn.flush()?;
        self.published = Some(properties);
        Ok(())
    }

    /// Writes each managed window's properties that differ from those last written. A
    /// window that's no longer managed loses its `_NET_WM_DESKTOP`, as EWMH asks of a
    /// withdrawn window; it may have gone, and X refuses that harmlessly.
    pub(super) fn publish_windows(
        &mut self,
        windows: impl IntoIterator<Item = (Window, WindowProperties)>,
    ) -> Result<(), Error> {
        let mut previous = std::mem::take(&mut self.published_windows);
        for (window, properties) in windows {
            let last = previous.remove(&window);
            if last.map(|last| last.desktop) != Some(properties.desktop) {
                self.replace32(
                    window,
                    self.atoms._NET_WM_DESKTOP,
                    AtomEnum::CARDINAL.into(),
                    &[properties.desktop],
                )?;
            }
            if last.map(|last| last.frame_extents) != Some(properties.frame_extents)
                || self.frame_extents_owed.contains(&window)
            {
                self.write_frame_extents(window, properties.frame_extents)?;
            }
            self.published_windows.insert(window, properties);
        }
        for window in previous.into_keys() {
            self.delete(window, self.atoms._NET_WM_DESKTOP)?;
        }
        self.frame_extents_owed.clear();
        self.conn.flush()?;
        Ok(())
    }

    /// `_NET_FRAME_EXTENTS`, which EWMH orders left, right, top, bottom.
    fn write_frame_extents(
        &self,
        window: Window,
        extents: FrameExtents<i32, Logical>,
    ) -> Result<(), Error> {
        let e = extents;
        let value = [e.left, e.right, e.top, e.bottom].map(|n| n.max(0) as u32);
        self.replace32(
            window,
            self.atoms._NET_FRAME_EXTENTS,
            AtomEnum::CARDINAL.into(),
            &value,
        )
    }

    /// Merges `resources` into the root's `RESOURCE_MANAGER` (47) if they differ from those
    /// last merged. What other clients wrote there, such as xrdb, is kept.
    pub(super) fn publish_resources(&mut self, resources: Resources) -> Result<(), Error> {
        if self.published_resources.as_ref() == Some(&resources) {
            return Ok(());
        }
        let existing = self
            .conn
            .get_property(
                false,
                self.root,
                AtomEnum::RESOURCE_MANAGER,
                AtomEnum::STRING,
                0,
                u32::MAX,
            )?
            .reply()?
            .value;
        let merged = merge_resources(&existing, &resources);
        if merged != existing {
            self.replace8(
                self.root,
                AtomEnum::RESOURCE_MANAGER.into(),
                AtomEnum::STRING.into(),
                &merged,
            )?;
            self.conn.flush()?;
        }
        self.published_resources = Some(resources);
        Ok(())
    }

    /// Waits until Xwayland has handled every request sent so far. x11rb queues the events
    /// that arrive meanwhile, which the refresh pass reads.
    #[cfg(test)]
    pub(super) fn sync(&mut self) -> Result<(), Error> {
        self.conn.sync()?;
        Ok(())
    }

    fn replace32(
        &self,
        window: Window,
        property: Atom,
        type_: Atom,
        data: &[u32],
    ) -> Result<(), Error> {
        self.assert_not_smithays(property);
        self.conn
            .change_property32(PropMode::REPLACE, window, property, type_, data)?;
        Ok(())
    }

    fn replace8(
        &self,
        window: Window,
        property: Atom,
        type_: Atom,
        data: &[u8],
    ) -> Result<(), Error> {
        self.assert_not_smithays(property);
        self.conn
            .change_property8(PropMode::REPLACE, window, property, type_, data)?;
        Ok(())
    }

    fn delete(&self, window: Window, property: Atom) -> Result<(), Error> {
        self.assert_not_smithays(property);
        self.conn.delete_property(window, property)?;
        Ok(())
    }

    /// The ownership rule (11).
    fn assert_not_smithays(&self, property: Atom) {
        debug_assert!(
            !self.atoms.smithay_maintained().contains(&property),
            "Camino's X connection never writes a property Smithay maintains (11)"
        );
    }
}

impl Drop for XConnection {
    fn drop(&mut self) {
        self.loop_handle.remove(self.source);
    }
}
