//! Input: the keyboard path, pointer focus, and interactive move and resize (15).

pub(crate) mod accessx;
pub mod bindings;
pub mod devices;
mod focus;
pub mod frame;
mod grab;
pub(crate) mod keyboard;
pub mod remaps;
pub(crate) mod repeat;
mod serials;

pub use focus::{KeyboardFocus, PointerFocus};
pub use grab::GrabKind;
pub(crate) use serials::InputSerials;

use caminito_policy::{
    Action, Crossing, GrabKey, Interaction, OutputId, OverviewScope, PointerTarget, PolicyWindow,
    ResizeEdges, WindowId,
};
use smithay::backend::input::{
    AbsolutePositionEvent, Axis, AxisRelativeDirection, AxisSource, ButtonState, Device, Event,
    InputBackend, InputEvent, InputTime, KeyState, KeyboardKeyEvent, Keycode, PointerAxisEvent,
    PointerButtonEvent, PointerMotionEvent,
};
use smithay::desktop::{LayerSurface, WindowSurfaceType};
use smithay::input::keyboard::{
    FilterResult, KeyboardTarget, Keysym, KeysymHandle, ModifiersState,
};
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
use self::bindings::accelerators::{Trigger, x_button};
use self::grab::FrameHold;
use self::remaps::{Combination, HeldBack};
use crate::dbus::a11y;
use crate::decoration::control::FrameControl;
use crate::output::{output_geometry, output_id};
use crate::scene::{self, SceneLayer};
use crate::settings::translate::ModifierClick;
use crate::state::{Caminito, SEAT, State};
use crate::window::takes_pointer_at;

// Linux input event codes.
pub(crate) const BTN_LEFT: u32 = 0x110;
pub(crate) const BTN_RIGHT: u32 = 0x111;
pub(crate) const BTN_MIDDLE: u32 = 0x112;
pub(crate) const BTN_SIDE: u32 = 0x113;

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
                let along = |axis| AxisScroll {
                    amount: event.amount(axis),
                    v120: event.amount_v120(axis),
                    relative_direction: event.relative_direction(axis),
                };
                let scroll = Scroll {
                    time: event.time(),
                    source: event.source(),
                    horizontal: horizontal.then(|| along(Axis::Horizontal)),
                    vertical: along(Axis::Vertical),
                };
                self.on_pointer_scroll(scroll);
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

    /// Whether a client has locked or confined the pointer where it is.
    pub(crate) fn pointer_is_constrained(&self) -> bool {
        let pointer = self.caminito.seat.get_pointer().unwrap();
        self.active_constraint(pointer.current_location()).is_some()
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

    /// Scrolling, sent to whatever has the pointer, after a titlebar under it has done what
    /// its `action-scroll-*-titlebar` keys ask (162).
    pub(crate) fn on_pointer_scroll(&mut self, scroll: Scroll) {
        self.idle_activity();
        // As a click does, so an Alt+scroll a client gets comes with its Alt (CMN-188).
        self.send_held_back(scroll.time);
        self.titlebar_scroll(&scroll);
        let mut frame = AxisFrame::new(scroll.time).source(scroll.source);
        let mut sent = false;
        let axes = [
            (Axis::Horizontal, scroll.horizontal),
            (Axis::Vertical, Some(scroll.vertical)),
        ];
        for (axis, along) in axes {
            let Some(along) = along else {
                continue;
            };
            let amount = along
                .amount
                .unwrap_or_else(|| along.v120.unwrap_or(0.) * WHEEL_CLICK / V120_CLICK);
            if amount != 0. {
                frame = frame
                    .relative_direction(axis, along.relative_direction)
                    .value(axis, amount);
                if let Some(v120) = along.v120 {
                    frame = frame.v120(axis, v120 as i32);
                }
                sent = true;
            }
            if scroll.source == AxisSource::Finger && along.amount == Some(0.) {
                frame = frame.stop(axis);
                sent = true;
            }
        }
        // All of it was horizontal scrolling the device's settings drop.
        if !sent && scroll.horizontal.is_none() {
            return;
        }
        let pointer = self.caminito.seat.get_pointer().unwrap();
        pointer.axis(self, frame);
        pointer.frame(self);
    }

    /// A vertical scroll with the pointer on an SSD titlebar runs `action-scroll-up-titlebar`
    /// or `action-scroll-down-titlebar` once for each click of the wheel, as Marco's frame
    /// does for each press of X's buttons 4 and 5, on the title alone and with no grab under
    /// way (`meta_frames_button_press_event`, frames.c:1768-1771 and 1937-1948). A wheel
    /// with finer steps, or a touchpad, acts once for each whole click its steps add up to,
    /// which is when the X server's driver presses the button.
    fn titlebar_scroll(&mut self, scroll: &Scroll) {
        let vertical = scroll.vertical;
        let v120 = vertical
            .v120
            .or(vertical
                .amount
                .map(|amount| amount * V120_CLICK / WHEEL_CLICK))
            .unwrap_or(0.);
        let mut scrolled = self.caminito.frame_pointer.scrolled + v120;
        // The fingers left the touchpad: what's left of a click doesn't count for the next.
        if scroll.source == AxisSource::Finger && vertical.amount == Some(0.) {
            scrolled = 0.;
        }
        // Each action may take the titlebar from under the pointer.
        loop {
            let Some(window) = self.titlebar_under_pointer() else {
                scrolled = 0.;
                break;
            };
            if scrolled.abs() < V120_CLICK {
                break;
            }
            let frames = &self.caminito.settings.frames;
            let action = if scrolled < 0. {
                frames.scroll_up
            } else {
                frames.scroll_down
            };
            scrolled -= V120_CLICK.copysign(scrolled);
            self.titlebar_action(window, action);
        }
        self.caminito.frame_pointer.scrolled = scrolled;
    }

    /// The window whose titlebar the pointer is on, clear of its buttons, unless a grab has
    /// the pointer.
    fn titlebar_under_pointer(&self) -> Option<WindowId> {
        let pointer = self.caminito.seat.get_pointer().unwrap();
        if pointer.is_grabbed() {
            return None;
        }
        match self.caminito.under(pointer.current_location())? {
            Under::Frame(window, FrameControl::Title) => Some(window),
            _ => None,
        }
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
        // A Key remap's last step held is let go at the next key event (CMN-187): the
        // trigger's release, or another key's press or release, so the client repeats the
        // step with its own modifiers alone and gets the next key with those held.
        self.release_held_step(time);
        // 3. The Keyboard monitor (135), while locked too (13). It sees the key before
        // xkbcommon does, so a key it takes changes no state, CapsLock's among them.
        if !self.keyboard_monitor_stage(keycode, key_state, time) {
            return;
        }

        // 4. VT switching, Caminito's bindings, the accelerator grabs (98), then the Key
        // remaps (CMN-186) and the modifiers they hold back (CMN-188), inside Smithay's
        // filter so they see xkb's keysyms; then 5. the keyboard sends whatever the filter
        // forwards to the focused client.
        let keyboard = self.caminito.seat.get_keyboard().unwrap();
        // The modifiers before this key's, which the client has.
        let before = keyboard.modifier_state();
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
            |state, mods, keysym| {
                let result = match state.bindings_stage(keycode, pressed, mods, &keysym) {
                    FilterResult::Forward => {
                        match state.grabs_stage(keycode, pressed, mods, &keysym) {
                            FilterResult::Forward => {
                                state.remaps_stage(keycode, pressed, mods, &keysym, &before)
                            }
                            intercepted => intercepted,
                        }
                    }
                    intercepted => intercepted,
                };
                state.held_back_stage(keycode, pressed, result, time)
            },
        );
        match intercepted {
            Some(Some(Bound::Binding { binding, key })) => {
                self.run_binding(binding, Trigger::Key(key), time);
            }
            Some(Some(Bound::Action(action))) => self.run_action(action, time),
            Some(Some(Bound::GrabKey(key))) => self.keyboard_grab_key(key, time),
            Some(Some(Bound::Remap(steps))) => self.send_remap(&steps, time),
            Some(Some(Bound::HeldBack)) => {}
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

    /// Runs one of Caminito's bindings, which the press of `trigger` at `time` fired: a key
    /// in the keyboard path, or a mouse button in the pointer's (184).
    fn run_binding(&mut self, binding: Binding, trigger: Trigger, time: InputTime) {
        match binding {
            Binding::Policy(action @ (Action::Switch { .. } | Action::Cycle { .. })) => {
                let mods = self.caminito.seat.get_keyboard().unwrap().modifier_state();
                self.switch_action(action, trigger, &mods, time);
            }
            Binding::Policy(action) if let Some(scope) = OverviewScope::of(action) => {
                self.toggle_overview(scope, time);
            }
            Binding::Policy(action) => self.run_action(action, time),
            // The time a client would have for the event.
            Binding::Panel(action) => self.caminito.ask_panel(action, time.millis()),
            Binding::Notifications(action) => self.caminito.ask_notifications(action),
            Binding::Command(command) => self.caminito.run_command(command),
            Binding::RenameWorkspace => self.caminito.rename_workspace(),
        }
    }

    /// Has policy do a binding's `action`, and shows the Workspace popup if it's one that
    /// does.
    pub(crate) fn run_action(&mut self, action: Action, time: InputTime) {
        // A client's open menu gives way to the window menu (161).
        if action == Action::ActivateWindowMenu {
            self.dismiss_menus(time);
        }
        // A switch is the pointer's output's, which it makes the Active output.
        if action.switches_workspace() {
            self.caminito.pointer_output_switches();
        }
        let now = self.caminito.now();
        // Before the action, which may leave no window with the focus a move needs.
        let shows_popup = self.caminito.workspace_popup_shows_for(action);
        self.caminito.policy.action(SEAT, action, now);
        // A switch or a move to the workspace beside shows the Workspace popup (152).
        if shows_popup {
            let mods = self.caminito.seat.get_keyboard().unwrap().modifier_state();
            self.caminito.show_workspace_popup(action, &mods);
        }
    }

    /// Releases every key still down, as when the host takes the keyboard from a nested
    /// Caminito: the releases will never come, and Alt would otherwise stay held. The keys
    /// AccessX holds down or back are among them, so its filter forgets them.
    pub fn release_pressed_keys(&mut self) {
        self.caminito.accessx.clear();
        self.caminito.keyboard_monitor.clear();
        // The client is told of no key held back, then or as it's released.
        if let Some(held_back) = &mut self.caminito.held_back {
            held_back.take();
        }
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
            // And the one that lets go of the Workspace popup's closes it (152).
            self.caminito.workspace_popup_key_released(mods);
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
        // The seat's Interaction has the keyboard while it lasts (187), before an inhibitor
        // has its say: none opens under one, and an inhibitor doesn't take the keys from one
        // under way. Each says which keys are its own.
        let caminito = &mut self.caminito;
        let theirs = match caminito.policy.interaction(SEAT) {
            // The keyboard's move or resize, and not the pointer's (156).
            Some(Interaction::MoveOrResize) => {
                caminito.keyboard_grab_key_pressed(keycode, mods, keysym)
            }
            Some(Interaction::WindowSwitcher) => {
                caminito.switcher_key_pressed(keycode, mods, keysym)
            }
            // As a GTK menu's grab has it (161).
            Some(Interaction::WindowMenu) => caminito.window_menu_key_pressed(keycode, keysym),
            Some(Interaction::WorkspacePopup) => {
                caminito.workspace_popup_key_pressed(keycode, mods, keysym)
            }
            Some(Interaction::Overview) => caminito.overview_key_pressed(keycode, mods, keysym),
            None => None,
        };
        if let Some(result) = theirs {
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
                FilterResult::Intercept(Some(Bound::Binding { binding, key }))
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

    /// The Key remaps (CMN-186, ADR 0038), after the bindings and the grabs, so a System
    /// shortcut is never remapped: a press a rule takes is sent as the rule's target, and the
    /// bindings stage swallows its release. None apply while the session is locked or during
    /// an Interaction. A shortcuts inhibitor doesn't stop them, as the spec names no such
    /// exception: a remote desktop's or a VM's Remap group with no remaps does.
    ///
    /// The press of a modifier that starts a rule for the focus, with Alt, Ctrl or Super
    /// among those held, is held back from the client (CMN-188): `before` are the modifiers
    /// the client has, which it's told again after a remap.
    fn remaps_stage(
        &mut self,
        keycode: Keycode,
        pressed: bool,
        mods: &ModifiersState,
        keysym: &KeysymHandle<'_>,
        before: &ModifiersState,
    ) -> FilterResult<Option<Bound>> {
        let caminito = &self.caminito;
        if !pressed
            || caminito.settings.key_remaps.is_empty()
            || caminito.lock.is_locked()
            || caminito.policy.interaction(SEAT).is_some()
        {
            return FilterResult::Forward;
        }
        let Some(combination) = Combination::pressed(mods, keysym) else {
            return FilterResult::Forward;
        };
        let app_id = self.keyboard_app_id();
        let remaps = &self.caminito.settings.key_remaps;
        if remaps::is_modifier(keysym.modified_sym()) {
            let held = combination.held;
            if !(held.alt || held.ctrl || held.logo)
                || !remaps.starts_a_rule(app_id.as_deref(), held)
            {
                return FilterResult::Forward;
            }
            let held_back = self
                .caminito
                .held_back
                .get_or_insert_with(|| HeldBack::new(before));
            held_back.hold(keycode, *mods);
            return FilterResult::Intercept(Some(Bound::HeldBack));
        }
        let Some(steps) = remaps.steps_for(app_id.as_deref(), &combination) else {
            return FilterResult::Forward;
        };
        let steps = steps.to_vec();
        self.caminito.suppressed_keys.insert(keycode);
        FilterResult::Intercept(Some(Bound::Remap(steps)))
    }

    /// The modifiers held back from the client (CMN-188), past every other stage with this
    /// key's `result`. A press a rule takes keeps them held back, and their releases are
    /// swallowed. Any other press, one that runs a binding among them, has them sent first,
    /// as is the release of one no rule has taken since, so the client sees a lone Alt as a
    /// tap. Another key's release is sent with the modifiers the client has, which those
    /// held back aren't among.
    fn held_back_stage(
        &mut self,
        keycode: Keycode,
        pressed: bool,
        result: FilterResult<Option<Bound>>,
        time: InputTime,
    ) -> FilterResult<Option<Bound>> {
        let Some(held_back) = &mut self.caminito.held_back else {
            return result;
        };
        match result {
            FilterResult::Intercept(Some(Bound::HeldBack)) => result,
            FilterResult::Intercept(Some(Bound::Remap(_))) => {
                held_back.take();
                result
            }
            _ if pressed => {
                self.send_held_back(time);
                result
            }
            FilterResult::Intercept(_) => result,
            FilterResult::Forward if !held_back.holds(keycode) => {
                self.send_held_modifiers();
                self.forward_key(keycode, KeyState::Released, time);
                FilterResult::Intercept(None)
            }
            FilterResult::Forward if !held_back.is_taken() => {
                self.send_held_back(time);
                result
            }
            FilterResult::Forward => {
                if held_back.forget(keycode) {
                    self.caminito.held_back = None;
                }
                FilterResult::Intercept(None)
            }
        }
    }

    /// Sends the client the presses of the modifiers held back from it, each with the
    /// modifiers it gave, as it would have had them, and holds none back any longer.
    fn send_held_back(&mut self, time: InputTime) {
        let Some(held_back) = self.caminito.held_back.take() else {
            return;
        };
        for press in held_back.into_presses() {
            self.send_modifiers(press.mods);
            self.forward_key(press.keycode, KeyState::Pressed, time);
        }
    }

    /// The modifiers the client with the keyboard has: those held, but any held back from it
    /// (CMN-188).
    fn client_modifiers(&mut self) -> ModifiersState {
        let keyboard = self.caminito.seat.get_keyboard().unwrap();
        let held = keyboard.modifier_state();
        let Some(held_back) = &self.caminito.held_back else {
            return held;
        };
        let mut mods = held_back.client_modifiers(&held);
        mods.serialized = keyboard.with_xkb_state(self, |context| {
            let xkb = context.xkb().lock().unwrap();
            // Safety: the state is only read, and no reference to it outlives the lock.
            let state = unsafe { xkb.state() };
            mods.serialize_back(state)
        });
        mods
    }

    /// The app id of the window whose client has the keyboard. A Layer surface, such as the
    /// panel, has none.
    fn keyboard_app_id(&self) -> Option<String> {
        let keyboard = self.caminito.seat.get_keyboard().unwrap();
        if let KeyboardFocus::Layer(_) = keyboard.current_focus()? {
            return None;
        }
        let policy = &self.caminito.policy;
        policy.window(policy.focus(SEAT)?)?.app_id()
    }

    /// Sends a Key remap's `steps`, for its trigger's press at `time`, to the client with the
    /// keyboard: each step's key pressed and released in turn, with the step's modifiers
    /// alone, except that the last stays pressed while the trigger is (CMN-187), so the
    /// client repeats it, until the next key event. The modifiers held are sent again after
    /// the last step's release. A step that's a System shortcut, one of Caminito's bindings or an
    /// accelerator grab, runs it instead, in its place, as Alt+F4 closes the window, and is
    /// never remapped again. No steps swallow the press.
    fn send_remap(&mut self, steps: &[Combination], time: InputTime) {
        let Some((last, others)) = steps.split_last() else {
            return;
        };
        let mut sent = false;
        for step in others {
            if let Some(step) = self.press_step(step, time) {
                self.forward_key(step, KeyState::Released, time);
                sent = true;
            }
        }
        match self.press_step(last, time) {
            Some(step) => self.caminito.held_remap_step = Some(step),
            None if sent => self.send_held_modifiers(),
            None => {}
        }
    }

    /// Presses one step of a Key remap's target at `time`, its key with the step's modifiers
    /// alone, and gives the key's keycode; or runs the System shortcut it is, and gives none.
    fn press_step(&mut self, step: &Combination, time: InputTime) -> Option<Keycode> {
        let keyboard = self.caminito.seat.get_keyboard().unwrap();
        let mut mods = step.modifiers(&keyboard.modifier_state());
        let bindings = &self.caminito.settings.bindings;
        if let Some(binding) = bindings.binding_for(&mods, &[step.key]) {
            self.run_binding(binding, Trigger::Key(step.key), time);
            return None;
        }
        let grab = self.caminito.accelerator_grabs.pressed(&mods, &[step.key]);
        if let Some((action, owner)) = grab {
            let owner = owner.to_owned();
            self.caminito
                .signal_accelerator(&owner, action, time.millis(), false);
            return None;
        }
        let Some(keycode) = keyboard.keycode_for_keysym(step.key) else {
            tracing::debug!("can't send a key remap: no key gives {:?}", step.key);
            return None;
        };
        let focus = keyboard.current_focus()?;
        // The keyboard's own state isn't touched, so the keys held stay as they are, and
        // the client is told the modifiers it would have for the step's combination.
        mods.serialized = keyboard.with_xkb_state(self, |context| {
            let xkb = context.xkb().lock().unwrap();
            // Safety: the state is only read, and no reference to it outlives the lock.
            let state = unsafe { xkb.state() };
            mods.serialize_back(state)
        });
        let seat = self.caminito.seat.clone();
        focus.modifiers(&seat, self, mods, SERIAL_COUNTER.next_serial());
        self.forward_key(keycode, KeyState::Pressed, time);
        Some(keycode)
    }

    /// Forwards a key a Key remap sends, or one held back (CMN-188), to the client with the
    /// keyboard.
    fn forward_key(&mut self, keycode: Keycode, key_state: KeyState, time: InputTime) {
        let keyboard = self.caminito.seat.get_keyboard().unwrap();
        let serial = SERIAL_COUNTER.next_serial();
        let focus = keyboard.current_focus();
        let surface = focus.as_ref().and_then(|focus| focus.wl_surface());
        self.caminito.input_serials.sent(serial, surface.as_deref());
        keyboard.input_forward(self, keycode, key_state, serial, time, false);
    }

    /// Releases the last step of a Key remap's sequence held, if one is, and sends the
    /// client the modifiers held again.
    fn release_held_step(&mut self, time: InputTime) {
        let Some(step) = self.caminito.held_remap_step.take() else {
            return;
        };
        self.forward_key(step, KeyState::Released, time);
        self.send_held_modifiers();
    }

    /// Sends the client with the keyboard the modifiers it has, after a Key remap sent it
    /// others: those held, but any held back from it (CMN-188).
    fn send_held_modifiers(&mut self) {
        let mods = self.client_modifiers();
        self.send_modifiers(mods);
    }

    /// Tells the client with the keyboard of `mods`, without touching the keyboard's own
    /// state.
    fn send_modifiers(&mut self, mods: ModifiersState) {
        let keyboard = self.caminito.seat.get_keyboard().unwrap();
        let Some(focus) = keyboard.current_focus() else {
            return;
        };
        let seat = self.caminito.seat.clone();
        focus.modifiers(&seat, self, mods, SERIAL_COUNTER.next_serial());
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
        // The open window menu's row under the pointer is selected (161).
        self.caminito.window_menu_motion(location);
        // A press held on a window of the Overview drags it (245).
        self.caminito.overview_motion(location);
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
        self.hot_corner_motion(location);
        let now = self.caminito.now();
        self.caminito.policy.intellihide_pointer(location, now);
        // The cursor moved.
        self.caminito.queue_redraw_all();
    }

    /// A pointer button, as a Linux input event code. With `mouse-button-modifier`'s
    /// modifiers held, the left button moves the window, and the middle and right ones
    /// resize it and open its menu, as `resize-with-right-button` says which (162). A
    /// button one of Caminito's bindings names runs it, wherever the pointer is, but where
    /// such a click on a window has the press (184), and one an accelerator grab names is
    /// its grabber's then, as a key is after Caminito's bindings (185). A press on a frame
    /// does what Marco's frames do. Otherwise a press on a window focuses and raises it
    /// through policy before the client sees the press. The client never sees a press with
    /// the modifiers held, a bound or grabbed button's or a press on the frame. An active
    /// shortcuts inhibitor on the focused client keeps the modifier clicks, the bindings and
    /// the grabs from Caminito, as it keeps the keys': the press is that client's.
    pub fn on_pointer_button(&mut self, button: u32, state: ButtonState, time: InputTime) {
        self.idle_activity();
        // A click with a modifier held back (CMN-188) has it sent first, so an Alt+click a
        // client gets comes with its Alt, and Alt+drag shows the client what it did before.
        if state == ButtonState::Pressed {
            self.send_held_back(time);
        }
        let pointer = self.caminito.seat.get_pointer().unwrap();
        let serial = SERIAL_COUNTER.next_serial();
        let location = pointer.current_location();
        // The Overview's first: the release of a press held on one of its windows is its
        // own (245), and the Window switcher swallows the others'.
        if self.overview_button(button, state, location, time)
            || self.window_menu_button(button, state, location)
            || self.switcher_button(button, state, location)
            || self.workspace_popup_button(button, state, time)
        {
            return;
        }
        let pressed = state == ButtonState::Pressed && !pointer.is_grabbed();
        let under = pressed.then(|| self.caminito.under(location)).flatten();
        let mods = self.caminito.seat.get_keyboard().unwrap().modifier_state();
        let inhibited = self.shortcuts_inhibited();
        let click = under
            .as_ref()
            .and_then(Under::window)
            .filter(|_| !inhibited)
            .and_then(|_| self.caminito.settings.modifier_click.does(button, &mods));
        if state == ButtonState::Pressed && click.is_none() && !inhibited && !self.pointer_is_held()
        {
            // Either way the release is swallowed with the Window switcher's, so the press
            // ends under the table it started under, and focus and stacking are as they were.
            if let Some((trigger, binding)) = self.caminito.bound_to_button(button, &mods) {
                self.caminito.suppressed_buttons.insert(button);
                self.run_binding(binding, trigger, time);
                return;
            }
            // Caminito's binding wins, as it does for a key (98). A held button doesn't
            // repeat.
            if let Some((action, owner)) = self.caminito.grabbed_button(button, &mods) {
                self.caminito.suppressed_buttons.insert(button);
                self.caminito
                    .signal_accelerator(&owner, action, time.millis(), false);
                return;
            }
        }
        if let Some(Under::Layer(layer, ..)) = &under {
            self.caminito.layer_pressed(layer);
            self.update_keyboard_focus();
        }
        // A press on what's no window, a layer surface or an output's bare background,
        // makes that output the Active output (222). A window's press says its own below.
        if pressed && under.as_ref().and_then(Under::window).is_none() {
            self.caminito.output_pressed(location);
        }
        // A press on an override-redirect window goes to it alone: it's never focused or
        // raised, and can't be dragged.
        if let Some(under) = under
            && let Some(id) = under.window()
        {
            self.caminito.window_pressed();
            // Whatever the press does: a Frame button's and the menu's are no click.
            self.caminito.policy.press_on_window(SEAT, id);
            match (click, under) {
                (None, Under::Frame(id, control)) if control != FrameControl::ClientArea => {
                    self.frame_press(id, control, button, location, serial, time);
                }
                // Marco raises the window for its menu, and doesn't focus it
                // (`event_callback`, display.c:2009-2018).
                (Some(ModifierAction::Menu), _) => {
                    if self.caminito.settings.policy.raise_on_click {
                        self.caminito.policy.raise(id);
                    }
                }
                _ => {
                    let now = self.caminito.now();
                    self.caminito.policy.click(SEAT, id, now);
                    self.update_keyboard_focus();
                }
            }
            if let Some(click) = click {
                let start_data = || grab::start_data(button, location);
                match click {
                    ModifierAction::Move => {
                        self.begin_window_grab(id, GrabKind::Move, start_data(), serial);
                    }
                    // Marco's `has_resize_func` (display.c:1962).
                    ModifierAction::Resize => {
                        if self.caminito.policy.functions(id).resize()
                            && let Some(edges) = self.caminito.nearest_corner(id, location)
                        {
                            let kind = GrabKind::Resize(edges);
                            self.begin_window_grab(id, kind, start_data(), serial);
                        }
                    }
                    // At the pointer (display.c:2013-2017).
                    ModifierAction::Menu => {
                        self.open_window_menu(id, location.to_i32_floor(), time);
                    }
                }
                // The press is Caminito's whatever came of it, as Marco's grab of the
                // button is: a window that won't move or resize gets no press either.
                if !pointer.is_grabbed() {
                    self.begin_frame_grab(FrameHold::Held, start_data(), serial);
                }
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

    /// Whether the session is locked, or something holds the pointer or the keyboard: a
    /// move or a resize, a drag-and-drop, a client's popup, a button already down on a
    /// client, or an X11 client's menu, whose grab of the pointer is inside Xwayland and
    /// whose grab of the keyboard is all Caminito sees of it. A button's press then runs no
    /// binding (184) and is no accelerator grab's (185): it's the holder's.
    pub(crate) fn pointer_is_held(&self) -> bool {
        let caminito = &self.caminito;
        let pointer = caminito.seat.get_pointer().unwrap();
        let x11_menu = caminito.xwayland_keyboard_grab.as_ref();
        caminito.lock.is_locked()
            || pointer.is_grabbed()
            || caminito.window_grab.is_some()
            || caminito.policy.keyboard_grab(SEAT).is_some()
            || x11_menu.is_some_and(|(grab, _)| grab.is_alive())
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
                match &focus {
                    Some(KeyboardFocus::Surface(surface)) => tracing::debug!(
                        target: crate::lock::TARGET,
                        "the keyboard goes to the lock surface on {}",
                        self.caminito
                            .lock_surface_output_name(surface)
                            .unwrap_or_default()
                    ),
                    _ => tracing::debug!(
                        target: crate::lock::TARGET,
                        "the session is locked, and no lock surface takes the keyboard"
                    ),
                }
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

/// A scroll event, as a backend's device gives one or the headless fixture makes it.
#[derive(Debug, Clone, Copy)]
pub(crate) struct Scroll {
    pub time: InputTime,
    pub source: AxisSource,
    /// `None` where the device's settings drop its horizontal scrolling (53).
    pub horizontal: Option<AxisScroll>,
    pub vertical: AxisScroll,
}

/// A scroll event's part along one axis.
#[derive(Debug, Clone, Copy)]
pub(crate) struct AxisScroll {
    /// In libinput's units, of which a wheel's click is 15.
    pub amount: Option<f64>,
    /// In 120ths of a wheel's click, for a wheel.
    pub v120: Option<f64>,
    pub relative_direction: AxisRelativeDirection,
}

/// A wheel's click in a scroll's units, as libinput and GTK have it.
pub(crate) const WHEEL_CLICK: f64 = 15.;
/// A wheel's click in a scroll's 120ths.
pub(crate) const V120_CLICK: f64 = 120.;

/// What a press with `mouse-button-modifier`'s modifiers held does to the window under it.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum ModifierAction {
    Move,
    Resize,
    Menu,
}

impl ModifierClick {
    /// What a press of `button` with `mods` held does, if the modifiers are the key's:
    /// Marco grabs buttons 1 to 3 with them (`meta_display_grab_window_buttons`,
    /// display.c:4084-4129), and `meta_prefs_get_mouse_button_resize` and `_menu` say which
    /// of 2 and 3 does what (prefs.c:2510-2520).
    pub(crate) fn does(&self, button: u32, mods: &ModifiersState) -> Option<ModifierAction> {
        if !self.modifier.held_by(mods, button == BTN_LEFT) {
            return None;
        }
        match button {
            BTN_LEFT => Some(ModifierAction::Move),
            BTN_RIGHT if self.resize_with_right_button => Some(ModifierAction::Resize),
            BTN_MIDDLE if !self.resize_with_right_button => Some(ModifierAction::Resize),
            BTN_RIGHT | BTN_MIDDLE => Some(ModifierAction::Menu),
            _ => None,
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
pub(crate) enum Bound {
    /// One of Caminito's bindings, with the key that fired it.
    Binding { binding: Binding, key: Keysym },
    /// What a binding does while the Workspace popup shows (152): something policy does.
    Action(Action),
    /// A grabbed accelerator (98): its owner's unique name on the bus, and the action id
    /// to signal it.
    Accelerator { owner: String, action: u32 },
    /// A key for the keyboard move or resize in progress (156), or the manual placement
    /// (154).
    GrabKey(GrabKey),
    /// A Key remap's target (CMN-186): the steps it's sent as, none for nothing.
    Remap(Vec<Combination>),
    /// A modifier's press held back from the client, as it starts a Key remap (CMN-188).
    HeldBack,
}

/// The VT an `XF86Switch_VT_n` keysym switches to.
/// The keysyms that name a pressed key: the first layout's on its first level, whichever
/// layout is active, and the one the press types, as Keyboard Shortcuts writes
/// `<Shift>exclam` (100). A key with nothing on its first level is named by what it types
/// alone.
pub(crate) fn press_keysyms(keysym: &KeysymHandle<'_>) -> [Keysym; 2] {
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
    /// What's bound to a press of `button`, a Linux input event code, with `mods` held, and
    /// the button as bindings name it (184).
    pub(crate) fn bound_to_button(
        &self,
        button: u32,
        mods: &ModifiersState,
    ) -> Option<(Trigger, Binding)> {
        let button = x_button(button)?;
        let binding = self.settings.bindings.binding_for_button(mods, button)?;
        Some((Trigger::Button(button), binding))
    }

    /// The accelerator grab of a press of `button`, a Linux input event code, with `mods`
    /// held (185): its action id, and its owner's unique name on the bus.
    pub(crate) fn grabbed_button(
        &self,
        button: u32,
        mods: &ModifiersState,
    ) -> Option<(u32, String)> {
        let grabs = &self.accelerator_grabs;
        let (action, owner) = grabs.pressed_button(mods, x_button(button)?)?;
        Some((action, owner.to_owned()))
    }

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

    /// The edges of the window's corner nearest `location`, which a resize with the modifier
    /// held moves.
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
        // The open window menu has the pointer wherever it is, as a GTK menu's grab has
        // (161): no client has it meanwhile, and no Frame.
        if self.policy.window_menu().is_some() {
            return None;
        }
        // And so has the open Overview (241): its windows are drawn away from their own
        // places, and a click is the Overview's.
        if self.policy.overview().is_some() {
            return None;
        }
        scene::LAYERS.into_iter().find_map(|layer| match layer {
            SceneLayer::Blank
            | SceneLayer::Cursor
            | SceneLayer::DragIcon
            | SceneLayer::Lock
            | SceneLayer::BellFlash
            | SceneLayer::Minimizing
            | SceneLayer::Switcher
            | SceneLayer::WorkspacePopup
            | SceneLayer::WindowMenu
            | SceneLayer::Overview => None,
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

    /// A button was pressed at `location` on what's no managed window: a panel or any
    /// other layer surface, caja's desktop, the wallpaper or an output's bare background.
    /// Its output becomes the Active output (ADR 0032), and the focus stays where it is.
    /// The lock screen's press says nothing.
    pub(crate) fn output_pressed(&mut self, location: Point<f64, Logical>) {
        if self.lock.is_locked() {
            return;
        }
        let pressed = self
            .outputs
            .iter()
            .find(|output| output_geometry(output).to_f64().contains(location));
        if let Some(output) = pressed {
            self.policy.press_on_output(SEAT, output_id(output));
        }
    }

    /// A binding that switches the workspace is about to run: the output under the pointer
    /// becomes the Active output, unless an Interaction holds one of its own.
    pub(crate) fn pointer_output_switches(&mut self) {
        if let Some(output) = self.pointer_output() {
            self.policy.switch_on_output(SEAT, output);
        }
    }

    /// The output the pointer is on.
    pub(crate) fn pointer_output(&self) -> Option<OutputId> {
        let location = self.seat.get_pointer().unwrap().current_location();
        let under = self
            .outputs
            .iter()
            .find(|output| output_geometry(output).to_f64().contains(location));
        under.map(output_id)
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
            .active_stacking()
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
        if self.lock.is_locked()
            || self.switcher_popup_at(location)
            || self.policy.window_menu().is_some()
        {
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
