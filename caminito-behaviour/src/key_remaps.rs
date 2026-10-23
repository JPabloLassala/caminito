//! The Key remaps page's keys (CMN-189, ADR 0038): `org.mate.Caminito`'s `key-remaps`
//! (CMN-186), which Caminito follows as it changes, and `key-remap-template`, the Remap
//! template they were last loaded from or saved as. What a rule is, and what an app id gets,
//! are here too, so the page only draws them.

use std::fmt;

use gio::prelude::*;

#[cfg(test)]
mod tests;

/// The Key remaps' key.
const KEY_REMAPS: &str = "key-remaps";
/// The Remap template's key.
const KEY_REMAP_TEMPLATE: &str = "key-remap-template";

/// `key-remaps`' value: each Remap group's name, app ids, whether it has no remaps, and its
/// rules, each a combination and its target's steps.
type Value = Vec<(String, Vec<String>, bool, Vec<(String, Vec<String>)>)>;

/// A combination: modifiers among Ctrl, Alt, Shift and Super, and a key by its keysym's name,
/// as `key-remaps` writes one: `Alt+T`, `Ctrl+Shift+Home`.
#[derive(Debug, Clone, Eq)]
pub struct Combination {
    pub ctrl: bool,
    pub alt: bool,
    pub shift: bool,
    pub super_: bool,
    /// The keysym's name, a letter in capitals.
    pub key: String,
}

impl Combination {
    /// The combination of `key` with those modifiers held.
    pub fn new(ctrl: bool, alt: bool, shift: bool, super_: bool, key: &str) -> Self {
        Combination {
            ctrl,
            alt,
            shift,
            super_,
            key: key_name(key),
        }
    }

    /// Parses a combination: the modifiers in any order and case, then the key.
    pub fn parse(text: &str) -> Result<Self, ParseError> {
        let mut parts: Vec<&str> = text.split('+').map(str::trim).collect();
        let key = parts.pop().filter(|key| !key.is_empty());
        let key = key.ok_or_else(|| ParseError::NoKey(text.trim().to_owned()))?;
        if key.contains(char::is_whitespace) {
            return Err(ParseError::UnknownKey(key.to_owned()));
        }
        let mut combination = Combination::new(false, false, false, false, key);
        for part in parts {
            match part.to_ascii_lowercase().as_str() {
                "ctrl" | "control" => combination.ctrl = true,
                "alt" => combination.alt = true,
                "shift" => combination.shift = true,
                "super" => combination.super_ = true,
                _ => return Err(ParseError::UnknownModifier(part.to_owned())),
            }
        }
        Ok(combination)
    }

    /// Whether a modifier is held in it, so that pressing that modifier may start it.
    pub fn has_modifier(&self) -> bool {
        self.ctrl || self.alt || self.shift || self.super_
    }
}

/// A keysym's name as the page shows it: a letter in capitals, as `Alt+T`. Caminito matches
/// a letter in either case.
fn key_name(key: &str) -> String {
    let mut chars = key.chars();
    match (chars.next(), chars.next()) {
        (Some(letter), None) => letter.to_ascii_uppercase().to_string(),
        _ => key.to_owned(),
    }
}

/// Two combinations are one if their keys' names differ only in case, as xkb finds a keysym
/// by its name in either case.
impl PartialEq for Combination {
    fn eq(&self, other: &Self) -> bool {
        (self.ctrl, self.alt, self.shift, self.super_)
            == (other.ctrl, other.alt, other.shift, other.super_)
            && self.key.eq_ignore_ascii_case(&other.key)
    }
}

impl fmt::Display for Combination {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        let held = [
            (self.ctrl, "Ctrl"),
            (self.alt, "Alt"),
            (self.shift, "Shift"),
            (self.super_, "Super"),
        ];
        for (_, name) in held.iter().filter(|(on, _)| *on) {
            write!(f, "{name}+")?;
        }
        f.write_str(&self.key)
    }
}

/// Why a combination or a target can't be read.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum ParseError {
    /// Nothing after the last `+`, in the text given.
    NoKey(String),
    /// A modifier that isn't Ctrl, Alt, Shift or Super.
    UnknownModifier(String),
    /// A key's name with a space in it, which no keysym has.
    UnknownKey(String),
}

impl fmt::Display for ParseError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            ParseError::NoKey(text) => write!(f, "“{text}” names no key"),
            ParseError::UnknownModifier(name) => write!(f, "{name} isn't a modifier"),
            ParseError::UnknownKey(name) => write!(f, "“{name}” isn't a key's name"),
        }
    }
}

impl std::error::Error for ParseError {}

/// Parses a target as the user types one: `Ctrl+T`, a sequence `Shift+Home, BackSpace`, or
/// `nothing`, for none.
pub fn parse_target(text: &str) -> Result<Vec<Combination>, ParseError> {
    let text = text.trim();
    if text.eq_ignore_ascii_case("nothing") {
        return Ok(Vec::new());
    }
    text.split(',').map(Combination::parse).collect()
}

/// A target as [`parse_target`] reads it.
pub fn target_text(steps: &[Combination]) -> String {
    if steps.is_empty() {
        return "nothing".to_owned();
    }
    let steps: Vec<String> = steps.iter().map(Combination::to_string).collect();
    steps.join(", ")
}

/// One rule: a combination, and the steps it's sent as, or none for nothing.
#[derive(Debug, Clone, PartialEq)]
pub struct Rule {
    pub combination: Combination,
    pub steps: Vec<Combination>,
}

/// A Remap group: a named list of app ids that share their Key remaps.
#[derive(Debug, Clone, PartialEq)]
pub struct Group {
    pub name: String,
    pub app_ids: Vec<String>,
    /// Whether the group has no remaps at all, whatever its rules and All applications' are.
    pub no_remaps: bool,
    pub rules: Vec<Rule>,
}

/// The Key remaps: All applications' rules, and each Remap group.
#[derive(Debug, Clone, Default, PartialEq)]
pub struct Remaps {
    pub all: Vec<Rule>,
    pub groups: Vec<Group>,
}

impl Remaps {
    /// The remaps of `key-remaps`' value, whose first group is All applications. A rule that
    /// can't be read is left out, as Caminito leaves it out.
    fn from_value(value: Value) -> Self {
        let read = |rules: Vec<(String, Vec<String>)>| -> Vec<Rule> {
            rules
                .into_iter()
                .filter_map(|(combination, steps)| {
                    let steps = steps.iter().map(|step| Combination::parse(step));
                    Some(Rule {
                        combination: Combination::parse(&combination).ok()?,
                        steps: steps.collect::<Result<_, _>>().ok()?,
                    })
                })
                .collect()
        };
        let mut groups = value.into_iter();
        let all = groups
            .next()
            .map(|(.., rules)| read(rules))
            .unwrap_or_default();
        let groups = groups
            .map(|(name, app_ids, no_remaps, rules)| Group {
                name,
                app_ids,
                no_remaps,
                rules: read(rules),
            })
            .collect();
        Remaps { all, groups }
    }

    /// `key-remaps`' value of the remaps.
    fn to_value(&self) -> Value {
        let write = |rules: &[Rule]| -> Vec<(String, Vec<String>)> {
            rules
                .iter()
                .map(|rule| {
                    let steps = rule.steps.iter().map(Combination::to_string).collect();
                    (rule.combination.to_string(), steps)
                })
                .collect()
        };
        let all = (String::new(), Vec::new(), false, write(&self.all));
        let groups = self.groups.iter().map(|group| {
            (
                group.name.clone(),
                group.app_ids.clone(),
                group.no_remaps,
                write(&group.rules),
            )
        });
        std::iter::once(all).chain(groups).collect()
    }

    /// The group `app_id` is in, by its place in [`groups`](Self::groups). An app id is in
    /// one group at most, and Caminito takes the first that lists it.
    pub fn group_of(&self, app_id: &str) -> Option<usize> {
        self.groups
            .iter()
            .position(|group| group.app_ids.iter().any(|id| id == app_id))
    }

    /// The rules a client of `app_id` gets, by combination, each with where it comes from:
    /// its group's, then All applications' for a combination the group has none for. A
    /// group with no remaps gets none, and no app id (the panel, caja's desktop) gets All
    /// applications'.
    pub fn in_effect(&self, app_id: Option<&str>) -> Vec<InEffect> {
        let group = app_id.and_then(|app_id| self.group_of(app_id));
        let own = match group {
            Some(index) if self.groups[index].no_remaps => return Vec::new(),
            Some(index) => self.groups[index].rules.as_slice(),
            None => &[],
        };
        let all_for = |combination: &Combination| {
            let rule = self
                .all
                .iter()
                .find(|rule| rule.combination == *combination);
            rule.map(|rule| rule.steps.clone())
        };
        let mut rows: Vec<InEffect> = own
            .iter()
            .map(|rule| InEffect {
                rule: rule.clone(),
                source: Source::Group(group.expect("a group's own rules are a group's")),
                replaces: all_for(&rule.combination),
            })
            .collect();
        for rule in &self.all {
            if !own.iter().any(|each| each.combination == rule.combination) {
                rows.push(InEffect {
                    rule: rule.clone(),
                    source: Source::AllApplications,
                    replaces: None,
                });
            }
        }
        rows.sort_by_key(|row| row.rule.combination.to_string());
        rows
    }

    /// The rules of `scope`.
    pub fn rules(&self, scope: Scope) -> &[Rule] {
        match scope {
            Scope::AllApplications => &self.all,
            Scope::Group(index) => &self.groups[index].rules,
        }
    }

    fn rules_mut(&mut self, scope: Scope) -> &mut Vec<Rule> {
        match scope {
            Scope::AllApplications => &mut self.all,
            Scope::Group(index) => &mut self.groups[index].rules,
        }
    }

    /// Adds `rule` to `scope`, in place of the rule of its combination there, if it has one.
    pub fn set_rule(&mut self, scope: Scope, rule: Rule) {
        let rules = self.rules_mut(scope);
        match rules
            .iter_mut()
            .find(|each| each.combination == rule.combination)
        {
            Some(each) => *each = rule,
            None => rules.push(rule),
        }
    }

    /// Removes the rule of `combination` from `scope`.
    pub fn remove_rule(&mut self, scope: Scope, combination: &Combination) {
        self.rules_mut(scope)
            .retain(|rule| rule.combination != *combination);
    }

    /// Copies All applications' rule of `combination` into the group at `index`, where the
    /// page then lets it be changed: the page's "Change for <group>".
    pub fn change_for(&mut self, index: usize, combination: &Combination) {
        let rule = self
            .all
            .iter()
            .find(|rule| rule.combination == *combination);
        if let Some(rule) = rule.cloned() {
            self.set_rule(Scope::Group(index), rule);
        }
    }

    /// Turns the group at `index`'s remaps off, or on again.
    pub fn set_no_remaps(&mut self, index: usize, no_remaps: bool) {
        self.groups[index].no_remaps = no_remaps;
    }

    /// Adds an empty group named `name`, and returns its place.
    pub fn add_group(&mut self, name: &str) -> usize {
        self.groups.push(Group {
            name: name.to_owned(),
            app_ids: Vec::new(),
            no_remaps: false,
            rules: Vec::new(),
        });
        self.groups.len() - 1
    }

    /// Puts `app_id` in the group at `to`, or in none, taking it out of every other group so
    /// it stays in one at most.
    pub fn move_app_id(&mut self, app_id: &str, to: Option<usize>) {
        for group in &mut self.groups {
            group.app_ids.retain(|id| id != app_id);
        }
        if let Some(index) = to {
            self.groups[index].app_ids.push(app_id.to_owned());
        }
    }
}

/// Where a rule's list is: All applications', or a group's by its place.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Scope {
    AllApplications,
    Group(usize),
}

/// Where a rule in effect for an app id comes from.
pub type Source = Scope;

/// A rule in effect for an app id.
#[derive(Debug, Clone, PartialEq)]
pub struct InEffect {
    pub rule: Rule,
    pub source: Source,
    /// The steps of All applications' rule a group's rule replaces, if it replaces one.
    pub replaces: Option<Vec<Combination>>,
}

/// Why the keys can't be opened, or written.
#[derive(Debug)]
pub enum Error {
    /// `org.mate.Caminito` isn't installed, or isn't compiled.
    NoSchema,
    /// The installed `org.mate.Caminito` is older than the Key remaps.
    NoKeys,
    /// Its key is locked down, or the backend failed.
    Key(&'static str, gio::glib::BoolError),
}

impl fmt::Display for Error {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Error::NoSchema => write!(f, "the schema {} isn't installed", crate::SCHEMA),
            Error::NoKeys => write!(f, "the schema {} has no key remaps", crate::SCHEMA),
            Error::Key(key, err) => write!(f, "can't write {key}: {err}"),
        }
    }
}

impl std::error::Error for Error {}

/// `key-remaps` and `key-remap-template`.
pub struct KeyRemapKeys {
    settings: gio::Settings,
}

impl KeyRemapKeys {
    /// The keys of `settings`, which are `org.mate.Caminito`'s.
    pub fn new(settings: gio::Settings) -> Self {
        KeyRemapKeys { settings }
    }

    /// Opens the installed `org.mate.Caminito`, which GSettings would abort on reading a key
    /// it hasn't.
    pub fn open() -> Result<Self, Error> {
        let settings = crate::installed().ok_or(Error::NoSchema)?;
        let schema = settings
            .settings_schema()
            .expect("a Settings made from a schema has one");
        if !(schema.has_key(KEY_REMAPS) && schema.has_key(KEY_REMAP_TEMPLATE)) {
            return Err(Error::NoKeys);
        }
        Ok(KeyRemapKeys::new(settings))
    }

    /// The Key remaps in use.
    pub fn remaps(&self) -> Remaps {
        let value = self.settings.value(KEY_REMAPS).get::<Value>();
        Remaps::from_value(value.expect("the schema types key-remaps a(sasba(sas))"))
    }

    /// Writes `remaps` at once: there's no Apply.
    pub fn set_remaps(&self, remaps: &Remaps) -> Result<(), Error> {
        self.settings
            .set_value(KEY_REMAPS, &remaps.to_value().to_variant())
            .map_err(|err| Error::Key(KEY_REMAPS, err))
    }

    /// The name of the Remap template the remaps were last loaded from or saved as, or `""`.
    pub fn template(&self) -> String {
        self.settings.string(KEY_REMAP_TEMPLATE).into()
    }

    /// Writes the Remap template's name.
    pub fn set_template(&self, name: &str) -> Result<(), Error> {
        self.settings
            .set_string(KEY_REMAP_TEMPLATE, name)
            .map_err(|err| Error::Key(KEY_REMAP_TEMPLATE, err))
    }

    /// Calls `f` each time either key changes, as by `gsettings` or by this.
    pub fn connect_changed(&self, f: impl Fn() + 'static) {
        self.settings.connect_changed(None, move |_, key| {
            if key == KEY_REMAPS || key == KEY_REMAP_TEMPLATE {
                f();
            }
        });
    }
}
