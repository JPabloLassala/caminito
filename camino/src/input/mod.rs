//! Input: the keyboard path, pointer focus, and interactive move and resize (15).

pub mod bindings;
pub mod devices;
mod focus;
pub mod frame;
mod grab;
pub(crate) mod keyboard;
pub(crate) mod repeat;
mod serials;

pub use focus::{KeyboardFocus, PointerFocus};
pub use grab::GrabKind;
pub(crate) use serials::InputSerials;

use camino_policy::{Action, ResizeEdges, WindowId};
use smithay::backend::input::{
    AbsolutePositionEvent, Axis, AxisSource, ButtonState, Device, Event, InputBackend, InputEvent,
    InputTime, KeyState, KeyboardKeyEvent, Keycode, PointerAxisEvent, PointerButtonEvent,
    PointerMotionEvent,
};
use smithay::desktop::{LayerSurface, WindowSurfaceType};
use smithay::input::keyboard::{FilterResult, Keysym, KeysymHandle, ModifiersState};
use smithay::input::pointer::{AxisFrame, ButtonEvent, MotionEvent, RelativeMotionEvent};
use smithay::reexports::wayland_server::Resource;
use smithay::utils::{Logical, Point, Rectangle, SERIAL_COUNTER};
use smithay::wayland::keyboard_shortcuts_inhibit::KeyboardShortcutsInhibitorSeat;
use smithay::wayland::pointer_constraints::{PointerConstraint, with_pointer_constraint};
use smithay::wayland::seat::WaylandFocus;
use smithay::xwayland::X11Surface;

use self::bindings::Binding;
use crate::commands::Command;
use crate::dbus::panel::PanelAction;
use crate::decoration::control::FrameControl;
use crate::output::output_geometry;
use crate::scene::{self, SceneLayer};
use crate::state::{Camino, SEAT, State};
use crate::window::takes_pointer_at;

// Linux input event codes.
pub(crate) const BTN_LEFT: u32 = 0x110;
pub(crate) const BTN_RIGHT: u32 = 0x111;
pub(crate) const BTN_MIDDLE: u32 = 0x112;

impl State {
    /// Handles one event from a backend's input devices, turning it into the calls below,
    /// which the headless fixture drives directly.
    pub fn process_input_event<I: InputBackend>(&mut self, event: InputEvent<I>) {
        match event {
            InputEvent::Keyboard { event } => {
                self.on_key(event.key_code(), event.state(), event.time());
            }
            InputEvent::PointerMotion { event } => {
                self.on_pointer_motion_relative(event.delta(), event.delta_unaccel(), event.time());
            }
            InputEvent::PointerMotionAbsolute { event } => {
                // Absolute devices (the winit window) span the first output.
                let Some(output) = self.camino.outputs.first() else {
                    return;
                };
                let geometry = output_geometry(output);
                let location = event.position_transformed(geometry.size) + geometry.loc.to_f64();
                self.on_pointer_motion(location, event.time());
            }
            InputEvent::PointerButton { event } => {
                self.on_pointer_button(event.button_code(), event.state(), event.time());
            }
            InputEvent::PointerAxis { event } => {
                let horizontal = self
                    .camino
                    .input_devices
                    .scrolls_horizontally(&event.device().id());
                self.on_pointer_axis::<I>(&event, horizontal);
            }
            _ => {}
        }
    }

    /// A mouse moved the pointer by `delta`.
    pub fn on_pointer_motion_relative(
        &mut self,
        delta: Point<f64, Logical>,
        delta_unaccel: Point<f64, Logical>,
        time: InputTime,
    ) {
        let pointer = self.camino.seat.get_pointer().unwrap();
        let location = pointer.current_location();
        let constraint = self.active_constraint(location);
        if constraint != Some(Constraint::Locked) {
            let target = self.camino.clamp_to_outputs(location + delta);
            // A confined pointer doesn't leave its surface's region.
            let escapes = constraint == Some(Constraint::Confined)
                && self.pointer_constraint_at(target).is_none();
            if !escapes {
                self.on_pointer_motion(target, time);
            }
        }

        let under = self.camino.surface_under(pointer.current_location());
        let event = RelativeMotionEvent {
            delta,
            delta_unaccel,
            time,
        };
        pointer.relative_motion(self, under, &event);
        pointer.frame(self);
    }

    /// The active constraint on the surface with the pointer, if the pointer is at `location`
    /// inside its region.
    fn active_constraint(&self, location: Point<f64, Logical>) -> Option<Constraint> {
        self.pointer_constraint_at(location)
            .filter(|(_, active)| *active)
            .map(|(constraint, _)| constraint)
    }

    /// The constraint on the surface with pointer focus, and whether it's active, if
    /// `location` is on that surface and inside the constraint's region.
    fn pointer_constraint_at(&self, location: Point<f64, Logical>) -> Option<(Constraint, bool)> {
        let pointer = self.camino.seat.get_pointer().unwrap();
        let (focus, origin) = self.camino.pointer_focus.clone()?;
        let (under, _) = self.camino.surface_under(location)?;
        if under != focus {
            return None;
        }
        let surface = focus.wl_surface()?;
        let local = (location - origin).to_i32_floor();
        with_pointer_constraint(&surface, &pointer, |constraint| {
            let constraint = constraint?;
            if constraint
                .region()
                .is_some_and(|region| !region.contains(local))
            {
                return None;
            }
            let kind = match &*constraint {
                PointerConstraint::Locked(_) => Constraint::Locked,
                PointerConstraint::Confined(_) => Constraint::Confined,
            };
            Some((kind, constraint.is_active()))
        })
    }

    /// Activates the constraint on the surface with pointer focus, if the pointer is inside
    /// its region.
    pub fn activate_pointer_constraint(&mut self) {
        let pointer = self.camino.seat.get_pointer().unwrap();
        if self
            .pointer_constraint_at(pointer.current_location())
            .is_none()
        {
            return;
        }
        let Some(surface) = self
            .camino
            .pointer_focus
            .as_ref()
            .and_then(|(focus, _)| focus.wl_surface())
        else {
            return;
        };
        with_pointer_constraint(&surface, &pointer, |constraint| {
            if let Some(constraint) = constraint
                && !constraint.is_active()
            {
                constraint.activate();
            }
        });
    }

    /// Scrolling, sent to whatever has the pointer. Without `horizontal`, the horizontal
    /// axis is dropped, as the device's settings ask (53).
    fn on_pointer_axis<I: InputBackend>(&mut self, event: &I::PointerAxisEvent, horizontal: bool) {
        let mut frame = AxisFrame::new(event.time()).source(event.source());
        let mut sent = false;
        let axes: &[Axis] = if horizontal {
            &[Axis::Horizontal, Axis::Vertical]
        } else {
            &[Axis::Vertical]
        };
        for &axis in axes {
            // A wheel click is 15 units, as libinput and GTK have it.
            let v120 = event.amount_v120(axis);
            let amount = event
                .amount(axis)
                .unwrap_or_else(|| v120.unwrap_or(0.) * 15. / 120.);
            if amount != 0. {
                frame = frame
                    .relative_direction(axis, event.relative_direction(axis))
                    .value(axis, amount);
                if let Some(v120) = v120 {
                    frame = frame.v120(axis, v120 as i32);
                }
                sent = true;
            }
            if event.source() == AxisSource::Finger && event.amount(axis) == Some(0.) {
                frame = frame.stop(axis);
                sent = true;
            }
        }
        // All of it was horizontal scrolling the device's settings drop.
        if !sent && !horizontal {
            return;
        }
        let pointer = self.camino.seat.get_pointer().unwrap();
        pointer.axis(self, frame);
        pointer.frame(self);
    }

    /// The keyboard path: one key event through 15's five stages, in order, each deciding
    /// whether it goes further. Only the bindings, the accelerator grabs and the client
    /// stage do anything yet.
    pub fn on_key(&mut self, keycode: Keycode, key_state: KeyState, time: InputTime) {
        // 1 to 3 see the raw key, before xkbcommon.
        if !(lock_passes(keycode) && accessx_passes(keycode) && keyboard_monitor_passes(keycode)) {
            return;
        }

        // 4. VT switching, Camino's bindings, then the accelerator grabs (98), inside
        // Smithay's filter so they see xkb's keysyms; then 5. the keyboard sends whatever
        // the filter forwards to the focused client.
        let keyboard = self.camino.seat.get_keyboard().unwrap();
        let serial = SERIAL_COUNTER.next_serial();
        let pressed = key_state == KeyState::Pressed;
        // A held accelerator repeats until its key's release or another key's press (99).
        if pressed {
            self.camino.stop_accelerator_repeat();
        } else {
            self.camino.stop_accelerator_repeat_of(keycode);
        }
        let intercepted = keyboard.input(
            self,
            keycode,
            key_state,
            serial,
            time,
            |state, mods, keysym| match state.bindings_stage(keycode, pressed, mods, &keysym) {
                FilterResult::Forward => state.grabs_stage(keycode, pressed, mods, &keysym),
                intercepted => intercepted,
            },
        );
        match intercepted {
            Some(Some(Bound::Action(action))) => {
                let now = self.camino.now();
                self.camino.policy.action(SEAT, action, now);
                // Task 4 of ticket 108's plan replaces this: the switcher stays open until
                // the modifier is released. Until then it commits at once.
                if matches!(action, Action::Switch { .. }) {
                    self.camino.policy.switcher_commit(now);
                }
                self.camino.queue_redraw_all();
            }
            Some(Some(Bound::Panel(action))) => {
                // The time a client would have for the event.
                self.camino.ask_panel(action, time.millis());
            }
            Some(Some(Bound::Command(command))) => self.camino.run_command(command),
            Some(Some(Bound::Accelerator { owner, action })) => {
                let time = time.millis();
                self.camino.signal_accelerator(&owner, action, time, false);
                self.camino
                    .start_accelerator_repeat(keycode, &owner, action, time);
            }
            Some(None) => {}
            None => {
                let focus = keyboard.current_focus();
                let surface = focus.as_ref().and_then(|focus| focus.wl_surface());
                self.camino.input_serials.sent(serial, surface.as_deref());
            }
        }
    }

    /// Releases every key still down, as when the host takes the keyboard from a nested
    /// Camino: the releases will never come, and Alt would otherwise stay held.
    pub fn release_pressed_keys(&mut self) {
        let keyboard = self.camino.seat.get_keyboard().unwrap();
        let time = InputTime::from_micros(self.camino.now().as_micros() as u64);
        for keycode in keyboard.pressed_keys() {
            self.on_key(keycode, KeyState::Released, time);
        }
    }

    /// Stage 4: VT switching (55) and Camino's own bindings. A key whose press switched VTs
    /// or ran a binding has its release swallowed too, so the client never sees half a key.
    fn bindings_stage(
        &mut self,
        keycode: Keycode,
        pressed: bool,
        mods: &ModifiersState,
        keysym: &KeysymHandle<'_>,
    ) -> FilterResult<Option<Bound>> {
        if !pressed {
            return if self.camino.suppressed_keys.remove(&keycode) {
                FilterResult::Intercept(None)
            } else {
                FilterResult::Forward
            };
        }
        // The keymap gives Ctrl+Alt+Fn these keysyms. No client can inhibit them, so none
        // can keep the user from leaving the VT.
        if let Some(vt) = vt_switch(keysym.modified_sym()) {
            self.backend.change_vt(vt);
            self.camino.suppressed_keys.insert(keycode);
            return FilterResult::Intercept(None);
        }
        if self.shortcuts_inhibited() {
            return FilterResult::Forward;
        }
        // Bindings name keys as the first layout does, whichever layout is active.
        let binding = keysym
            .raw_latin_sym_or_raw_current_sym()
            .and_then(|sym| bindings::binding_for(mods, sym));
        match binding {
            Some(binding) => {
                self.camino.suppressed_keys.insert(keycode);
                let bound = match binding {
                    Binding::Policy(action) => Bound::Action(action),
                    Binding::Panel(action) => Bound::Panel(action),
                    Binding::Command(command) => Bound::Command(command),
                };
                FilterResult::Intercept(Some(bound))
            }
            None => FilterResult::Forward,
        }
    }

    /// The accelerator grabs (98), a stage of their own between Camino's bindings and the
    /// client: a press that a caller of `org.mate.Camino` grabbed is its grabber's, and
    /// reaches no client. The bindings stage swallows the release. An active shortcuts
    /// inhibitor passes the grabs to the client as it does Camino's bindings, as Mutter's
    /// does.
    fn grabs_stage(
        &mut self,
        keycode: Keycode,
        pressed: bool,
        mods: &ModifiersState,
        keysym: &KeysymHandle<'_>,
    ) -> FilterResult<Option<Bound>> {
        if !pressed || self.shortcuts_inhibited() {
            return FilterResult::Forward;
        }
        // Accelerators name keys as the first layout does, like the bindings.
        let grab = keysym
            .raw_latin_sym_or_raw_current_sym()
            .and_then(|sym| self.camino.accelerator_grabs.pressed(mods, sym));
        match grab {
            Some((action, owner)) => {
                let owner = owner.to_owned();
                self.camino.suppressed_keys.insert(keycode);
                FilterResult::Intercept(Some(Bound::Accelerator { owner, action }))
            }
            None => FilterResult::Forward,
        }
    }

    /// Whether the surface with the keyboard holds an active shortcuts inhibitor.
    fn shortcuts_inhibited(&self) -> bool {
        let seat = &self.camino.seat;
        seat.get_keyboard()
            .and_then(|keyboard| keyboard.current_focus())
            .and_then(|focus| focus.wl_surface().map(|surface| surface.into_owned()))
            .and_then(|surface| seat.keyboard_shortcuts_inhibitor_for_surface(&surface))
            .is_some_and(|inhibitor| inhibitor.is_active())
    }

    /// The pointer moved to `location`, in global logical coordinates. The surface under it
    /// gets pointer focus, unless a grab decides otherwise, and a frame under it lights up
    /// its button.
    pub fn on_pointer_motion(&mut self, location: Point<f64, Logical>, time: InputTime) {
        let pointer = self.camino.seat.get_pointer().unwrap();
        let target = self.camino.under(location);
        let under = target.as_ref().and_then(Under::surface);
        let hover = target.as_ref().and_then(Under::frame_control);
        self.camino.pointer_focus = under.clone();
        self.camino.frame_pointer.hover = hover;
        let event = MotionEvent {
            location,
            serial: SERIAL_COUNTER.next_serial(),
            time,
        };
        pointer.motion(self, under, &event);
        pointer.frame(self);
        self.activate_pointer_constraint();
        // The cursor moved.
        self.camino.queue_redraw_all();
    }

    /// A pointer button, as a Linux input event code. With Alt held, as Marco's
    /// `mouse-button-modifier` defaults to, the left button moves the window and the middle
    /// one resizes it. A press on a frame does what Marco's frames do. Otherwise a press on a
    /// window focuses and raises it through policy before the client sees the press. The
    /// client never sees an Alt press or a press on the frame.
    pub fn on_pointer_button(&mut self, button: u32, state: ButtonState, time: InputTime) {
        let pointer = self.camino.seat.get_pointer().unwrap();
        let serial = SERIAL_COUNTER.next_serial();
        let location = pointer.current_location();
        let under = (state == ButtonState::Pressed && !pointer.is_grabbed())
            .then(|| self.camino.under(location))
            .flatten();
        if let Some(Under::Layer(layer, ..)) = &under {
            self.camino.layer_pressed(layer);
            self.update_keyboard_focus();
        }
        // A press on an override-redirect window goes to it alone: it's never focused or
        // raised, and can't be dragged.
        if let Some(under) = under
            && let Some(id) = under.window()
        {
            self.camino.window_pressed();
            let alt = self
                .camino
                .seat
                .get_keyboard()
                .unwrap()
                .modifier_state()
                .alt;
            let drag = match button {
                BTN_LEFT if alt => Some(GrabKind::Move),
                BTN_MIDDLE if alt => self
                    .camino
                    .nearest_corner(id, location)
                    .map(GrabKind::Resize),
                _ => None,
            };
            match under {
                Under::Frame(id, control)
                    if drag.is_none() && control != FrameControl::ClientArea =>
                {
                    self.frame_press(id, control, button, location, serial, time);
                }
                _ => {
                    let now = self.camino.now();
                    self.camino.policy.click(SEAT, id, now);
                    self.update_keyboard_focus();
                }
            }
            if let Some(kind) = drag {
                self.begin_window_grab(id, kind, grab::start_data(button, location), serial);
            }
        }
        let event = ButtonEvent {
            serial,
            time,
            button,
            state,
        };
        let focus = pointer.current_focus();
        let surface = focus.as_ref().and_then(|focus| focus.wl_surface());
        self.camino.input_serials.sent(serial, surface.as_deref());
        pointer.button(self, &event);
        pointer.frame(self);
    }

    /// Gives the keyboard to the layer surface that takes it (63), or else to the window
    /// policy focuses, unless a grab holds it. A shaded window keeps the focus, but as in
    /// Marco it's its frame that has it, so its client gets no keys.
    pub fn update_keyboard_focus(&mut self) {
        let keyboard = self.camino.seat.get_keyboard().unwrap();
        // Xwayland destroys its grab when the X client lets go, and Smithay's grab would
        // only notice at the next key, which it would send to the menu.
        if let Some((_, serial)) = self
            .camino
            .xwayland_keyboard_grab
            .take_if(|(grab, _)| !grab.is_alive())
            && keyboard.has_grab(serial)
        {
            keyboard.unset_grab(self);
        }
        if keyboard.is_grabbed() {
            return;
        }
        let layer = self.camino.layer_with_keyboard();
        let policy = &self.camino.policy;
        let focus = layer.map(KeyboardFocus::Layer).or_else(|| {
            policy
                .focus(SEAT)
                .and_then(|id| policy.window(id))
                .filter(|mapped| !mapped.is_shaded())
                .map(|mapped| mapped.keyboard_target())
        });
        if keyboard.current_focus() != focus {
            keyboard.set_focus(self, focus, SERIAL_COUNTER.next_serial());
        }
    }

    /// Brings pointer focus up to date when what's under the pointer changed without it
    /// moving: a window mapped, moved, shaded or went away beneath it, or a grab ended. A
    /// grab decides for itself.
    pub fn update_pointer_focus(&mut self) {
        let pointer = self.camino.seat.get_pointer().unwrap();
        if pointer.is_grabbed() {
            return;
        }
        let location = pointer.current_location();
        let current = pointer.current_focus();
        let target = self.camino.under(location);
        let under = target.as_ref().and_then(Under::surface);
        let hover = target.as_ref().and_then(Under::frame_control);
        let stale = current.as_ref() != under.as_ref().map(|(surface, _)| surface)
            || self.camino.pointer_focus != under
            || self.camino.frame_pointer.hover != hover;
        if stale {
            let time = InputTime::from_micros(self.camino.now().as_micros() as u64);
            self.on_pointer_motion(location, time);
        }
    }
}

/// What the pointer is on.
#[derive(Debug, Clone, PartialEq)]
pub enum Under {
    /// One of a managed window's surfaces, or an XWayland window itself, and its origin in
    /// global logical coordinates.
    Surface(WindowId, PointerFocus, Point<f64, Logical>),
    /// A managed window's frame.
    Frame(WindowId, FrameControl),
    /// An override-redirect window, and its origin, which policy doesn't know.
    OverrideRedirect(Box<X11Surface>, Point<f64, Logical>),
    /// One of a layer surface's surfaces, and its origin.
    Layer(LayerSurface, PointerFocus, Point<f64, Logical>),
}

impl Under {
    /// The managed window it's in.
    pub fn window(&self) -> Option<WindowId> {
        match self {
            Under::Surface(id, ..) | Under::Frame(id, _) => Some(*id),
            Under::OverrideRedirect(..) | Under::Layer(..) => None,
        }
    }

    /// The pointer focus and its origin.
    pub fn surface(&self) -> Option<(PointerFocus, Point<f64, Logical>)> {
        match self {
            Under::Surface(_, focus, origin) | Under::Layer(_, focus, origin) => {
                Some((focus.clone(), *origin))
            }
            Under::OverrideRedirect(window, origin) => {
                Some((PointerFocus::X11(window.clone()), *origin))
            }
            Under::Frame(..) => None,
        }
    }

    /// The window and the frame control, as the frame pointer takes them.
    pub fn frame_control(&self) -> Option<(WindowId, FrameControl)> {
        match self {
            Under::Frame(id, control) => Some((*id, *control)),
            Under::Surface(..) | Under::OverrideRedirect(..) | Under::Layer(..) => None,
        }
    }
}

/// A pointer constraint's kind.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum Constraint {
    Locked,
    Confined,
}

/// What a key press that reaches no client does instead.
enum Bound {
    /// One of Camino's bindings: something policy does.
    Action(Action),
    /// One of Camino's bindings: something it asks the panel for (95).
    Panel(PanelAction),
    /// One of Camino's bindings: a command to run (104).
    Command(Command),
    /// A grabbed accelerator (98): its owner's unique name on the bus, and the action id
    /// to signal it.
    Accelerator { owner: String, action: u32 },
}

/// Stage 1, the lock (M7): while locked, only VT and layout switching, AccessX and the
/// keyboard monitor pass. There's no lock yet.
fn lock_passes(_keycode: Keycode) -> bool {
    true
}

/// The VT an `XF86Switch_VT_n` keysym switches to.
fn vt_switch(keysym: Keysym) -> Option<i32> {
    let first = Keysym::XF86_Switch_VT_1.raw();
    (first..=Keysym::XF86_Switch_VT_12.raw())
        .contains(&keysym.raw())
        .then(|| (keysym.raw() - first + 1) as i32)
}

/// Stage 2, AccessX (M7): sticky, slow and bounce keys. Nothing is filtered yet.
fn accessx_passes(_keycode: Keycode) -> bool {
    true
}

/// Stage 3, the keyboard monitor's grabs and watches (M7). There's no monitor yet.
fn keyboard_monitor_passes(_keycode: Keycode) -> bool {
    true
}

impl Camino {
    /// `location` moved onto the nearest output, so the pointer never leaves them.
    fn clamp_to_outputs(&self, location: Point<f64, Logical>) -> Point<f64, Logical> {
        let clamp = |geometry: Rectangle<i32, Logical>| {
            let geometry = geometry.to_f64();
            Point::from((
                location
                    .x
                    .clamp(geometry.loc.x, geometry.loc.x + geometry.size.w - 1.),
                location
                    .y
                    .clamp(geometry.loc.y, geometry.loc.y + geometry.size.h - 1.),
            ))
        };
        self.outputs
            .iter()
            .map(|output| clamp(output_geometry(output)))
            .min_by(|a, b| {
                let distance = |p: &Point<f64, Logical>| (p.x - location.x).hypot(p.y - location.y);
                distance(a).total_cmp(&distance(b))
            })
            .unwrap_or(location)
    }

    /// The edges of the window's corner nearest `location`, which Alt+middle-drag resizes.
    fn nearest_corner(&self, id: WindowId, location: Point<f64, Logical>) -> Option<ResizeEdges> {
        let geometry = self.policy.geometry(id)?.to_f64();
        let center = geometry.loc + geometry.size.downscale(2.).to_point();
        let left = location.x < center.x;
        let top = location.y < center.y;
        Some(ResizeEdges {
            top,
            bottom: !top,
            left,
            right: !left,
        })
    }

    /// What's at `location` in the topmost window or layer surface that takes input there,
    /// by the scene's order.
    pub fn under(&self, location: Point<f64, Logical>) -> Option<Under> {
        // M7: nothing but the lock takes the pointer while locked (12).
        scene::LAYERS.into_iter().find_map(|layer| match layer {
            SceneLayer::Cursor | SceneLayer::DragIcon | SceneLayer::Lock => None,
            SceneLayer::Overlay
            | SceneLayer::Top
            | SceneLayer::LayerPopups
            | SceneLayer::Bottom
            | SceneLayer::Background => self.layer_surface_under(layer, location),
            SceneLayer::OverrideRedirect => self.override_redirect_under(location),
            SceneLayer::FullscreenWindows | SceneLayer::Windows => {
                self.managed_under(layer, location)
            }
        })
    }

    /// What's at `location` in the topmost layer surface of one of the scene's layers that
    /// hold them that takes input there: one of its surfaces or its popups', of those the
    /// layer has, inside its input region.
    fn layer_surface_under(
        &self,
        layer: SceneLayer,
        location: Point<f64, Logical>,
    ) -> Option<Under> {
        let part = scene::layer_surface_part(layer);
        self.outputs
            .iter()
            .filter(|output| output_geometry(output).to_f64().contains(location))
            .find_map(|output| {
                scene::layer_surfaces_in(self, output, layer)
                    .into_iter()
                    .find_map(|(surface, geometry)| {
                        let origin = geometry.loc.to_f64();
                        let (under, loc) = surface.surface_under(location - origin, part)?;
                        let focus = PointerFocus::Surface(under);
                        Some(Under::Layer(surface, focus, loc.to_f64() + origin))
                    })
            })
    }

    /// The topmost override-redirect window that takes input at `location`.
    fn override_redirect_under(&self, location: Point<f64, Logical>) -> Option<Under> {
        // X11Surface::surface_under passes over override-redirect windows during an XDND
        // drag, since the X client's drag icon is one.
        self.override_redirect_windows().find_map(|window| {
            let origin = scene::override_redirect_bbox(window).loc;
            let (_, origin) = window.surface_under(location, origin, WindowSurfaceType::ALL)?;
            Some(Under::OverrideRedirect(
                Box::new(window.clone()),
                origin.to_f64(),
            ))
        })
    }

    /// What's at `location` in the topmost managed window of `layer`, of the two that hold
    /// windows, that takes input there: one of its surfaces, which are above its frame, or
    /// its frame. A shaded window's surfaces have left the scene.
    fn managed_under(&self, layer: SceneLayer, location: Point<f64, Logical>) -> Option<Under> {
        let policy = &self.policy;
        policy
            .stacking(policy.active_workspace())
            .iter()
            .rev()
            .filter(|id| policy.is_showing(**id) && scene::window_is_in(self, **id, layer))
            .find_map(|&id| {
                let mapped = policy.window(id)?;
                let window = mapped.window();
                let geometry = policy.geometry(id)?;
                let origin = (geometry.loc - window.geometry().loc).to_f64();
                let surface = (!mapped.is_shaded())
                    .then(|| window.surface_under(location - origin, WindowSurfaceType::ALL))
                    .flatten()
                    .filter(|(_, loc)| {
                        window.x11_surface().is_none_or(|x11| {
                            takes_pointer_at(x11, location - origin - loc.to_f64())
                        })
                    });
                if let Some((surface, loc)) = surface {
                    let focus = match window.x11_surface() {
                        Some(x11) => PointerFocus::X11(Box::new(x11.clone())),
                        None => PointerFocus::Surface(surface),
                    };
                    return Some(Under::Surface(id, focus, loc.to_f64() + origin));
                }
                let control = mapped
                    .frame()
                    .control_at(location - geometry.loc.to_f64())?;
                Some(Under::Frame(id, control))
            })
    }

    /// The pointer focus under `location` and its origin.
    pub fn surface_under(
        &self,
        location: Point<f64, Logical>,
    ) -> Option<(PointerFocus, Point<f64, Logical>)> {
        self.under(location)?.surface()
    }

    /// The window under `location`.
    pub fn window_under(&self, location: Point<f64, Logical>) -> Option<WindowId> {
        self.under(location)?.window()
    }
}
