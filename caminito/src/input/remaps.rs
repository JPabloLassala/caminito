//! Key remaps (CMN-186, ADR 0038), read from `org.mate.Caminito`'s `key-remaps`: the
//! combinations swapped for others, or for nothing, before the focused window's client gets
//! them, for all applications and for each Remap group of app ids.

use std::fmt;

use smithay::input::keyboard::{Keycode, Keysym, KeysymHandle, ModifiersState, xkb};

use super::bindings::Held;
use crate::settings::translate::RemapGroupValue;

/// A combination a Key remap names: modifiers among Ctrl, Alt, Shift and Super, and a key by
/// the keysym it gives without modifiers.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) struct Combination {
    pub(crate) held: Held,
    pub(crate) key: Keysym,
}

impl Combination {
    /// The combination a key's press with `mods` held is: the keysym of its key's first level
    /// in the current layout, so a rule follows the layout. A press with AltGr or the fifth
    /// level's shift held is no combination, so `AltGr+Q` never matches `Alt+Q`.
    pub(crate) fn pressed(mods: &ModifiersState, keysym: &KeysymHandle<'_>) -> Option<Self> {
        if mods.iso_level3_shift || mods.iso_level5_shift {
            return None;
        }
        let key = *keysym.raw_syms().first()?;
        Some(Self {
            held: Held::from(mods),
            key,
        })
    }

    /// Parses a combination as `key-remaps` writes one: `Alt+T`, `Ctrl+Shift+Home` or
    /// `BackSpace`, the modifiers in any order and case.
    pub(crate) fn parse(text: &str) -> Result<Self, ParseError> {
        let mut parts: Vec<&str> = text.split('+').map(str::trim).collect();
        let key = parts.pop().filter(|key| !key.is_empty());
        let key = key.ok_or(ParseError::NoKey)?;
        let mut held = Held::NOTHING;
        for part in parts {
            match part.to_ascii_lowercase().as_str() {
                "ctrl" | "control" => held.ctrl = true,
                "alt" => held.alt = true,
                "shift" => held.shift = true,
                "super" => held.logo = true,
                _ => return Err(ParseError::UnknownModifier(part.to_owned())),
            }
        }
        Ok(Self {
            held,
            key: keysym(key)?,
        })
    }

    /// The modifiers a client is sent with this combination's key: its own, with the locks
    /// `held` has.
    pub(crate) fn modifiers(&self, held: &ModifiersState) -> ModifiersState {
        ModifiersState {
            ctrl: self.held.ctrl,
            alt: self.held.alt,
            shift: self.held.shift,
            logo: self.held.logo,
            caps_lock: held.caps_lock,
            num_lock: held.num_lock,
            ..ModifiersState::default()
        }
    }
}

/// The keysym a key's name names, as its key's first level gives it: a letter is lowercase
/// there, as `T` is `t`.
fn keysym(name: &str) -> Result<Keysym, ParseError> {
    let named = |flags| Some(xkb::keysym_from_name(name, flags)).filter(|sym| sym.raw() != 0);
    let keysym = named(xkb::KEYSYM_NO_FLAGS)
        .or_else(|| named(xkb::KEYSYM_CASE_INSENSITIVE))
        .ok_or_else(|| ParseError::UnknownKey(name.to_owned()))?;
    Ok(match keysym.raw() {
        upper @ 0x41..=0x5a => Keysym::new(upper + 0x20),
        _ => keysym,
    })
}

/// Why a rule's combination isn't one Caminito can remap.
#[derive(Debug, PartialEq)]
pub(crate) enum ParseError {
    /// Nothing after the last `+`.
    NoKey,
    /// A modifier that isn't Ctrl, Alt, Shift or Super.
    UnknownModifier(String),
    /// A key that isn't a keysym's name.
    UnknownKey(String),
}

impl fmt::Display for ParseError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::NoKey => write!(f, "it names no key"),
            Self::UnknownModifier(name) => write!(f, "{name} isn't a modifier"),
            Self::UnknownKey(name) => write!(f, "{name} isn't a keysym"),
        }
    }
}

impl std::error::Error for ParseError {}

/// One rule: a combination, and the steps it's sent as. No step is nothing: the combination
/// is swallowed.
#[derive(Debug, Clone, PartialEq)]
struct Remap {
    trigger: Combination,
    steps: Vec<Combination>,
}

impl Remap {
    /// Parses a rule of `key-remaps`: its combination, and its target's steps.
    fn parse(trigger: &str, steps: &[String]) -> Result<Self, ParseError> {
        let steps = steps.iter().map(|step| Combination::parse(step));
        Ok(Self {
            trigger: Combination::parse(trigger)?,
            steps: steps.collect::<Result<_, _>>()?,
        })
    }
}

/// A Remap group: its app ids, and its rules, or `None` for a group with no remaps at all.
#[derive(Debug, Clone, PartialEq)]
struct Group {
    app_ids: Vec<String>,
    remaps: Option<Vec<Remap>>,
}

/// The Key remaps in use: All applications', and each Remap group's.
#[derive(Debug, Clone, Default, PartialEq)]
pub struct KeyRemaps {
    all: Vec<Remap>,
    groups: Vec<Group>,
}

/// A rule `key-remaps` has that isn't one Caminito can remap, which is left out.
#[derive(Debug, PartialEq)]
pub(crate) struct Unremappable {
    /// The rule as `key-remaps` writes it, `Alt+T = Ctrl+T`.
    pub(crate) rule: String,
    pub(crate) reason: ParseError,
}

impl KeyRemaps {
    /// Builds the remaps from `key-remaps`' groups, of which the first is All applications'.
    /// A rule whose combination or a step of whose target can't be parsed is left out.
    pub(crate) fn from_groups(groups: &[RemapGroupValue]) -> (Self, Vec<Unremappable>) {
        let mut unremappable = Vec::new();
        let mut read = Vec::new();
        for (_, app_ids, no_remaps, rules) in groups {
            let mut remaps = Vec::new();
            for (trigger, steps) in rules {
                match Remap::parse(trigger, steps) {
                    Ok(remap) => remaps.push(remap),
                    Err(reason) => {
                        let target = if steps.is_empty() {
                            "nothing".to_owned()
                        } else {
                            steps.join(", ")
                        };
                        let rule = format!("{trigger} = {target}");
                        unremappable.push(Unremappable { rule, reason });
                    }
                }
            }
            read.push(Group {
                app_ids: app_ids.clone(),
                remaps: (!no_remaps).then_some(remaps),
            });
        }
        let mut groups = read.into_iter();
        let all = groups.next().and_then(|all| all.remaps).unwrap_or_default();
        let remaps = Self {
            all,
            groups: groups.collect(),
        };
        (remaps, unremappable)
    }

    /// Whether there's no rule at all, so no key need be looked up.
    pub(crate) fn is_empty(&self) -> bool {
        self.all.is_empty()
            && self
                .groups
                .iter()
                .all(|group| group.remaps.as_ref().is_none_or(Vec::is_empty))
    }

    /// The steps `pressed` is sent as to a client of `app_id`, if a rule takes it: its Remap
    /// group's, then All applications'.
    pub(crate) fn steps_for(
        &self,
        app_id: Option<&str>,
        pressed: &Combination,
    ) -> Option<&[Combination]> {
        let remap = self
            .rules_for(app_id)
            .find(|remap| remap.trigger == *pressed)?;
        Some(remap.steps.as_slice())
    }

    /// Whether some rule for a client of `app_id` has a combination of `held` and more, so a
    /// modifier pressed to make `held` starts it (CMN-188).
    pub(crate) fn starts_a_rule(&self, app_id: Option<&str>, held: Held) -> bool {
        let within = |rule: Held| {
            (!held.ctrl || rule.ctrl)
                && (!held.alt || rule.alt)
                && (!held.shift || rule.shift)
                && (!held.logo || rule.logo)
        };
        self.rules_for(app_id)
            .any(|remap| within(remap.trigger.held))
    }

    /// The rules for a client of `app_id`, its Remap group's before All applications', so
    /// the first that takes a combination is the one that applies. A group with no remaps
    /// has none, and a focus with no app id has All applications'. An app id is in one group
    /// at most, and the first that lists it is its own.
    fn rules_for(&self, app_id: Option<&str>) -> impl Iterator<Item = &Remap> {
        let group = app_id.and_then(|app_id| {
            self.groups
                .iter()
                .find(|group| group.app_ids.iter().any(|id| id == app_id))
        });
        let (own, all): (&[Remap], &[Remap]) = match group.map(|group| group.remaps.as_deref()) {
            Some(None) => (&[], &[]),
            Some(Some(remaps)) => (remaps, &self.all),
            None => (&[], &self.all),
        };
        own.iter().chain(all)
    }
}

/// Whether `keysym` is a modifier's key that may start a rule, and be held back: Ctrl, Alt,
/// Shift or Super, either side's. AltGr is none of them.
pub(crate) fn is_modifier(keysym: Keysym) -> bool {
    matches!(
        keysym,
        Keysym::Control_L
            | Keysym::Control_R
            | Keysym::Alt_L
            | Keysym::Alt_R
            | Keysym::Meta_L
            | Keysym::Meta_R
            | Keysym::Shift_L
            | Keysym::Shift_R
            | Keysym::Super_L
            | Keysym::Super_R
    )
}

/// The modifiers' keys held back from the client (CMN-188, ADR 0038) because their press
/// starts a rule, so a remap never shows it a stray Alt.
#[derive(Debug, Clone)]
pub(crate) struct HeldBack {
    /// Each press held back, in the order pressed.
    presses: Vec<HeldPress>,
    /// The modifiers the client was last told of, before the first.
    client: Held,
    /// Whether a rule has taken a key since, so the keys' releases are swallowed.
    taken: bool,
}

/// A modifier's press held back, with the modifiers it gave.
#[derive(Debug, Clone, Copy)]
pub(crate) struct HeldPress {
    pub(crate) keycode: Keycode,
    pub(crate) mods: ModifiersState,
}

impl HeldBack {
    /// Nothing held back yet from a client that has `client`.
    pub(crate) fn new(client: &ModifiersState) -> Self {
        Self {
            presses: Vec::new(),
            client: Held::from(client),
            taken: false,
        }
    }

    /// Holds back `keycode`'s press, which gave `mods`.
    pub(crate) fn hold(&mut self, keycode: Keycode, mods: ModifiersState) {
        self.presses.push(HeldPress { keycode, mods });
    }

    /// A rule took a key while these are held back.
    pub(crate) fn take(&mut self) {
        self.taken = true;
    }

    /// Whether a rule took a key while these were held back.
    pub(crate) fn is_taken(&self) -> bool {
        self.taken
    }

    /// Whether `keycode`'s press is held back.
    pub(crate) fn holds(&self, keycode: Keycode) -> bool {
        self.presses.iter().any(|press| press.keycode == keycode)
    }

    /// Forgets `keycode`'s press, as it's released, and says whether none is held back now.
    pub(crate) fn forget(&mut self, keycode: Keycode) -> bool {
        self.presses.retain(|press| press.keycode != keycode);
        self.presses.is_empty()
    }

    /// The presses held back, in the order pressed.
    pub(crate) fn into_presses(self) -> Vec<HeldPress> {
        self.presses
    }

    /// The modifiers the client has while `held` are: those held back aren't among them,
    /// and a release since may have let go of one it had. Its `serialized` is `held`'s, for
    /// the caller to serialize back.
    pub(crate) fn client_modifiers(&self, held: &ModifiersState) -> ModifiersState {
        ModifiersState {
            ctrl: held.ctrl && self.client.ctrl,
            alt: held.alt && self.client.alt,
            shift: held.shift && self.client.shift,
            logo: held.logo && self.client.logo,
            ..*held
        }
    }
}

#[cfg(test)]
mod tests;
