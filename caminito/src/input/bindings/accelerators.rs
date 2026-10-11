//! Accelerators grabbed over `org.mate.Caminito`'s `GrabAccelerators` (98): GTK's syntax for
//! a key combination, as MATE's keys store it, and which caller has grabbed which.

use std::collections::BTreeMap;
use std::fmt;

use smithay::input::keyboard::{Keysym, ModifiersState, xkb};

use super::Held;

/// A key combination: a keysym with exactly these modifiers held.
#[derive(Debug, Clone, Copy, PartialEq)]
pub(super) struct Accelerator {
    pub(super) held: Held,
    pub(super) keysym: Keysym,
}

impl Accelerator {
    /// Whether a press with `mods` held is this combination, where `keysyms` are the ones
    /// that name the key pressed.
    pub(super) fn matches(&self, mods: &ModifiersState, keysyms: &[Keysym]) -> bool {
        self.held == Held::from(mods) && keysyms.contains(&self.keysym)
    }
}

/// Why a string isn't an accelerator Caminito can grab.
#[derive(Debug, PartialEq)]
pub(crate) enum ParseError {
    /// A `<` with no `>` after it.
    UnclosedModifier,
    /// A modifier GTK doesn't name.
    UnknownModifier(String),
    /// A modifier GTK names and Caminito doesn't match: `<Release>`, since a grab is of the
    /// press, and `<Mod2>`, `<Mod3>` and `<Mod5>`, the locks and level shifts.
    UnsupportedModifier(String),
    /// Modifiers with no key after them.
    NoKey,
    /// A key by its code, as `0x1f`, which msd writes for a key with no keysym.
    Keycode,
    /// A key that isn't a keysym's name.
    UnknownKey(String),
    /// A key where modifiers alone are asked for, as `mouse-button-modifier` asks (162).
    NotAModifier(String),
}

impl fmt::Display for ParseError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::UnclosedModifier => write!(f, "a modifier has no closing >"),
            Self::UnknownModifier(name) => write!(f, "<{name}> isn't a modifier"),
            Self::UnsupportedModifier(name) => write!(f, "<{name}> can't be grabbed"),
            Self::NoKey => write!(f, "it names no key"),
            Self::Keycode => write!(f, "a key by its code can't be grabbed"),
            Self::UnknownKey(name) => write!(f, "{name} isn't a keysym"),
            Self::NotAModifier(name) => write!(f, "{name} isn't a modifier"),
        }
    }
}

impl std::error::Error for ParseError {}

/// Parses an accelerator in GTK's syntax (`gtk_accelerator_parse`, and msd's
/// `egg_accelerator_parse_virtual` for `<Shft>` and `<Ctl>`): modifiers in angle brackets,
/// in any order and case, then a keysym's name. The empty string and `disabled`, which
/// MATE stores for a shortcut with no keys, are no accelerator.
pub(super) fn parse(accelerator: &str) -> Result<Option<Accelerator>, ParseError> {
    if accelerator.is_empty() || accelerator == "disabled" {
        return Ok(None);
    }
    let (held, rest) = modifiers(accelerator)?;
    if rest.is_empty() {
        return Err(ParseError::NoKey);
    }
    // xkbcommon would read it as a keysym's number.
    if rest.starts_with("0x") {
        return Err(ParseError::Keycode);
    }
    let named = |flags| Some(xkb::keysym_from_name(rest, flags)).filter(|sym| sym.raw() != 0);
    let keysym = named(xkb::KEYSYM_NO_FLAGS)
        .or_else(|| named(xkb::KEYSYM_CASE_INSENSITIVE))
        .ok_or_else(|| ParseError::UnknownKey(rest.to_owned()))?;
    // GTK lowercases the key, and the keyboard path matches a letter by its key's first
    // level, where the letters are lowercase.
    let keysym = match keysym.raw() {
        upper @ 0x41..=0x5a => Keysym::new(upper + 0x20),
        _ => keysym,
    };
    Ok(Some(Accelerator { held, keysym }))
}

/// The modifiers an accelerator starts with, and what's after them.
pub(super) fn modifiers(accelerator: &str) -> Result<(Held, &str), ParseError> {
    let mut held = Held::from(&ModifiersState::default());
    let mut rest = accelerator;
    while let Some(after) = rest.strip_prefix('<') {
        let (name, after) = after.split_once('>').ok_or(ParseError::UnclosedModifier)?;
        // The virtual modifiers are where xkeyboard-config's evdev rules put them.
        match name.to_ascii_lowercase().as_str() {
            "control" | "ctrl" | "ctl" | "primary" => held.ctrl = true,
            "shift" | "shft" => held.shift = true,
            "alt" | "meta" | "mod1" => held.alt = true,
            "super" | "hyper" | "mod4" => held.logo = true,
            "release" | "mod2" | "mod3" | "mod5" => {
                return Err(ParseError::UnsupportedModifier(name.to_owned()));
            }
            _ => return Err(ParseError::UnknownModifier(name.to_owned())),
        }
        rest = after;
    }
    Ok((held, rest))
}

/// One caller's grab of a combination.
pub(crate) struct Grab {
    /// The grabber's unique name on the bus.
    pub(crate) owner: String,
    /// The accelerator as the grabber wrote it.
    pub(crate) accelerator: String,
    combination: Accelerator,
}

/// The grabs, by action id. A combination has one grab at most.
#[derive(Default)]
pub(crate) struct Grabs {
    grabs: BTreeMap<u32, Grab>,
    last_action: u32,
}

impl Grabs {
    /// Grabs `accelerator` for `owner`, and returns the grab's action id, which is the one
    /// it had if `owner` has the combination already. It returns 0, which is never an id,
    /// for no accelerator, for one that doesn't parse and for one another caller has.
    pub(crate) fn grab(&mut self, owner: &str, accelerator: &str) -> u32 {
        let combination = match parse(accelerator) {
            Ok(Some(combination)) => combination,
            Ok(None) => return 0,
            Err(err) => {
                tracing::warn!("can't grab the accelerator {accelerator:?} for {owner}: {err}");
                return 0;
            }
        };
        let mut grabs = self.grabs.iter();
        if let Some((action, grab)) = grabs.find(|(_, grab)| grab.combination == combination) {
            if grab.owner == owner {
                return *action;
            }
            let has = &grab.owner;
            tracing::warn!("can't grab the accelerator {accelerator:?} for {owner}: {has} has it");
            return 0;
        }
        loop {
            self.last_action = self.last_action.wrapping_add(1);
            if self.last_action != 0 && !self.grabs.contains_key(&self.last_action) {
                break;
            }
        }
        let grab = Grab {
            owner: owner.to_owned(),
            accelerator: accelerator.to_owned(),
            combination,
        };
        self.grabs.insert(self.last_action, grab);
        self.last_action
    }

    /// Releases the grab with this action id, if it's `owner`'s.
    pub(crate) fn ungrab(&mut self, owner: &str, action: u32) {
        if self
            .grabs
            .get(&action)
            .is_some_and(|grab| grab.owner == owner)
        {
            self.grabs.remove(&action);
        } else {
            tracing::debug!("{owner} has no accelerator {action} to release");
        }
    }

    /// Releases the grabs of a caller that left the bus.
    pub(crate) fn owner_left(&mut self, owner: &str) {
        self.grabs.retain(|_, grab| grab.owner != owner);
    }

    /// The grab of a press with `mods` held, if there is one: its action id and its owner.
    /// `keysyms` are the ones that name the key pressed: the one on its first level, and
    /// the one the press types, which is how Keyboard Shortcuts writes a key that Shift or
    /// NumLock changes (100). Two grabs can then be of one press, as `<Shift>1` and
    /// `<Shift>exclam`: the one grabbed first has it.
    pub(crate) fn pressed(&self, mods: &ModifiersState, keysyms: &[Keysym]) -> Option<(u32, &str)> {
        self.iter()
            .find(|(_, grab)| grab.combination.matches(mods, keysyms))
            .map(|(action, grab)| (action, grab.owner.as_str()))
    }

    /// The owner of the grab with this action id, if it's still held.
    pub(crate) fn owner(&self, action: u32) -> Option<&str> {
        self.grabs.get(&action).map(|grab| grab.owner.as_str())
    }

    /// Every grab with its action id, in the ids' order.
    pub(crate) fn iter(&self) -> impl Iterator<Item = (u32, &Grab)> {
        self.grabs.iter().map(|(action, grab)| (*action, grab))
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    const NONE: ModifiersState = ModifiersState {
        ctrl: false,
        alt: false,
        shift: false,
        caps_lock: false,
        logo: false,
        num_lock: false,
        iso_level3_shift: false,
        iso_level5_shift: false,
        serialized: smithay::input::keyboard::SerializedMods {
            depressed: 0,
            latched: 0,
            locked: 0,
            layout_effective: 0,
        },
    };

    fn matches(accelerator: &str, mods: ModifiersState, keysym: Keysym) -> bool {
        let parsed = parse(accelerator).unwrap().unwrap();
        parsed.matches(&mods, &[keysym])
    }

    #[test]
    fn a_key_alone_is_its_keysym_with_nothing_held() {
        assert!(matches("F9", NONE, Keysym::F9));
        assert!(!matches("F9", NONE, Keysym::F8));
        let ctrl = ModifiersState { ctrl: true, ..NONE };
        assert!(!matches("F9", ctrl, Keysym::F9));
    }

    #[test]
    fn modifiers_come_in_any_order_and_case() {
        let ctrl_alt = ModifiersState {
            ctrl: true,
            alt: true,
            ..NONE
        };
        for accelerator in [
            "<Control><Alt>t",
            "<Alt><Control>t",
            "<control><ALT>t",
            "<Ctrl><Mod1>t",
            "<Ctl><Meta>t",
            "<Primary><alt>t",
        ] {
            assert!(matches(accelerator, ctrl_alt, Keysym::t), "{accelerator}");
        }
        let shift_super = ModifiersState {
            shift: true,
            logo: true,
            ..NONE
        };
        for accelerator in ["<Shift><Super>e", "<Mod4><Shft>e", "<hyper><SHIFT>e"] {
            assert!(
                matches(accelerator, shift_super, Keysym::e),
                "{accelerator}"
            );
        }
    }

    #[test]
    fn a_modifier_named_twice_is_held_once() {
        let ctrl = ModifiersState { ctrl: true, ..NONE };
        assert!(matches("<Control><Ctrl>t", ctrl, Keysym::t));
    }

    #[test]
    fn the_xf86_media_keys_are_keysyms() {
        assert!(matches("XF86AudioMute", NONE, Keysym::XF86_AudioMute));
        assert!(matches(
            "XF86AudioRaiseVolume",
            NONE,
            Keysym::XF86_AudioRaiseVolume
        ));
        let shift = ModifiersState {
            shift: true,
            ..NONE
        };
        assert!(matches(
            "<Shift>XF86AudioPlay",
            shift,
            Keysym::XF86_AudioPlay
        ));
    }

    #[test]
    fn a_letter_is_the_key_that_types_it_in_either_case() {
        // GTK lowercases the key, and the first level of a letter's key is its lowercase.
        let shift = ModifiersState {
            shift: true,
            ..NONE
        };
        assert!(matches("<Shift>A", shift, Keysym::a));
        assert!(matches("<Shift>a", shift, Keysym::a));
        assert!(matches("print", NONE, Keysym::Print));
    }

    #[test]
    fn locks_dont_count_as_modifiers() {
        let locked = ModifiersState {
            caps_lock: true,
            num_lock: true,
            ..NONE
        };
        assert!(matches("XF86AudioMute", locked, Keysym::XF86_AudioMute));
    }

    #[test]
    fn the_empty_and_disabled_strings_are_no_accelerator() {
        assert_eq!(parse(""), Ok(None));
        assert_eq!(parse("disabled"), Ok(None));
    }

    #[test]
    fn what_isnt_an_accelerator_doesnt_parse() {
        assert_eq!(
            parse("<Control>NoSuchKey"),
            Err(ParseError::UnknownKey("NoSuchKey".into()))
        );
        assert_eq!(
            parse("<Contrl>t"),
            Err(ParseError::UnknownModifier("Contrl".into()))
        );
        assert_eq!(parse("<Control"), Err(ParseError::UnclosedModifier));
        assert_eq!(parse("<Control><Alt>"), Err(ParseError::NoKey));
    }

    #[test]
    fn what_caminito_cant_grab_doesnt_parse() {
        // A release, a lock or a level shift as a modifier, and a key by its code.
        for modifier in ["Release", "Mod2", "Mod3", "Mod5"] {
            assert_eq!(
                parse(&format!("<{modifier}>t")),
                Err(ParseError::UnsupportedModifier(modifier.into()))
            );
        }
        assert_eq!(parse("<Control>0x1f"), Err(ParseError::Keycode));
    }
}
