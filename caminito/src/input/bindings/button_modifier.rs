//! `mouse-button-modifier` (162): the modifiers held for a click on a window to move it,
//! resize it or open its menu, as Marco's `meta_ui_parse_modifier` reads them (ui.c:905-950).

use std::fmt;

use smithay::input::keyboard::ModifiersState;

use super::Held;
use super::accelerators::{ParseError, modifiers};

/// The modifiers a click on a window is held with, or none, which turns those clicks off.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct ButtonModifier(Option<Held>);

impl Default for ButtonModifier {
    /// Marco's default, `<Alt>`.
    fn default() -> Self {
        Self(Some(Held {
            alt: true,
            ..NOTHING
        }))
    }
}

const NOTHING: Held = Held {
    ctrl: false,
    alt: false,
    shift: false,
    logo: false,
};

impl ButtonModifier {
    /// Parses the key's value: modifiers in GTK's accelerator syntax, one or more, and no
    /// key after them. The empty string and `disabled` are no modifier.
    pub(crate) fn parse(value: &str) -> Result<Self, ParseError> {
        if value.is_empty() || value == "disabled" {
            return Ok(Self(None));
        }
        match modifiers(value)? {
            (held, "") => Ok(Self(Some(held))),
            (_, key) => Err(ParseError::NotAModifier(key.to_owned())),
        }
    }

    /// Whether a press with `mods` held is one with these modifiers: exactly them, as
    /// Marco's button grabs are of them and nothing else but the locks
    /// (`meta_display_grab_window_buttons`, display.c:4084-4129). `or_shift` takes Shift
    /// with them too, as Marco's grab of the left button does, for a move that snaps.
    pub(crate) fn held_by(&self, mods: &ModifiersState, or_shift: bool) -> bool {
        let Some(held) = self.0 else {
            return false;
        };
        let pressed = Held::from(mods);
        let shifted = Held {
            shift: true,
            ..held
        };
        pressed == held || (or_shift && pressed == shifted)
    }
}

impl fmt::Display for ButtonModifier {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        let Some(held) = self.0 else {
            return f.write_str("disabled");
        };
        for (on, name) in [
            (held.ctrl, "<Control>"),
            (held.shift, "<Shift>"),
            (held.alt, "<Alt>"),
            (held.logo, "<Super>"),
        ] {
            if on {
                f.write_str(name)?;
            }
        }
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[derive(Clone, Copy, Default)]
    struct Mods {
        ctrl: bool,
        alt: bool,
        shift: bool,
        logo: bool,
        caps_lock: bool,
        num_lock: bool,
    }

    fn state(mods: Mods) -> ModifiersState {
        ModifiersState {
            ctrl: mods.ctrl,
            alt: mods.alt,
            shift: mods.shift,
            logo: mods.logo,
            caps_lock: mods.caps_lock,
            num_lock: mods.num_lock,
            ..ModifiersState::default()
        }
    }

    const ALT: Mods = Mods {
        ctrl: false,
        alt: true,
        shift: false,
        logo: false,
        caps_lock: false,
        num_lock: false,
    };
    const SUPER: Mods = Mods {
        alt: false,
        logo: true,
        ..ALT
    };

    fn parsed(value: &str) -> ButtonModifier {
        ButtonModifier::parse(value).unwrap_or_else(|err| panic!("{value}: {err}"))
    }

    fn held(value: &str, mods: Mods) -> bool {
        parsed(value).held_by(&state(mods), false)
    }

    fn error(value: &str) -> String {
        match ButtonModifier::parse(value) {
            Ok(modifier) => panic!("{value} parsed as {modifier}"),
            Err(err) => err.to_string(),
        }
    }

    #[test]
    fn alt_is_alt_and_the_default() {
        assert!(held("<Alt>", ALT));
        assert!(!held("<Alt>", SUPER));
        assert!(!held("<Alt>", Mods::default()));
        assert_eq!(parsed("<Alt>"), ButtonModifier::default());
        assert_eq!(parsed("<Mod1>"), ButtonModifier::default());
    }

    #[test]
    fn super_and_mod4_are_both_the_logo_key() {
        for value in ["<Super>", "<Mod4>", "<super>"] {
            assert!(held(value, SUPER), "{value}");
            assert!(!held(value, ALT), "{value}");
        }
    }

    #[test]
    fn several_modifiers_are_all_held_and_no_other() {
        let ctrl_alt = Mods { ctrl: true, ..ALT };
        for value in ["<Control><Alt>", "<Alt><Ctrl>"] {
            assert!(held(value, ctrl_alt), "{value}");
            assert!(!held(value, ALT), "{value}");
        }
        // One modifier more than the key names isn't Marco's grab either.
        assert!(!held("<Alt>", ctrl_alt));
    }

    #[test]
    fn the_locks_dont_count() {
        let locked = Mods {
            caps_lock: true,
            num_lock: true,
            ..ALT
        };
        assert!(held("<Alt>", locked));
    }

    #[test]
    fn shift_is_taken_with_them_only_where_asked() {
        let alt_shift = Mods { shift: true, ..ALT };
        let modifier = parsed("<Alt>");
        assert!(modifier.held_by(&state(alt_shift), true));
        assert!(!modifier.held_by(&state(alt_shift), false));
        assert!(modifier.held_by(&state(ALT), true));
    }

    #[test]
    fn the_empty_string_and_disabled_are_no_modifier() {
        for value in ["", "disabled"] {
            assert!(!held(value, ALT), "{value:?}");
            assert!(!held(value, Mods::default()), "{value:?}");
            assert!(!parsed(value).held_by(&state(Mods::default()), true));
            assert_eq!(parsed(value).to_string(), "disabled");
        }
    }

    #[test]
    fn garbage_a_key_and_a_modifier_caminito_cant_match_dont_parse() {
        assert_eq!(error("garbage"), "garbage isn't a modifier");
        assert_eq!(error("<Alt>a"), "a isn't a modifier");
        assert_eq!(error("<Alt"), "a modifier has no closing >");
        assert_eq!(error("<Foo>"), "<Foo> isn't a modifier");
        assert_eq!(error("<Release><Alt>"), "<Release> can't be grabbed");
        assert_eq!(error("<Mod5>"), "<Mod5> can't be grabbed");
    }

    #[test]
    fn it_is_written_as_gtk_writes_one() {
        assert_eq!(parsed("<Alt>").to_string(), "<Alt>");
        assert_eq!(parsed("<Mod4><Ctl>").to_string(), "<Control><Super>");
    }
}
