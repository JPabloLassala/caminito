//! Input: the keyboard path, pointer focus, and interactive move and resize (15).

pub(crate) mod accessx;
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
use grab::PlacementKey;
pub(crate) use serials::InputSerials;

use caminito_policy::{Action, Crossing, PointerTarget, ResizeEdges, WindowId};
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
use smithay::wayland::compositor::get_role;
use smithay::wayland::keyboard_shortcuts_inhibit::KeyboardShortcutsInhibitorSeat;
use smithay::wayland::pointer_constraints::{PointerConstraint, with_pointer_constraint};
use smithay::wayland::seat::WaylandFocus;
use smithay::wayland::shell::wlr_layer::Layer;
use smithay::wayland::shell::xdg::XDG_POPUP_ROLE;
use smithay::xwayland::X11Surface;

use self::bindings::Binding;
use crate::commands::Command;
use crate::dbus::a11y;
use crate::dbus::panel::PanelAction;
use crate::decoration::control::FrameControl;
use crate::output::output_geometry;
use crate::scene::{self, SceneLayer};
use crate::state::{Caminito, SEAT, State};
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
                let Some(output) = self.caminito.outputs.first() else {
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
                    .caminito
                    .input_devices
                    .scrolls_horizontally(&event.device().id());
                self.on_pointer_axis::<I>(&event, horizontal);
            }
            InputEvent::DeviceAdded { .. }
            | InputEvent::DeviceRemoved { .. }
            | InputEvent::SwitchToggle { .. }
            | InputEvent::Special(_) => {}
            // Gestures, touch and tablets, which Caminito passes on to no client yet, are
            // activity all the same (128).
            _ => self.idle_activity(),
        }
    }

    /// A mouse moved the pointer by `delta`.
    pub fn on_pointer_motion_relative(
        &mut self,
        delta: Point<f64, Logical>,
        delta_unaccel: Point<f64, Logical>,
        time: InputTime,
    ) {
        // A locked pointer still moves the mouse (128).
        self.idle_activity();
        let pointer = self.caminito.seat.get_pointer().unwrap();
        let location = pointer.current_location();
        let constraint = self.active_constraint(location);
        if constraint != Some(Constraint::Locked) {
            let target = self.caminito.clamp_to_outputs(location + delta);
            // A confined pointer doesn't leave its surface's region.
            let escapes = constraint == Some(Constraint::Confined)
                && self.pointer_constraint_at(target).is_none();
            if !escapes {
                self.pointer_motion(target, time, Crossing::Motion);
            }
        }

        let under = self.caminito.surface_under(pointer.current_location());
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
        let pointer = self.caminito.seat.get_pointer().unwrap();
        let (focus, origin) = self.caminito.pointer_focus.clone()?;
        let (under, _) = self.caminito.surface_under(location)?;
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
        let pointer = self.caminito.seat.get_pointer().unwrap();
        if self
            .pointer_constraint_at(pointer.current_location())
            .is_none()
        {
            return;
        }
        let Some(surface) = self
            .caminito
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
        self.idle_activity();
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
        let pointer = self.caminito.seat.get_pointer().unwrap();
        pointer.axis(self, frame);
        pointer.frame(self);
    }

    /// The keyboard path: one key event through 15's five stages, in order, each deciding
    /// whether it goes further.
    pub fn on_key(&mut self, keycode: Keycode, key_state: KeyState, time: InputTime) {
        // 0. Any key is activity, which ends a blank (128); the key goes on all the same.
        self.idle_activity();
        // 1. The lock (12). 15 has it as a gate before xkbcommon, but it drops no key,
        // since the locker is typed to: while the session is locked it's stage 4 that
        // runs nothing but VT switching, and the keyboard itself that switches layouts, as
        // xkbcommon does for the keymap's `grp` options.
        // 2. AccessX, while locked too (13): it sends on the keys it lets through, when it
        // does.
        self.accessx_stage(keycode, key_state, time);
    }

    /// The keyboard path past AccessX, for a key event its filter let through or made.
    fn key_past_accessx(&mut self, keycode: Keycode, key_state: KeyState, time: InputTime) {
        // No grab has the keys instead of the lock, whenever AccessX sends them on.
        self.release_keyboard_grab_if_locked();
        // 3. The Keyboard monitor (135), while locked too (13). It sees the key before
        // xkbcommon does, so a key it takes changes no state, CapsLock's among them.
        if !self.keyboard_monitor_stage(keycode, key_state, time) {
            return;
        }

        // 4. VT switching, Caminito's bindings, then the accelerator grabs (98), inside
        // Smithay's filter so they see xkb's keysyms; then 5. the keyboard sends whatever
        // the filter forwards to the focused client.
        let keyboard = self.caminito.seat.get_keyboard().unwrap();
        let serial = SERIAL_COUNTER.next_serial();
        let pressed = key_state == KeyState::Pressed;
        // A held accelerator repeats until its key's release or another key's press (99).
        if pressed {
            self.caminito.stop_accelerator_repeat();
        } else {
            self.caminito.stop_accelerator_repeat_of(keycode);
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
                let now = self.caminito.now();
                self.caminito.policy.action(SEAT, action, now);
                self.caminito.queue_redraw_all();
            }
            Some(Some(Bound::Switch { action, key })) => {
                let mods = keyboard.modifier_state();
                self.switch_action(action, key, &mods, time);
            }
            Some(Some(Bound::Panel(action))) => {
                // The time a client would have for the event.
                self.caminito.ask_panel(action, time.millis());
            }
            Some(Some(Bound::Command(command))) => self.caminito.run_command(command),
            Some(Some(Bound::Placement(key))) => self.placement_key(key, time),
            Some(Some(Bound::Accelerator { owner, action })) => {
                let time = time.millis();
                self.caminito
                    .signal_accelerator(&owner, action, time, false);
                self.caminito
                    .start_accelerator_repeat(keycode, &owner, action, time);
            }
            Some(None) => {}
            None => {
                let focus = keyboard.current_focus();
                let surface = focus.as_ref().and_then(|focus| focus.wl_surface());
                self.caminito.input_serials.sent(serial, surface.as_deref());
            }
        }
    }

    /// Releases every key still down, as when the host takes the keyboard from a nested
    /// Caminito: the releases will never come, and Alt would otherwise stay held. The keys
    /// AccessX holds down or back are among them, so its filter forgets them.
    pub fn release_pressed_keys(&mut self) {
        self.caminito.accessx.clear();
        self.caminito.keyboard_monitor.clear();
        let keyboard = self.caminito.seat.get_keyboard().unwrap();
        let time = InputTime::from_micros(self.caminito.now().as_micros() as u64);
        for keycode in keyboard.pressed_keys() {
            self.key_past_accessx(keycode, KeyState::Released, time);
        }
    }

    /// Stage 3, the Keyboard monitor's watches and grabs (135): its callers are told of the
    /// key, and it says whether the key goes further.
    fn keyboard_monitor_stage(
        &mut self,
        keycode: Keycode,
        key_state: KeyState,
        time: InputTime,
    ) -> bool {
        let keyboard = self.caminito.seat.get_keyboard().unwrap();
        let mods = keyboard.modifier_state().serialized;
        let (keysym, unichar) = keyboard.with_xkb_state(self, |context| {
            let xkb = context.xkb().lock().unwrap();
            // Safety: the state is only read, and no reference to it outlives the lock.
            let state = unsafe { xkb.state() };
            (state.key_get_one_sym(keycode), state.key_get_utf32(keycode))
        });
        let key = a11y::Key {
            keycode,
            pressed: key_state == KeyState::Pressed,
            time: time.millis(),
            state: mods.depressed | mods.latched | mods.locked,
            keysym: keysym.raw(),
            unichar,
        };
        let repeat_delay = u32::try_from(self.caminito.settings.keyboard.repeat_delay).unwrap_or(0);
        let verdict = self.caminito.keyboard_monitor.key(&key, repeat_delay);
        self.caminito.signal_key_event(&key, verdict.told);
        verdict.passes
    }

    /// Stage 4: VT switching (55) and Caminito's own bindings. A key whose press switched VTs
    /// or ran a binding has its release swallowed too, so the client never sees half a key.
    fn bindings_stage(
        &mut self,
        keycode: Keycode,
        pressed: bool,
        mods: &ModifiersState,
        keysym: &KeysymHandle<'_>,
    ) -> FilterResult<Option<Bound>> {
        if !pressed {
            // The release that lets go of the Window switcher's modifier commits it (108),
            // and goes on to the client, which saw the press.
            self.caminito.switcher_key_released(mods);
            return if self.caminito.suppressed_keys.remove(&keycode) {
                FilterResult::Intercept(None)
            } else {
                FilterResult::Forward
            };
        }
        // The keymap gives Ctrl+Alt+Fn these keysyms. No client can inhibit them, so none
        // can keep the user from leaving the VT.
        if let Some(vt) = vt_switch(keysym.modified_sym()) {
            self.backend.change_vt(vt);
            self.caminito.suppressed_keys.insert(keycode);
            return FilterResult::Intercept(None);
        }
        // Nothing else of Caminito's runs while the session is locked (12).
        if self.caminito.lock.is_locked() {
            return FilterResult::Forward;
        }
        // A manual placement has the keyboard while it lasts (154), as Marco's keyboard move
        // does, and nothing can open over it.
        if self.caminito.policy.placing(SEAT).is_some()
            && let Some(key) = PlacementKey::of(mods, keysym)
        {
            self.caminito.suppressed_keys.insert(keycode);
            return FilterResult::Intercept(Some(Bound::Placement(key)));
        }
        // The open Window switcher has the keyboard (108), before an inhibitor has its say:
        // it never opens under one.
        if let Some(result) = self.caminito.switcher_key_pressed(keycode, mods, keysym) {
            return result;
        }
        if self.shortcuts_inhibited() {
            return FilterResult::Forward;
        }
        // Bindings name keys as the accelerator grabs do (148).
        let keysyms = press_keysyms(keysym);
        let key = keysyms[0];
        let binding = self.caminito.settings.bindings.binding_for(mods, &keysyms);
        match binding {
            Some(binding) => {
                self.caminito.suppressed_keys.insert(keycode);
                let bound = match binding {
                    Binding::Policy(action @ Action::Switch { .. }) => {
                        Bound::Switch { action, key }
                    }
                    Binding::Policy(action) => Bound::Action(action),
                    Binding::Panel(action) => Bound::Panel(action),
                    Binding::Command(command) => Bound::Command(command),
                };
                FilterResult::Intercept(Some(bound))
            }
            None => FilterResult::Forward,
        }
    }

    /// The accelerator grabs (98), a stage of their own between Caminito's bindings and the
    /// client: a press that a caller of `org.mate.Caminito` grabbed is its grabber's, and
    /// reaches no client. The bindings stage swallows the release. An active shortcuts
    /// inhibitor passes the grabs to the client as it does Caminito's bindings, as Mutter's
    /// does. While the session is locked none is activated, msd's media keys among them, as
    /// mate-screensaver's keyboard grab keeps msd's from firing on X11 (12).
    fn grabs_stage(
        &mut self,
        keycode: Keycode,
        pressed: bool,
        mods: &ModifiersState,
        keysym: &KeysymHandle<'_>,
    ) -> FilterResult<Option<Bound>> {
        if !pressed || self.caminito.lock.is_locked() || self.shortcuts_inhibited() {
            return FilterResult::Forward;
        }
        // Accelerators name keys as the first layout does, like the bindings, or by the
        // keysym the press types, as Keyboard Shortcuts writes `<Shift>exclam` (100).
        let grabs = &self.caminito.accelerator_grabs;
        let grab = grabs.pressed(mods, &press_keysyms(keysym));
        match grab {
            Some((action, owner)) => {
                let owner = owner.to_owned();
                self.caminito.suppressed_keys.insert(keycode);
                FilterResult::Intercept(Some(Bound::Accelerator { owner, action }))
            }
            None => FilterResult::Forward,
        }
    }

    /// Whether the surface with the keyboard holds an active shortcuts inhibitor.
    fn shortcuts_inhibited(&self) -> bool {
        let seat = &self.caminito.seat;
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
        self.idle_activity();
        self.pointer_motion(location, time, Crossing::Motion);
    }

    /// The pointer is at `location`: what [`State::on_pointer_motion`] does but count as
    /// activity, for the refresh pass to give the focus to what's now under a pointer that
    /// didn't move. A window that maps there doesn't end a blank (128). Policy hears what
    /// the pointer is over, for the pointer focus modes (153), unless a grab has the pointer.
    fn pointer_motion(&mut self, location: Point<f64, Logical>, time: InputTime, why: Crossing) {
        let pointer = self.caminito.seat.get_pointer().unwrap();
        let target = self.caminito.under(location);
        let under = target.as_ref().and_then(Under::surface);
        let hover = target.as_ref().and_then(Under::frame_control);
        self.caminito.pointer_focus = under.clone();
        self.caminito.frame_pointer.hover = hover;
        if !pointer.is_grabbed() {
            let crossed = self.caminito.pointer_target(location, target.as_ref());
            let caminito = &mut self.caminito;
            let focus = caminito.policy.focus(SEAT);
            let now = caminito.now();
            caminito.policy.pointer_entered(SEAT, crossed, why, now);
            if caminito.policy.focus(SEAT) != focus {
                self.update_keyboard_focus();
            }
        }
        let event = MotionEvent {
            location,
            serial: SERIAL_COUNTER.next_serial(),
            time,
        };
        pointer.motion(self, under, &event);
        pointer.frame(self);
        self.activate_pointer_constraint();
        // The cursor moved.
        self.caminito.queue_redraw_all();
    }

    /// A pointer button, as a Linux input event code. With Alt held, as Marco's
    /// `mouse-button-modifier` defaults to, the left button moves the window and the middle
    /// one resizes it. A press on a frame does what Marco's frames do. Otherwise a press on a
    /// window focuses and raises it through policy before the client sees the press. The
    /// client never sees an Alt press or a press on the frame.
    pub fn on_pointer_button(&mut self, button: u32, state: ButtonState, time: InputTime) {
        self.idle_activity();
        let pointer = self.caminito.seat.get_pointer().unwrap();
        let serial = SERIAL_COUNTER.next_serial();
        let location = pointer.current_location();
        if self.switcher_button(button, state, location) {
            return;
        }
        let under = (state == ButtonState::Pressed && !pointer.is_grabbed())
            .then(|| self.caminito.under(location))
            .flatten();
        if let Some(Under::Layer(layer, ..)) = &under {
            self.caminito.layer_pressed(layer);
            self.update_keyboard_focus();
        }
        // A press on an override-redirect window goes to it alone: it's never focused or
        // raised, and can't be dragged.
        if let Some(under) = under
            && let Some(id) = under.window()
        {
            self.caminito.window_pressed();
            let alt = self
                .caminito
                .seat
                .get_keyboard()
                .unwrap()
                .modifier_state()
                .alt;
            let drag = match button {
                BTN_LEFT if alt => Some(GrabKind::Move),
                BTN_MIDDLE if alt => self
                    .caminito
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
                    let now = self.caminito.now();
                    self.caminito.policy.click(SEAT, id, now);
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
        self.caminito.input_serials.sent(serial, surface.as_deref());
        pointer.button(self, &event);
        pointer.frame(self);
    }

    /// Gives the keyboard to the layer surface that takes it (63), or else to the window
    /// policy focuses, unless a grab holds it. A shaded window keeps the focus, but as in
    /// Marco it's its frame that has it, so its client gets no keys. While the session is
    /// locked the keyboard is the locker's lock surface's, or no client's (12), and policy's
    /// focus is kept for the unlock.
    pub fn update_keyboard_focus(&mut self) {
        let keyboard = self.caminito.seat.get_keyboard().unwrap();
        // Xwayland destroys its grab when the X client lets go, and Smithay's grab would
        // only notice at the next key, which it would send to the menu.
        if let Some((_, serial)) = self
            .caminito
            .xwayland_keyboard_grab
            .take_if(|(grab, _)| !grab.is_alive())
            && keyboard.has_grab(serial)
        {
            keyboard.unset_grab(self);
        }
        if self.caminito.lock.is_locked() {
            self.release_keyboard_grab_if_locked();
            let focus = self.caminito.lock_surface_with_keyboard();
            let focus = focus.map(KeyboardFocus::Surface);
            if keyboard.current_focus() != focus {
                keyboard.set_focus(self, focus, SERIAL_COUNTER.next_serial());
            }
            return;
        }
        if keyboard.is_grabbed() {
            return;
        }
        let layer = self.caminito.layer_with_keyboard();
        let policy = &self.caminito.policy;
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

    /// Ends a keyboard grab that was taken under the lock. Every grab is let go as the lock
    /// comes on, and the popup and Xwayland grabs are refused under it, but an input
    /// method's is Smithay's to grant, with no say of Caminito's.
    fn release_keyboard_grab_if_locked(&mut self) {
        let keyboard = self.caminito.seat.get_keyboard().unwrap();
        if self.caminito.lock.is_locked() && keyboard.is_grabbed() {
            keyboard.unset_grab(self);
        }
    }

    /// Brings pointer focus up to date when what's under the pointer changed without it
    /// moving: a window mapped, moved, shaded or went away beneath it, or a grab ended. A
    /// grab decides for itself.
    pub fn update_pointer_focus(&mut self) {
        let pointer = self.caminito.seat.get_pointer().unwrap();
        if pointer.is_grabbed() {
            return;
        }
        let location = pointer.current_location();
        let current = pointer.current_focus();
        let target = self.caminito.under(location);
        let under = target.as_ref().and_then(Under::surface);
        let hover = target.as_ref().and_then(Under::frame_control);
        let stale = current.as_ref() != under.as_ref().map(|(surface, _)| surface)
            || self.caminito.pointer_focus != under
            || self.caminito.frame_pointer.hover != hover;
        if stale {
            let time = InputTime::from_micros(self.caminito.now().as_micros() as u64);
            self.pointer_motion(location, time, Crossing::Scene);
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
    /// One of a lock surface's surfaces, and its origin (12).
    Lock(PointerFocus, Point<f64, Logical>),
}

impl Under {
    /// The managed window it's in.
    pub fn window(&self) -> Option<WindowId> {
        match self {
            Under::Surface(id, ..) | Under::Frame(id, _) => Some(*id),
            Under::OverrideRedirect(..) | Under::Layer(..) | Under::Lock(..) => None,
        }
    }

    /// The pointer focus and its origin.
    pub fn surface(&self) -> Option<(PointerFocus, Point<f64, Logical>)> {
        match self {
            Under::Surface(_, focus, origin)
            | Under::Layer(_, focus, origin)
            | Under::Lock(focus, origin) => Some((focus.clone(), *origin)),
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
            Under::Surface(..)
            | Under::OverrideRedirect(..)
            | Under::Layer(..)
            | Under::Lock(..) => None,
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
    /// One of Caminito's bindings: something policy does.
    Action(Action),
    /// One of Caminito's bindings: a `switch-*` action (108), with the key that fired it.
    Switch { action: Action, key: Keysym },
    /// One of Caminito's bindings: something it asks the panel for (95).
    Panel(PanelAction),
    /// One of Caminito's bindings: a command to run (104).
    Command(Command),
    /// A grabbed accelerator (98): its owner's unique name on the bus, and the action id
    /// to signal it.
    Accelerator { owner: String, action: u32 },
    /// A key for the manual placement in progress (154).
    Placement(PlacementKey),
}

/// The VT an `XF86Switch_VT_n` keysym switches to.
/// The keysyms that name a pressed key: the first layout's on its first level, whichever
/// layout is active, and the one the press types, as Keyboard Shortcuts writes
/// `<Shift>exclam` (100). A key with nothing on its first level is named by what it types
/// alone.
fn press_keysyms(keysym: &KeysymHandle<'_>) -> [Keysym; 2] {
    let typed = keysym.modified_sym();
    let first_level = keysym.raw_latin_sym_or_raw_current_sym();
    [first_level.unwrap_or(typed), typed]
}

fn vt_switch(keysym: Keysym) -> Option<i32> {
    let first = Keysym::XF86_Switch_VT_1.raw();
    (first..=Keysym::XF86_Switch_VT_12.raw())
        .contains(&keysym.raw())
        .then(|| (keysym.raw() - first + 1) as i32)
}

impl Caminito {
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
    /// by the scene's order. While the session is locked it's the lock alone (12).
    pub fn under(&self, location: Point<f64, Logical>) -> Option<Under> {
        if self.lock.is_locked() {
            return self.lock_under(location);
        }
        // The Window switcher's popup covers what's under it, for no client (108).
        if self.switcher_popup_at(location) {
            return None;
        }
        scene::LAYERS.into_iter().find_map(|layer| match layer {
            SceneLayer::Blank
            | SceneLayer::Cursor
            | SceneLayer::DragIcon
            | SceneLayer::Lock
            | SceneLayer::BellFlash
            | SceneLayer::Switcher => None,
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
            if !takes_pointer_at(window, location - origin.to_f64()) {
                return None;
            }
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

    /// What policy is told the pointer is over at `location`, where `under` is (153). A
    /// Layer surface below the windows, such as caja's desktop, is the desktop, and its
    /// popups aren't, as Marco ignores an override-redirect menu's crossings.
    fn pointer_target(
        &self,
        location: Point<f64, Logical>,
        under: Option<&Under>,
    ) -> PointerTarget {
        if self.lock.is_locked() || self.switcher_popup_at(location) {
            return PointerTarget::Other;
        }
        match under {
            None => PointerTarget::Desktop,
            Some(Under::Surface(id, ..) | Under::Frame(id, _)) => PointerTarget::Window(*id),
            Some(Under::Layer(layer, focus, _))
                if matches!(layer.layer(), Layer::Bottom | Layer::Background)
                    && focus
                        .wl_surface()
                        .is_none_or(|surface| get_role(&surface) != Some(XDG_POPUP_ROLE)) =>
            {
                PointerTarget::Desktop
            }
            Some(Under::Layer(..) | Under::OverrideRedirect(..) | Under::Lock(..)) => {
                PointerTarget::Other
            }
        }
    }
}
