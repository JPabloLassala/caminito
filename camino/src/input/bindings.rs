//! Camino's keybindings: a hard-coded set of Marco's defaults, standing in until M8 reads
//! `org.mate.Marco.*` (17). The accelerators other programs grab are `accelerators`' (98).

pub(crate) mod accelerators;

use camino_policy::Action;
use smithay::input::keyboard::{Keysym, ModifiersState};

use crate::commands::Command;

/// The modifiers a binding names. Locks don't count.
#[derive(Debug, Clone, Copy, PartialEq)]
struct Held {
    ctrl: bool,
    alt: bool,
    shift: bool,
    logo: bool,
}

const NONE: Held = Held {
    ctrl: false,
    alt: false,
    shift: false,
    logo: false,
};

const SHIFT: Held = Held {
    ctrl: false,
    alt: false,
    shift: true,
    logo: false,
};

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

/// What a binding does.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum Binding {
    /// Something policy does.
    Policy(Action),
    /// A command to run (104).
    Command(Command),
}

/// Marco's defaults for the actions M1 binds, for `show-desktop` (94), and for its three
/// screenshot commands (104). `<Shift>Print` is the default of the Marco that has `run-command-area-screenshot`
/// (`org.mate.marco.gschema.xml:692-693` at `8522ed1`); 1.29.0 doesn't bind it.
const BINDINGS: [(Held, Keysym, Binding); 9] = [
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
    (ALT, Keysym::F4, Binding::Policy(Action::Close)),
    (ALT, Keysym::F10, Binding::Policy(Action::ToggleMaximized)),
    (ALT, Keysym::Tab, Binding::Policy(Action::SwitchWindows)),
    (CTRL_ALT, Keysym::d, Binding::Policy(Action::ShowDesktop)),
    (NONE, Keysym::Print, Binding::Command(Command::Screenshot)),
    (
        ALT,
        Keysym::Print,
        Binding::Command(Command::WindowScreenshot),
    ),
    (
        SHIFT,
        Keysym::Print,
        Binding::Command(Command::AreaScreenshot),
    ),
];

/// What's bound to `keysym` with exactly `mods` held, if anything.
pub(crate) fn binding_for(mods: &ModifiersState, keysym: Keysym) -> Option<Binding> {
    let held = Held::from(mods);
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

    const NONE: (bool, bool, bool) = (false, false, false);
    const SHIFT: (bool, bool, bool) = (false, false, true);
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
        assert_eq!(bound(CTRL_ALT, Keysym::d), policy(Action::ShowDesktop));
    }

    #[test]
    fn marcos_screenshot_bindings() {
        let command = |command| Some(Binding::Command(command));
        assert_eq!(bound(NONE, Keysym::Print), command(Command::Screenshot));
        assert_eq!(
            bound(ALT, Keysym::Print),
            command(Command::WindowScreenshot)
        );
        assert_eq!(
            bound(SHIFT, Keysym::Print),
            command(Command::AreaScreenshot)
        );
        assert_eq!(bound(CTRL_ALT, Keysym::Print), None);
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
