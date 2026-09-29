//! The X11 adapter: what `Mapped` reads from an XWayland window through Smithay's `X11Wm`,
//! and asks of it (11). Its facts are Marco's reading of the ICCCM and EWMH properties.

use std::sync::Mutex;

use camino_policy::{SizeHints, WindowStates, WindowType};
use smithay::reexports::x11rb::protocol::xproto::Gravity;
use smithay::utils::{FrameExtents, Logical, Point, Rectangle, Size};
use smithay::xwayland::X11Surface;
use smithay::xwayland::xwm::{WmInputModel, WmWindowType};

use super::{Adapter, PreMapConfigure, Skips, WindowKey};
use crate::decoration::IconSource;
use crate::input::KeyboardFocus;

/// Marco's type for an X window (`recalc_window_type`, window.c:6568-6607): its first
/// `_NET_WM_WINDOW_TYPE` Marco knows, or with none a dialog if it's transient for another
/// window and else a normal window. A dialog with `_NET_WM_STATE_MODAL` is a modal dialog.
///
/// Marco doesn't know the override-redirect types, so on a managed window they count as no
/// type. It would carry on to a later type in the list; Smithay only reads the first it
/// knows, so here they end the search.
fn marco_type(ewmh: Option<WmWindowType>, transient: bool, modal: bool) -> WindowType {
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
/// Marco's `adjust_for_gravity`, window.c:3279-3405). Camino's frames have no X border.
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
/// inside a frame of `extents`. What it leaves out stays as it is.
pub(crate) fn configure_request_geometry(
    surface: &X11Surface,
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
        let gravity = surface
            .size_hints()
            .and_then(|hints| hints.win_gravity)
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
    /// `_NET_WM_STATE_DEMANDS_ATTENTION`, as the client asked.
    demands_attention: Option<bool>,
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

/// Records the client's request to demand attention or not, as [`request_modal`] does.
pub(crate) fn request_demands_attention(surface: &X11Surface, demands_attention: bool) {
    unflushed(surface, |unflushed| {
        unflushed.demands_attention = Some(demands_attention)
    });
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

    /// Unless `_MOTIF_WM_HINTS` turns decorations off.
    fn wants_frame(&self) -> bool {
        !self.is_decorated()
    }

    /// `_NET_WM_ICON` comes with Camino's X connection (39).
    fn icon(&self) -> IconSource {
        IconSource::AppId(self.app_id())
    }

    /// `WM_TRANSIENT_FOR`.
    fn parent(&self) -> Option<WindowKey> {
        self.is_transient_for().map(WindowKey::X11)
    }

    /// `WM_HINTS`'s urgency, or `_NET_WM_STATE_DEMANDS_ATTENTION`.
    fn urgent(&self) -> bool {
        let demands_attention = unflushed(self, |unflushed| unflushed.demands_attention)
            .unwrap_or_else(|| self.demands_attention());
        self.hints().is_some_and(|hints| hints.urgent) || demands_attention
    }

    /// ICCCM's input model: the input hint or `WM_TAKE_FOCUS`.
    fn takes_input(&self) -> bool {
        self.input_model() != WmInputModel::None
    }

    /// `_NET_WM_STATE_SKIP_TASKBAR` and `_NET_WM_STATE_SKIP_PAGER`.
    fn skips(&self) -> Skips {
        Skips {
            taskbar: self.is_skip_taskbar(),
            pager: self.is_skip_pager(),
        }
    }

    fn set_geometry(&self, geometry: Rectangle<i32, Logical>, states: WindowStates) {
        unflushed(self, |unflushed| {
            unflushed.geometry = Some((geometry, states))
        });
    }

    fn flush(&self) {
        let Unflushed {
            geometry,
            modal,
            demands_attention,
        } = unflushed(self, std::mem::take);
        let mut set = Ok(());
        if let Some(modal) = modal {
            set = set.and(self.set_modal(modal));
        }
        if let Some(demands_attention) = demands_attention {
            set = set.and(self.set_demands_attention(demands_attention));
        }
        if let Some((geometry, states)) = geometry {
            set_net_wm_states(self, states.maximized, states.fullscreen);
            set = set.and(self.set_shaded(states.shaded));
            if let Err(err) = self.configure(x_rect(self, geometry)) {
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

    /// Sets the states it'll map with, and the size if Camino sets it, where it is now. The
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
