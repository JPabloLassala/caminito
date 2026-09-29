//! `impl XwmHandler for State`, and xwayland-shell, which pairs Xwayland's surfaces with its
//! windows (11). The handlers only translate X's events into `window::lifecycle`'s and
//! policy's: a map request creates a pending window, which maps into policy on its first
//! commit with content; an unmap withdraws it; its requests go through policy.
//! Override-redirect windows are 36's.

use camino_policy::{ResizeEdges, WindowId};
use smithay::backend::renderer::utils::with_renderer_surface_state;
use smithay::desktop::Window;
use smithay::reexports::wayland_server::Resource;
use smithay::reexports::wayland_server::protocol::wl_surface::WlSurface;
use smithay::utils::{Logical, Rectangle, SERIAL_COUNTER};
use smithay::wayland::xwayland_shell::{XWaylandShellHandler, XWaylandShellState};
use smithay::xwayland::xwm::{Reorder, ResizeEdge, X11Window, XwmId};
use smithay::xwayland::{X11Surface, X11Wm, XwmHandler};

use crate::input::GrabKind;
use crate::state::{Camino, State};
use crate::window::{StateRequest, WindowKey, configure_request_geometry};

/// The key `window::lifecycle` knows an X window by.
fn key(window: &X11Surface) -> WindowKey {
    WindowKey::X11(window.window_id())
}

impl XwmHandler for State {
    fn xwm_state(&mut self, _xwm: XwmId) -> &mut X11Wm {
        self.camino
            .xwm()
            .expect("X11Wm only calls back while Camino holds it")
    }

    fn new_window(&mut self, _xwm: XwmId, _window: X11Surface) {}

    fn new_override_redirect_window(&mut self, _xwm: XwmId, _window: X11Surface) {}

    /// Grants the map at once. The window is pending until Xwayland pairs it with a surface
    /// and that commits, with the states its initial `_NET_WM_STATE` asked for.
    fn map_window_request(&mut self, _xwm: XwmId, window: X11Surface) {
        let camino = &mut self.camino;
        let key = key(&window);
        if !camino.is_pending(&key) && camino.mapped_for_key(&key).is_none() {
            camino.window_created(Window::new_x11_window(window.clone()));
            if window.is_fullscreen() {
                camino.window_state_request(&key, StateRequest::Fullscreen(None));
            } else if window.is_maximized() {
                camino.window_state_request(&key, StateRequest::Maximized(true));
            }
        }
        if let Err(err) = window.set_mapped(true) {
            tracing::debug!("can't map an X window: {err}");
        }
    }

    fn mapped_override_redirect_window(&mut self, _xwm: XwmId, _window: X11Surface) {}

    fn unmapped_window(&mut self, _xwm: XwmId, window: X11Surface) {
        self.camino.window_withdrawn(&key(&window));
    }

    fn destroyed_window(&mut self, _xwm: XwmId, window: X11Surface) {
        self.camino.window_destroyed(&key(&window));
    }

    /// Policy decides for a managed window, whose stacking is Camino's alone. Any other is
    /// configured as it asks. Smithay then tells the client where it is, either way.
    fn configure_request(
        &mut self,
        _xwm: XwmId,
        window: X11Surface,
        x: Option<i32>,
        y: Option<i32>,
        w: Option<u32>,
        h: Option<u32>,
        _reorder: Option<Reorder>,
    ) {
        let camino = &mut self.camino;
        if let Some(id) = camino.mapped_for_key(&key(&window)) {
            let mapped = camino.policy.window(id).unwrap();
            let extents = mapped.visible_extents();
            let requested = configure_request_geometry(&window, (x, y), (w, h), extents);
            camino
                .policy
                .request_geometry(id, requested.loc, requested.size);
            camino.queue_redraw_all();
            return;
        }
        let last = window.last_configure();
        let geometry = Rectangle::new(
            (x.unwrap_or(last.loc.x), y.unwrap_or(last.loc.y)).into(),
            (
                w.map_or(last.size.w, |w| w as i32),
                h.map_or(last.size.h, |h| h as i32),
            )
                .into(),
        );
        if let Err(err) = window.configure(geometry) {
            tracing::debug!("can't configure an X window: {err}");
        }
    }

    fn configure_notify(
        &mut self,
        _xwm: XwmId,
        _window: X11Surface,
        _geometry: Rectangle<i32, Logical>,
        _above: Option<X11Window>,
    ) {
    }

    /// A managed window's title, hints, type or transient parent may have changed.
    fn property_notify(
        &mut self,
        _xwm: XwmId,
        window: X11Surface,
        _property: smithay::xwayland::xwm::WmWindowProperty,
    ) {
        if let Some(id) = self.camino.mapped_for_key(&key(&window)) {
            self.camino.window_changed(id);
        }
    }

    fn maximize_request(&mut self, _xwm: XwmId, window: X11Surface) {
        self.camino
            .window_state_request(&key(&window), StateRequest::Maximized(true));
    }

    fn unmaximize_request(&mut self, _xwm: XwmId, window: X11Surface) {
        self.camino
            .window_state_request(&key(&window), StateRequest::Maximized(false));
    }

    fn fullscreen_request(&mut self, _xwm: XwmId, window: X11Surface) {
        self.camino
            .window_state_request(&key(&window), StateRequest::Fullscreen(None));
    }

    fn unfullscreen_request(&mut self, _xwm: XwmId, window: X11Surface) {
        self.camino
            .window_state_request(&key(&window), StateRequest::Unfullscreen);
    }

    fn minimize_request(&mut self, _xwm: XwmId, window: X11Surface) {
        self.camino
            .window_state_request(&key(&window), StateRequest::Minimized);
    }

    fn modal_request(&mut self, _xwm: XwmId, window: X11Surface) {
        self.set_modal(&window, true);
    }

    fn unmodal_request(&mut self, _xwm: XwmId, window: X11Surface) {
        self.set_modal(&window, false);
    }

    fn above_request(&mut self, _xwm: XwmId, window: X11Surface) {
        self.policy_request(&window, |camino, id| camino.policy.set_above(id, true));
    }

    fn unabove_request(&mut self, _xwm: XwmId, window: X11Surface) {
        self.policy_request(&window, |camino, id| camino.policy.set_above(id, false));
    }

    fn below_request(&mut self, _xwm: XwmId, window: X11Surface) {
        self.policy_request(&window, |camino, id| camino.policy.set_below(id, true));
    }

    fn unbelow_request(&mut self, _xwm: XwmId, window: X11Surface) {
        self.policy_request(&window, |camino, id| camino.policy.set_below(id, false));
    }

    fn stick_request(&mut self, _xwm: XwmId, window: X11Surface) {
        self.policy_request(&window, |camino, id| camino.policy.set_sticky(id, true));
    }

    fn unstick_request(&mut self, _xwm: XwmId, window: X11Surface) {
        self.policy_request(&window, |camino, id| camino.policy.set_sticky(id, false));
    }

    fn shade_request(&mut self, _xwm: XwmId, window: X11Surface) {
        self.policy_request(&window, |camino, id| camino.policy.set_shaded(id, true));
    }

    fn unshade_request(&mut self, _xwm: XwmId, window: X11Surface) {
        self.policy_request(&window, |camino, id| camino.policy.set_shaded(id, false));
    }

    fn demands_attention_request(&mut self, _xwm: XwmId, window: X11Surface) {
        self.set_demands_attention(&window, true);
    }

    fn undemands_attention_request(&mut self, _xwm: XwmId, window: X11Surface) {
        self.set_demands_attention(&window, false);
    }

    /// `_NET_WM_MOVERESIZE`'s directions 0 to 7.
    fn resize_request(
        &mut self,
        _xwm: XwmId,
        window: X11Surface,
        _button: u32,
        resize_edge: ResizeEdge,
    ) {
        use ResizeEdge as E;
        let edges = ResizeEdges {
            top: matches!(resize_edge, E::Top | E::TopLeft | E::TopRight),
            bottom: matches!(resize_edge, E::Bottom | E::BottomLeft | E::BottomRight),
            left: matches!(resize_edge, E::Left | E::TopLeft | E::BottomLeft),
            right: matches!(resize_edge, E::Right | E::TopRight | E::BottomRight),
        };
        self.x11_interactive_request(&window, GrabKind::Resize(edges));
    }

    /// `_NET_WM_MOVERESIZE`'s direction 8.
    fn move_request(&mut self, _xwm: XwmId, window: X11Surface, _button: u32) {
        self.x11_interactive_request(&window, GrabKind::Move);
    }
}

impl State {
    /// Passes a request on to policy if the window is managed.
    fn policy_request(&mut self, window: &X11Surface, request: impl FnOnce(&mut Camino, WindowId)) {
        if let Some(id) = self.camino.mapped_for_key(&key(window)) {
            request(&mut self.camino, id);
            self.camino.queue_redraw_all();
        }
    }

    /// Sets `_NET_WM_STATE_MODAL`, which makes a dialog a modal one.
    fn set_modal(&mut self, window: &X11Surface, modal: bool) {
        if let Err(err) = window.set_modal(modal) {
            tracing::debug!("can't set an X window's state: {err}");
        }
        if let Some(id) = self.camino.mapped_for_key(&key(window)) {
            self.camino.window_changed(id);
        }
    }

    /// Sets `_NET_WM_STATE_DEMANDS_ATTENTION`, which makes the window urgent.
    fn set_demands_attention(&mut self, window: &X11Surface, demands_attention: bool) {
        if let Err(err) = window.set_demands_attention(demands_attention) {
            tracing::debug!("can't set an X window's state: {err}");
        }
    }

    /// Starts a move or resize a managed window asked for, if a button still holds the
    /// pointer on one of Xwayland's surfaces, as the press the client answered did.
    fn x11_interactive_request(&mut self, window: &X11Surface, kind: GrabKind) {
        let Some(id) = self.camino.mapped_for_key(&key(window)) else {
            return;
        };
        let pointer = self.camino.seat.get_pointer().unwrap();
        let Some(start_data) = pointer.grab_start_data() else {
            return;
        };
        let on_xwayland = start_data.focus.as_ref().is_some_and(|(focus, _)| {
            window
                .wl_surface()
                .is_some_and(|surface| focus.id().same_client_as(&surface.id()))
        });
        if !on_xwayland {
            return;
        }
        self.begin_window_grab(id, kind, start_data, SERIAL_COUNTER.next_serial());
    }

    /// Handles a commit to one of Xwayland's surfaces: content on a window's surface maps it
    /// if it's pending, or tells policy what changed.
    pub(crate) fn xwayland_commit(&mut self, surface: &WlSurface) {
        let has_buffer =
            with_renderer_surface_state(surface, |state| state.buffer().is_some()).unwrap_or(false);
        if !has_buffer {
            return;
        }
        if let Some(key) = self.camino.window_key_for_surface(surface) {
            self.camino.window_committed(&key);
        }
    }
}

impl XWaylandShellHandler for State {
    fn xwayland_shell_state(&mut self) -> &mut XWaylandShellState {
        &mut self.camino.xwayland_shell_state
    }

    /// The surface may already have its content, if it committed before Xwayland paired
    /// it with its window.
    fn surface_associated(&mut self, _xwm: XwmId, surface: WlSurface, _window: X11Surface) {
        self.xwayland_commit(&surface);
    }
}
