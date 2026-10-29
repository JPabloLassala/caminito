//! Caminito's bindings, read from `org.mate.Marco.global-keybindings` and
//! `org.mate.Marco.window-keybindings` as Marco reads them (148), and with a mouse button
//! where Marco reads none (184, ADR 0028). The Overview's, which Marco has no action
//! for, is `org.mate.Caminito`'s (241), as are the Notification center's and Do Not
//! Disturb's (CMN-263). The accelerators other programs grab are `accelerators`' (98), and
//! the modifier a click on a window is held with is `button_modifier`'s (162).

pub(crate) mod accelerators;
pub(crate) mod button_modifier;

use std::fmt;

use caminito_policy::{Action, Anchor, Direction, SwitchList, Tile};
use smithay::input::keyboard::{Keysym, ModifiersState, xkb};

use self::accelerators::{Accelerator, Trigger, parse};
use crate::commands::Command;
use crate::dbus::notifications_daemon::NotificationsAction;
use crate::dbus::panel::PanelAction;
use crate::settings::translate::{
    CAMINITO, MARCO_GLOBAL_KEYBINDINGS, MARCO_WINDOW_KEYBINDINGS, Problem,
};
use crate::window_menu::labels;

/// The modifiers a binding or a Key remap names. Locks don't count.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) struct Held {
    pub(crate) ctrl: bool,
    pub(crate) alt: bool,
    pub(crate) shift: bool,
    pub(crate) logo: bool,
}

impl Held {
    /// No modifier.
    pub(crate) const NOTHING: Self = Self {
        ctrl: false,
        alt: false,
        shift: false,
        logo: false,
    };
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
    /// Something Caminito asks caminito-notifications for (CMN-263).
    Notifications(NotificationsAction),
    /// A command to run (104).
    Command(Command),
    /// `rename-workspace`'s dialog (167).
    RenameWorkspace,
}

/// What an action Caminito binds does when its combination is pressed.
#[derive(Debug, Clone, Copy)]
enum Does {
    Run(Binding),
    /// A `switch-*` action (108), or with no `popup` a `cycle-*` one (163), which Marco
    /// flags `BINDING_REVERSES`: Shift turns it round, and a `-backward` one is `reversed`
    /// to start with.
    Switch {
        list: SwitchList,
        reversed: bool,
        popup: bool,
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

/// `org.mate.Caminito`'s key for the Overview's shortcut (241).
pub(crate) const OVERVIEW_KEY: &str = "overview";

/// `org.mate.Caminito`'s key for the Application overview's shortcut (CMN-185).
pub(crate) const APPLICATION_OVERVIEW_KEY: &str = "application-overview";

/// `org.mate.Caminito`'s key for the Notification center's shortcut (CMN-263).
pub(crate) const NOTIFICATION_CENTER_KEY: &str = "notification-center";

/// `org.mate.Caminito`'s key for Do Not Disturb's shortcut (CMN-263).
pub(crate) const TOGGLE_DO_NOT_DISTURB_KEY: &str = "toggle-do-not-disturb";

/// `org.mate.Caminito`'s keys that are bindings.
pub(crate) const CAMINITO_KEYS: [&str; 4] = [
    OVERVIEW_KEY,
    APPLICATION_OVERVIEW_KEY,
    NOTIFICATION_CENTER_KEY,
    TOGGLE_DO_NOT_DISTURB_KEY,
];

/// An action Marco hasn't, whose key is `org.mate.Caminito`'s.
const fn caminito(key: &'static str, default: &'static str, does: Does) -> Bindable {
    Bindable {
        schema: CAMINITO,
        key,
        default,
        does,
    }
}

const fn policy(action: Action) -> Does {
    Does::Run(Binding::Policy(action))
}

const fn switch(list: SwitchList, reversed: bool) -> Does {
    Does::Switch {
        list,
        reversed,
        popup: true,
    }
}

const fn cycle(list: SwitchList, reversed: bool) -> Does {
    Does::Switch {
        list,
        reversed,
        popup: false,
    }
}

/// The keys of a family numbered 1 to 12, such as `switch-to-workspace-1` to `-12`.
macro_rules! twelve {
    ($prefix:literal) => {
        [
            concat!($prefix, 1),
            concat!($prefix, 2),
            concat!($prefix, 3),
            concat!($prefix, 4),
            concat!($prefix, 5),
            concat!($prefix, 6),
            concat!($prefix, 7),
            concat!($prefix, 8),
            concat!($prefix, 9),
            concat!($prefix, 10),
            concat!($prefix, 11),
            concat!($prefix, 12),
        ]
    };
}

/// Each key of a family numbered from 1, with what its number does. Marco's default for
/// every one is `disabled`.
fn numbered(
    keys: [&'static str; 12],
    make: fn(&'static str, &'static str, Does) -> Bindable,
    does: impl Fn(usize) -> Does,
) -> impl Iterator<Item = Bindable> {
    keys.into_iter()
        .enumerate()
        .map(move |(i, key)| make(key, "disabled", does(i + 1)))
}

const fn command(command: Command) -> Does {
    Does::Run(Binding::Command(command))
}

/// The actions that have a behaviour, in Marco's order (`src/include/all-keybindings.h`
/// at 1.29.0, the global actions before the window ones), which is the order its
/// `process_event` tries them in (`core/keybindings.c:1203-1260`), so of two keys with one
/// combination the first wins. The moves to the workspace beside are among the global
/// ones there, after the switches, though their keys are in `window-keybindings`;
/// `switch-to-workspace-prev` is last (150). Every other action's key is read and parsed,
/// and binds nothing until its ticket gives it a behaviour (147).
/// `run-command-area-screenshot` is master's (`8522ed1`), after the window screenshot as
/// there; where the installed schema hasn't the key, it has master's default. The
/// Overview's (241), which isn't Marco's, is after them all, so a combination of Marco's
/// wins over it, and so are the Notification center's and Do Not Disturb's (CMN-263).
fn bindable() -> impl Iterator<Item = Bindable> {
    let switch_to_workspace = numbered(twelve!("switch-to-workspace-"), global, |n| {
        policy(Action::SwitchToWorkspace(n - 1))
    });
    let run_command = numbered(twelve!("run-command-"), global, |n| {
        command(Command::Numbered(n))
    });
    let move_to_workspace = numbered(twelve!("move-to-workspace-"), window, |n| {
        policy(Action::MoveToWorkspace(n - 1))
    });
    switch_to_workspace
        .chain([
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
                "switch-to-workspace-up",
                "<Control><Alt>Up",
                policy(Action::SwitchToWorkspaceUp),
            ),
            global(
                "switch-to-workspace-down",
                "<Control><Alt>Down",
                policy(Action::SwitchToWorkspaceDown),
            ),
            window(
                "move-to-workspace-left",
                "<Control><Shift><Alt>Left",
                policy(Action::MoveToWorkspaceLeft),
            ),
            window(
                "move-to-workspace-right",
                "<Control><Shift><Alt>Right",
                policy(Action::MoveToWorkspaceRight),
            ),
            window(
                "move-to-workspace-up",
                "<Control><Shift><Alt>Up",
                policy(Action::MoveToWorkspaceUp),
            ),
            window(
                "move-to-workspace-down",
                "<Control><Shift><Alt>Down",
                policy(Action::MoveToWorkspaceDown),
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
            global("cycle-group", "<Alt>F6", cycle(SwitchList::Group, false)),
            global(
                "cycle-group-backward",
                "disabled",
                cycle(SwitchList::Group, true),
            ),
            global(
                "cycle-windows",
                "<Alt>Escape",
                cycle(SwitchList::Windows, false),
            ),
            global(
                "cycle-windows-backward",
                "disabled",
                cycle(SwitchList::Windows, true),
            ),
            global(
                "cycle-panels",
                "<Control><Alt>Escape",
                cycle(SwitchList::Panels, false),
            ),
            global(
                "cycle-panels-backward",
                "disabled",
                cycle(SwitchList::Panels, true),
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
        ])
        .chain(run_command)
        .chain([
            global(
                "run-command-screenshot",
                "Print",
                command(Command::Screenshot),
            ),
            global(
                "run-command-window-screenshot",
                "<Alt>Print",
                command(Command::WindowScreenshot),
            ),
            global(
                "run-command-area-screenshot",
                "<Shift>Print",
                command(Command::AreaScreenshot),
            ),
            global(
                "run-command-terminal",
                "disabled",
                command(Command::Terminal),
            ),
            global(
                "rename-workspace",
                "disabled",
                Does::Run(Binding::RenameWorkspace),
            ),
            window(
                "activate-window-menu",
                "<Alt>space",
                policy(Action::ActivateWindowMenu),
            ),
            window(
                "toggle-fullscreen",
                "disabled",
                policy(Action::ToggleFullscreen),
            ),
            window(
                "toggle-maximized",
                "<Alt>F10",
                policy(Action::ToggleMaximized),
            ),
            window("toggle-above", "disabled", policy(Action::ToggleAbove)),
            window("maximize", "disabled", policy(Action::Maximize)),
            window("unmaximize", "<Alt>F5", policy(Action::Unmaximize)),
            window("toggle-shaded", "disabled", policy(Action::ToggleShaded)),
            window("minimize", "<Alt>F9", policy(Action::Minimize)),
            window("close", "<Alt>F4", policy(Action::Close)),
            window("begin-move", "<Alt>F7", policy(Action::BeginMove)),
            window("begin-resize", "<Alt>F8", policy(Action::BeginResize)),
            window(
                "toggle-on-all-workspaces",
                "disabled",
                policy(Action::ToggleOnAllWorkspaces),
            ),
        ])
        .chain(move_to_workspace)
        .chain([
            window("raise-or-lower", "disabled", policy(Action::RaiseOrLower)),
            window("raise", "disabled", policy(Action::Raise)),
            window("lower", "disabled", policy(Action::Lower)),
            window(
                "maximize-vertically",
                "disabled",
                policy(Action::ToggleMaximizedVertically),
            ),
            window(
                "maximize-horizontally",
                "disabled",
                policy(Action::ToggleMaximizedHorizontally),
            ),
            window(
                "tile-to-side-e",
                "disabled",
                policy(Action::Tile(Tile::Right)),
            ),
            window(
                "tile-to-side-w",
                "disabled",
                policy(Action::Tile(Tile::Left)),
            ),
            window(
                "tile-to-corner-nw",
                "disabled",
                policy(Action::Tile(Tile::TopLeft)),
            ),
            window(
                "tile-to-corner-ne",
                "disabled",
                policy(Action::Tile(Tile::TopRight)),
            ),
            window(
                "tile-to-corner-se",
                "disabled",
                policy(Action::Tile(Tile::BottomRight)),
            ),
            window(
                "tile-to-corner-sw",
                "disabled",
                policy(Action::Tile(Tile::BottomLeft)),
            ),
            window(
                "move-to-corner-nw",
                "disabled",
                policy(Action::MoveTo(Anchor::NorthWest)),
            ),
            window(
                "move-to-corner-ne",
                "disabled",
                policy(Action::MoveTo(Anchor::NorthEast)),
            ),
            window(
                "move-to-corner-sw",
                "disabled",
                policy(Action::MoveTo(Anchor::SouthWest)),
            ),
            window(
                "move-to-corner-se",
                "disabled",
                policy(Action::MoveTo(Anchor::SouthEast)),
            ),
            window(
                "move-to-side-n",
                "disabled",
                policy(Action::MoveTo(Anchor::North)),
            ),
            window(
                "move-to-side-s",
                "disabled",
                policy(Action::MoveTo(Anchor::South)),
            ),
            window(
                "move-to-side-e",
                "disabled",
                policy(Action::MoveTo(Anchor::East)),
            ),
            window(
                "move-to-side-w",
                "disabled",
                policy(Action::MoveTo(Anchor::West)),
            ),
            window(
                "move-to-center",
                "disabled",
                policy(Action::MoveTo(Anchor::Centre)),
            ),
            window(
                "move-to-monitor-n",
                "disabled",
                policy(Action::MoveToMonitor(Direction::Up)),
            ),
            window(
                "move-to-monitor-s",
                "disabled",
                policy(Action::MoveToMonitor(Direction::Down)),
            ),
            window(
                "move-to-monitor-e",
                "disabled",
                policy(Action::MoveToMonitor(Direction::Right)),
            ),
            window(
                "move-to-monitor-w",
                "disabled",
                policy(Action::MoveToMonitor(Direction::Left)),
            ),
            global(
                "switch-to-workspace-prev",
                "disabled",
                policy(Action::SwitchToWorkspacePrev),
            ),
            caminito(OVERVIEW_KEY, "<Super>w", policy(Action::Overview)),
            caminito(
                APPLICATION_OVERVIEW_KEY,
                "disabled",
                policy(Action::ApplicationOverview),
            ),
            caminito(
                NOTIFICATION_CENTER_KEY,
                "<Super>n",
                Does::Run(Binding::Notifications(NotificationsAction::OpenCenter)),
            ),
            caminito(
                TOGGLE_DO_NOT_DISTURB_KEY,
                "disabled",
                Does::Run(Binding::Notifications(
                    NotificationsAction::ToggleDoNotDisturb,
                )),
            ),
        ])
}

/// The schema and key of every action Caminito binds, for the keybinding manifests' test.
#[cfg(test)]
pub(crate) fn bound_keys() -> Vec<(&'static str, &'static str)> {
    bindable().map(|b| (b.schema, b.key)).collect()
}

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
    /// In `bindable`'s order, each Shift variant right after its combination.
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
    /// Marco's two schemas, whether Caminito binds its action or not, so every value is
    /// parsed, and the keys of Caminito's own that are bindings.
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
        for bindable in bindable() {
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
            let (list, reversed, popup) = match bindable.does {
                Does::Run(binding) => {
                    rows.push(row(combination, binding));
                    continue;
                }
                Does::Switch {
                    list,
                    reversed,
                    popup,
                } => (list, reversed, popup),
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
                Binding::Policy(if popup {
                    Action::Switch { list, backward }
                } else {
                    Action::Cycle { list, backward }
                })
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

    /// What's bound to a press of a mouse button, by X's number for it, with exactly `mods`
    /// held (184). The first row that matches wins, as for a key.
    pub(crate) fn binding_for_button(&self, mods: &ModifiersState, button: u32) -> Option<Binding> {
        self.rows
            .iter()
            .find(|row| row.combination.matches_button(mods, button))
            .map(|row| row.binding)
    }

    /// The first combination in force for `key`, as a menu shows one beside an item:
    /// `Alt+F9` (161).
    pub(crate) fn label_of(&self, key: &str) -> Option<String> {
        let row = self.rows.iter().find(|row| row.key == key)?;
        Some(row.combination.label())
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

impl Accelerator {
    /// The combination as GTK's accelerator labels write one, which Marco's menu shows
    /// (`meta_accel_label_update`, metaaccellabel.c:336-461): `Shift+Ctrl+Alt+Super+` and the key,
    /// a character in upper case, `Space` and `Backslash` by name, and any other key by its
    /// keysym's name. The modifiers' names aren't translated, as Marco's are. A mouse button,
    /// which Marco's menu never shows, has the window menu's name for it (184).
    fn label(&self) -> String {
        let held = self.held;
        let mut label = String::new();
        for (on, name) in [
            (held.shift, "Shift+"),
            (held.ctrl, "Ctrl+"),
            (held.alt, "Alt+"),
            (held.logo, "Super+"),
        ] {
            if on {
                label.push_str(name);
            }
        }
        let keysym = match self.trigger {
            Trigger::Key(keysym) => keysym,
            Trigger::Button(button) => {
                label.push_str(&labels::button(button));
                return label;
            }
        };
        match keysym.key_char() {
            Some(' ') => label.push_str("Space"),
            Some('\\') => label.push_str("Backslash"),
            Some(c) if !c.is_control() => label.extend(c.to_uppercase()),
            _ => label.push_str(&xkb::keysym_get_name(keysym)),
        }
        label
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
        match self.trigger {
            Trigger::Key(keysym) => f.write_str(&xkb::keysym_get_name(keysym)),
            Trigger::Button(button) => write!(f, "Button{button}"),
        }
    }
}

#[cfg(test)]
mod tests;
