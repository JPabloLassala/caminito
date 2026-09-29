//! `impl XwmHandler for State`, and xwayland-shell, which pairs Xwayland's surfaces with its
//! windows (11). The handlers only translate X's events into `window::lifecycle`'s and
//! policy's: a map request creates a pending window, which maps into policy on its first
//! commit with content; an unmap withdraws it; its requests go through policy.
//! Override-redirect windows skip all that: they're shown as they are, in X's stacking
//! order, in the scene's override-redirect layer (36).

use camino_policy::{ResizeEdges, WindowId};
use smithay::backend::renderer::utils::with_renderer_surface_state;
use smithay::desktop::Window;
use smithay::input::Seat;
use smithay::reexports::wayland_server::Resource;
use smithay::reexports::wayland_server::protocol::wl_surface::WlSurface;
use smithay::utils::{Logical, Rectangle, SERIAL_COUNTER};
use smithay::wayland::xwayland_keyboard_grab::{XWaylandKeyboardGrab, XWaylandKeyboardGrabHandler};
use smithay::wayland::xwayland_shell::{XWaylandShellHandler, XWaylandShellState};
use smithay::xwayland::xwm::{Reorder, ResizeEdge, X11Window, XwmId};
use smithay::xwayland::{X11Surface, X11Wm, XwmHandler};

use super::stack::RootStack;
use crate::input::{GrabKind, KeyboardFocus};
use crate::state::{Camino, State};
use crate::window::{
    StateRequest, WindowKey, configure_request_geometry, request_demands_attention, request_modal,
};

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

    fn new_window(&mut self, _xwm: XwmId, window: X11Surface) {
        self.camino
            .with_root_stack(|stack| stack.created(window.window_id()));
    }

    fn new_override_redirect_window(&mut self, _xwm: XwmId, window: X11Surface) {
        self.camino
            .with_root_stack(|stack| stack.created(window.window_id()));
    }

    /// Grants the map at once. The window is pending until Xwayland pairs it with a surface
    /// and that commits, with the states its initial `_NET_WM_STATE` asked for.
    fn map_window_request(&mut self, _xwm: XwmId, window: X11Surface) {
        let camino = &mut self.camino;
        if let Some(frame) = window.mapped_window_id() {
            camino.with_root_stack(|stack| stack.framed(window.window_id(), frame));
        }
        let key = key(&window);
        if !camino.is_pending(&key) && camino.mapped_for_key(&key).is_none() {
            camino.window_created(Window::new_x11_window(window.clone()));
            let asked = [
                (window.is_fullscreen(), StateRequest::Fullscreen(None)),
                (window.is_maximized(), StateRequest::Maximized(true)),
                (window.is_above(), StateRequest::Above(true)),
                (window.is_below(), StateRequest::Below(true)),
                (window.is_sticky(), StateRequest::Sticky(true)),
            ];
            for (_, request) in asked.into_iter().filter(|(asked, _)| *asked) {
                camino.window_state_request(&key, request);
            }
        }
        if let Err(err) = window.set_mapped(true) {
            tracing::debug!("can't map an X window: {err}");
        }
    }

    /// Shows it at once. It draws once Xwayland pairs it with a surface that commits.
    fn mapped_override_redirect_window(&mut self, _xwm: XwmId, window: X11Surface) {
        self.camino.override_redirect_mapped(window);
    }

    /// A managed window is withdrawn, and Smithay puts it back on the root; an
    /// override-redirect one leaves the scene.
    fn unmapped_window(&mut self, _xwm: XwmId, window: X11Surface) {
        let camino = &mut self.camino;
        camino.with_root_stack(|stack| stack.unframed(window.window_id()));
        camino.override_redirect_unmapped(&window);
        camino.window_withdrawn(&key(&window));
    }

    fn destroyed_window(&mut self, _xwm: XwmId, window: X11Surface) {
        let camino = &mut self.camino;
        camino.with_root_stack(|stack| stack.destroyed(window.window_id()));
        camino.override_redirect_unmapped(&window);
        camino.window_destroyed(&key(&window));
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
            let extents = camino.policy.frame_extents(id);
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

    /// X moved or restacked a root child: a managed window's frame, which Camino put there,
    /// or an override-redirect window, which put itself there.
    fn configure_notify(
        &mut self,
        _xwm: XwmId,
        window: X11Surface,
        _geometry: Rectangle<i32, Logical>,
        above: Option<X11Window>,
    ) {
        let camino = &mut self.camino;
        camino.with_root_stack(|stack| stack.restacked(window.window_id(), above));
        if camino.is_override_redirect_shown(&window) {
            camino.queue_redraw_all();
        }
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
        self.camino
            .window_state_request(&key(&window), StateRequest::Above(true));
    }

    fn unabove_request(&mut self, _xwm: XwmId, window: X11Surface) {
        self.camino
            .window_state_request(&key(&window), StateRequest::Above(false));
    }

    fn below_request(&mut self, _xwm: XwmId, window: X11Surface) {
        self.camino
            .window_state_request(&key(&window), StateRequest::Below(true));
    }

    fn unbelow_request(&mut self, _xwm: XwmId, window: X11Surface) {
        self.camino
            .window_state_request(&key(&window), StateRequest::Below(false));
    }

    fn stick_request(&mut self, _xwm: XwmId, window: X11Surface) {
        self.camino
            .window_state_request(&key(&window), StateRequest::Sticky(true));
    }

    fn unstick_request(&mut self, _xwm: XwmId, window: X11Surface) {
        self.camino
            .window_state_request(&key(&window), StateRequest::Sticky(false));
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
        request_modal(window, modal);
        if let Some(id) = self.camino.mapped_for_key(&key(window)) {
            self.camino.window_changed(id);
        }
    }

    /// Sets `_NET_WM_STATE_DEMANDS_ATTENTION`, which makes the window urgent.
    fn set_demands_attention(&mut self, window: &X11Surface, demands_attention: bool) {
        request_demands_attention(window, demands_attention);
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
        } else if self.camino.override_redirect_for_surface(surface).is_some() {
            self.camino.queue_redraw_all();
        }
    }
}

impl Camino {
    /// Tells X's stacking order of the root's children what changed, once Xwayland runs.
    fn with_root_stack(&mut self, change: impl FnOnce(&mut RootStack)) {
        if let Some(xwayland) = self.xwayland.as_mut() {
            change(&mut xwayland.root_stack);
        }
    }

    /// Puts an override-redirect window in the scene, once.
    fn override_redirect_mapped(&mut self, window: X11Surface) {
        let Some(xwayland) = self.xwayland.as_mut() else {
            return;
        };
        let id = window.window_id();
        if !xwayland
            .override_redirect
            .iter()
            .any(|w| w.window_id() == id)
        {
            xwayland.override_redirect.push(window);
        }
        self.queue_redraw_all();
    }

    fn is_override_redirect_shown(&self, window: &X11Surface) -> bool {
        self.override_redirect_windows()
            .any(|shown| shown.window_id() == window.window_id())
    }

    /// Takes an override-redirect window out of the scene, if it was in it.
    fn override_redirect_unmapped(&mut self, window: &X11Surface) {
        let Some(xwayland) = self.xwayland.as_mut() else {
            return;
        };
        let before = xwayland.override_redirect.len();
        xwayland
            .override_redirect
            .retain(|shown| shown.window_id() != window.window_id());
        if xwayland.override_redirect.len() != before {
            self.queue_redraw_all();
        }
    }
}

/// Xwayland asks for the keyboard when an X client grabs it for an override-redirect window,
/// as a menu does. The window gets it as a surface, so X's input focus stays with the
/// toplevel the menu belongs to.
impl XWaylandKeyboardGrabHandler for State {
    /// Only Xwayland sees the global, and it asks only for override-redirect windows. It
    /// asks as the menu maps, before Camino has heard of the map from X, so the surface
    /// isn't checked against the windows Camino shows.
    fn keyboard_focus_for_xsurface(&self, surface: &WlSurface) -> Option<KeyboardFocus> {
        Some(KeyboardFocus::Surface(surface.clone()))
    }

    /// The window has the keyboard at once, rather than from the next key as Smithay's grab
    /// would have it.
    fn grab(&mut self, surface: WlSurface, seat: Seat<Self>, grab: XWaylandKeyboardGrab<Self>) {
        // M7: refused while locked, and released when the lock comes on (12).
        let keyboard = seat.get_keyboard().unwrap();
        let serial = SERIAL_COUNTER.next_serial();
        keyboard.set_focus(self, Some(KeyboardFocus::Surface(surface)), serial);
        self.camino.xwayland_keyboard_grab = Some((grab.grab().clone(), serial));
        keyboard.set_grab(self, grab, serial);
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
