//! Camino's keybindings: a hard-coded set of Marco's defaults, standing in until M8 reads
//! `org.mate.Marco.*` (17). The accelerators other programs grab are `accelerators`' (98).

pub(crate) mod accelerators;

use camino_policy::Action;
use smithay::input::keyboard::{Keysym, ModifiersState};

/// The modifiers a binding names. Locks don't count.
#[derive(Debug, Clone, Copy, PartialEq)]
struct Held {
    ctrl: bool,
    alt: bool,
    shift: bool,
    logo: bool,
}

impl From<&ModifiersState> for Held {
    fn from(mods: &ModifiersState) -> Self {
        Self {
            ctrl: mods.ctrl,
            alt: mods.alt,
            shift: mods.shift,
            logo: mods.logo,
        }
    }
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

/// Marco's defaults for the actions M1 binds, and for `show-desktop` (94).
const BINDINGS: [(Held, Keysym, Action); 6] = [
    (CTRL_ALT, Keysym::Left, Action::SwitchToWorkspaceLeft),
    (CTRL_ALT, Keysym::Right, Action::SwitchToWorkspaceRight),
    (ALT, Keysym::F4, Action::Close),
    (ALT, Keysym::F10, Action::ToggleMaximized),
    (ALT, Keysym::Tab, Action::SwitchWindows),
    (CTRL_ALT, Keysym::d, Action::ShowDesktop),
];

/// The action bound to `keysym` with exactly `mods` held, if any.
pub fn action_for(mods: &ModifiersState, keysym: Keysym) -> Option<Action> {
    let held = Held::from(mods);
    BINDINGS
        .iter()
        .find(|(mods, sym, _)| *mods == held && *sym == keysym)
        .map(|(_, _, action)| *action)
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

    fn bound(held: (bool, bool, bool), keysym: Keysym) -> Option<Action> {
        action_for(&mods(held.0, held.1, held.2), keysym)
    }

    #[test]
    fn marcos_default_bindings() {
        assert_eq!(
            bound(CTRL_ALT, Keysym::Left),
            Some(Action::SwitchToWorkspaceLeft)
        );
        assert_eq!(
            bound(CTRL_ALT, Keysym::Right),
            Some(Action::SwitchToWorkspaceRight)
        );
        assert_eq!(bound(ALT, Keysym::F4), Some(Action::Close));
        assert_eq!(bound(ALT, Keysym::F10), Some(Action::ToggleMaximized));
        assert_eq!(bound(ALT, Keysym::Tab), Some(Action::SwitchWindows));
        assert_eq!(bound(CTRL_ALT, Keysym::d), Some(Action::ShowDesktop));
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
        assert_eq!(action_for(&held, Keysym::F4), Some(Action::Close));
    }

    #[test]
    fn unbound_keys_have_no_action() {
        assert_eq!(bound(ALT, Keysym::a), None);
    }
}
