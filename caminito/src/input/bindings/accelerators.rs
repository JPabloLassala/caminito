//! Accelerators grabbed over `org.mate.Caminito`'s `GrabAccelerators` (98): GTK's syntax for
//! a key combination, as MATE's keys store it, with a mouse button where a key's name would
//! be (ADR 0028), and which caller has grabbed which.

use std::collections::BTreeMap;
use std::fmt;

use smithay::input::keyboard::{Keysym, ModifiersState, xkb};

use super::Held;

/// What a combination's press is of.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum Trigger {
    /// A key, by its keysym.
    Key(Keysym),
    /// A mouse button, by X's number for it.
    Button(u32),
}

/// A combination: a key or a mouse button with exactly these modifiers held.
#[derive(Debug, Clone, Copy, PartialEq)]
pub(super) struct Accelerator {
    pub(super) held: Held,
    pub(super) trigger: Trigger,
}

impl Accelerator {
    /// Whether a key's press with `mods` held is this combination, where `keysyms` are the
    /// ones that name the key pressed.
    pub(super) fn matches(&self, mods: &ModifiersState, keysyms: &[Keysym]) -> bool {
        self.held == Held::from(mods)
            && matches!(self.trigger, Trigger::Key(keysym) if keysyms.contains(&keysym))
    }

    /// Whether a press of `button`, by X's number, with `mods` held is this combination.
    pub(super) fn matches_button(&self, mods: &ModifiersState, button: u32) -> bool {
        self.held == Held::from(mods) && self.trigger == Trigger::Button(button)
    }
}

// Linux input event codes.
const BTN_LEFT: u32 = 0x110;
const BTN_RIGHT: u32 = 0x111;
const BTN_MIDDLE: u32 = 0x112;
const BTN_SIDE: u32 = 0x113;
/// The last of the kernel's mouse buttons: `BTN_JOYSTICK` is next.
const BTN_MOUSE_LAST: u32 = 0x11f;

/// The highest button an accelerator names, which is `BTN_MOUSE_LAST`'s number.
const LAST_BUTTON: u32 = 8 + BTN_MOUSE_LAST - BTN_SIDE;

/// X's number for a button by its Linux input event code, as xf86-input-libinput
/// (`btn_linux2xorg`, src/util.c) and Xwayland (`pointer_handle_button`,
/// hw/xwayland/xwayland-input.c) number one: the left, middle and right buttons are 1 to 3,
/// 4 to 7 are the wheel's, and the side buttons follow from 8 in the codes' order. `None`
/// for a code that isn't a mouse button's.
pub(crate) fn x_button(code: u32) -> Option<u32> {
    match code {
        BTN_LEFT => Some(1),
        BTN_MIDDLE => Some(2),
        BTN_RIGHT => Some(3),
        BTN_SIDE..=BTN_MOUSE_LAST => Some(8 + code - BTN_SIDE),
        _ => None,
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
    /// A button X has no number for or Caminito doesn't read: `Button0`, `Button` with no
    /// number, or one past the kernel's mouse buttons.
    NoSuchButton(String),
    /// `Button4` to `Button7`, which are the wheel's.
    Wheel(u32),
    /// `Button1` or `Button3` with no modifier, which would take every click from every
    /// client.
    BareButton(u32),
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
            Self::NoSuchButton(name) => write!(f, "{name} isn't a mouse button"),
            Self::Wheel(button) => write!(f, "Button{button} is the wheel's"),
            Self::BareButton(button) => write!(f, "Button{button} needs a modifier"),
        }
    }
}

impl std::error::Error for ParseError {}

/// Parses an accelerator in GTK's syntax (`gtk_accelerator_parse`, and msd's
/// `egg_accelerator_parse_virtual` for `<Shft>` and `<Ctl>`): modifiers in angle brackets,
/// in any order and case, then a keysym's name, or `Button` and X's number for a mouse
/// button (ADR 0028). The empty string and `disabled`, which MATE stores for a shortcut with
/// no keys, are no accelerator.
pub(super) fn parse(accelerator: &str) -> Result<Option<Accelerator>, ParseError> {
    if accelerator.is_empty() || accelerator == "disabled" {
        return Ok(None);
    }
    let (held, rest) = modifiers(accelerator)?;
    if rest.is_empty() {
        return Err(ParseError::NoKey);
    }
    // No keysym's name starts so.
    if let Some(number) = strip_prefix_ignore_case(rest, "button") {
        let trigger = Trigger::Button(button(rest, number, held)?);
        return Ok(Some(Accelerator { held, trigger }));
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
    let trigger = Trigger::Key(keysym);
    Ok(Some(Accelerator { held, trigger }))
}

/// `text` without `prefix`, if it starts with it in any case.
fn strip_prefix_ignore_case<'a>(text: &'a str, prefix: &str) -> Option<&'a str> {
    let (start, rest) = text.split_at_checked(prefix.len())?;
    start.eq_ignore_ascii_case(prefix).then_some(rest)
}

/// The button `name` names with `number` after `Button`, for a combination with `held`.
fn button(name: &str, number: &str, held: Held) -> Result<u32, ParseError> {
    // As X writes a number: `u32`'s parser would take `+8` and `08` too.
    let written = !number.starts_with('0') && number.bytes().all(|b| b.is_ascii_digit());
    let button = number.parse().ok().filter(|_| written);
    match button {
        Some(button @ (1 | 3)) if held == Held::NOTHING => Err(ParseError::BareButton(button)),
        Some(button @ 4..=7) => Err(ParseError::Wheel(button)),
        Some(button @ 1..=LAST_BUTTON) => Ok(button),
        _ => Err(ParseError::NoSuchButton(name.to_owned())),
    }
}

/// The modifiers an accelerator starts with, and what's after them.
pub(super) fn modifiers(accelerator: &str) -> Result<(Held, &str), ParseError> {
    let mut held = Held::NOTHING;
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
            // A button is granted once it can be signalled (185).
            Ok(Some(Accelerator {
                trigger: Trigger::Button(_),
                ..
            })) => {
                tracing::warn!(
                    "can't grab the accelerator {accelerator:?} for {owner}: a mouse button can't be grabbed yet"
                );
                return 0;
            }
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

    // Mouse buttons (ADR 0028).

    fn matches_button(accelerator: &str, mods: ModifiersState, button: u32) -> bool {
        let parsed = parse(accelerator).unwrap().unwrap();
        parsed.matches_button(&mods, button)
    }

    #[test]
    fn a_button_alone_is_xs_number_for_it_with_nothing_held() {
        assert!(matches_button("Button2", NONE, 2));
        assert!(matches_button("Button8", NONE, 8));
        assert!(!matches_button("Button8", NONE, 9));
        let ctrl = ModifiersState { ctrl: true, ..NONE };
        assert!(!matches_button("Button8", ctrl, 8));
        assert!(matches_button("Button20", NONE, 20));
    }

    #[test]
    fn a_button_takes_modifiers_in_any_order_and_case() {
        let logo = ModifiersState { logo: true, ..NONE };
        for accelerator in ["<Super>Button8", "<super>button8", "<Mod4>BUTTON8"] {
            assert!(matches_button(accelerator, logo, 8), "{accelerator}");
            assert!(!matches_button(accelerator, NONE, 8), "{accelerator}");
        }
        let ctrl_alt = ModifiersState {
            ctrl: true,
            alt: true,
            ..NONE
        };
        for accelerator in ["<Control><Alt>Button9", "<alt><CTRL>button9"] {
            assert!(matches_button(accelerator, ctrl_alt, 9), "{accelerator}");
        }
    }

    #[test]
    fn locks_dont_count_for_a_button_either() {
        let locked = ModifiersState {
            caps_lock: true,
            num_lock: true,
            ..NONE
        };
        assert!(matches_button("Button9", locked, 9));
    }

    #[test]
    fn the_left_and_right_buttons_need_a_modifier() {
        assert_eq!(parse("Button1"), Err(ParseError::BareButton(1)));
        assert_eq!(parse("Button3"), Err(ParseError::BareButton(3)));
        let alt = ModifiersState { alt: true, ..NONE };
        assert!(matches_button("<Alt>Button1", alt, 1));
        assert!(matches_button("<Alt>Button3", alt, 3));
        // Shift is one.
        let shift = ModifiersState {
            shift: true,
            ..NONE
        };
        assert!(matches_button("<Shift>Button1", shift, 1));
    }

    #[test]
    fn the_wheels_buttons_dont_parse() {
        for button in 4..=7 {
            assert_eq!(
                parse(&format!("Button{button}")),
                Err(ParseError::Wheel(button))
            );
            assert_eq!(
                parse(&format!("<Super>Button{button}")),
                Err(ParseError::Wheel(button))
            );
        }
    }

    #[test]
    fn a_button_x_has_no_number_for_doesnt_parse() {
        // No button 0, none with no number, none past the kernel's mouse buttons, and a
        // number is written as X writes it.
        for name in [
            "Button0",
            "Button",
            "Button21",
            "Button99999999999",
            "Button-1",
            "Button+8",
            "Button08",
            "Button8a",
            "Button 8",
            "Buttons",
        ] {
            assert_eq!(
                parse(&format!("<Super>{name}")),
                Err(ParseError::NoSuchButton(name.into())),
                "{name}"
            );
        }
    }

    #[test]
    fn a_button_is_no_key_and_a_key_no_button() {
        let logo = ModifiersState { logo: true, ..NONE };
        let button = parse("<Super>Button8").unwrap().unwrap();
        assert!(!button.matches(&logo, &[Keysym::_8, Keysym::Pointer_Button1]));
        let key = parse("<Super>8").unwrap().unwrap();
        assert!(!key.matches_button(&logo, 8));
    }

    #[test]
    fn a_button_is_written_back_as_it_parses() {
        for accelerator in ["Button2", "<Super>Button8", "<Control><Shift><Alt>Button12"] {
            let parsed = parse(accelerator).unwrap().unwrap();
            assert_eq!(parsed.to_string(), accelerator);
        }
        assert_eq!(
            parse("<mod4><ctl>button9").unwrap().unwrap().to_string(),
            "<Control><Super>Button9"
        );
    }

    #[test]
    fn the_kernels_button_codes_have_xs_numbers() {
        // As xf86-input-libinput and Xwayland number them.
        assert_eq!(x_button(0x110), Some(1));
        assert_eq!(x_button(0x112), Some(2));
        assert_eq!(x_button(0x111), Some(3));
        assert_eq!(x_button(0x113), Some(8));
        assert_eq!(x_button(0x114), Some(9));
        assert_eq!(x_button(0x115), Some(10));
        assert_eq!(x_button(0x117), Some(12));
        assert_eq!(x_button(0x11f), Some(20));
        // A joystick's trigger, and a key.
        assert_eq!(x_button(0x120), None);
        assert_eq!(x_button(0x10f), None);
    }

    #[test]
    fn a_button_cant_be_grabbed_before_185() {
        let mut grabs = Grabs::default();
        assert_eq!(grabs.grab(":1.5", "<Super>Button8"), 0);
        assert_eq!(grabs.iter().count(), 0);
        assert_ne!(grabs.grab(":1.5", "<Super>8"), 0);
    }
}
