//! `impl XwmHandler for State`, and xwayland-shell, which pairs Xwayland's surfaces with its
//! windows (11). The handlers only translate X's events into `window::lifecycle`'s and
//! policy's: a map request creates a pending window, which maps into policy on its first
//! commit with content; an unmap withdraws it; its requests go through policy.
//! Override-redirect windows skip all that: they're shown as they are, in X's stacking
//! order, in the scene's override-redirect layer (36).

use std::os::fd::OwnedFd;

use caminito_policy::{ResizeEdges, WindowId};
use smithay::backend::renderer::utils::with_renderer_surface_state;
use smithay::desktop::Window;
use smithay::input::Seat;
use smithay::reexports::wayland_server::Resource;
use smithay::reexports::wayland_server::protocol::wl_surface::WlSurface;
use smithay::reexports::x11rb::protocol::xproto::Gravity;
use smithay::utils::{Logical, Rectangle, SERIAL_COUNTER};
use smithay::wayland::seat::WaylandFocus;
use smithay::wayland::selection::SelectionTarget;
use smithay::wayland::xwayland_keyboard_grab::{XWaylandKeyboardGrab, XWaylandKeyboardGrabHandler};
use smithay::wayland::xwayland_shell::{XWaylandShellHandler, XWaylandShellState};
use smithay::xwayland::xwm::{Reorder, ResizeEdge, WmWindowProperty, X11Window, XwmId};
use smithay::xwayland::{X11Surface, X11Wm, XwmHandler};

use super::stack::RootStack;
use crate::input::{GrabKind, KeyboardFocus};
use crate::state::{Caminito, State};
use crate::window::{
    STICKY, StateRequest, WindowKey, configure_request_geometry, request_demands_attention,
    request_modal, with_x_facts,
};

/// The key `window::lifecycle` knows an X window by.
fn key(window: &X11Surface) -> WindowKey {
    WindowKey::X11(window.window_id())
}

impl XwmHandler for State {
    fn xwm_state(&mut self, _xwm: XwmId) -> &mut X11Wm {
        self.caminito
            .xwm()
            .expect("X11Wm only calls back while Caminito holds it")
    }

    /// Xwayland has gone (42).
    fn disconnected(&mut self, xwm: XwmId) {
        self.xwayland_disconnected(xwm);
    }

    fn new_window(&mut self, _xwm: XwmId, window: X11Surface) {
        self.caminito
            .with_root_stack(|stack| stack.created(window.window_id()));
    }

    fn new_override_redirect_window(&mut self, _xwm: XwmId, window: X11Surface) {
        self.caminito
            .with_root_stack(|stack| stack.created(window.window_id()));
    }

    /// Grants the map at once. The window is pending until Xwayland pairs it with a surface
    /// and that commits, with the states its initial `_NET_WM_STATE` asked for.
    fn map_window_request(&mut self, _xwm: XwmId, window: X11Surface) {
        let caminito = &mut self.caminito;
        if let Some(frame) = window.mapped_window_id() {
            caminito.with_root_stack(|stack| stack.framed(window.window_id(), frame));
        }
        let key = key(&window);
        if !caminito.is_pending(&key) && caminito.mapped_for_key(&key).is_none() {
            caminito.read_x_facts(&window);
            caminito.window_created(Window::new_x11_window(window.clone()));
            let sticky = with_x_facts(&window, |facts| facts.desktop) == Some(STICKY);
            let asked = [
                (window.is_fullscreen(), StateRequest::Fullscreen(None)),
                (window.is_maximized(), StateRequest::Maximized(true)),
                (window.is_above(), StateRequest::Above(true)),
                (window.is_below(), StateRequest::Below(true)),
                (window.is_sticky() || sticky, StateRequest::Sticky(true)),
            ];
            for (_, request) in asked.into_iter().filter(|(asked, _)| *asked) {
                caminito.window_state_request(&key, request);
            }
        }
        if let Err(err) = window.set_mapped(true) {
            tracing::debug!("can't map an X window: {err}");
        }
    }

    /// Shows it at once. It draws once Xwayland pairs it with a surface that commits.
    fn mapped_override_redirect_window(&mut self, _xwm: XwmId, window: X11Surface) {
        self.caminito.override_redirect_mapped(window);
    }

    /// A managed window is withdrawn, and Smithay puts it back on the root; an
    /// override-redirect one leaves the scene.
    fn unmapped_window(&mut self, _xwm: XwmId, window: X11Surface) {
        let caminito = &mut self.caminito;
        caminito.with_root_stack(|stack| stack.unframed(window.window_id()));
        caminito.override_redirect_unmapped(&window);
        caminito.window_withdrawn(&key(&window));
    }

    fn destroyed_window(&mut self, _xwm: XwmId, window: X11Surface) {
        let caminito = &mut self.caminito;
        caminito.with_root_stack(|stack| stack.destroyed(window.window_id()));
        caminito.override_redirect_unmapped(&window);
        caminito.window_destroyed(&key(&window));
        caminito
            .app_menu_registrations
            .window_destroyed(window.window_id());
    }

    /// Policy decides for a managed window, whose stacking is Caminito's alone. Any other is
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
        let caminito = &mut self.caminito;
        if caminito.x_configure_request(&window, None, (x, y), (w, h)) {
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

    /// X moved or restacked a root child: a managed window's frame, which Caminito put there,
    /// or an override-redirect window, which put itself there.
    fn configure_notify(
        &mut self,
        _xwm: XwmId,
        window: X11Surface,
        _geometry: Rectangle<i32, Logical>,
        above: Option<X11Window>,
    ) {
        let caminito = &mut self.caminito;
        caminito.with_root_stack(|stack| stack.restacked(window.window_id(), above));
        if caminito.is_override_redirect_shown(&window) {
            caminito.queue_redraw_all();
        }
    }

    /// A window's title, hints, type or transient parent may have changed, or a property
    /// Caminito reads itself (39). Others Smithay doesn't parse, such as those Caminito and
    /// Smithay write, change nothing Caminito knows.
    fn property_notify(&mut self, _xwm: XwmId, window: X11Surface, property: WmWindowProperty) {
        let caminito = &mut self.caminito;
        if let WmWindowProperty::Other(atom) = property
            && !caminito.x_property_changed(&window, atom)
        {
            return;
        }
        if let Some(id) = caminito.mapped_for_key(&key(&window)) {
            caminito.window_changed(id);
        }
    }

    fn maximize_request(&mut self, _xwm: XwmId, window: X11Surface) {
        self.caminito
            .window_state_request(&key(&window), StateRequest::Maximized(true));
    }

    fn unmaximize_request(&mut self, _xwm: XwmId, window: X11Surface) {
        self.caminito
            .window_state_request(&key(&window), StateRequest::Maximized(false));
    }

    fn fullscreen_request(&mut self, _xwm: XwmId, window: X11Surface) {
        self.caminito
            .window_state_request(&key(&window), StateRequest::Fullscreen(None));
    }

    fn unfullscreen_request(&mut self, _xwm: XwmId, window: X11Surface) {
        self.caminito
            .window_state_request(&key(&window), StateRequest::Unfullscreen);
    }

    fn minimize_request(&mut self, _xwm: XwmId, window: X11Surface) {
        self.caminito
            .window_state_request(&key(&window), StateRequest::Minimized);
    }

    fn modal_request(&mut self, _xwm: XwmId, window: X11Surface) {
        self.set_modal(&window, true);
    }

    fn unmodal_request(&mut self, _xwm: XwmId, window: X11Surface) {
        self.set_modal(&window, false);
    }

    fn above_request(&mut self, _xwm: XwmId, window: X11Surface) {
        self.caminito
            .window_state_request(&key(&window), StateRequest::Above(true));
    }

    fn unabove_request(&mut self, _xwm: XwmId, window: X11Surface) {
        self.caminito
            .window_state_request(&key(&window), StateRequest::Above(false));
    }

    fn below_request(&mut self, _xwm: XwmId, window: X11Surface) {
        self.caminito
            .window_state_request(&key(&window), StateRequest::Below(true));
    }

    fn unbelow_request(&mut self, _xwm: XwmId, window: X11Surface) {
        self.caminito
            .window_state_request(&key(&window), StateRequest::Below(false));
    }

    fn stick_request(&mut self, _xwm: XwmId, window: X11Surface) {
        self.caminito
            .window_state_request(&key(&window), StateRequest::Sticky(true));
    }

    fn unstick_request(&mut self, _xwm: XwmId, window: X11Surface) {
        self.caminito
            .window_state_request(&key(&window), StateRequest::Sticky(false));
    }

    fn shade_request(&mut self, _xwm: XwmId, window: X11Surface) {
        self.policy_request(&window, |caminito, id| caminito.policy.set_shaded(id, true));
    }

    fn unshade_request(&mut self, _xwm: XwmId, window: X11Surface) {
        self.policy_request(&window, |caminito, id| {
            caminito.policy.set_shaded(id, false)
        });
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

    /// `_NET_ACTIVE_WINDOW`: a launcher's utility window, which isn't focused as it maps,
    /// gets the keyboard this way. X's timestamp isn't on Caminito's clock, so the request
    /// counts as made now.
    fn active_window_request(
        &mut self,
        _xwm: XwmId,
        window: X11Surface,
        _timestamp: u32,
        _currently_active_window: Option<X11Surface>,
    ) {
        self.caminito
            .window_state_request(&key(&window), StateRequest::Active);
    }

    fn allow_selection_access(&mut self, _xwm: XwmId, _selection: SelectionTarget) -> bool {
        self.allow_x_selection_access()
    }

    fn send_selection(
        &mut self,
        _xwm: XwmId,
        selection: SelectionTarget,
        mime_type: String,
        fd: OwnedFd,
    ) {
        self.send_wayland_selection_to_x(selection, mime_type, fd);
    }

    fn new_selection(&mut self, _xwm: XwmId, selection: SelectionTarget, mime_types: Vec<String>) {
        self.x_selection_set(selection, mime_types);
    }

    fn cleared_selection(&mut self, _xwm: XwmId, selection: SelectionTarget) {
        self.x_selection_cleared(selection);
    }
}

impl State {
    /// Passes a request on to policy if the window is managed.
    fn policy_request(
        &mut self,
        window: &X11Surface,
        request: impl FnOnce(&mut Caminito, WindowId),
    ) {
        if let Some(id) = self.caminito.mapped_for_key(&key(window)) {
            request(&mut self.caminito, id);
            self.caminito.queue_redraw_all();
        }
    }

    /// Sets `_NET_WM_STATE_MODAL`, which makes a dialog a modal one.
    fn set_modal(&mut self, window: &X11Surface, modal: bool) {
        request_modal(window, modal);
        if let Some(id) = self.caminito.mapped_for_key(&key(window)) {
            self.caminito.window_changed(id);
        }
    }

    /// Sets `_NET_WM_STATE_DEMANDS_ATTENTION`, which makes the window urgent.
    fn set_demands_attention(&mut self, window: &X11Surface, demands_attention: bool) {
        request_demands_attention(window, demands_attention);
    }

    /// Starts a move or resize a managed window asked for, if a button still holds the
    /// pointer on one of Xwayland's surfaces, as the press the client answered did.
    fn x11_interactive_request(&mut self, window: &X11Surface, kind: GrabKind) {
        let Some(id) = self.caminito.mapped_for_key(&key(window)) else {
            return;
        };
        let pointer = self.caminito.seat.get_pointer().unwrap();
        let Some(start_data) = pointer.grab_start_data() else {
            return;
        };
        let on_xwayland = start_data.focus.as_ref().is_some_and(|(focus, _)| {
            window
                .wl_surface()
                .zip(focus.wl_surface())
                .is_some_and(|(surface, focus)| focus.id().same_client_as(&surface.id()))
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
        if let Some(key) = self.caminito.window_key_for_surface(surface) {
            self.caminito.window_committed(&key);
        } else if self
            .caminito
            .override_redirect_for_surface(surface)
            .is_some()
        {
            self.caminito.queue_redraw_all();
        }
    }
}

impl Caminito {
    /// A managed window asks to move or resize, by `ConfigureRequest` or
    /// `_NET_MOVERESIZE_WINDOW`, with `gravity` in place of its own if it names one: policy
    /// decides. Returns false if the window isn't managed.
    pub(crate) fn x_configure_request(
        &mut self,
        window: &X11Surface,
        gravity: Option<Gravity>,
        loc: (Option<i32>, Option<i32>),
        size: (Option<u32>, Option<u32>),
    ) -> bool {
        let Some(id) = self.mapped_for_key(&key(window)) else {
            return false;
        };
        let extents = self.policy.frame_extents(id);
        let requested = configure_request_geometry(window, gravity, loc, size, extents);
        self.policy
            .request_geometry(id, requested.loc, requested.size);
        self.queue_redraw_all();
        true
    }

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
    /// asks as the menu maps, before Caminito has heard of the map from X, so the surface
    /// isn't checked against the windows Caminito shows. While the session is locked the
    /// grab is refused: a keyboard grab never beats the lock (11, Smithay #1714).
    fn keyboard_focus_for_xsurface(&self, surface: &WlSurface) -> Option<KeyboardFocus> {
        (!self.caminito.lock.is_locked()).then(|| KeyboardFocus::Surface(surface.clone()))
    }

    /// The window has the keyboard at once, rather than from the next key as Smithay's grab
    /// would have it.
    fn grab(&mut self, surface: WlSurface, seat: Seat<Self>, grab: XWaylandKeyboardGrab<Self>) {
        let keyboard = seat.get_keyboard().unwrap();
        let serial = SERIAL_COUNTER.next_serial();
        keyboard.set_focus(self, Some(KeyboardFocus::Surface(surface)), serial);
        self.caminito.xwayland_keyboard_grab = Some((grab.grab().clone(), serial));
        keyboard.set_grab(self, grab, serial);
    }
}

impl XWaylandShellHandler for State {
    fn xwayland_shell_state(&mut self) -> &mut XWaylandShellState {
        &mut self.caminito.xwayland_shell_state
    }

    /// The surface may already have its content, if it committed before Xwayland paired
    /// it with its window.
    fn surface_associated(&mut self, _xwm: XwmId, surface: WlSurface, _window: X11Surface) {
        self.xwayland_commit(&surface);
    }
}
