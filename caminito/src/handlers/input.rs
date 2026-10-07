//! The input protocols: pointer-constraints, relative-pointer, pointer-gestures, tablet-v2,
//! keyboard-shortcuts-inhibit, text-input-v3, input-method-v2 and cursor-shape.

use smithay::desktop::{PopupKind, PopupManager};
use smithay::input::pointer::PointerHandle;
use smithay::input::tablet::TabletSeatHandler;
use smithay::reexports::wayland_server::protocol::wl_surface::WlSurface;
use smithay::utils::{Logical, Rectangle};
use smithay::wayland::input_method::{InputMethodHandler, PopupSurface};
use smithay::wayland::keyboard_shortcuts_inhibit::{
    KeyboardShortcutsInhibitHandler, KeyboardShortcutsInhibitState, KeyboardShortcutsInhibitor,
};
use smithay::wayland::pointer_constraints::PointerConstraintsHandler;

use crate::state::State;

impl PointerConstraintsHandler for State {
    fn new_constraint(&mut self, _surface: &WlSurface, _pointer: &PointerHandle<Self>) {
        // Active at once if the pointer is already on the surface; otherwise when it enters.
        self.activate_pointer_constraint();
    }
}

impl TabletSeatHandler for State {
    type ToolFocus = WlSurface;
}

impl KeyboardShortcutsInhibitHandler for State {
    fn keyboard_shortcuts_inhibit_state(&mut self) -> &mut KeyboardShortcutsInhibitState {
        &mut self.caminito.keyboard_shortcuts_inhibit_state
    }

    fn new_inhibitor(&mut self, inhibitor: KeyboardShortcutsInhibitor) {
        // Granted; the keyboard path skips Caminito's bindings while the focused surface holds
        // an active inhibitor.
        inhibitor.activate();
    }
}

impl InputMethodHandler for State {
    fn new_popup(&mut self, surface: PopupSurface) {
        if let Err(err) = self.caminito.popups.track_popup(PopupKind::from(surface)) {
            tracing::warn!("error tracking an input-method popup: {err}");
        }
    }

    fn popup_repositioned(&mut self, _surface: PopupSurface) {}

    fn dismiss_popup(&mut self, surface: PopupSurface) {
        if let Some(parent) = surface.get_parent().map(|parent| parent.surface.clone()) {
            let _ = PopupManager::dismiss_popup(&parent, &PopupKind::from(surface));
        }
    }

    fn parent_geometry(&self, parent: &WlSurface) -> Rectangle<i32, Logical> {
        self.caminito
            .mapped_for_surface(parent)
            .and_then(|id| self.caminito.policy.geometry(id))
            .unwrap_or_default()
    }
}
