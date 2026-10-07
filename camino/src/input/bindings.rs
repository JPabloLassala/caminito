//! Camino's keybindings: a hard-coded set of Marco's defaults, standing in until M8 reads
//! `org.mate.Marco.*` (17).

use camino_policy::Action;
use smithay::input::keyboard::{Keysym, ModifiersState};

use crate::dbus::panel::PanelAction;

/// The modifiers a binding names. Locks don't count.
#[derive(PartialEq)]
struct Held {
    ctrl: bool,
    alt: bool,
    shift: bool,
    logo: bool,
}

const ALT: Held = Held {
    ctrl: false,
    alt: true,
    shift: false,
    logo: false,
};

const CTRL_ALT: Held = Held {
    ctrl: true,
    alt: true,
    shift: false,
    logo: false,
};

/// What a binding does.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Binding {
    /// Something policy does.
    Policy(Action),
    /// Something Camino asks the panel for (95).
    Panel(PanelAction),
}

/// Marco's defaults for the actions M1 binds, and for the panel's two (95).
const BINDINGS: [(Held, Keysym, Binding); 7] = [
    (
        CTRL_ALT,
        Keysym::Left,
        Binding::Policy(Action::SwitchToWorkspaceLeft),
    ),
    (
        CTRL_ALT,
        Keysym::Right,
        Binding::Policy(Action::SwitchToWorkspaceRight),
    ),
    (ALT, Keysym::F1, Binding::Panel(PanelAction::MainMenu)),
    (ALT, Keysym::F2, Binding::Panel(PanelAction::RunDialog)),
    (ALT, Keysym::F4, Binding::Policy(Action::Close)),
    (ALT, Keysym::F10, Binding::Policy(Action::ToggleMaximized)),
    (ALT, Keysym::Tab, Binding::Policy(Action::SwitchWindows)),
];

/// What's bound to `keysym` with exactly `mods` held, if anything.
pub fn binding_for(mods: &ModifiersState, keysym: Keysym) -> Option<Binding> {
    let held = Held {
        ctrl: mods.ctrl,
        alt: mods.alt,
        shift: mods.shift,
        logo: mods.logo,
    };
    BINDINGS
        .iter()
        .find(|(mods, sym, _)| *mods == held && *sym == keysym)
        .map(|(_, _, binding)| *binding)
}

#[cfg(test)]
mod tests {
    use super::*;

    fn mods(ctrl: bool, alt: bool, shift: bool) -> ModifiersState {
        ModifiersState {
            ctrl,
            alt,
            shift,
            ..ModifiersState::default()
        }
    }

    const ALT: (bool, bool, bool) = (false, true, false);
    const CTRL_ALT: (bool, bool, bool) = (true, true, false);

    fn bound(held: (bool, bool, bool), keysym: Keysym) -> Option<Binding> {
        binding_for(&mods(held.0, held.1, held.2), keysym)
    }

    fn policy(action: Action) -> Option<Binding> {
        Some(Binding::Policy(action))
    }

    #[test]
    fn marcos_default_bindings() {
        assert_eq!(
            bound(CTRL_ALT, Keysym::Left),
            policy(Action::SwitchToWorkspaceLeft)
        );
        assert_eq!(
            bound(CTRL_ALT, Keysym::Right),
            policy(Action::SwitchToWorkspaceRight)
        );
        assert_eq!(bound(ALT, Keysym::F4), policy(Action::Close));
        assert_eq!(bound(ALT, Keysym::F10), policy(Action::ToggleMaximized));
        assert_eq!(bound(ALT, Keysym::Tab), policy(Action::SwitchWindows));
    }

    #[test]
    fn the_panels_bindings() {
        assert_eq!(
            bound(ALT, Keysym::F1),
            Some(Binding::Panel(PanelAction::MainMenu))
        );
        assert_eq!(
            bound(ALT, Keysym::F2),
            Some(Binding::Panel(PanelAction::RunDialog))
        );
    }

    #[test]
    fn modifiers_must_match_exactly() {
        assert_eq!(bound((false, false, false), Keysym::F4), None);
        assert_eq!(bound((true, true, false), Keysym::F4), None);
        assert_eq!(bound((false, true, true), Keysym::Tab), None);
        assert_eq!(bound(ALT, Keysym::Left), None);
    }

    #[test]
    fn locks_dont_count_as_modifiers() {
        let held = ModifiersState {
            alt: true,
            caps_lock: true,
            num_lock: true,
            ..ModifiersState::default()
        };
        assert_eq!(binding_for(&held, Keysym::F4), policy(Action::Close));
    }

    #[test]
    fn unbound_keys_have_no_action() {
        assert_eq!(bound(ALT, Keysym::a), None);
    }
}
