//! The X11 adapter: what `Mapped` reads from an XWayland window through Smithay's `X11Wm`,
//! and asks of it (11). Its facts are Marco's reading of the ICCCM and EWMH properties.

use std::sync::Mutex;

use caminito_policy::{PreMapConfigure, SizeHints, WindowStates, WindowType};
use smithay::reexports::x11rb::protocol::xproto::Gravity;
use smithay::utils::{FrameExtents, Logical, Point, Rectangle, Serial, Size};
use smithay::xwayland::X11Surface;
use smithay::xwayland::xwm::{PingError, WmInputModel, WmWindowType};

use super::{Adapter, Skips, WindowKey};
use crate::decoration::{IconSource, NetWmIcon};
use crate::input::KeyboardFocus;

/// `_NET_WM_DESKTOP`'s value for a window on every desktop.
pub(crate) const STICKY: u32 = 0xFFFF_FFFF;

/// Marco's type for an X window (`recalc_window_type`, window.c:6568-6607): its first
/// `_NET_WM_WINDOW_TYPE` Marco knows, or with none a dialog if it's transient for another
/// window and else a normal window. A dialog with `_NET_WM_STATE_MODAL` is a modal dialog.
///
/// Marco doesn't know the override-redirect types, so on a managed window they count as no
/// type. It would carry on to a later type in the list; Smithay only reads the first it
/// knows, so here they end the search.
pub(crate) fn marco_type(ewmh: Option<WmWindowType>, transient: bool, modal: bool) -> WindowType {
    let window_type = match ewmh {
        Some(WmWindowType::Normal) => WindowType::Normal,
        Some(WmWindowType::Desktop) => WindowType::Desktop,
        Some(WmWindowType::Dock) => WindowType::Dock,
        Some(WmWindowType::Toolbar) => WindowType::Toolbar,
        Some(WmWindowType::Menu) => WindowType::Menu,
        Some(WmWindowType::Dialog) => WindowType::Dialog,
        Some(WmWindowType::Utility) => WindowType::Utility,
        Some(WmWindowType::Splash) => WindowType::Splashscreen,
        Some(
            WmWindowType::DropdownMenu
            | WmWindowType::PopupMenu
            | WmWindowType::Tooltip
            | WmWindowType::Notification
            | WmWindowType::Combo
            | WmWindowType::Dnd,
        )
        | None
            if transient =>
        {
            WindowType::Dialog
        }
        Some(_) | None => WindowType::Normal,
    };
    if window_type == WindowType::Dialog && modal {
        WindowType::ModalDialog
    } else {
        window_type
    }
}

/// Where an X window goes when its client asks for `rect`, with a frame of `extents` around
/// it: the reference point its gravity names stays where the client put it (ICCCM 4.1.2.3,
/// Marco's `adjust_for_gravity`, window.c:3279-3405). Caminito's frames have no X border.
fn gravity_position(
    gravity: Gravity,
    rect: Rectangle<i32, Logical>,
    extents: FrameExtents<i32, Logical>,
) -> Point<i32, Logical> {
    let (x, y, w, h) = (rect.loc.x, rect.loc.y, rect.size.w, rect.size.h);
    let frame_w = extents.left + w + extents.right;
    let frame_h = extents.top + h + extents.bottom;
    // How far along each axis the reference point is: 0 at the start, 1 in the middle and
    // 2 at the end.
    let (along_x, along_y) = match gravity {
        Gravity::NORTH_WEST => (0, 0),
        Gravity::NORTH => (1, 0),
        Gravity::NORTH_EAST => (2, 0),
        Gravity::WEST => (0, 1),
        Gravity::CENTER => (1, 1),
        Gravity::EAST => (2, 1),
        Gravity::SOUTH_WEST => (0, 2),
        Gravity::SOUTH => (1, 2),
        Gravity::SOUTH_EAST => (2, 2),
        // Static, and anything unknown: the window stays where it is.
        _ => return rect.loc,
    };
    let axis = |start: i32, len: i32, frame_len: i32, child: i32, along: i32| {
        let reference = start + len * along / 2;
        reference - frame_len * along / 2 + child
    };
    Point::new(
        axis(x, w, frame_w, extents.left, along_x),
        axis(y, h, frame_h, extents.top, along_y),
    )
}

/// Where and how big an X window asks to be, as window geometry. `None` leaves it be.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) struct RequestedGeometry {
    pub loc: Option<Point<i32, Logical>>,
    pub size: Option<Size<i32, Logical>>,
}

/// The geometry an X window asks for with a `ConfigureRequest`, as the window geometry
/// inside a frame of `extents`, placed by `gravity` or else its own. What it leaves out
/// stays as it is.
pub(crate) fn configure_request_geometry(
    surface: &X11Surface,
    gravity: Option<Gravity>,
    loc: (Option<i32>, Option<i32>),
    size: (Option<u32>, Option<u32>),
    extents: FrameExtents<i32, Logical>,
) -> RequestedGeometry {
    let last = surface.last_configure();
    let toolkit = surface.frame_extents();
    let requested_size = (size.0.is_some() || size.1.is_some()).then(|| {
        let dim = |v: Option<u32>, last: i32| v.map_or(last, |v| v as i32);
        Size::new(dim(size.0, last.size.w), dim(size.1, last.size.h))
    });
    let requested_loc = (loc.0.is_some() || loc.1.is_some()).then(|| {
        let rect = Rectangle::new(
            (loc.0.unwrap_or(last.loc.x), loc.1.unwrap_or(last.loc.y)).into(),
            requested_size.unwrap_or(last.size),
        );
        let gravity = gravity
            .or_else(|| surface.size_hints().and_then(|hints| hints.win_gravity))
            .unwrap_or(Gravity::NORTH_WEST);
        gravity_position(gravity, rect, extents) + Point::new(toolkit.left, toolkit.top)
    });
    let requested_size = requested_size.map(|size| {
        Size::new(
            size.w - toolkit.left - toolkit.right,
            size.h - toolkit.top - toolkit.bottom,
        )
    });
    RequestedGeometry {
        loc: requested_loc,
        size: requested_size,
    }
}

/// What was asked of the window since the last flush, for [`Adapter::flush`] to send.
#[derive(Default)]
struct Requested(Mutex<Unflushed>);

#[derive(Default)]
struct Unflushed {
    /// Policy's geometry and states.
    geometry: Option<(Rectangle<i32, Logical>, WindowStates)>,
    /// `_NET_WM_STATE_MODAL`, as the client asked.
    modal: Option<bool>,
}

/// Runs `f` on what hasn't been flushed yet.
fn unflushed<T>(surface: &X11Surface, f: impl FnOnce(&mut Unflushed) -> T) -> T {
    let requested = surface
        .user_data()
        .get_or_insert_threadsafe(Requested::default);
    f(&mut requested.0.lock().unwrap())
}

/// Records the client's request to be a modal dialog or not. It's a fact at once, and
/// `_NET_WM_STATE` follows at the next flush, as the refresh pass sends outward views.
pub(crate) fn request_modal(surface: &X11Surface, modal: bool) {
    unflushed(surface, |unflushed| unflushed.modal = Some(modal));
}

/// Records the client's request to skip the taskbar or pager, or stop: `action` 0 removes,
/// 1 adds and 2 toggles, as `_NET_WM_STATE` has it. It's Caminito's to keep (11).
pub(crate) fn request_skips(surface: &X11Surface, action: u32, taskbar: bool, pager: bool) {
    let mut skips = Adapter::skips(surface);
    let apply = |skip: &mut bool| match action {
        0 => *skip = false,
        1 => *skip = true,
        2 => *skip = !*skip,
        _ => {}
    };
    if taskbar {
        apply(&mut skips.taskbar);
    }
    if pager {
        apply(&mut skips.pager);
    }
    with_x_facts(surface, |facts| facts.skips = Some(skips));
}

/// What Caminito's X connection has read of a window that Smithay doesn't (39).
#[derive(Debug, Clone, Default)]
pub(crate) struct XFacts {
    /// `_NET_WM_DESKTOP` as it was when the window asked to map. `0xFFFFFFFF` is sticky.
    pub desktop: Option<u32>,
    /// `_NET_WM_ICON`, kept up to date.
    pub icon: Option<NetWmIcon>,
    /// `_NET_WM_STRUT_PARTIAL`, else `_NET_WM_STRUT`, kept up to date. While the window
    /// is managed, they're reserved in policy's work area (64).
    pub struts: Vec<Strut>,
    /// The input region SHAPE gives it, from its origin, kept up to date; `None` takes the
    /// pointer everywhere (35).
    pub input_shape: Option<Vec<Rectangle<i32, Logical>>>,
    /// Skip-taskbar and skip-pager as asked after map, which Smithay drops. Caminito keeps
    /// them and never writes them back to `_NET_WM_STATE` (11).
    pub skips: Option<Skips>,
}

/// Whether an XWayland window takes the pointer at `point` from its origin. GTK's CSD windows
/// leave their shadow out of their input region, which Xwayland doesn't pass on to the
/// `wl_surface` (35).
pub(crate) fn takes_pointer_at(surface: &X11Surface, point: Point<f64, Logical>) -> bool {
    with_x_facts(surface, |facts| {
        facts
            .input_shape
            .as_ref()
            .is_none_or(|rects| rects.iter().any(|rect| rect.to_f64().contains(point)))
    })
}

/// An edge of the screen.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum Side {
    Left,
    Right,
    Top,
    Bottom,
}

/// Space a window reserves along one edge of the screen, in logical pixels.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) struct Strut {
    pub side: Side,
    /// How far in from the edge it reaches.
    pub width: i32,
    /// The first and last pixel along the edge it covers, or `None` for all of it, as
    /// `_NET_WM_STRUT` has it.
    pub span: Option<(i32, i32)>,
}

/// A window's struts from `_NET_WM_STRUT_PARTIAL`, else from `_NET_WM_STRUT`, as EWMH
/// has it: left, right, top and bottom widths, then for `_PARTIAL` each edge's start and
/// end in that order. Values stay in X client space, which the caller divides by the X11
/// scale (41).
pub(crate) fn struts(partial: Option<&[u32]>, strut: Option<&[u32]>) -> Vec<Strut> {
    let sides = [Side::Left, Side::Right, Side::Top, Side::Bottom];
    let (widths, spans) = match (partial, strut) {
        (Some(partial), _) if partial.len() >= 12 => (&partial[..4], Some(&partial[4..12])),
        (_, Some(strut)) if strut.len() >= 4 => (&strut[..4], None),
        _ => return Vec::new(),
    };
    sides
        .into_iter()
        .zip(widths)
        .enumerate()
        .filter(|(_, (_, width))| **width > 0)
        .map(|(n, (side, width))| Strut {
            side,
            width: *width as i32,
            span: spans.map(|spans| (spans[2 * n] as i32, spans[2 * n + 1] as i32)),
        })
        .collect()
}

#[derive(Default)]
struct XFactsData(Mutex<XFacts>);

/// Runs `f` on what Caminito's X connection has read of the window.
pub(crate) fn with_x_facts<T>(surface: &X11Surface, f: impl FnOnce(&mut XFacts) -> T) -> T {
    let facts = surface
        .user_data()
        .get_or_insert_threadsafe(XFactsData::default);
    f(&mut facts.0.lock().unwrap())
}

/// The X window's rectangle for window geometry `geometry`: toolkits that draw shadows
/// say how far they reach with `_GTK_FRAME_EXTENTS`.
fn x_rect(surface: &X11Surface, geometry: Rectangle<i32, Logical>) -> Rectangle<i32, Logical> {
    let toolkit = surface.frame_extents();
    Rectangle::new(
        (geometry.loc.x - toolkit.left, geometry.loc.y - toolkit.top).into(),
        (
            geometry.size.w + toolkit.left + toolkit.right,
            geometry.size.h + toolkit.top + toolkit.bottom,
        )
            .into(),
    )
}

/// Sets `_NET_WM_STATE`'s maximised and fullscreen states.
fn set_net_wm_states(surface: &X11Surface, maximized: bool, fullscreen: bool) {
    let set = surface
        .set_maximized(maximized)
        .and_then(|()| surface.set_fullscreen(fullscreen));
    if let Err(err) = set {
        tracing::debug!("can't set an X window's state: {err}");
    }
}

impl Adapter for X11Surface {
    /// `_NET_WM_NAME`, or `WM_NAME`.
    fn title(&self) -> String {
        X11Surface::title(self)
    }

    /// `WM_CLASS`'s class.
    fn app_id(&self) -> Option<String> {
        let class = self.class();
        (!class.is_empty()).then_some(class)
    }

    /// Marco types by `WM_TRANSIENT_FOR` itself, whether its parent is managed or not.
    fn window_type(&self, _has_parent: bool) -> WindowType {
        marco_type(
            X11Surface::window_type(self),
            self.is_transient_for().is_some(),
            unflushed(self, |unflushed| unflushed.modal).unwrap_or_else(|| self.is_modal()),
        )
    }

    /// `WM_NORMAL_HINTS`'s minimum and maximum.
    fn size_hints(&self) -> SizeHints {
        SizeHints {
            min: self.min_size().unwrap_or_default(),
            max: self.max_size().unwrap_or_default(),
        }
    }

    /// Where the window is, if `WM_NORMAL_HINTS` has `USPosition` or `PPosition`, as its
    /// gravity places it in a frame of `extents`.
    fn requested_position(
        &self,
        extents: FrameExtents<i32, Logical>,
    ) -> Option<Point<i32, Logical>> {
        let hints = X11Surface::size_hints(self)?;
        hints.position?;
        let gravity = hints.win_gravity.unwrap_or(Gravity::NORTH_WEST);
        let toolkit = self.frame_extents();
        let at = gravity_position(gravity, self.last_configure(), extents);
        Some(at + Point::new(toolkit.left, toolkit.top))
    }

    /// `_NET_WM_PID`, never Xwayland's credentials.
    fn pid(&self) -> Option<u32> {
        X11Surface::pid(self)
    }

    /// `_NET_WM_DESKTOP` as it was when the window asked to map (39).
    fn requested_workspace(&self) -> Option<usize> {
        let desktop = with_x_facts(self, |facts| facts.desktop)?;
        (desktop != STICKY).then_some(desktop as usize)
    }

    /// Unless `_MOTIF_WM_HINTS` turns decorations off.
    fn wants_frame(&self) -> bool {
        !self.is_decorated()
    }

    /// `_NET_WM_ICON`, else the desktop entry's through `WM_CLASS` (07).
    fn icon(&self) -> IconSource {
        let app_id = self.app_id();
        match with_x_facts(self, |facts| facts.icon.clone()) {
            Some(icon) => IconSource::X11 { icon, app_id },
            None => IconSource::AppId(app_id),
        }
    }

    /// `WM_TRANSIENT_FOR`.
    fn parent(&self) -> Option<WindowKey> {
        self.is_transient_for().map(WindowKey::X11)
    }

    /// `WM_HINTS`'s urgency. `_NET_WM_STATE_DEMANDS_ATTENTION` is policy's to keep (166).
    fn urgent(&self) -> bool {
        self.hints().is_some_and(|hints| hints.urgent)
    }

    /// ICCCM's input model: the input hint or `WM_TAKE_FOCUS`.
    fn takes_input(&self) -> bool {
        self.input_model() != WmInputModel::None
    }

    /// `_NET_WM_STATE_SKIP_TASKBAR` and `_NET_WM_STATE_SKIP_PAGER`, as asked at map or
    /// since.
    fn skips(&self) -> Skips {
        with_x_facts(self, |facts| facts.skips).unwrap_or_else(|| Skips {
            taskbar: self.is_skip_taskbar(),
            pager: self.is_skip_pager(),
        })
    }

    fn set_geometry(&self, geometry: Rectangle<i32, Logical>, states: WindowStates) {
        unflushed(self, |unflushed| {
            unflushed.geometry = Some((geometry, states))
        });
    }

    fn flush(&self, _size: Size<i32, Logical>) {
        let Unflushed { geometry, modal } = unflushed(self, std::mem::take);
        let mut set = Ok(());
        if let Some(modal) = modal {
            set = set.and(self.set_modal(modal));
        }
        if let Some((geometry, states)) = geometry {
            set_net_wm_states(self, states.maximized, states.fullscreen);
            set = set.and(self.set_shaded(states.shaded));
            // A window whose size changes is asked to say when it has drawn at the new one,
            // with `_NET_WM_SYNC_REQUEST`, and keeps the size it last drew at until then: X
            // resizes it at once, and what it hasn't drawn yet is black. That's so whether
            // the user resizes it or it asks itself, as a launcher does as its results
            // change. A size asked for while one is waited for is sent once it's drawn, so a
            // resize's last size is waited for too. A window that doesn't take the request
            // is configured as ever.
            if let Err(err) = self.configure_with_sync(x_rect(self, geometry), None) {
                tracing::debug!("can't configure an X window: {err}");
            }
        }
        if let Err(err) = set {
            tracing::debug!("can't set an X window's state: {err}");
        }
    }

    /// `_NET_WM_STATE_FOCUSED`.
    fn set_activated(&self, activated: bool) {
        if let Err(err) = X11Surface::set_activated(self, activated) {
            tracing::debug!("can't set an X window's state: {err}");
        }
    }

    /// With `WM_DELETE_WINDOW`, or by killing the client if it doesn't take it.
    fn close(&self) {
        if let Err(err) = X11Surface::close(self) {
            tracing::debug!("can't close an X window: {err}");
        }
    }

    /// `_NET_WM_PING`, if `WM_PROTOCOLS` has it. Marco takes a window without it as one
    /// that answered (`meta_display_ping_window`, `core/display.c:4441-4447`).
    fn ping(&self, serial: Serial) -> bool {
        match self.send_ping(u32::from(serial)) {
            Ok(()) | Err(PingError::PingAlreadyPending(_)) => true,
            Err(PingError::NotSupported | PingError::InvalidTimestamp) => false,
            Err(PingError::Connection(err)) => {
                tracing::debug!("can't ping an XWayland window: {err}");
                false
            }
        }
    }

    /// Sets the states it'll map with, and the size if Caminito sets it, where it is now. The
    /// bounds are xdg-shell's only.
    fn configure_pending(&self, configure: &PreMapConfigure) {
        set_net_wm_states(self, configure.maximized, configure.fullscreen);
        if let Some(size) = configure.size {
            let last = self.last_configure();
            let geometry = Rectangle::new(last.loc, size);
            if let Err(err) = self.configure(x_rect(self, geometry)) {
                tracing::debug!("can't configure an X window: {err}");
            }
        }
    }

    fn keyboard_target(&self) -> KeyboardFocus {
        KeyboardFocus::X11(Box::new(self.clone()))
    }
}

#[cfg(test)]
mod tests;
