//! The bindings table, built from key values (148), with no fixture and no policy: what a
//! press with some modifiers held is bound to.

use super::*;

const GLOBAL: &str = MARCO_GLOBAL_KEYBINDINGS;
const WINDOW: &str = MARCO_WINDOW_KEYBINDINGS;

#[derive(Clone, Copy)]
struct Mods {
    ctrl: bool,
    alt: bool,
    shift: bool,
    logo: bool,
}

const NONE: Mods = Mods {
    ctrl: false,
    alt: false,
    shift: false,
    logo: false,
};
const SHIFT: Mods = Mods {
    shift: true,
    ..NONE
};
const ALT: Mods = Mods { alt: true, ..NONE };
const ALT_SHIFT: Mods = Mods {
    alt: true,
    shift: true,
    ..NONE
};
const CTRL_ALT: Mods = Mods {
    ctrl: true,
    alt: true,
    ..NONE
};
const CTRL_ALT_SHIFT: Mods = Mods {
    ctrl: true,
    alt: true,
    shift: true,
    ..NONE
};
const SUPER: Mods = Mods { logo: true, ..NONE };
const SUPER_SHIFT: Mods = Mods {
    logo: true,
    shift: true,
    ..NONE
};

fn state(mods: Mods) -> ModifiersState {
    ModifiersState {
        ctrl: mods.ctrl,
        alt: mods.alt,
        shift: mods.shift,
        logo: mods.logo,
        ..ModifiersState::default()
    }
}

/// The table built from `keys`, which must have no problem.
fn table(keys: &[(&str, &str, &str)]) -> Bindings {
    let (bindings, problems) = Bindings::from_keys(keys.iter().copied());
    assert_eq!(problems, [], "no problems expected");
    bindings
}

fn bound(bindings: &Bindings, mods: Mods, keysym: Keysym) -> Option<Binding> {
    bindings.binding_for(&state(mods), &[keysym])
}

fn policy(action: Action) -> Option<Binding> {
    Some(Binding::Policy(action))
}

fn switch(list: SwitchList, backward: bool) -> Option<Binding> {
    policy(Action::Switch { list, backward })
}

fn unbindable(schema: &str, key: &str, value: &str, reason: &str) -> Problem {
    Problem::Unbindable {
        schema: schema.into(),
        key: key.into(),
        value: format!("'{value}'"),
        reason: reason.into(),
    }
}

// The defaults: what M1 to 108 bound, now from the keys.

#[test]
fn marcos_defaults_bind_what_caminito_bound_before_reading_the_keys() {
    let t = table(&[]);
    assert_eq!(
        bound(&t, CTRL_ALT, Keysym::Left),
        policy(Action::SwitchToWorkspaceLeft)
    );
    assert_eq!(
        bound(&t, CTRL_ALT, Keysym::Right),
        policy(Action::SwitchToWorkspaceRight)
    );
    assert_eq!(
        bound(&t, ALT, Keysym::F1),
        Some(Binding::Panel(PanelAction::MainMenu))
    );
    assert_eq!(
        bound(&t, ALT, Keysym::F2),
        Some(Binding::Panel(PanelAction::RunDialog))
    );
    assert_eq!(bound(&t, ALT, Keysym::F4), policy(Action::Close));
    assert_eq!(bound(&t, ALT, Keysym::F10), policy(Action::ToggleMaximized));
    assert_eq!(bound(&t, CTRL_ALT, Keysym::d), policy(Action::ShowDesktop));
    let command = |command| Some(Binding::Command(command));
    assert_eq!(bound(&t, NONE, Keysym::Print), command(Command::Screenshot));
    assert_eq!(
        bound(&t, ALT, Keysym::Print),
        command(Command::WindowScreenshot)
    );
    assert_eq!(
        bound(&t, SHIFT, Keysym::Print),
        command(Command::AreaScreenshot)
    );
    assert_eq!(
        bound(&t, ALT, Keysym::Tab),
        switch(SwitchList::Windows, false)
    );
    assert_eq!(
        bound(&t, ALT, Keysym::grave),
        switch(SwitchList::Group, false)
    );
    assert_eq!(
        bound(&t, CTRL_ALT, Keysym::Tab),
        switch(SwitchList::Panels, false)
    );
}

#[test]
fn the_defaults_are_the_table_built_from_the_schemas_default_values() {
    let defaults = [
        (GLOBAL, "switch-to-workspace-left", "<Control><Alt>Left"),
        (GLOBAL, "switch-to-workspace-right", "<Control><Alt>Right"),
        (GLOBAL, "switch-to-workspace-up", "<Control><Alt>Up"),
        (GLOBAL, "switch-to-workspace-down", "<Control><Alt>Down"),
        (WINDOW, "move-to-workspace-up", "<Control><Shift><Alt>Up"),
        (
            WINDOW,
            "move-to-workspace-down",
            "<Control><Shift><Alt>Down",
        ),
        (GLOBAL, "switch-group", "<Alt>grave"),
        (GLOBAL, "switch-group-backward", "disabled"),
        (GLOBAL, "switch-windows", "<Alt>Tab"),
        (GLOBAL, "switch-windows-backward", "disabled"),
        (GLOBAL, "switch-windows-all", "disabled"),
        (GLOBAL, "switch-windows-all-backward", "disabled"),
        (GLOBAL, "switch-panels", "<Control><Alt>Tab"),
        (GLOBAL, "switch-panels-backward", "disabled"),
        (GLOBAL, "show-desktop", "<Control><Alt>d"),
        (GLOBAL, "panel-main-menu", "<Alt>F1"),
        (GLOBAL, "panel-run-dialog", "<Alt>F2"),
        (GLOBAL, "run-command-screenshot", "Print"),
        (GLOBAL, "run-command-window-screenshot", "<Alt>Print"),
        (GLOBAL, "run-command-area-screenshot", "<Shift>Print"),
        (
            WINDOW,
            "move-to-workspace-left",
            "<Control><Shift><Alt>Left",
        ),
        (
            WINDOW,
            "move-to-workspace-right",
            "<Control><Shift><Alt>Right",
        ),
        (WINDOW, "toggle-maximized", "<Alt>F10"),
        (WINDOW, "unmaximize", "<Alt>F5"),
        (WINDOW, "minimize", "<Alt>F9"),
        (WINDOW, "close", "<Alt>F4"),
        (WINDOW, "begin-move", "<Alt>F7"),
        (WINDOW, "begin-resize", "<Alt>F8"),
        (GLOBAL, "switch-to-workspace-prev", "disabled"),
    ];
    assert_eq!(table(&defaults), table(&[]));
    assert_eq!(Bindings::default(), table(&[]));
}

#[test]
fn up_down_and_prev_have_marcos_defaults() {
    // 150's: prev is unbound by default, as in Marco.
    let t = table(&[]);
    assert_eq!(
        bound(&t, CTRL_ALT, Keysym::Up),
        policy(Action::SwitchToWorkspaceUp)
    );
    assert_eq!(
        bound(&t, CTRL_ALT, Keysym::Down),
        policy(Action::SwitchToWorkspaceDown)
    );
    assert_eq!(
        bound(&t, CTRL_ALT_SHIFT, Keysym::Up),
        policy(Action::MoveToWorkspaceUp)
    );
    assert_eq!(
        bound(&t, CTRL_ALT_SHIFT, Keysym::Down),
        policy(Action::MoveToWorkspaceDown)
    );
    assert!(!t.iter().any(|(key, _)| key == "switch-to-workspace-prev"));

    let t = table(&[(GLOBAL, "switch-to-workspace-prev", "<Super>Tab")]);
    assert_eq!(
        bound(&t, SUPER, Keysym::Tab),
        policy(Action::SwitchToWorkspacePrev)
    );
}

#[test]
fn shift_with_a_switch_binding_steps_backward() {
    let t = table(&[]);
    assert_eq!(
        bound(&t, ALT_SHIFT, Keysym::Tab),
        switch(SwitchList::Windows, true)
    );
    assert_eq!(
        bound(&t, ALT_SHIFT, Keysym::grave),
        switch(SwitchList::Group, true)
    );
    assert_eq!(
        bound(&t, CTRL_ALT_SHIFT, Keysym::Tab),
        switch(SwitchList::Panels, true)
    );
}

#[test]
fn switch_windows_all_is_unbound_by_default_and_bindable() {
    let all = |t: &Bindings| {
        t.rows.iter().any(|row| {
            matches!(
                row.binding,
                Binding::Policy(Action::Switch {
                    list: SwitchList::WindowsAll,
                    ..
                })
            )
        })
    };
    assert!(!all(&table(&[])));

    let t = table(&[(GLOBAL, "switch-windows-all", "<Super>Tab")]);
    assert_eq!(
        bound(&t, SUPER, Keysym::Tab),
        switch(SwitchList::WindowsAll, false)
    );
    assert_eq!(
        bound(&t, SUPER_SHIFT, Keysym::Tab),
        switch(SwitchList::WindowsAll, true)
    );
}

#[test]
fn marcos_defaults_bind_the_window_and_workspace_actions() {
    let t = table(&[]);
    assert_eq!(bound(&t, ALT, Keysym::F9), policy(Action::Minimize));
    assert_eq!(bound(&t, ALT, Keysym::F7), policy(Action::BeginMove));
    assert_eq!(bound(&t, ALT, Keysym::F8), policy(Action::BeginResize));
    assert_eq!(bound(&t, ALT, Keysym::F5), policy(Action::Unmaximize));
    assert_eq!(
        bound(&t, CTRL_ALT_SHIFT, Keysym::Left),
        policy(Action::MoveToWorkspaceLeft)
    );
    assert_eq!(
        bound(&t, CTRL_ALT_SHIFT, Keysym::Right),
        policy(Action::MoveToWorkspaceRight)
    );
}

#[test]
fn the_window_and_workspace_actions_marco_leaves_disabled_bind_once_set() {
    let command = |command| Some(Binding::Command(command));
    let cases = [
        (
            GLOBAL,
            "switch-to-workspace-1",
            policy(Action::SwitchToWorkspace(0)),
        ),
        (
            GLOBAL,
            "switch-to-workspace-12",
            policy(Action::SwitchToWorkspace(11)),
        ),
        (GLOBAL, "run-command-1", command(Command::Numbered(1))),
        (GLOBAL, "run-command-12", command(Command::Numbered(12))),
        (GLOBAL, "run-command-terminal", command(Command::Terminal)),
        (
            WINDOW,
            "toggle-fullscreen",
            policy(Action::ToggleFullscreen),
        ),
        (WINDOW, "maximize", policy(Action::Maximize)),
        (WINDOW, "toggle-shaded", policy(Action::ToggleShaded)),
        // 155's.
        (WINDOW, "toggle-above", policy(Action::ToggleAbove)),
        (
            WINDOW,
            "toggle-on-all-workspaces",
            policy(Action::ToggleOnAllWorkspaces),
        ),
        (
            WINDOW,
            "move-to-workspace-1",
            policy(Action::MoveToWorkspace(0)),
        ),
        (
            WINDOW,
            "move-to-workspace-12",
            policy(Action::MoveToWorkspace(11)),
        ),
        (WINDOW, "raise-or-lower", policy(Action::RaiseOrLower)),
        (WINDOW, "raise", policy(Action::Raise)),
        (WINDOW, "lower", policy(Action::Lower)),
        (
            WINDOW,
            "maximize-vertically",
            policy(Action::ToggleMaximizedVertically),
        ),
        (
            WINDOW,
            "maximize-horizontally",
            policy(Action::ToggleMaximizedHorizontally),
        ),
        (WINDOW, "tile-to-side-e", policy(Action::Tile(Tile::Right))),
        (WINDOW, "tile-to-side-w", policy(Action::Tile(Tile::Left))),
        (
            WINDOW,
            "tile-to-corner-nw",
            policy(Action::Tile(Tile::TopLeft)),
        ),
        (
            WINDOW,
            "tile-to-corner-ne",
            policy(Action::Tile(Tile::TopRight)),
        ),
        (
            WINDOW,
            "tile-to-corner-se",
            policy(Action::Tile(Tile::BottomRight)),
        ),
        (
            WINDOW,
            "tile-to-corner-sw",
            policy(Action::Tile(Tile::BottomLeft)),
        ),
        // 160's.
        (
            WINDOW,
            "move-to-corner-nw",
            policy(Action::MoveTo(Anchor::NorthWest)),
        ),
        (
            WINDOW,
            "move-to-corner-ne",
            policy(Action::MoveTo(Anchor::NorthEast)),
        ),
        (
            WINDOW,
            "move-to-corner-sw",
            policy(Action::MoveTo(Anchor::SouthWest)),
        ),
        (
            WINDOW,
            "move-to-corner-se",
            policy(Action::MoveTo(Anchor::SouthEast)),
        ),
        (
            WINDOW,
            "move-to-side-n",
            policy(Action::MoveTo(Anchor::North)),
        ),
        (
            WINDOW,
            "move-to-side-s",
            policy(Action::MoveTo(Anchor::South)),
        ),
        (
            WINDOW,
            "move-to-side-e",
            policy(Action::MoveTo(Anchor::East)),
        ),
        (
            WINDOW,
            "move-to-side-w",
            policy(Action::MoveTo(Anchor::West)),
        ),
        (
            WINDOW,
            "move-to-center",
            policy(Action::MoveTo(Anchor::Centre)),
        ),
        (
            WINDOW,
            "move-to-monitor-n",
            policy(Action::MoveToMonitor(Direction::Up)),
        ),
        (
            WINDOW,
            "move-to-monitor-s",
            policy(Action::MoveToMonitor(Direction::Down)),
        ),
        (
            WINDOW,
            "move-to-monitor-e",
            policy(Action::MoveToMonitor(Direction::Right)),
        ),
        (
            WINDOW,
            "move-to-monitor-w",
            policy(Action::MoveToMonitor(Direction::Left)),
        ),
    ];
    for (schema, key, binding) in cases {
        let t = table(&[(schema, key, "<Super>q")]);
        assert_eq!(bound(&t, SUPER, Keysym::q), binding, "{key}");
    }
}

#[test]
fn an_action_with_no_behaviour_yet_binds_nothing() {
    // `cycle-windows`'s default, and values set by the user.
    let t = table(&[
        (GLOBAL, "cycle-windows", "<Alt>Escape"),
        (WINDOW, "activate-window-menu", "<Super>Up"),
    ]);
    assert_eq!(bound(&t, ALT, Keysym::Escape), None);
    assert_eq!(bound(&t, SUPER, Keysym::Up), None);
}

#[test]
fn modifiers_must_match_exactly_and_locks_dont_count() {
    let t = table(&[]);
    assert_eq!(bound(&t, NONE, Keysym::F4), None);
    assert_eq!(bound(&t, CTRL_ALT, Keysym::F4), None);
    assert_eq!(bound(&t, ALT_SHIFT, Keysym::F4), None);
    assert_eq!(bound(&t, ALT, Keysym::Left), None);
    let locked = ModifiersState {
        alt: true,
        caps_lock: true,
        num_lock: true,
        ..ModifiersState::default()
    };
    assert_eq!(t.binding_for(&locked, &[Keysym::F4]), policy(Action::Close));
}

#[test]
fn a_press_is_named_by_either_of_its_keysyms() {
    // Keyboard Shortcuts writes `<Shift>exclam` for Shift and 1 (100), as for the grabs.
    let t = table(&[(WINDOW, "close", "<Shift><Super>exclam")]);
    let press = state(SUPER_SHIFT);
    assert_eq!(
        t.binding_for(&press, &[Keysym::_1, Keysym::exclam]),
        policy(Action::Close)
    );
    assert_eq!(t.binding_for(&press, &[Keysym::_1]), None);
}

// A changed key.

#[test]
fn a_changed_key_moves_its_binding() {
    let t = table(&[(WINDOW, "close", "<Super>q")]);
    assert_eq!(bound(&t, SUPER, Keysym::q), policy(Action::Close));
    assert_eq!(bound(&t, ALT, Keysym::F4), None);
    // The others keep their defaults.
    assert_eq!(bound(&t, ALT, Keysym::F10), policy(Action::ToggleMaximized));
}

#[test]
fn every_spelling_of_a_modifier_binds_alike() {
    for value in [
        "<Super>q",
        "<Mod4>q",
        "<super>Q",
        "<Hyper>q",
        "<Control><Alt>q",
        "<Primary><Alt>q",
        "<Ctrl><Mod1>q",
    ] {
        let t = table(&[(WINDOW, "close", value)]);
        let mods = if value.contains("Alt") || value.contains("Mod1") {
            CTRL_ALT
        } else {
            SUPER
        };
        assert_eq!(bound(&t, mods, Keysym::q), policy(Action::Close), "{value}");
    }
}

// What binds nothing.

#[test]
fn disabled_and_the_empty_string_bind_nothing_without_a_word() {
    for value in ["disabled", ""] {
        let t = table(&[(WINDOW, "close", value)]);
        assert_eq!(bound(&t, ALT, Keysym::F4), None, "{value:?}");
        assert!(t.iter().all(|(key, _)| key != "close"), "{value:?}");
    }
}

#[test]
fn an_unparseable_value_binds_nothing_and_is_a_problem() {
    let (t, problems) = Bindings::from_keys([(WINDOW, "close", "<Alt>NoSuchKey")]);
    assert_eq!(bound(&t, ALT, Keysym::F4), None);
    assert_eq!(
        problems,
        [unbindable(
            WINDOW,
            "close",
            "<Alt>NoSuchKey",
            "NoSuchKey isn't a keysym"
        )]
    );
    assert_eq!(
        problems[0].to_string(),
        "org.mate.Marco.window-keybindings close is '<Alt>NoSuchKey', which isn't a key \
         combination Caminito can bind: NoSuchKey isn't a keysym; binding nothing"
    );
}

#[test]
fn an_unparseable_value_of_an_action_with_no_behaviour_is_a_problem_too() {
    let (_, problems) = Bindings::from_keys([(WINDOW, "minimize", "<Contrl>F9")]);
    assert_eq!(
        problems,
        [unbindable(
            WINDOW,
            "minimize",
            "<Contrl>F9",
            "<Contrl> isn't a modifier"
        )]
    );
}

// Two keys with one combination.

#[test]
fn of_two_keys_with_one_combination_the_first_in_marcos_order_wins() {
    // `toggle-maximized` comes before `close` in Marco's table.
    let t = table(&[(WINDOW, "close", "<Alt>F10")]);
    assert_eq!(bound(&t, ALT, Keysym::F10), policy(Action::ToggleMaximized));
    // The global actions come before the window ones.
    let t = table(&[(WINDOW, "close", "<Control><Alt>d")]);
    assert_eq!(bound(&t, CTRL_ALT, Keysym::d), policy(Action::ShowDesktop));
    // Whichever is changed.
    let t = table(&[(GLOBAL, "show-desktop", "<Alt>F4")]);
    assert_eq!(bound(&t, ALT, Keysym::F4), policy(Action::ShowDesktop));
}

// The `-backward` rule.

#[test]
fn a_backward_key_binds_its_combination_backward_and_with_shift_forward() {
    let t = table(&[(GLOBAL, "switch-windows-backward", "<Alt>q")]);
    assert_eq!(bound(&t, ALT, Keysym::q), switch(SwitchList::Windows, true));
    assert_eq!(
        bound(&t, ALT_SHIFT, Keysym::q),
        switch(SwitchList::Windows, false)
    );
    // Shift with `switch-windows`'s binding still steps backward.
    assert_eq!(
        bound(&t, ALT_SHIFT, Keysym::Tab),
        switch(SwitchList::Windows, true)
    );
}

#[test]
fn a_switch_binding_that_holds_shift_already_is_turned_round_by_it() {
    let t = table(&[(GLOBAL, "switch-windows", "<Alt><Shift>w")]);
    assert_eq!(
        bound(&t, ALT_SHIFT, Keysym::w),
        switch(SwitchList::Windows, true)
    );
    assert_eq!(bound(&t, ALT, Keysym::w), None);
    let t = table(&[(GLOBAL, "switch-panels-backward", "<Control><Alt><Shift>Tab")]);
    // `switch-panels`'s Shift variant comes first, and steps backward too.
    assert_eq!(
        bound(&t, CTRL_ALT_SHIFT, Keysym::Tab),
        switch(SwitchList::Panels, true)
    );
}

#[test]
fn a_switch_binding_with_no_modifier_but_shift_binds_nothing_and_is_a_problem() {
    for value in ["Tab", "<Shift>Tab"] {
        let (t, problems) = Bindings::from_keys([(GLOBAL, "switch-windows", value)]);
        assert_eq!(bound(&t, NONE, Keysym::Tab), None, "{value}");
        assert_eq!(bound(&t, SHIFT, Keysym::Tab), None, "{value}");
        assert_eq!(
            problems,
            [unbindable(
                GLOBAL,
                "switch-windows",
                value,
                "a binding Shift reverses needs a modifier such as Ctrl or Alt"
            )]
        );
    }
}

#[test]
fn shift_reverses_only_the_switch_actions() {
    let t = table(&[]);
    assert_eq!(bound(&t, ALT_SHIFT, Keysym::F4), None);
    assert_eq!(bound(&t, ALT_SHIFT, Keysym::F9), None);
}

// The debug dump's list.

#[test]
fn the_table_lists_each_combination_with_its_key() {
    let t = table(&[(WINDOW, "close", "<Mod4>q")]);
    let rows: Vec<(&str, String)> = t.iter().collect();
    assert!(rows.contains(&("close", "<Super>q".into())), "{rows:?}");
    assert!(rows.contains(&("switch-windows", "<Alt>Tab".into())));
    assert!(rows.contains(&("switch-windows", "<Shift><Alt>Tab".into())));
    assert!(rows.contains(&("run-command-screenshot", "Print".into())));
    assert_eq!(rows.len(), 26, "{rows:?}");
}
