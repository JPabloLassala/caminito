//! Key remaps (CMN-186, ADR 0038), read from `org.mate.Caminito`'s `key-remaps`: the
//! combinations swapped for others, for a sequence of others (CMN-187), or for nothing, before the focused window's client gets
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

/// The last step of a Key remap's sequence, which stays pressed while its trigger is
/// (CMN-187), so the client repeats it.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) struct HeldStep {
    /// The trigger's key, whose release lets go of the step.
    pub(crate) trigger: Keycode,
    /// The step's key, as the client was sent it.
    pub(crate) step: Keycode,
}

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
    /// group's, then All applications'. A group with no remaps takes none, and a focus with
    /// no app id has All applications'. An app id is in one group at most, and the first
    /// that lists it is its own.
    pub(crate) fn steps_for(
        &self,
        app_id: Option<&str>,
        pressed: &Combination,
    ) -> Option<&[Combination]> {
        fn find<'a>(remaps: &'a [Remap], pressed: &Combination) -> Option<&'a [Combination]> {
            let remap = remaps.iter().find(|remap| remap.trigger == *pressed)?;
            Some(remap.steps.as_slice())
        }
        let group = app_id.and_then(|app_id| {
            self.groups
                .iter()
                .find(|group| group.app_ids.iter().any(|id| id == app_id))
        });
        match group.map(|group| group.remaps.as_deref()) {
            Some(None) => None,
            Some(Some(remaps)) => find(remaps, pressed).or_else(|| find(&self.all, pressed)),
            None => find(&self.all, pressed),
        }
    }
}

#[cfg(test)]
mod tests;
