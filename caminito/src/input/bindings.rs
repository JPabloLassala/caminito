//! Caminito's keybindings, read from `org.mate.Marco.global-keybindings` and
//! `org.mate.Marco.window-keybindings` as Marco reads them (148). The accelerators other
//! programs grab are `accelerators`' (98).

pub(crate) mod accelerators;

use std::fmt;

use caminito_policy::{Action, SwitchList};
use smithay::input::keyboard::{Keysym, ModifiersState, xkb};

use self::accelerators::{Accelerator, parse};
use crate::commands::Command;
use crate::dbus::panel::PanelAction;
use crate::settings::translate::{MARCO_GLOBAL_KEYBINDINGS, MARCO_WINDOW_KEYBINDINGS, Problem};

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

/// What a binding does.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum Binding {
    /// Something policy does.
    Policy(Action),
    /// Something Caminito asks the panel for (95).
    Panel(PanelAction),
    /// A command to run (104).
    Command(Command),
}

/// What an action Caminito binds does when its combination is pressed.
#[derive(Debug, Clone, Copy)]
enum Does {
    Run(Binding),
    /// A `switch-*` action (108), which Marco flags `BINDING_REVERSES`: Shift turns it
    /// round, and a `-backward` one is `reversed` to start with.
    Switch {
        list: SwitchList,
        reversed: bool,
    },
}

/// An action Caminito binds: its key, Marco's default for it, and what it does.
struct Bindable {
    schema: &'static str,
    key: &'static str,
    default: &'static str,
    does: Does,
}

const fn global(key: &'static str, default: &'static str, does: Does) -> Bindable {
    Bindable {
        schema: MARCO_GLOBAL_KEYBINDINGS,
        key,
        default,
        does,
    }
}

const fn window(key: &'static str, default: &'static str, does: Does) -> Bindable {
    Bindable {
        schema: MARCO_WINDOW_KEYBINDINGS,
        key,
        default,
        does,
    }
}

const fn policy(action: Action) -> Does {
    Does::Run(Binding::Policy(action))
}

const fn switch(list: SwitchList, reversed: bool) -> Does {
    Does::Switch { list, reversed }
}

/// The actions that have a behaviour, in Marco's order (`src/include/all-keybindings.h`
/// at 1.29.0, the global actions before the window ones), which is the order its
/// `process_event` tries them in (`core/keybindings.c:1203-1260`), so of two keys with one
/// combination the first wins. Every other action's key is read and parsed, and binds
/// nothing until its ticket gives it a behaviour (147). `run-command-area-screenshot` is
/// master's (`8522ed1`), after the window screenshot as there; where the installed schema
/// hasn't the key, it has master's default.
const BINDABLE: [Bindable; 18] = [
    global(
        "switch-to-workspace-left",
        "<Control><Alt>Left",
        policy(Action::SwitchToWorkspaceLeft),
    ),
    global(
        "switch-to-workspace-right",
        "<Control><Alt>Right",
        policy(Action::SwitchToWorkspaceRight),
    ),
    global(
        "switch-group",
        "<Alt>grave",
        switch(SwitchList::Group, false),
    ),
    global(
        "switch-group-backward",
        "disabled",
        switch(SwitchList::Group, true),
    ),
    global(
        "switch-windows",
        "<Alt>Tab",
        switch(SwitchList::Windows, false),
    ),
    global(
        "switch-windows-backward",
        "disabled",
        switch(SwitchList::Windows, true),
    ),
    global(
        "switch-windows-all",
        "disabled",
        switch(SwitchList::WindowsAll, false),
    ),
    global(
        "switch-windows-all-backward",
        "disabled",
        switch(SwitchList::WindowsAll, true),
    ),
    global(
        "switch-panels",
        "<Control><Alt>Tab",
        switch(SwitchList::Panels, false),
    ),
    global(
        "switch-panels-backward",
        "disabled",
        switch(SwitchList::Panels, true),
    ),
    global(
        "show-desktop",
        "<Control><Alt>d",
        policy(Action::ShowDesktop),
    ),
    global(
        "panel-main-menu",
        "<Alt>F1",
        Does::Run(Binding::Panel(PanelAction::MainMenu)),
    ),
    global(
        "panel-run-dialog",
        "<Alt>F2",
        Does::Run(Binding::Panel(PanelAction::RunDialog)),
    ),
    global(
        "run-command-screenshot",
        "Print",
        Does::Run(Binding::Command(Command::Screenshot)),
    ),
    global(
        "run-command-window-screenshot",
        "<Alt>Print",
        Does::Run(Binding::Command(Command::WindowScreenshot)),
    ),
    global(
        "run-command-area-screenshot",
        "<Shift>Print",
        Does::Run(Binding::Command(Command::AreaScreenshot)),
    ),
    window(
        "toggle-maximized",
        "<Alt>F10",
        policy(Action::ToggleMaximized),
    ),
    window("close", "<Alt>F4", policy(Action::Close)),
];

/// One combination in force, and the key it came from.
#[derive(Debug, Clone, PartialEq)]
struct Row {
    key: &'static str,
    combination: Accelerator,
    binding: Binding,
}

/// The bindings in force, which the keyboard path's bindings stage matches a press
/// against. The settings thread builds it from the keys, and a change to one replaces it
/// whole. A press that ran a binding has its release swallowed whatever the table is by
/// then, so it ends under the table it started under.
#[derive(Debug, Clone, PartialEq)]
pub struct Bindings {
    /// In `BINDABLE`'s order, each Shift variant right after its combination.
    rows: Vec<Row>,
}

impl Default for Bindings {
    /// Marco's defaults.
    fn default() -> Self {
        Self::from_keys([]).0
    }
}

impl Bindings {
    /// Builds the table from the keys' values, as `(schema, key, value)`: every key of
    /// both schemas, whether Caminito binds its action or not, so every value is parsed.
    /// An action Caminito binds whose key `keys` hasn't has Marco's default. A value that
    /// can't be bound binds nothing, and is a problem.
    pub(crate) fn from_keys<'a>(
        keys: impl IntoIterator<Item = (&'a str, &'a str, &'a str)>,
    ) -> (Self, Vec<Problem>) {
        let keys: Vec<(&str, &str, &str)> = keys.into_iter().collect();
        // Every value parsed, in Marco's way (`meta_ui_parse_accelerator`): `disabled` and
        // the empty string are no combination, and a value that doesn't parse binds nothing.
        let mut problems = Vec::new();
        for (schema, key, value) in &keys {
            if let Err(err) = parse(value) {
                problems.push(unbindable(schema, key, value, &err.to_string()));
            }
        }
        let mut rows = Vec::new();
        for bindable in &BINDABLE {
            let value = keys
                .iter()
                .find(|(schema, key, _)| *schema == bindable.schema && *key == bindable.key)
                .map_or(bindable.default, |(_, _, value)| value);
            let Ok(Some(combination)) = parse(value) else {
                continue;
            };
            let row = |combination, binding| Row {
                key: bindable.key,
                combination,
                binding,
            };
            let (list, reversed) = match bindable.does {
                Does::Run(binding) => {
                    rows.push(row(combination, binding));
                    continue;
                }
                Does::Switch { list, reversed } => (list, reversed),
            };
            let held = combination.held;
            // Marco refuses a binding Shift reverses that holds nothing but Shift
            // (`update_binding`, `core/prefs.c:1902-1950`), which would take the key from
            // every client. It writes the old value back; Caminito binds nothing.
            if !(held.ctrl || held.alt || held.logo) {
                problems.push(unbindable(
                    bindable.schema,
                    bindable.key,
                    value,
                    "a binding Shift reverses needs a modifier such as Ctrl or Alt",
                ));
                continue;
            }
            // `do_choose_window` turns the action round when Shift is down
            // (`core/keybindings.c:3036-3038`), so a combination with Shift in it steps the
            // other way.
            let switch = |shift: bool| {
                let backward = reversed != shift;
                Binding::Policy(Action::Switch { list, backward })
            };
            rows.push(row(combination, switch(held.shift)));
            // And `rebuild_binding_table` binds it with Shift too, right after it
            // (`core/keybindings.c:403-420`).
            if !held.shift {
                let held = Held {
                    shift: true,
                    ..held
                };
                rows.push(row(
                    Accelerator {
                        held,
                        ..combination
                    },
                    switch(true),
                ));
            }
        }
        (Self { rows }, problems)
    }

    /// What's bound to a press with exactly `mods` held, where `keysyms` name the key
    /// pressed as they do for the accelerator grabs (100). The first row that matches wins.
    pub(crate) fn binding_for(&self, mods: &ModifiersState, keysyms: &[Keysym]) -> Option<Binding> {
        self.rows
            .iter()
            .find(|row| row.combination.matches(mods, keysyms))
            .map(|row| row.binding)
    }

    /// Each combination in force, in GTK's syntax, with the key it came from.
    pub(crate) fn iter(&self) -> impl Iterator<Item = (&'static str, String)> + '_ {
        self.rows
            .iter()
            .map(|row| (row.key, row.combination.to_string()))
    }
}

fn unbindable(schema: &str, key: &str, value: &str, reason: &str) -> Problem {
    Problem::Unbindable {
        schema: schema.into(),
        key: key.into(),
        value: format!("'{value}'"),
        reason: reason.into(),
    }
}

impl fmt::Display for Accelerator {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        let held = self.held;
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
        f.write_str(&xkb::keysym_get_name(self.keysym))
    }
}

#[cfg(test)]
mod tests;
