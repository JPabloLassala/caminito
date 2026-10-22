use super::*;
use crate::input::bindings::Bindings;

/// A reading of every key at the schema defaults of Marco 1.29.0 and MATE 1.28.
fn defaults() -> Reading {
    let mut reading = Reading::default();
    let mut put =
        |schema: &str, key: &str, value: Value| reading.insert(schema, key, value.clone(), value);
    let s = |s: &str| Value::Str(s.into());
    put(MARCO_GENERAL, "theme", s("Menta"));
    put(MARCO_GENERAL, "audible-bell", Value::Bool(true));
    put(MARCO_GENERAL, "visual-bell", Value::Bool(false));
    put(MARCO_GENERAL, "visual-bell-type", s("fullscreen"));
    put(MARCO_GENERAL, "titlebar-font", s("Sans Bold 10"));
    put(
        MARCO_GENERAL,
        "titlebar-uses-system-font",
        Value::Bool(false),
    );
    put(
        MARCO_GENERAL,
        "button-layout",
        s("menu:minimize,maximize,close"),
    );
    put(
        MARCO_GENERAL,
        "action-double-click-titlebar",
        s("toggle_maximize"),
    );
    put(MARCO_GENERAL, "action-middle-click-titlebar", s("lower"));
    put(MARCO_GENERAL, "action-right-click-titlebar", s("menu"));
    put(MARCO_GENERAL, "action-scroll-up-titlebar", s("none"));
    put(MARCO_GENERAL, "action-scroll-down-titlebar", s("none"));
    put(MARCO_GENERAL, "mouse-button-modifier", s("<Alt>"));
    put(MARCO_GENERAL, "resize-with-right-button", Value::Bool(true));
    put(MARCO_GENERAL, "num-workspaces", Value::Int(4));
    put(MARCO_GENERAL, "compositing-manager", Value::Bool(true));
    put(MARCO_GENERAL, "reduced-resources", Value::Bool(false));
    put(
        MARCO_GENERAL,
        "compositing-fast-alt-tab",
        Value::Bool(false),
    );
    put(MARCO_GENERAL, "icon-size", Value::Int(48));
    put(MARCO_GENERAL, "alt-tab-max-columns", Value::Int(5));
    put(MARCO_GENERAL, "alt-tab-raise-windows", Value::Bool(false));
    put(MARCO_GENERAL, "focus-mode", s("click"));
    put(MARCO_GENERAL, "focus-new-windows", s("smart"));
    put(MARCO_GENERAL, "raise-on-click", Value::Bool(true));
    put(MARCO_GENERAL, "auto-raise", Value::Bool(false));
    put(MARCO_GENERAL, "auto-raise-delay", Value::Int(500));
    put(MARCO_GENERAL, "placement-mode", s("automatic"));
    put(MARCO_GENERAL, "center-new-windows", Value::Bool(true));
    put(MARCO_GENERAL, "wrap-style", s("no wrap"));
    put(MARCO_GENERAL, "show-desktop-skip-list", s(""));
    for n in 1..=36 {
        put(MARCO_WORKSPACE_NAMES, &format!("name-{n}"), s(""));
    }
    put(
        MARCO_KEYBINDING_COMMANDS,
        "command-screenshot",
        s("mate-screenshot"),
    );
    put(
        MARCO_KEYBINDING_COMMANDS,
        "command-window-screenshot",
        s("mate-screenshot --window"),
    );
    put(INTERFACE, "gtk-theme", s("Menta"));
    put(INTERFACE, "font-name", s("Sans 10"));
    put(INTERFACE, "icon-theme", s("menta"));
    put(INTERFACE, "gtk-key-theme", s("Default"));
    put(INTERFACE, "toolbar-style", s("both-horiz"));
    put(INTERFACE, "toolbar-icons-size", s("large-toolbar"));
    put(INTERFACE, "cursor-blink", Value::Bool(true));
    put(INTERFACE, "cursor-blink-time", Value::Int(1200));
    put(INTERFACE, "gtk-color-scheme", s(""));
    put(INTERFACE, "gtk-im-preedit-style", s("callback"));
    put(INTERFACE, "gtk-im-status-style", s("callback"));
    put(INTERFACE, "gtk-im-module", s(""));
    put(INTERFACE, "file-chooser-backend", s("gio"));
    put(
        INTERFACE,
        "gtk-decoration-layout",
        s("menu:minimize,maximize,close"),
    );
    put(INTERFACE, "gtk-shell-shows-app-menu", Value::Bool(false));
    put(INTERFACE, "gtk-shell-shows-menubar", Value::Bool(false));
    put(INTERFACE, "menus-have-icons", Value::Bool(true));
    put(INTERFACE, "buttons-have-icons", Value::Bool(true));
    put(INTERFACE, "menubar-accel", s("F10"));
    put(INTERFACE, "show-input-method-menu", Value::Bool(true));
    put(INTERFACE, "show-unicode-menu", Value::Bool(true));
    put(INTERFACE, "automatic-mnemonics", Value::Bool(true));
    put(INTERFACE, "gtk-enable-primary-paste", Value::Bool(true));
    put(INTERFACE, "gtk-enable-animations", Value::Bool(true));
    put(INTERFACE, "enable-animations", Value::Bool(true));
    put(INTERFACE, "gtk-dialogs-use-header", Value::Bool(false));
    put(INTERFACE, "window-scaling-factor", Value::Int(0));
    put(FONT_RENDERING, "antialiasing", s("rgba"));
    put(FONT_RENDERING, "hinting", s("slight"));
    put(FONT_RENDERING, "rgba-order", s("rgb"));
    put(FONT_RENDERING, "dpi", Value::Double(0.0));
    put(MOUSE, "cursor-theme", s(""));
    put(MOUSE, "cursor-size", Value::Int(24));
    put(MOUSE, "double-click", Value::Int(400));
    put(MOUSE, "drag-threshold", Value::Int(8));
    put(MOUSE, "left-handed", Value::Bool(false));
    put(MOUSE, "motion-acceleration", Value::Double(-1.0));
    put(MOUSE, "motion-threshold", Value::Int(-1));
    put(MOUSE, "accel-profile", s("default"));
    put(MOUSE, "middle-button-enabled", Value::Bool(true));
    put(MOUSE, "locate-pointer", Value::Bool(false));
    put(TOUCHPAD, "disable-while-typing", Value::Bool(false));
    put(TOUCHPAD, "tap-to-click", Value::Bool(false));
    put(TOUCHPAD, "vertical-edge-scrolling", Value::Bool(true));
    put(TOUCHPAD, "horizontal-edge-scrolling", Value::Bool(false));
    put(TOUCHPAD, "vertical-two-finger-scrolling", Value::Bool(true));
    put(
        TOUCHPAD,
        "horizontal-two-finger-scrolling",
        Value::Bool(false),
    );
    put(TOUCHPAD, "natural-scroll", Value::Bool(false));
    put(TOUCHPAD, "touchpad-enabled", Value::Bool(true));
    put(TOUCHPAD, "two-finger-click", Value::Int(0));
    put(TOUCHPAD, "three-finger-click", Value::Int(0));
    put(TOUCHPAD, "tap-button-one-finger", Value::Int(1));
    put(TOUCHPAD, "tap-button-two-finger", Value::Int(3));
    put(TOUCHPAD, "tap-button-three-finger", Value::Int(2));
    put(TOUCHPAD, "left-handed", s("mouse"));
    put(TOUCHPAD, "motion-acceleration", Value::Double(-1.0));
    put(TOUCHPAD, "motion-threshold", Value::Int(-1));
    put(TOUCHPAD, "accel-profile", s("default"));
    put(KEYBOARD, "repeat", Value::Bool(true));
    put(KEYBOARD, "click", Value::Bool(true));
    put(KEYBOARD, "rate", Value::Int(30));
    put(KEYBOARD, "delay", Value::Int(500));
    put(KEYBOARD, "click-volume", Value::Int(0));
    put(KEYBOARD, "bell-mode", s("on"));
    put(KEYBOARD, "bell-pitch", Value::Int(400));
    put(KEYBOARD, "bell-duration", Value::Int(100));
    put(KEYBOARD, "bell-custom-file", s(""));
    put(KEYBOARD, "remember-numlock-state", Value::Bool(true));
    put(KEYBOARD, "numlock-state", s("unknown"));
    for key in [
        "enable",
        "feature-state-change-beep",
        "timeout-enable",
        "bouncekeys-enable",
        "bouncekeys-beep-reject",
        "mousekeys-enable",
        "slowkeys-enable",
        "slowkeys-beep-press",
        "slowkeys-beep-accept",
        "slowkeys-beep-reject",
        "stickykeys-enable",
        "stickykeys-two-key-off",
        "stickykeys-modifier-beep",
        "togglekeys-enable",
        "capslock-beep-enable",
    ] {
        put(A11Y_KEYBOARD, key, Value::Bool(false));
    }
    put(A11Y_KEYBOARD, "stickykeys-latch-to-lock", Value::Bool(true));
    put(A11Y_KEYBOARD, "timeout", Value::Int(200));
    put(A11Y_KEYBOARD, "bouncekeys-delay", Value::Int(300));
    put(A11Y_KEYBOARD, "slowkeys-delay", Value::Int(300));
    put(A11Y_KEYBOARD, "togglekeys-backend", s("xkb"));
    put(KEYBOARD_XKB, "model", s(""));
    put(KEYBOARD_XKB, "layouts", Value::StrList(Vec::new()));
    put(KEYBOARD_XKB, "options", Value::StrList(Vec::new()));
    put(SOUND, "theme-name", s("freedesktop"));
    put(SOUND, "event-sounds", Value::Bool(false));
    put(SOUND, "input-feedback-sounds", Value::Bool(false));
    put(CAMINITO, "workspace-layout-orientation", s("horizontal"));
    put(CAMINITO, "workspace-layout-rows", Value::Int(1));
    put(CAMINITO, "workspaces-per-output", Value::Bool(true));
    put(CAMINITO, "workspace-slide", Value::Bool(true));
    put(CAMINITO, "overview", s("<Super>w"));
    put(CAMINITO, "workspace-layout-columns", Value::Int(0));
    for corner in ["top-left", "top-right", "bottom-left", "bottom-right"] {
        put(CAMINITO, &format!("hot-corner-{corner}"), s("none"));
    }
    put(
        CAMINITO,
        "locker-command",
        Value::StrList(vec!["caminito-locker".into()]),
    );
    put(LOCKDOWN, "disable-lock-screen", Value::Bool(false));
    put(SESSION, "idle-delay", Value::Int(5));
    put(SCREENSAVER, "idle-activation-enabled", Value::Bool(true));
    put(SCREENSAVER, "lock-enabled", Value::Bool(true));
    put(SCREENSAVER, "lock-delay", Value::Int(0));
    put(POWER_MANAGER, "sleep-display-ac", Value::Int(1800));
    put(POWER_MANAGER, "sleep-display-battery", Value::Int(600));
    put(POWER_MANAGER, "sleep-display-ups", Value::Int(600));
    reading
}

/// The defaults with one key set to `value`.
fn with(schema: &str, key: &str, value: Value) -> Reading {
    let mut reading = defaults();
    let default = reading
        .get(schema, key)
        .map_or(value.clone(), |e| e.default.clone());
    reading.insert(schema, key, value, default);
    reading
}

fn string(value: &str) -> Value {
    Value::Str(value.into())
}

fn settings(reading: &Reading) -> Settings {
    let translation = translate(reading);
    assert_eq!(translation.problems, [], "no problems expected");
    translation.settings
}

/// The one problem translating `reading` has.
fn problem(reading: &Reading) -> (Settings, Problem) {
    let Translation {
        settings,
        mut problems,
    } = translate(reading);
    assert_eq!(problems.len(), 1, "one problem expected: {problems:?}");
    (settings, problems.remove(0))
}

fn out_of_range(schema: &str, key: &str, value: &str, used: &str) -> Problem {
    Problem::OutOfRange {
        schema: schema.into(),
        key: key.into(),
        value: value.into(),
        used: used.into(),
    }
}

fn invalid(schema: &str, key: &str, value: &str, used: &str) -> Problem {
    Problem::Invalid {
        schema: schema.into(),
        key: key.into(),
        value: value.into(),
        used: used.into(),
    }
}

// Workspaces.

#[test]
fn the_defaults_give_four_workspaces_with_marcos_names() {
    let config = settings(&defaults()).policy;
    assert_eq!(config.workspace_count, 4);
    assert_eq!(
        config.workspace_names,
        ["Workspace 1", "Workspace 2", "Workspace 3", "Workspace 4"]
    );
}

#[test]
fn num_workspaces_sets_the_count() {
    let config = settings(&with(MARCO_GENERAL, "num-workspaces", Value::Int(6))).policy;
    assert_eq!(config.workspace_count, 6);
    assert_eq!(config.workspace_names.len(), 6);
    assert_eq!(config.workspace_names[5], "Workspace 6");
}

#[test]
fn num_workspaces_above_36_is_clamped() {
    let (settings, problem) = problem(&with(MARCO_GENERAL, "num-workspaces", Value::Int(50)));
    assert_eq!(settings.policy.workspace_count, 36);
    assert_eq!(
        problem,
        out_of_range(MARCO_GENERAL, "num-workspaces", "50", "36")
    );
}

#[test]
fn num_workspaces_below_1_is_clamped() {
    let (settings, problem) = problem(&with(MARCO_GENERAL, "num-workspaces", Value::Int(0)));
    assert_eq!(settings.policy.workspace_count, 1);
    assert_eq!(
        problem,
        out_of_range(MARCO_GENERAL, "num-workspaces", "0", "1")
    );
}

#[test]
fn workspace_names_are_used_when_set() {
    let reading = with(MARCO_WORKSPACE_NAMES, "name-2", string("Mail"));
    let names = settings(&reading).policy.workspace_names;
    assert_eq!(names, ["Workspace 1", "Mail", "Workspace 3", "Workspace 4"]);
}

#[test]
fn names_beyond_the_count_are_not_passed_on() {
    let reading = with(MARCO_WORKSPACE_NAMES, "name-5", string("Spare"));
    assert_eq!(settings(&reading).policy.workspace_names.len(), 4);
}

#[test]
fn each_output_has_its_own_workspaces_unless_the_key_is_off() {
    // On by default, unlike Marco (ADR 0032).
    assert!(settings(&defaults()).policy.workspaces_per_output);
    let reading = with(CAMINITO, "workspaces-per-output", Value::Bool(false));
    assert!(!settings(&reading).policy.workspaces_per_output);
}

#[test]
fn dynamic_workspaces_is_off_where_the_schema_has_no_key_for_it() {
    // Marco 1.29.0's schema, which `defaults` reads, has no `dynamic-workspaces`.
    assert!(!settings(&defaults()).policy.dynamic_workspaces);
}

#[test]
fn dynamic_workspaces_is_read_where_the_schema_has_it_and_then_every_name_is_passed_on() {
    let mut reading = with(MARCO_GENERAL, "dynamic-workspaces", Value::Bool(true));
    reading.insert(MARCO_WORKSPACE_NAMES, "name-9", string("Spare"), string(""));
    let config = settings(&reading).policy;
    assert!(config.dynamic_workspaces);
    // The count is Caminito's then, and can reach any of them (151).
    assert_eq!(config.workspace_names.len(), 36);
    assert_eq!(config.workspace_names[8], "Spare");
    assert_eq!(config.workspace_names[35], "Workspace 36");
}

// The workspace grid (96).

#[test]
fn the_defaults_give_one_row_of_workspaces() {
    let layout = settings(&defaults()).policy.workspace_layout;
    assert_eq!(layout, WorkspaceLayout::default());
    assert_eq!(layout.orientation, Orientation::Horizontal);
    assert_eq!((layout.rows, layout.columns), (1, 0));
}

#[test]
fn the_workspace_layout_keys_are_read() {
    let reading = with_all(&[
        (CAMINITO, "workspace-layout-orientation", string("vertical")),
        (CAMINITO, "workspace-layout-rows", Value::Int(0)),
        (CAMINITO, "workspace-layout-columns", Value::Int(2)),
    ]);
    let layout = settings(&reading).policy.workspace_layout;
    assert_eq!(layout.orientation, Orientation::Vertical);
    assert_eq!((layout.rows, layout.columns), (0, 2));
}

#[test]
fn an_unknown_workspace_layout_orientation_falls_back_to_the_default() {
    let key = "workspace-layout-orientation";
    let (settings, problem) = problem(&with(CAMINITO, key, string("diagonal")));
    assert_eq!(
        settings.policy.workspace_layout.orientation,
        Orientation::Horizontal
    );
    assert_eq!(
        problem,
        invalid(CAMINITO, key, "'diagonal'", "'horizontal'")
    );
}

#[test]
fn workspace_layout_rows_and_columns_out_of_range_are_clamped() {
    let key = "workspace-layout-rows";
    let (settings, problem) = problem(&with(CAMINITO, key, Value::Int(-1)));
    assert_eq!(settings.policy.workspace_layout.rows, 0);
    assert_eq!(problem, out_of_range(CAMINITO, key, "-1", "0"));

    let key = "workspace-layout-columns";
    let (settings, problem) = self::problem(&with(CAMINITO, key, Value::Int(50)));
    assert_eq!(settings.policy.workspace_layout.columns, 36);
    assert_eq!(problem, out_of_range(CAMINITO, key, "50", "36"));
}

#[test]
fn the_default_wrap_style_is_no_wrap() {
    for reading in [defaults(), Reading::default()] {
        assert_eq!(settings(&reading).policy.wrap_style, WrapStyle::NoWrap);
    }
}

#[test]
fn wrap_style_is_passed_to_policy() {
    for (nick, wrap) in [
        ("classic", WrapStyle::Classic),
        ("toroidal", WrapStyle::Toroidal),
    ] {
        let reading = with(MARCO_GENERAL, "wrap-style", string(nick));
        // And with no problem: it isn't unsupported any more (150).
        assert_eq!(settings(&reading).policy.wrap_style, wrap);
    }
}

#[test]
fn an_unknown_wrap_style_falls_back_to_no_wrap() {
    let (settings, problem) = problem(&with(MARCO_GENERAL, "wrap-style", string("spiral")));
    assert_eq!(settings.policy.wrap_style, WrapStyle::NoWrap);
    assert_eq!(
        problem,
        invalid(MARCO_GENERAL, "wrap-style", "'spiral'", "'no wrap'")
    );
}

#[test]
fn the_default_locker_is_caminito_locker_and_locking_is_allowed() {
    let locker = settings(&defaults()).locker;
    assert_eq!(locker.command, ["caminito-locker"]);
    assert!(!locker.lock_disabled);
}

#[test]
fn the_locker_command_is_an_argv() {
    let argv = Value::StrList(vec!["swaylock".into(), "-c".into(), "000000".into()]);
    let locker = settings(&with(CAMINITO, "locker-command", argv)).locker;
    assert_eq!(locker.command, ["swaylock", "-c", "000000"]);
}

/// An empty command is a failed start, which ends in the give-up state (126).
#[test]
fn an_empty_locker_command_is_kept_empty() {
    let locker = settings(&with(
        CAMINITO,
        "locker-command",
        Value::StrList(Vec::new()),
    ))
    .locker;
    assert!(locker.command.is_empty());
}

#[test]
fn disable_lock_screen_is_read() {
    let reading = with(LOCKDOWN, "disable-lock-screen", Value::Bool(true));
    assert!(settings(&reading).locker.lock_disabled);
}

// Idle, blanking and the lock after it (128).

#[test]
fn the_defaults_blank_after_five_minutes_and_lock_at_once() {
    let idle = settings(&defaults()).idle;
    assert_eq!(idle.delay, Duration::from_secs(5 * 60));
    assert!(idle.blank);
    assert_eq!(idle.lock, Some(Duration::ZERO));
}

#[test]
fn idle_delay_is_in_minutes() {
    let idle = settings(&with(SESSION, "idle-delay", Value::Int(1))).idle;
    assert_eq!(idle.delay, Duration::from_secs(60));
}

/// As in mate-session, which watches for idle only with a delay above 0
/// (`gsm-presence.c:170-172`).
#[test]
fn an_idle_delay_of_0_is_never_idle() {
    let idle = settings(&with(SESSION, "idle-delay", Value::Int(0))).idle;
    assert_eq!(idle.delay, Duration::ZERO);
}

#[test]
fn a_negative_idle_delay_is_the_default() {
    let (settings, problem) = problem(&with(SESSION, "idle-delay", Value::Int(-1)));
    assert_eq!(settings.idle.delay, Duration::from_secs(5 * 60));
    assert_eq!(problem, invalid(SESSION, "idle-delay", "-1", "5"));
}

#[test]
fn idle_activation_enabled_off_doesnt_blank() {
    let reading = with(SCREENSAVER, "idle-activation-enabled", Value::Bool(false));
    assert!(!settings(&reading).idle.blank);
}

#[test]
fn lock_enabled_off_asks_for_no_lock() {
    let reading = with(SCREENSAVER, "lock-enabled", Value::Bool(false));
    assert_eq!(settings(&reading).idle.lock, None);
}

#[test]
fn lock_delay_is_in_minutes() {
    let reading = with(SCREENSAVER, "lock-delay", Value::Int(2));
    assert_eq!(settings(&reading).idle.lock, Some(Duration::from_secs(120)));
}

#[test]
fn a_negative_lock_delay_is_the_default() {
    let (settings, problem) = problem(&with(SCREENSAVER, "lock-delay", Value::Int(-3)));
    assert_eq!(settings.idle.lock, Some(Duration::ZERO));
    assert_eq!(problem, invalid(SCREENSAVER, "lock-delay", "-3", "0"));
}

#[test]
fn without_the_session_and_screensaver_schemas_idle_has_their_defaults() {
    let mut reading = defaults();
    reading.remove(SESSION, "idle-delay");
    for key in ["idle-activation-enabled", "lock-enabled", "lock-delay"] {
        reading.remove(SCREENSAVER, key);
    }
    reading.set_missing(SESSION);
    reading.set_missing(SCREENSAVER);
    let idle = translate(&reading).settings.idle;
    assert_eq!(idle, settings(&defaults()).idle);
}

#[test]
fn the_outputs_power_off_after_half_an_hour_on_ac_and_ten_minutes_on_battery() {
    let sleep = settings(&defaults()).idle.sleep_display;
    assert_eq!(sleep.ac, Duration::from_secs(30 * 60));
    assert_eq!(sleep.battery, Duration::from_secs(10 * 60));
}

#[test]
fn sleep_display_is_in_seconds() {
    let reading = with(POWER_MANAGER, "sleep-display-ac", Value::Int(90));
    assert_eq!(
        settings(&reading).idle.sleep_display.ac,
        Duration::from_secs(90)
    );
    let reading = with(POWER_MANAGER, "sleep-display-battery", Value::Int(45));
    assert_eq!(
        settings(&reading).idle.sleep_display.battery,
        Duration::from_secs(45)
    );
}

#[test]
fn a_sleep_display_of_0_is_kept_as_never() {
    let reading = with(POWER_MANAGER, "sleep-display-ac", Value::Int(0));
    assert_eq!(settings(&reading).idle.sleep_display.ac, Duration::ZERO);
}

#[test]
fn a_negative_sleep_display_is_the_default() {
    let (settings, problem) = problem(&with(
        POWER_MANAGER,
        "sleep-display-battery",
        Value::Int(-1),
    ));
    assert_eq!(
        settings.idle.sleep_display.battery,
        Duration::from_secs(600)
    );
    assert_eq!(
        problem,
        invalid(POWER_MANAGER, "sleep-display-battery", "-1", "600")
    );
}

/// A UPS counts as AC (09): its key is never read, so it's no problem either.
#[test]
fn sleep_display_ups_is_never_read() {
    let reading = with(POWER_MANAGER, "sleep-display-ups", Value::Int(5));
    assert_eq!(settings(&reading).idle, settings(&defaults()).idle);
}

#[test]
fn without_the_power_manager_schema_the_outputs_power_off_at_its_defaults() {
    let mut reading = defaults();
    for key in [
        "sleep-display-ac",
        "sleep-display-battery",
        "sleep-display-ups",
    ] {
        reading.remove(POWER_MANAGER, key);
    }
    reading.set_missing(POWER_MANAGER);
    let idle = translate(&reading).settings.idle;
    assert_eq!(idle, settings(&defaults()).idle);
}

#[test]
fn without_caminitos_schema_the_locker_is_caminito_locker() {
    let mut reading = defaults();
    reading.remove(CAMINITO, "locker-command");
    reading.set_missing(CAMINITO);
    assert_eq!(
        translate(&reading).settings.locker.command,
        ["caminito-locker"]
    );
}

#[test]
fn without_caminitos_schema_the_workspaces_are_one_row_and_it_s_said() {
    // A development run that never installed the schema (96).
    let mut reading = defaults();
    reading.set_missing(CAMINITO);
    for key in ["orientation", "rows", "columns"] {
        reading.remove(CAMINITO, &format!("workspace-layout-{key}"));
    }
    let (settings, problem) = problem(&reading);
    assert_eq!(settings.policy.workspace_layout, WorkspaceLayout::default());
    assert_eq!(problem, Problem::MissingSchema(CAMINITO.into()));
    assert_eq!(
        problem.to_string(),
        "the org.mate.Caminito schema isn't installed; using its defaults"
    );
}

// Frames.

#[test]
fn the_defaults_give_marcos_frames() {
    let frames = settings(&defaults()).frames;
    assert_eq!(
        frames,
        Frames {
            theme: "Menta".into(),
            titlebar_font: "Sans Bold 10".into(),
            button_layout: ButtonLayout::parse("menu:minimize,maximize,close"),
            double_click: TitlebarAction::ToggleMaximize,
            middle_click: TitlebarAction::Lower,
            right_click: TitlebarAction::Menu,
            scroll_up: TitlebarAction::None,
            scroll_down: TitlebarAction::None,
            shadows: true,
        }
    );
}

// The clicks with a modifier held (162).

#[test]
fn the_defaults_give_marcos_modifier_click() {
    assert_eq!(
        settings(&defaults()).modifier_click,
        ModifierClick {
            modifier: ButtonModifier::default(),
            resize_with_right_button: true,
        }
    );
}

#[test]
fn the_mouse_button_modifier_is_parsed() {
    let modifier = |value: &str| {
        let reading = with(MARCO_GENERAL, "mouse-button-modifier", string(value));
        settings(&reading).modifier_click.modifier.to_string()
    };
    assert_eq!(modifier("<Super>"), "<Super>");
    assert_eq!(modifier("<Mod4>"), "<Super>");
    assert_eq!(modifier("<Control><Alt>"), "<Control><Alt>");
    assert_eq!(modifier(""), "disabled");
    assert_eq!(modifier("disabled"), "disabled");
}

#[test]
fn a_mouse_button_modifier_that_doesnt_parse_falls_back_to_alt_and_is_logged() {
    let (settings, problem) = problem(&with(
        MARCO_GENERAL,
        "mouse-button-modifier",
        string("garbage"),
    ));
    assert_eq!(settings.modifier_click.modifier, ButtonModifier::default());
    assert_eq!(
        problem,
        invalid(
            MARCO_GENERAL,
            "mouse-button-modifier",
            "'garbage'",
            "'<Alt>'"
        )
    );
    assert_eq!(
        problem.to_string(),
        "org.mate.Marco.general mouse-button-modifier is 'garbage', which isn't valid; \
         using '<Alt>'"
    );
}

#[test]
fn resize_with_right_button_is_read() {
    let reading = with(
        MARCO_GENERAL,
        "resize-with-right-button",
        Value::Bool(false),
    );
    assert!(!settings(&reading).modifier_click.resize_with_right_button);
}

#[test]
fn the_titlebar_scroll_actions_are_read_by_nick() {
    let mut reading = with(MARCO_GENERAL, "action-scroll-up-titlebar", string("shade"));
    reading.insert(
        MARCO_GENERAL,
        "action-scroll-down-titlebar",
        string("unshade"),
        string("none"),
    );
    let frames = settings(&reading).frames;
    assert_eq!(frames.scroll_up, TitlebarAction::Shade);
    assert_eq!(frames.scroll_down, TitlebarAction::Unshade);
}

#[test]
fn the_defaults_give_marcos_screenshot_commands() {
    let commands = settings(&defaults()).commands;
    assert_eq!(commands.screenshot, "mate-screenshot");
    assert_eq!(commands.window_screenshot, "mate-screenshot --window");
    // Marco 1.29.0's schema has no key for it; this is its default in the Marco that has.
    assert_eq!(commands.area_screenshot, "mate-screenshot --area");
}

#[test]
fn the_screenshot_commands_are_read_where_the_schema_has_them() {
    let reading = with(
        MARCO_KEYBINDING_COMMANDS,
        "command-area-screenshot",
        string("mate-screenshot --area --clipboard"),
    );
    assert_eq!(
        settings(&reading).commands.area_screenshot,
        "mate-screenshot --area --clipboard"
    );
}

#[test]
fn the_numbered_commands_and_the_terminal_are_read_with_their_defaults() {
    let commands = settings(&defaults()).commands;
    assert_eq!(commands.numbered.len(), 12);
    assert_eq!(commands.terminal, "mate-terminal");

    let reading = with(MARCO_KEYBINDING_COMMANDS, "command-12", string("caja ~"));
    assert_eq!(settings(&reading).commands.numbered[11], "caja ~");
    let reading = with(TERMINAL, "exec", string("xterm"));
    assert_eq!(settings(&reading).commands.terminal, "xterm");
}

// Keybindings (148). The table itself is `input::bindings`'s, and tested there.

#[test]
fn without_the_keybinding_keys_the_bindings_are_marcos_defaults() {
    assert_eq!(settings(&defaults()).bindings, Bindings::default());
}

#[test]
fn the_keybinding_keys_of_both_schemas_are_read() {
    let mut reading = with(MARCO_WINDOW_KEYBINDINGS, "close", string("<Super>q"));
    reading.insert(
        MARCO_GLOBAL_KEYBINDINGS,
        "run-command-area-screenshot",
        string("<Control>Print"),
        string("<Shift>Print"),
    );
    let (expected, _) = Bindings::from_keys([
        (MARCO_WINDOW_KEYBINDINGS, "close", "<Super>q"),
        (
            MARCO_GLOBAL_KEYBINDINGS,
            "run-command-area-screenshot",
            "<Control>Print",
        ),
    ]);
    let bindings = settings(&reading).bindings;
    assert_eq!(bindings, expected);
    assert_ne!(bindings, Bindings::default());
}

#[test]
fn the_overviews_shortcut_is_read_from_caminitos_schema() {
    let reading = with(CAMINITO, "overview", string("<Super>Tab"));
    let (expected, _) = Bindings::from_keys([(CAMINITO, "overview", "<Super>Tab")]);
    let bindings = settings(&reading).bindings;
    assert_eq!(bindings, expected);
    assert_ne!(bindings, Bindings::default());
}

#[test]
fn an_overview_shortcut_that_doesnt_parse_is_a_problem() {
    let reading = with(CAMINITO, "overview", string("<Super>F99"));
    let (settings, problem) = problem(&reading);
    assert_eq!(
        problem.to_string(),
        "org.mate.Caminito overview is '<Super>F99', which isn't a key combination Caminito \
         can bind: F99 isn't a keysym; binding nothing"
    );
    assert!(settings.bindings.iter().all(|(key, _)| key != "overview"));
}

#[test]
fn a_keybinding_that_doesnt_parse_is_a_problem_logged_once_per_value() {
    let reading = with(MARCO_WINDOW_KEYBINDINGS, "minimize", string("<Alt>F99"));
    let (_, first) = problem(&reading);
    assert_eq!(
        first.to_string(),
        "org.mate.Marco.window-keybindings minimize is '<Alt>F99', which isn't a key \
         combination Caminito can bind: F99 isn't a keysym; binding nothing"
    );
    let mut logged = Logged::default();
    assert_eq!(logged.first_time(vec![first.clone()]).len(), 1);
    assert_eq!(logged.first_time(vec![first]).len(), 0);
    let reading = with(MARCO_WINDOW_KEYBINDINGS, "minimize", string("<Alt>F98"));
    let (_, second) = problem(&reading);
    assert_eq!(logged.first_time(vec![second]).len(), 1);
}

#[test]
fn theme_names_the_marco_theme() {
    let reading = with(MARCO_GENERAL, "theme", string("ClearlooksRe"));
    assert_eq!(settings(&reading).frames.theme, "ClearlooksRe");
}

#[test]
fn an_empty_theme_falls_back_to_the_default() {
    let (settings, problem) = problem(&with(MARCO_GENERAL, "theme", string("")));
    assert_eq!(settings.frames.theme, "Menta");
    assert_eq!(problem, invalid(MARCO_GENERAL, "theme", "''", "'Menta'"));
}

#[test]
fn titlebar_font_is_used_without_the_system_font() {
    let reading = with(MARCO_GENERAL, "titlebar-font", string("Serif Italic 12"));
    assert_eq!(settings(&reading).frames.titlebar_font, "Serif Italic 12");
}

#[test]
fn titlebar_uses_system_font_takes_the_interface_font() {
    let mut reading = with(
        MARCO_GENERAL,
        "titlebar-uses-system-font",
        Value::Bool(true),
    );
    reading.insert(
        INTERFACE,
        "font-name",
        string("Cantarell 11"),
        string("Sans 10"),
    );
    assert_eq!(settings(&reading).frames.titlebar_font, "Cantarell 11");
}

#[test]
fn an_empty_titlebar_font_falls_back_to_the_default() {
    let (settings, problem) = problem(&with(MARCO_GENERAL, "titlebar-font", string("")));
    assert_eq!(settings.frames.titlebar_font, "Sans Bold 10");
    assert_eq!(
        problem,
        invalid(MARCO_GENERAL, "titlebar-font", "''", "'Sans Bold 10'")
    );
}

#[test]
fn button_layout_is_parsed() {
    let reading = with(MARCO_GENERAL, "button-layout", string("close:minimize"));
    assert_eq!(
        settings(&reading).frames.button_layout,
        ButtonLayout::parse("close:minimize")
    );
}

#[test]
fn titlebar_actions_are_read_by_nick() {
    let mut reading = with(
        MARCO_GENERAL,
        "action-double-click-titlebar",
        string("toggle_shade"),
    );
    let mut set = |key, nick| reading.insert(MARCO_GENERAL, key, string(nick), string("x"));
    set("action-middle-click-titlebar", "toggle_maximize_vertically");
    set("action-right-click-titlebar", "none");
    let frames = settings(&reading).frames;
    assert_eq!(frames.double_click, TitlebarAction::ToggleShade);
    assert_eq!(
        frames.middle_click,
        TitlebarAction::ToggleMaximizeVertically
    );
    assert_eq!(frames.right_click, TitlebarAction::None);
}

#[test]
fn every_titlebar_action_nick_is_known() {
    let nicks = [
        ("close", TitlebarAction::Close),
        ("minimize", TitlebarAction::Minimize),
        ("toggle_maximize", TitlebarAction::ToggleMaximize),
        (
            "toggle_maximize_horizontally",
            TitlebarAction::ToggleMaximizeHorizontally,
        ),
        (
            "toggle_maximize_vertically",
            TitlebarAction::ToggleMaximizeVertically,
        ),
        ("toggle_shade", TitlebarAction::ToggleShade),
        ("shade", TitlebarAction::Shade),
        ("unshade", TitlebarAction::Unshade),
        ("raise", TitlebarAction::Raise),
        ("lower", TitlebarAction::Lower),
        ("toggle_stick", TitlebarAction::ToggleStick),
        ("toggle_above", TitlebarAction::ToggleAbove),
        ("menu", TitlebarAction::Menu),
        ("none", TitlebarAction::None),
    ];
    for (nick, action) in nicks {
        let reading = with(MARCO_GENERAL, "action-right-click-titlebar", string(nick));
        assert_eq!(settings(&reading).frames.right_click, action, "{nick}");
    }
}

#[test]
fn an_unknown_titlebar_action_falls_back_to_the_default() {
    // `last` is the enum's end marker, not an action.
    let reading = with(
        MARCO_GENERAL,
        "action-middle-click-titlebar",
        string("last"),
    );
    let (settings, problem) = problem(&reading);
    assert_eq!(settings.frames.middle_click, TitlebarAction::Lower);
    assert_eq!(
        problem,
        invalid(
            MARCO_GENERAL,
            "action-middle-click-titlebar",
            "'last'",
            "'lower'"
        )
    );
}

#[test]
fn attach_modal_dialogs_is_read_when_a_schema_has_it() {
    let reading = with(MARCO_GENERAL, "attach-modal-dialogs", Value::Bool(true));
    assert!(settings(&reading).policy.attach_modal_dialogs);
}

#[test]
fn a_value_of_the_wrong_type_falls_back_to_the_default() {
    let reading = with(MARCO_GENERAL, "titlebar-uses-system-font", Value::Int(1));
    let (settings, problem) = problem(&reading);
    assert_eq!(settings.frames.titlebar_font, "Sans Bold 10");
    assert_eq!(
        problem,
        invalid(MARCO_GENERAL, "titlebar-uses-system-font", "1", "false")
    );
}

// Focus-stealing prevention and urgent windows (165).

#[test]
fn focus_new_windows_is_passed_to_policy_and_isnt_reported_as_unsupported() {
    assert_eq!(
        settings(&defaults()).policy.focus_new_windows,
        FocusNewWindows::Smart
    );
    // `settings` checks there's no problem.
    let reading = with(MARCO_GENERAL, "focus-new-windows", string("strict"));
    assert_eq!(
        settings(&reading).policy.focus_new_windows,
        FocusNewWindows::Strict
    );
}

#[test]
fn urgent_placement_keeps_an_urgent_windows_place_where_the_schema_has_no_key_for_it() {
    // Marco 1.29.0's schema, which `defaults` reads, has no `alt-tab-urgent-placement`.
    assert_eq!(
        settings(&defaults()).policy.urgent_placement,
        UrgentPlacement::Mru
    );
}

#[test]
fn urgent_placement_follows_masters_key_and_its_default() {
    for (value, placement) in [
        ("start", UrgentPlacement::Start),
        ("mru", UrgentPlacement::Mru),
    ] {
        let mut reading = defaults();
        reading.insert(
            MARCO_GENERAL,
            "alt-tab-urgent-placement",
            string(value),
            string("start"),
        );
        assert_eq!(settings(&reading).policy.urgent_placement, placement);
    }
}

// The focus modes and auto-raise (153).

#[test]
fn the_defaults_give_click_focus_raise_on_click_and_no_auto_raise() {
    for reading in [defaults(), Reading::default()] {
        let policy = settings(&reading).policy;
        assert_eq!(policy.focus_mode, FocusMode::Click);
        assert!(policy.raise_on_click);
        assert!(!policy.auto_raise);
        assert_eq!(policy.auto_raise_delay, Duration::from_millis(500));
    }
}

#[test]
fn focus_mode_is_passed_to_policy() {
    for (nick, mode) in [("sloppy", FocusMode::Sloppy), ("mouse", FocusMode::Mouse)] {
        let reading = with(MARCO_GENERAL, "focus-mode", string(nick));
        assert_eq!(settings(&reading).policy.focus_mode, mode);
    }
}

#[test]
fn an_unknown_focus_mode_falls_back_to_click() {
    let (settings, problem) = problem(&with(MARCO_GENERAL, "focus-mode", string("lazy")));
    assert_eq!(settings.policy.focus_mode, FocusMode::Click);
    assert_eq!(
        problem,
        invalid(MARCO_GENERAL, "focus-mode", "'lazy'", "'click'")
    );
}

#[test]
fn raise_on_click_and_auto_raise_are_passed_to_policy() {
    let reading = with(MARCO_GENERAL, "raise-on-click", Value::Bool(false));
    assert!(!settings(&reading).policy.raise_on_click);
    let reading = with(MARCO_GENERAL, "auto-raise", Value::Bool(true));
    assert!(settings(&reading).policy.auto_raise);
    let reading = with(MARCO_GENERAL, "auto-raise-delay", Value::Int(250));
    assert_eq!(
        settings(&reading).policy.auto_raise_delay,
        Duration::from_millis(250)
    );
}

#[test]
fn auto_raise_delay_out_of_range_is_clamped_to_marcos_bounds() {
    let reading = with(MARCO_GENERAL, "auto-raise-delay", Value::Int(-1));
    let (settings, problem) = problem(&reading);
    assert_eq!(settings.policy.auto_raise_delay, Duration::ZERO);
    assert_eq!(
        problem,
        out_of_range(MARCO_GENERAL, "auto-raise-delay", "-1", "0")
    );

    let reading = with(MARCO_GENERAL, "auto-raise-delay", Value::Int(10_001));
    let (settings, problem) = self::problem(&reading);
    assert_eq!(settings.policy.auto_raise_delay, Duration::from_secs(10));
    assert_eq!(
        problem,
        out_of_range(MARCO_GENERAL, "auto-raise-delay", "10001", "10000")
    );
}

// The Window switcher.

#[test]
fn the_defaults_give_a_window_switcher_of_five_columns_of_thumbnails() {
    let settings = settings(&defaults());
    assert_eq!(
        settings.switcher,
        SwitcherSettings {
            max_columns: 5,
            thumbnails: true,
            icon_size: 48,
        }
    );
    assert!(!settings.policy.alt_tab_raise_windows);
}

#[test]
fn without_the_window_switchers_keys_it_has_marcos_defaults() {
    let settings = settings(&Reading::default());
    assert_eq!(
        settings.switcher,
        SwitcherSettings {
            max_columns: 5,
            thumbnails: true,
            icon_size: 48,
        }
    );
    assert!(!settings.policy.alt_tab_raise_windows);
}

#[test]
fn alt_tab_max_columns_sets_the_window_switchers_columns() {
    let reading = with(MARCO_GENERAL, "alt-tab-max-columns", Value::Int(3));
    assert_eq!(settings(&reading).switcher.max_columns, 3);
}

#[test]
fn alt_tab_max_columns_out_of_range_is_clamped() {
    let reading = with(MARCO_GENERAL, "alt-tab-max-columns", Value::Int(0));
    let (settings, problem) = problem(&reading);
    assert_eq!(settings.switcher.max_columns, 1);
    assert_eq!(
        problem,
        out_of_range(MARCO_GENERAL, "alt-tab-max-columns", "0", "1")
    );

    let reading = with(MARCO_GENERAL, "alt-tab-max-columns", Value::Int(65));
    let (settings, problem) = self::problem(&reading);
    assert_eq!(settings.switcher.max_columns, 64);
    assert_eq!(
        problem,
        out_of_range(MARCO_GENERAL, "alt-tab-max-columns", "65", "64")
    );
}

#[test]
fn alt_tab_raise_windows_is_passed_to_policy() {
    let reading = with(MARCO_GENERAL, "alt-tab-raise-windows", Value::Bool(true));
    assert!(settings(&reading).policy.alt_tab_raise_windows);
}

#[test]
fn alt_tab_minimized_placement_is_end_where_the_schema_has_no_key_for_it() {
    // Marco 1.29.0's schema, which `defaults` reads, hasn't master's key.
    for reading in [defaults(), Reading::default()] {
        let policy = settings(&reading).policy;
        assert_eq!(policy.alt_tab_minimized_placement, MinimizedPlacement::End);
    }
}

#[test]
fn every_alt_tab_minimized_placement_nick_is_known() {
    for (nick, placement) in [
        ("mru", MinimizedPlacement::Mru),
        ("end", MinimizedPlacement::End),
        ("hidden", MinimizedPlacement::Hidden),
    ] {
        let reading = with(MARCO_GENERAL, "alt-tab-minimized-placement", string(nick));
        let policy = settings(&reading).policy;
        assert_eq!(policy.alt_tab_minimized_placement, placement, "{nick}");
    }
}

// Placement (154).

#[test]
fn the_defaults_place_new_windows_automatically_in_the_middle_of_the_work_area() {
    for reading in [defaults(), Reading::default()] {
        let policy = settings(&reading).policy;
        assert_eq!(policy.placement_mode, PlacementMode::Automatic);
        assert!(policy.center_new_windows);
    }
}

#[test]
fn every_placement_mode_nick_is_known() {
    for (nick, mode) in [
        ("automatic", PlacementMode::Automatic),
        ("pointer", PlacementMode::Pointer),
        ("manual", PlacementMode::Manual),
    ] {
        let reading = with(MARCO_GENERAL, "placement-mode", string(nick));
        assert_eq!(settings(&reading).policy.placement_mode, mode, "{nick}");
    }
}

#[test]
fn center_new_windows_off_is_passed_to_policy() {
    let reading = with(MARCO_GENERAL, "center-new-windows", Value::Bool(false));
    assert!(!settings(&reading).policy.center_new_windows);
}

// `show-desktop-skip-list` (164).

/// Set, it isn't reported as unsupported, which `settings` checks.
#[test]
fn the_show_desktop_skip_list_is_passed_to_policy() {
    let reading = with(
        MARCO_GENERAL,
        "show-desktop-skip-list",
        string("Conky, xclock"),
    );
    assert_eq!(
        settings(&reading).policy.show_desktop_skip_list,
        ShowDesktopSkipList::parse("Conky,xclock")
    );
}

// Tiling (158).

#[test]
fn the_defaults_tile_at_the_sides_and_corners_and_maximise_at_the_top() {
    for reading in [defaults(), Reading::default()] {
        let policy = settings(&reading).policy;
        assert!(policy.allow_tiling);
        assert!(policy.allow_top_tiling);
        assert!(policy.allow_tile_cycling);
        assert_eq!(policy.drag_threshold, 8);
    }
}

/// Set off, neither is reported as unsupported, which `settings` checks.
#[test]
fn the_tiling_keys_set_off_are_passed_to_policy() {
    let mut reading = with(MARCO_GENERAL, "allow-tiling", Value::Bool(false));
    for key in ["allow-top-tiling", "allow-tile-cycling"] {
        reading.insert(MARCO_GENERAL, key, Value::Bool(false), Value::Bool(true));
    }
    let policy = settings(&reading).policy;
    assert!(!policy.allow_tiling);
    assert!(!policy.allow_top_tiling);
    assert!(!policy.allow_tile_cycling);
}

#[test]
fn the_drag_threshold_sizes_tilings_zones_too() {
    let reading = with(MOUSE, "drag-threshold", Value::Int(20));
    assert_eq!(settings(&reading).policy.drag_threshold, 20);
}

#[test]
fn the_window_switcher_shows_icons_alone_without_the_compositing_manager() {
    let reading = with(MARCO_GENERAL, "compositing-manager", Value::Bool(false));
    assert!(!settings(&reading).switcher.thumbnails);
}

// The minimise animation (172).

#[test]
fn the_defaults_animate_a_minimised_window() {
    assert!(settings(&defaults()).minimize_animation);
    assert!(settings(&Reading::default()).minimize_animation);
}

#[test]
fn each_of_three_keys_turns_the_minimise_animation_off() {
    for (schema, key, value) in [
        (MARCO_GENERAL, "compositing-manager", false),
        (MARCO_GENERAL, "reduced-resources", true),
        (INTERFACE, "enable-animations", false),
    ] {
        let reading = with(schema, key, Value::Bool(value));
        assert!(!settings(&reading).minimize_animation, "{key}");
    }
}

// The Workspace slide (233).

#[test]
fn the_defaults_slide_a_workspace_switch() {
    assert!(settings(&defaults()).workspace_slide);
    assert!(settings(&Reading::default()).workspace_slide);
}

#[test]
fn its_own_key_and_the_minimise_animations_three_turn_the_workspace_slide_off() {
    for (schema, key, value) in [
        (CAMINITO, "workspace-slide", false),
        (MARCO_GENERAL, "compositing-manager", false),
        (MARCO_GENERAL, "reduced-resources", true),
        (INTERFACE, "enable-animations", false),
    ] {
        let reading = with(schema, key, Value::Bool(value));
        assert!(!settings(&reading).workspace_slide, "{key}");
    }
}

#[test]
fn the_workspace_slides_key_leaves_the_minimise_animation_on() {
    let reading = with(CAMINITO, "workspace-slide", Value::Bool(false));
    assert!(settings(&reading).minimize_animation);
}

// The Overview's transition (243).

#[test]
fn the_defaults_animate_the_overview() {
    assert!(settings(&defaults()).overview_animation);
    assert!(settings(&Reading::default()).overview_animation);
}

#[test]
fn its_own_key_and_the_minimise_animations_three_turn_the_overviews_transition_off() {
    for (schema, key, value) in [
        (CAMINITO, "overview-animation", false),
        (MARCO_GENERAL, "compositing-manager", false),
        (MARCO_GENERAL, "reduced-resources", true),
        (INTERFACE, "enable-animations", false),
    ] {
        let reading = with(schema, key, Value::Bool(value));
        assert!(!settings(&reading).overview_animation, "{key}");
    }
}

#[test]
fn the_overviews_key_leaves_the_other_animations_on() {
    let reading = with(CAMINITO, "overview-animation", Value::Bool(false));
    assert!(settings(&reading).minimize_animation);
    assert!(settings(&reading).workspace_slide);
}

// The Opening animation (251).

#[test]
fn the_defaults_animate_a_window_that_opens() {
    assert!(settings(&defaults()).open_close_animation);
    assert!(settings(&Reading::default()).open_close_animation);
}

#[test]
fn its_own_key_and_the_minimise_animations_three_turn_the_opening_animation_off() {
    for (schema, key, value) in [
        (CAMINITO, "open-close-animation", false),
        (MARCO_GENERAL, "compositing-manager", false),
        (MARCO_GENERAL, "reduced-resources", true),
        (INTERFACE, "enable-animations", false),
    ] {
        let reading = with(schema, key, Value::Bool(value));
        assert!(!settings(&reading).open_close_animation, "{key}");
    }
}

#[test]
fn the_opening_animations_key_leaves_the_other_animations_on() {
    let reading = with(CAMINITO, "open-close-animation", Value::Bool(false));
    let settings = settings(&reading);
    assert!(settings.minimize_animation);
    assert!(settings.workspace_slide);
    assert!(settings.overview_animation);
}

// The Maximise animation (CMN-174).

#[test]
fn the_defaults_animate_a_window_that_is_maximised() {
    assert!(settings(&defaults()).maximize_animation);
    assert!(settings(&Reading::default()).maximize_animation);
}

#[test]
fn its_own_key_and_the_minimise_animations_three_turn_the_maximise_animation_off() {
    for (schema, key, value) in [
        (CAMINITO, "maximize-animation", false),
        (MARCO_GENERAL, "compositing-manager", false),
        (MARCO_GENERAL, "reduced-resources", true),
        (INTERFACE, "enable-animations", false),
    ] {
        let reading = with(schema, key, Value::Bool(value));
        assert!(!settings(&reading).maximize_animation, "{key}");
    }
}

#[test]
fn the_maximise_animations_key_leaves_the_other_animations_on() {
    let reading = with(CAMINITO, "maximize-animation", Value::Bool(false));
    let settings = settings(&reading);
    assert!(settings.minimize_animation);
    assert!(settings.workspace_slide);
    assert!(settings.overview_animation);
    assert!(settings.open_close_animation);
    assert!(settings.popup_slide);
}

// The Popup slide (ADR 0037).

#[test]
fn the_defaults_slide_a_popup_that_opens() {
    assert!(settings(&defaults()).popup_slide);
    assert!(settings(&Reading::default()).popup_slide);
}

#[test]
fn its_own_key_and_the_minimise_animations_three_turn_the_popup_slide_off() {
    for (schema, key, value) in [
        (CAMINITO, "popup-slide", false),
        (MARCO_GENERAL, "compositing-manager", false),
        (MARCO_GENERAL, "reduced-resources", true),
        (INTERFACE, "enable-animations", false),
    ] {
        let reading = with(schema, key, Value::Bool(value));
        assert!(!settings(&reading).popup_slide, "{key}");
    }
}

#[test]
fn the_popup_slides_key_leaves_the_other_animations_on() {
    let reading = with(CAMINITO, "popup-slide", Value::Bool(false));
    let settings = settings(&reading);
    assert!(settings.minimize_animation);
    assert!(settings.workspace_slide);
    assert!(settings.overview_animation);
    assert!(settings.open_close_animation);
    assert!(settings.maximize_animation);
}

#[test]
fn frames_have_no_shadows_without_the_compositing_manager() {
    let reading = with(MARCO_GENERAL, "compositing-manager", Value::Bool(false));
    assert!(!settings(&reading).frames.shadows);
    // Fast Alt+Tab is the Window switcher's alone.
    let reading = with(MARCO_GENERAL, "compositing-fast-alt-tab", Value::Bool(true));
    assert!(settings(&reading).frames.shadows);
}

#[test]
fn the_window_switcher_shows_icons_alone_with_fast_alt_tab() {
    let reading = with(MARCO_GENERAL, "compositing-fast-alt-tab", Value::Bool(true));
    assert!(!settings(&reading).switcher.thumbnails);
}

#[test]
fn icon_size_sets_the_window_switchers_icons() {
    let reading = with(MARCO_GENERAL, "icon-size", Value::Int(96));
    assert_eq!(settings(&reading).switcher.icon_size, 96);
}

#[test]
fn icon_size_out_of_range_is_clamped() {
    let reading = with(MARCO_GENERAL, "icon-size", Value::Int(4));
    let (settings, problem) = problem(&reading);
    assert_eq!(settings.switcher.icon_size, 8);
    assert_eq!(problem, out_of_range(MARCO_GENERAL, "icon-size", "4", "8"));

    let reading = with(MARCO_GENERAL, "icon-size", Value::Int(300));
    let (settings, problem) = self::problem(&reading);
    assert_eq!(settings.switcher.icon_size, 256);
    assert_eq!(
        problem,
        out_of_range(MARCO_GENERAL, "icon-size", "300", "256")
    );
}

// org.mate.interface.

/// The GTK options at MATE 1.28's defaults.
fn default_gtk_options() -> GtkOptions {
    GtkOptions {
        key_theme: "Default".into(),
        toolbar_style: "both-horiz".into(),
        toolbar_icons_size: "large-toolbar".into(),
        cursor_blink: true,
        cursor_blink_time: 1200,
        color_scheme: String::new(),
        im_preedit_style: "callback".into(),
        im_status_style: "callback".into(),
        im_module: String::new(),
        file_chooser_backend: "gio".into(),
        decoration_layout: "menu:minimize,maximize,close".into(),
        shell_shows_app_menu: false,
        shell_shows_menubar: false,
        menus_have_icons: true,
        buttons_have_icons: true,
        menubar_accel: "F10".into(),
        show_input_method_menu: true,
        show_unicode_menu: true,
        automatic_mnemonics: true,
        enable_primary_paste: true,
        enable_animations: true,
        dialogs_use_header: false,
    }
}

#[test]
fn the_defaults_give_mates_interface() {
    assert_eq!(
        settings(&defaults()).interface,
        Interface {
            gtk_theme: "Menta".into(),
            font_name: "Sans 10".into(),
            icon_theme: "menta".into(),
            gtk: default_gtk_options(),
            window_scaling_factor: WindowScalingFactor::Auto,
        }
    );
}

#[test]
fn the_builtin_interface_matches_the_schema_defaults() {
    assert_eq!(
        settings(&Reading::default()).interface,
        settings(&defaults()).interface
    );
}

#[test]
fn the_gtk_options_are_read() {
    let mut reading = defaults();
    let mut set = |key: &str, value: Value| {
        let default = reading.get(INTERFACE, key).unwrap().default.clone();
        reading.insert(INTERFACE, key, value, default);
    };
    set("gtk-key-theme", string("Emacs"));
    set("toolbar-style", string("icons"));
    set("toolbar-icons-size", string("small-toolbar"));
    set("cursor-blink", Value::Bool(false));
    set("cursor-blink-time", Value::Int(800));
    set("gtk-color-scheme", string("fg_color:#000"));
    set("gtk-im-preedit-style", string("nothing"));
    set("gtk-im-status-style", string("none"));
    set("gtk-im-module", string("ibus"));
    set("file-chooser-backend", string("gnome-vfs"));
    set("gtk-decoration-layout", string("close:"));
    set("gtk-shell-shows-app-menu", Value::Bool(true));
    set("gtk-shell-shows-menubar", Value::Bool(true));
    set("menus-have-icons", Value::Bool(false));
    set("buttons-have-icons", Value::Bool(false));
    set("menubar-accel", string("F9"));
    set("show-input-method-menu", Value::Bool(false));
    set("show-unicode-menu", Value::Bool(false));
    set("automatic-mnemonics", Value::Bool(false));
    set("gtk-enable-primary-paste", Value::Bool(false));
    set("gtk-enable-animations", Value::Bool(false));
    set("gtk-dialogs-use-header", Value::Bool(true));
    assert_eq!(
        settings(&reading).interface.gtk,
        GtkOptions {
            key_theme: "Emacs".into(),
            toolbar_style: "icons".into(),
            toolbar_icons_size: "small-toolbar".into(),
            cursor_blink: false,
            cursor_blink_time: 800,
            color_scheme: "fg_color:#000".into(),
            im_preedit_style: "nothing".into(),
            im_status_style: "none".into(),
            im_module: "ibus".into(),
            file_chooser_backend: "gnome-vfs".into(),
            decoration_layout: "close:".into(),
            shell_shows_app_menu: true,
            shell_shows_menubar: true,
            menus_have_icons: false,
            buttons_have_icons: false,
            menubar_accel: "F9".into(),
            show_input_method_menu: false,
            show_unicode_menu: false,
            automatic_mnemonics: false,
            enable_primary_paste: false,
            enable_animations: false,
            dialogs_use_header: true,
        }
    );
}

#[test]
fn interface_keys_are_read() {
    let mut reading = with(INTERFACE, "gtk-theme", string("BlackMATE"));
    reading.insert(
        INTERFACE,
        "font-name",
        string("Cantarell 11"),
        string("Sans 10"),
    );
    reading.insert(INTERFACE, "icon-theme", string("mate"), string("menta"));
    assert_eq!(
        settings(&reading).interface,
        Interface {
            gtk_theme: "BlackMATE".into(),
            font_name: "Cantarell 11".into(),
            icon_theme: "mate".into(),
            gtk: default_gtk_options(),
            window_scaling_factor: WindowScalingFactor::Auto,
        }
    );
}

#[test]
fn window_scaling_factor_0_leaves_outputs_to_the_default_rule() {
    let reading = with(INTERFACE, "window-scaling-factor", Value::Int(0));
    assert_eq!(
        settings(&reading).interface.window_scaling_factor,
        WindowScalingFactor::Auto
    );
}

#[test]
fn window_scaling_factor_1_or_2_forces_that_integer() {
    for factor in [1, 2] {
        let reading = with(INTERFACE, "window-scaling-factor", Value::Int(factor));
        assert_eq!(
            settings(&reading).interface.window_scaling_factor,
            WindowScalingFactor::Forced(factor as u32)
        );
    }
}

#[test]
fn window_scaling_factor_out_of_range_is_clamped() {
    let (above, above_problem) = problem(&with(INTERFACE, "window-scaling-factor", Value::Int(3)));
    assert_eq!(
        above.interface.window_scaling_factor,
        WindowScalingFactor::Forced(2)
    );
    assert_eq!(
        above_problem,
        out_of_range(INTERFACE, "window-scaling-factor", "3", "2")
    );

    let (below, below_problem) = problem(&with(INTERFACE, "window-scaling-factor", Value::Int(-1)));
    assert_eq!(
        below.interface.window_scaling_factor,
        WindowScalingFactor::Auto
    );
    assert_eq!(
        below_problem,
        out_of_range(INTERFACE, "window-scaling-factor", "-1", "0")
    );
}

#[test]
fn an_empty_gtk_theme_or_font_falls_back_to_the_default() {
    let (theme, theme_problem) = problem(&with(INTERFACE, "gtk-theme", string("")));
    assert_eq!(theme.interface.gtk_theme, "Menta");
    assert_eq!(
        theme_problem,
        invalid(INTERFACE, "gtk-theme", "''", "'Menta'")
    );

    let (font, font_problem) = problem(&with(INTERFACE, "font-name", string("")));
    assert_eq!(font.interface.font_name, "Sans 10");
    assert_eq!(
        font_problem,
        invalid(INTERFACE, "font-name", "''", "'Sans 10'")
    );
}

#[test]
fn an_empty_icon_theme_falls_back_to_the_default() {
    let (settings, problem) = problem(&with(INTERFACE, "icon-theme", string("")));
    assert_eq!(settings.interface.icon_theme, "menta");
    assert_eq!(problem, invalid(INTERFACE, "icon-theme", "''", "'menta'"));
}

// org.mate.font-rendering.

#[test]
fn the_defaults_give_mates_font_rendering() {
    assert_eq!(
        settings(&defaults()).font_rendering,
        FontRendering {
            antialiasing: Antialiasing::Rgba,
            hinting: Hinting::Slight,
            rgba_order: RgbaOrder::Rgb,
            dpi: 0.0,
        }
    );
}

#[test]
fn dpi_is_read_as_it_is() {
    // What a DPI of 0 or less means is XSETTINGS' to say (40).
    for dpi in [120.0, -3.0] {
        let reading = with(FONT_RENDERING, "dpi", Value::Double(dpi));
        assert_eq!(settings(&reading).font_rendering.dpi, dpi);
    }
}

#[test]
fn every_font_rendering_nick_is_known() {
    for (nick, value) in [
        ("none", Antialiasing::None),
        ("grayscale", Antialiasing::Grayscale),
        ("rgba", Antialiasing::Rgba),
    ] {
        let reading = with(FONT_RENDERING, "antialiasing", string(nick));
        assert_eq!(settings(&reading).font_rendering.antialiasing, value);
    }
    for (nick, value) in [
        ("none", Hinting::None),
        ("slight", Hinting::Slight),
        ("medium", Hinting::Medium),
        ("full", Hinting::Full),
    ] {
        let reading = with(FONT_RENDERING, "hinting", string(nick));
        assert_eq!(settings(&reading).font_rendering.hinting, value);
    }
    for (nick, value) in [
        ("rgb", RgbaOrder::Rgb),
        ("bgr", RgbaOrder::Bgr),
        ("vrgb", RgbaOrder::Vrgb),
        ("vbgr", RgbaOrder::Vbgr),
    ] {
        let reading = with(FONT_RENDERING, "rgba-order", string(nick));
        assert_eq!(settings(&reading).font_rendering.rgba_order, value);
    }
}

#[test]
fn an_unknown_font_rendering_nick_falls_back_to_the_default() {
    let (settings, problem) = problem(&with(FONT_RENDERING, "hinting", string("extreme")));
    assert_eq!(settings.font_rendering.hinting, Hinting::Slight);
    assert_eq!(
        problem,
        invalid(FONT_RENDERING, "hinting", "'extreme'", "'slight'")
    );
}

// org.mate.sound.

#[test]
fn the_defaults_give_mates_sound() {
    assert_eq!(
        settings(&defaults()).sound,
        Sound {
            theme_name: "freedesktop".into(),
            event_sounds: false,
            input_feedback_sounds: false,
        }
    );
}

#[test]
fn sound_keys_are_read() {
    let mut reading = with(SOUND, "theme-name", string("mate"));
    reading.insert(SOUND, "event-sounds", Value::Bool(true), Value::Bool(false));
    reading.insert(
        SOUND,
        "input-feedback-sounds",
        Value::Bool(true),
        Value::Bool(false),
    );
    assert_eq!(
        settings(&reading).sound,
        Sound {
            theme_name: "mate".into(),
            event_sounds: true,
            input_feedback_sounds: true,
        }
    );
}

#[test]
fn without_the_sound_schema_its_keys_are_defaults() {
    let mut reading = Reading::default();
    reading.set_missing(SOUND);
    let (settings, problem) = problem(&reading);
    assert_eq!(settings.sound, self::settings(&defaults()).sound);
    assert_eq!(problem, Problem::MissingSchema(SOUND.into()));
}

// Marco's bell keys.

#[test]
fn the_defaults_give_marcos_bell() {
    assert_eq!(
        settings(&defaults()).bell,
        Bell {
            audible: true,
            visual: false,
            visual_type: VisualBellType::Fullscreen,
        }
    );
}

#[test]
fn bell_keys_are_read() {
    let reading = with_all(&[
        (MARCO_GENERAL, "audible-bell", Value::Bool(false)),
        (MARCO_GENERAL, "visual-bell", Value::Bool(true)),
        (MARCO_GENERAL, "visual-bell-type", string("frame_flash")),
    ]);
    assert_eq!(
        translate(&reading).settings.bell,
        Bell {
            audible: false,
            visual: true,
            visual_type: VisualBellType::FrameFlash,
        }
    );
}

#[test]
fn bell_mode_off_silences_the_audible_bell_and_leaves_the_visual_one() {
    let reading = with_all(&[
        (MARCO_GENERAL, "audible-bell", Value::Bool(true)),
        (MARCO_GENERAL, "visual-bell", Value::Bool(true)),
        (KEYBOARD, "bell-mode", string("off")),
    ]);
    let bell = translate(&reading).settings.bell;
    assert!(!bell.audible);
    assert!(bell.visual);
}

#[test]
fn bell_mode_on_and_custom_leave_the_audible_bell_to_its_key() {
    for mode in ["on", "custom"] {
        for audible in [true, false] {
            let reading = with_all(&[
                (MARCO_GENERAL, "audible-bell", Value::Bool(audible)),
                (KEYBOARD, "bell-mode", string(mode)),
            ]);
            assert_eq!(translate(&reading).settings.bell.audible, audible);
        }
    }
}

#[test]
fn an_unknown_visual_bell_type_is_fullscreen() {
    let (settings, problem) = problem(&with(MARCO_GENERAL, "visual-bell-type", string("ripple")));
    assert_eq!(settings.bell.visual_type, VisualBellType::Fullscreen);
    assert_eq!(
        problem,
        invalid(
            MARCO_GENERAL,
            "visual-bell-type",
            "'ripple'",
            "'fullscreen'"
        )
    );
}

// org.mate.peripherals-mouse.

#[test]
fn the_defaults_give_mates_mouse() {
    assert_eq!(
        settings(&defaults()).mouse,
        Mouse {
            cursor_theme: String::new(),
            cursor_size: 24,
            double_click_time: Duration::from_millis(400),
            drag_threshold: 8,
        }
    );
}

#[test]
fn mouse_keys_are_read() {
    let mut reading = with(MOUSE, "cursor-theme", string("mate-black"));
    reading.insert(MOUSE, "cursor-size", Value::Int(48), Value::Int(24));
    reading.insert(MOUSE, "double-click", Value::Int(250), Value::Int(400));
    reading.insert(MOUSE, "drag-threshold", Value::Int(0), Value::Int(8));
    assert_eq!(
        settings(&reading).mouse,
        Mouse {
            cursor_theme: "mate-black".into(),
            cursor_size: 48,
            double_click_time: Duration::from_millis(250),
            drag_threshold: 0,
        }
    );
}

#[test]
fn a_cursor_size_below_1_falls_back_to_the_default() {
    let (settings, problem) = problem(&with(MOUSE, "cursor-size", Value::Int(0)));
    assert_eq!(settings.mouse.cursor_size, 24);
    assert_eq!(problem, invalid(MOUSE, "cursor-size", "0", "24"));
}

#[test]
fn a_double_click_time_below_1_falls_back_to_the_default() {
    let (settings, problem) = problem(&with(MOUSE, "double-click", Value::Int(-5)));
    assert_eq!(settings.mouse.double_click_time, Duration::from_millis(400));
    assert_eq!(problem, invalid(MOUSE, "double-click", "-5", "400"));
}

#[test]
fn a_negative_drag_threshold_falls_back_to_the_default() {
    let (settings, problem) = problem(&with(MOUSE, "drag-threshold", Value::Int(-1)));
    assert_eq!(settings.mouse.drag_threshold, 8);
    assert_eq!(problem, invalid(MOUSE, "drag-threshold", "-1", "8"));
}

// org.mate.peripherals-mouse and -touchpad on devices (53).

/// The defaults with each key set.
fn with_all(keys: &[(&str, &str, Value)]) -> Reading {
    let mut reading = defaults();
    for (schema, key, value) in keys {
        let default = reading
            .get(schema, key)
            .map_or(value.clone(), |e| e.default.clone());
        reading.insert(schema, key, value.clone(), default);
    }
    reading
}

fn mouse_device(reading: &Reading) -> DeviceSettings {
    settings(reading).devices.mouse
}

fn touchpad(reading: &Reading) -> DeviceSettings {
    settings(reading).devices.touchpad
}

#[test]
fn the_defaults_give_a_mouse_as_msd_sets_it() {
    assert_eq!(
        mouse_device(&defaults()),
        DeviceSettings {
            send_events: None,
            left_handed: false,
            accel_speed: None,
            accel_profile: AccelProfile::Default,
            natural_scroll: false,
            middle_emulation: Some(true),
            tap: None,
            disable_while_typing: None,
            click_method: None,
            scroll_method: None,
            horizontal_scroll: true,
        }
    );
}

#[test]
fn the_defaults_give_a_touchpad_as_msd_sets_it() {
    assert_eq!(
        touchpad(&defaults()),
        DeviceSettings {
            send_events: Some(true),
            left_handed: false,
            accel_speed: None,
            accel_profile: AccelProfile::Default,
            natural_scroll: false,
            middle_emulation: None,
            tap: Some(false),
            disable_while_typing: Some(false),
            click_method: Some(ClickMethod::ButtonAreas),
            scroll_method: Some(ScrollMethod::TwoFinger),
            horizontal_scroll: false,
        }
    );
}

#[test]
fn mouse_device_keys_are_read() {
    let reading = with_all(&[
        (MOUSE, "left-handed", Value::Bool(true)),
        (MOUSE, "accel-profile", string("flat")),
        (MOUSE, "middle-button-enabled", Value::Bool(false)),
        (MOUSE, "natural-scroll", Value::Bool(true)),
    ]);
    let mouse = mouse_device(&reading);
    assert!(mouse.left_handed);
    assert_eq!(mouse.accel_profile, AccelProfile::Flat);
    assert_eq!(mouse.middle_emulation, Some(false));
    assert!(mouse.natural_scroll);
}

#[test]
fn natural_scroll_is_off_where_the_mouse_schema_lacks_it() {
    // mate-settings-daemon 1.28's schema has no such key.
    assert!(!mouse_device(&defaults()).natural_scroll);
}

#[test]
fn motion_acceleration_minus_1_is_the_devices_default_speed() {
    let reading = with(MOUSE, "motion-acceleration", Value::Double(-1.0));
    assert_eq!(mouse_device(&reading).accel_speed, None);
}

#[test]
fn motion_acceleration_maps_1_to_10_onto_libinputs_speed() {
    for (m, speed) in [(1.0, -1.0), (5.5, 0.0), (10.0, 1.0), (3.25, -0.5)] {
        let reading = with(MOUSE, "motion-acceleration", Value::Double(m));
        assert_eq!(mouse_device(&reading).accel_speed, Some(speed), "for {m}");
    }
}

#[test]
fn motion_acceleration_beyond_the_range_is_clamped() {
    for (m, speed) in [(0.0, -1.0), (-3.0, -1.0), (12.0, 1.0)] {
        let reading = with(MOUSE, "motion-acceleration", Value::Double(m));
        assert_eq!(mouse_device(&reading).accel_speed, Some(speed), "for {m}");
    }
}

#[test]
fn every_accel_profile_nick_is_known() {
    for (nick, profile) in [
        ("default", AccelProfile::Default),
        ("adaptive", AccelProfile::Adaptive),
        ("flat", AccelProfile::Flat),
    ] {
        let reading = with_all(&[
            (MOUSE, "accel-profile", string(nick)),
            (TOUCHPAD, "accel-profile", string(nick)),
        ]);
        assert_eq!(mouse_device(&reading).accel_profile, profile);
        assert_eq!(touchpad(&reading).accel_profile, profile);
    }
}

#[test]
fn the_touchpad_has_its_own_acceleration_and_natural_scroll() {
    let reading = with_all(&[
        (TOUCHPAD, "motion-acceleration", Value::Double(10.0)),
        (TOUCHPAD, "accel-profile", string("adaptive")),
        (TOUCHPAD, "natural-scroll", Value::Bool(true)),
    ]);
    let settings = settings(&reading);
    assert_eq!(settings.devices.touchpad.accel_speed, Some(1.0));
    assert_eq!(
        settings.devices.touchpad.accel_profile,
        AccelProfile::Adaptive
    );
    assert!(settings.devices.touchpad.natural_scroll);
    assert_eq!(settings.devices.mouse.accel_speed, None);
    assert_eq!(settings.devices.mouse.accel_profile, AccelProfile::Default);
    assert!(!settings.devices.mouse.natural_scroll);
}

#[test]
fn touchpad_enabled_off_stops_touchpads_sending_events() {
    let reading = with(TOUCHPAD, "touchpad-enabled", Value::Bool(false));
    assert_eq!(touchpad(&reading).send_events, Some(false));
}

#[test]
fn touchpad_left_handed_left_and_right_ignore_the_mouse() {
    for (mouse, nick, left) in [
        (true, "right", false),
        (false, "left", true),
        (true, "left", true),
    ] {
        let reading = with_all(&[
            (MOUSE, "left-handed", Value::Bool(mouse)),
            (TOUCHPAD, "left-handed", string(nick)),
        ]);
        assert_eq!(touchpad(&reading).left_handed, left, "{nick}");
    }
}

#[test]
fn touchpad_left_handed_mouse_follows_the_mouse_key() {
    for mouse in [false, true] {
        let reading = with_all(&[
            (MOUSE, "left-handed", Value::Bool(mouse)),
            (TOUCHPAD, "left-handed", string("mouse")),
        ]);
        assert_eq!(touchpad(&reading).left_handed, mouse);
    }
}

#[test]
fn tap_to_click_and_disable_while_typing_are_read() {
    let reading = with_all(&[
        (TOUCHPAD, "tap-to-click", Value::Bool(true)),
        (TOUCHPAD, "disable-while-typing", Value::Bool(true)),
    ]);
    assert_eq!(touchpad(&reading).tap, Some(true));
    assert_eq!(touchpad(&reading).disable_while_typing, Some(true));
}

#[test]
fn a_two_or_three_finger_click_button_picks_clickfinger() {
    for (two, three, method) in [
        (0, 0, ClickMethod::ButtonAreas),
        (3, 0, ClickMethod::Clickfinger),
        (0, 2, ClickMethod::Clickfinger),
        (1, 1, ClickMethod::Clickfinger),
    ] {
        let reading = with_all(&[
            (TOUCHPAD, "two-finger-click", Value::Int(two)),
            (TOUCHPAD, "three-finger-click", Value::Int(three)),
        ]);
        assert_eq!(
            touchpad(&reading).click_method,
            Some(method),
            "{two} {three}"
        );
    }
}

#[test]
fn two_finger_scrolling_beats_edge_scrolling_and_neither_means_none() {
    for (two_finger, edge, method) in [
        (true, true, ScrollMethod::TwoFinger),
        (true, false, ScrollMethod::TwoFinger),
        (false, true, ScrollMethod::Edge),
        (false, false, ScrollMethod::NoScroll),
    ] {
        let reading = with_all(&[
            (
                TOUCHPAD,
                "vertical-two-finger-scrolling",
                Value::Bool(two_finger),
            ),
            (TOUCHPAD, "vertical-edge-scrolling", Value::Bool(edge)),
        ]);
        assert_eq!(
            touchpad(&reading).scroll_method,
            Some(method),
            "{two_finger} {edge}"
        );
    }
}

#[test]
fn horizontal_scrolling_follows_the_key_of_the_chosen_method() {
    let cases = [
        // Two-finger scrolling, whatever the edge keys say.
        (true, true, false, true, false),
        (true, true, true, false, true),
        // Edge scrolling.
        (false, true, true, false, false),
        (false, true, false, true, true),
    ];
    for (two_finger, edge, horizontal_two_finger, horizontal_edge, horizontal) in cases {
        let reading = with_all(&[
            (
                TOUCHPAD,
                "vertical-two-finger-scrolling",
                Value::Bool(two_finger),
            ),
            (TOUCHPAD, "vertical-edge-scrolling", Value::Bool(edge)),
            (
                TOUCHPAD,
                "horizontal-two-finger-scrolling",
                Value::Bool(horizontal_two_finger),
            ),
            (
                TOUCHPAD,
                "horizontal-edge-scrolling",
                Value::Bool(horizontal_edge),
            ),
        ]);
        assert_eq!(touchpad(&reading).horizontal_scroll, horizontal);
    }
}

#[test]
fn the_never_keys_change_nothing_and_arent_reported() {
    let reading = with_all(&[
        (MOUSE, "motion-threshold", Value::Int(5)),
        (TOUCHPAD, "motion-threshold", Value::Int(5)),
        (TOUCHPAD, "tap-button-one-finger", Value::Int(3)),
        (TOUCHPAD, "tap-button-two-finger", Value::Int(1)),
        (TOUCHPAD, "tap-button-three-finger", Value::Int(1)),
        (KEYBOARD, "click", Value::Bool(false)),
        (KEYBOARD, "click-volume", Value::Int(50)),
        (KEYBOARD, "bell-pitch", Value::Int(800)),
        (KEYBOARD, "bell-duration", Value::Int(10)),
    ]);
    assert_eq!(settings(&reading), settings(&defaults()));
}

#[test]
fn the_visual_bell_set_on_is_honoured_without_a_problem() {
    let problems = translate(&with(MARCO_GENERAL, "visual-bell", Value::Bool(true))).problems;
    assert_eq!(problems, []);
}

#[test]
fn locate_pointer_set_on_is_reported_as_unsupported() {
    let problems = translate(&with(MOUSE, "locate-pointer", Value::Bool(true))).problems;
    assert_eq!(
        problems,
        [Problem::Unsupported {
            schema: MOUSE.into(),
            key: "locate-pointer".into(),
            value: "true".into(),
        }]
    );
}

// org.mate.accessibility-keyboard (134).

#[test]
fn the_defaults_turn_every_accessx_feature_off() {
    let millis = Duration::from_millis;
    assert_eq!(
        settings(&defaults()).accessx,
        AccessX {
            gestures: false,
            sticky_keys: false,
            sticky_latch_to_lock: true,
            sticky_two_key_off: false,
            sticky_modifier_beep: false,
            slow_keys: false,
            slow_delay: millis(300),
            slow_beep_press: false,
            slow_beep_accept: false,
            slow_beep_reject: false,
            bounce_keys: false,
            bounce_delay: millis(300),
            bounce_beep_reject: false,
            toggle_keys: false,
        }
    );
}

#[test]
fn each_accessx_key_sets_its_own_setting() {
    let on = |key: &str| settings(&with(A11Y_KEYBOARD, key, Value::Bool(true))).accessx;
    assert!(on("enable").gestures);
    assert!(on("stickykeys-enable").sticky_keys);
    assert!(on("stickykeys-two-key-off").sticky_two_key_off);
    assert!(on("stickykeys-modifier-beep").sticky_modifier_beep);
    assert!(on("slowkeys-enable").slow_keys);
    assert!(on("slowkeys-beep-press").slow_beep_press);
    assert!(on("slowkeys-beep-accept").slow_beep_accept);
    assert!(on("slowkeys-beep-reject").slow_beep_reject);
    assert!(on("bouncekeys-enable").bounce_keys);
    assert!(on("bouncekeys-beep-reject").bounce_beep_reject);
    assert!(on("togglekeys-enable").toggle_keys);
    let latch = with(
        A11Y_KEYBOARD,
        "stickykeys-latch-to-lock",
        Value::Bool(false),
    );
    assert!(!settings(&latch).accessx.sticky_latch_to_lock);

    let reading = with_all(&[
        (A11Y_KEYBOARD, "slowkeys-delay", Value::Int(120)),
        (A11Y_KEYBOARD, "bouncekeys-delay", Value::Int(80)),
    ]);
    let accessx = settings(&reading).accessx;
    assert_eq!(accessx.slow_delay, Duration::from_millis(120));
    assert_eq!(accessx.bounce_delay, Duration::from_millis(80));
}

#[test]
fn a_negative_accessx_delay_takes_the_default() {
    let translation = translate(&with(A11Y_KEYBOARD, "slowkeys-delay", Value::Int(-1)));
    assert_eq!(
        translation.settings.accessx.slow_delay,
        Duration::from_millis(300)
    );
    assert_eq!(
        translation.problems,
        [Problem::Invalid {
            schema: A11Y_KEYBOARD.into(),
            key: "slowkeys-delay".into(),
            value: "-1".into(),
            used: "300".into(),
        }]
    );
}

#[test]
fn the_accessx_keys_the_filter_doesnt_apply_are_reported_as_unsupported_when_on() {
    for key in [
        "mousekeys-enable",
        "timeout-enable",
        "capslock-beep-enable",
        "feature-state-change-beep",
    ] {
        let problems = translate(&with(A11Y_KEYBOARD, key, Value::Bool(true))).problems;
        assert_eq!(
            problems,
            [Problem::Unsupported {
                schema: A11Y_KEYBOARD.into(),
                key: key.into(),
                value: "true".into(),
            }]
        );
    }
}

// org.mate.peripherals-keyboard (53).

#[test]
fn the_defaults_give_mates_keyboard() {
    assert_eq!(
        settings(&defaults()).keyboard,
        Keyboard {
            repeat_rate: 30,
            repeat_delay: 500,
            remember_numlock: true,
            numlock: None,
        }
    );
}

#[test]
fn rate_and_delay_are_the_repeat_info() {
    let reading = with_all(&[
        (KEYBOARD, "rate", Value::Int(50)),
        (KEYBOARD, "delay", Value::Int(250)),
    ]);
    let keyboard = settings(&reading).keyboard;
    assert_eq!((keyboard.repeat_rate, keyboard.repeat_delay), (50, 250));
}

#[test]
fn repeat_off_sends_rate_0() {
    let reading = with(KEYBOARD, "repeat", Value::Bool(false));
    let keyboard = settings(&reading).keyboard;
    assert_eq!((keyboard.repeat_rate, keyboard.repeat_delay), (0, 500));
}

#[test]
fn a_rate_of_0_or_less_sends_rate_0() {
    for rate in [0, -4] {
        let reading = with(KEYBOARD, "rate", Value::Int(rate));
        assert_eq!(settings(&reading).keyboard.repeat_rate, 0, "for {rate}");
    }
}

#[test]
fn a_delay_below_1_becomes_1() {
    for delay in [0, -100] {
        let reading = with(KEYBOARD, "delay", Value::Int(delay));
        assert_eq!(settings(&reading).keyboard.repeat_delay, 1, "for {delay}");
    }
}

#[test]
fn numlock_state_is_read_by_nick() {
    for (nick, state) in [("on", Some(true)), ("off", Some(false)), ("unknown", None)] {
        let reading = with(KEYBOARD, "numlock-state", string(nick));
        assert_eq!(settings(&reading).keyboard.numlock, state, "{nick}");
    }
}

#[test]
fn remember_numlock_state_is_read() {
    let reading = with(KEYBOARD, "remember-numlock-state", Value::Bool(false));
    assert!(!settings(&reading).keyboard.remember_numlock);
}

// org.mate.peripherals-keyboard-xkb.kbd (54).

fn strings(values: &[&str]) -> Value {
    Value::StrList(values.iter().map(|&v| v.into()).collect())
}

fn keymap(keys: &[(&str, Value)]) -> Keymap {
    let keys: Vec<_> = keys
        .iter()
        .map(|(key, value)| (KEYBOARD_XKB, *key, value.clone()))
        .collect();
    settings(&with_all(&keys)).keymap
}

fn rmlvo(model: &str, layouts: &str, variants: &str, options: &str) -> Keymap {
    Keymap {
        model: model.into(),
        layouts: layouts.into(),
        variants: variants.into(),
        options: options.into(),
    }
}

#[test]
fn the_defaults_with_no_system_keymap_leave_every_part_to_xkbcommon() {
    assert_eq!(settings(&defaults()).keymap, Keymap::default());
}

#[test]
fn one_layout_without_a_variant_is_the_layout() {
    assert_eq!(
        keymap(&[("layouts", strings(&["us"]))]),
        rmlvo("", "us", "", "")
    );
}

#[test]
fn one_layout_with_a_variant_splits_at_the_tab() {
    assert_eq!(
        keymap(&[("layouts", strings(&["us\tintl"]))]),
        rmlvo("", "us", "intl", "")
    );
}

#[test]
fn several_layouts_are_comma_joined_with_a_variant_each() {
    assert_eq!(
        keymap(&[("layouts", strings(&["us\tdvorak", "es", "de\tnodeadkeys"]))]),
        rmlvo("", "us,es,de", "dvorak,,nodeadkeys", "")
    );
}

#[test]
fn options_drop_their_group_and_are_comma_joined() {
    assert_eq!(
        keymap(&[(
            "options",
            strings(&["grp\tgrp:alt_shift_toggle", "ctrl\tctrl:nocaps"])
        )]),
        rmlvo("", "", "", "grp:alt_shift_toggle,ctrl:nocaps")
    );
}

#[test]
fn an_option_without_a_group_is_taken_whole() {
    assert_eq!(
        keymap(&[("options", strings(&["compose:ralt"]))]),
        rmlvo("", "", "", "compose:ralt")
    );
}

#[test]
fn the_model_is_taken_as_it_is() {
    assert_eq!(
        keymap(&[("model", string("pc105"))]),
        rmlvo("pc105", "", "", "")
    );
}

#[test]
fn empty_parts_take_the_system_keymap() {
    let mut reading = with_all(&[(KEYBOARD_XKB, "layouts", strings(&["es"]))]);
    reading.set_system_keymap(rmlvo("pc104", "de", "nodeadkeys", "caps:escape"));
    assert_eq!(
        settings(&reading).keymap,
        rmlvo("pc104", "es", "", "caps:escape")
    );
}

#[test]
fn set_parts_ignore_the_system_keymap() {
    let mut reading = with_all(&[
        (KEYBOARD_XKB, "model", string("pc105")),
        (KEYBOARD_XKB, "layouts", strings(&["es"])),
        (KEYBOARD_XKB, "options", strings(&["ctrl\tctrl:nocaps"])),
    ]);
    reading.set_system_keymap(rmlvo("pc104", "de", "nodeadkeys", "caps:escape"));
    assert_eq!(
        settings(&reading).keymap,
        rmlvo("pc105", "es", "", "ctrl:nocaps")
    );
}

#[test]
fn the_environment_names_the_system_keymap_before_localed() {
    let environment = |name: &str| match name {
        "XKB_DEFAULT_MODEL" => Some("pc105".to_string()),
        "XKB_DEFAULT_LAYOUT" => Some("fr".to_string()),
        "XKB_DEFAULT_VARIANT" => Some("azerty".to_string()),
        "XKB_DEFAULT_OPTIONS" => Some("compose:ralt".to_string()),
        _ => None,
    };
    let localed = rmlvo("pc104", "de", "nodeadkeys", "caps:escape");
    assert_eq!(
        Keymap::from_environment(environment).or(&localed),
        rmlvo("pc105", "fr", "azerty", "compose:ralt")
    );
}

#[test]
fn what_the_environment_leaves_empty_comes_from_localed() {
    let localed = rmlvo("pc104", "de", "nodeadkeys", "caps:escape");
    assert_eq!(Keymap::from_environment(|_| None).or(&localed), localed);
}

#[test]
fn a_variant_comes_only_with_its_layout() {
    // As xkbcommon does: XKB_DEFAULT_VARIANT without XKB_DEFAULT_LAYOUT is ignored.
    let environment = |name: &str| (name == "XKB_DEFAULT_VARIANT").then(|| "azerty".to_string());
    let localed = rmlvo("", "de", "nodeadkeys", "");
    assert_eq!(
        Keymap::from_environment(environment).or(&localed),
        rmlvo("", "de", "nodeadkeys", "")
    );
}

#[test]
fn without_the_xkb_schema_the_keymap_is_the_system_one() {
    let mut reading = Reading::default();
    reading.insert(
        MARCO_GENERAL,
        "num-workspaces",
        Value::Int(4),
        Value::Int(4),
    );
    reading.set_missing(KEYBOARD_XKB);
    reading.set_system_keymap(rmlvo("", "de", "", ""));
    assert_eq!(translate(&reading).settings.keymap, rmlvo("", "de", "", ""));
}

#[test]
fn without_the_device_schemas_their_keys_are_defaults() {
    let mut reading = Reading::default();
    reading.insert(
        MARCO_GENERAL,
        "num-workspaces",
        Value::Int(4),
        Value::Int(4),
    );
    reading.set_missing(TOUCHPAD);
    reading.set_missing(KEYBOARD);
    let missing = translate(&reading).settings;
    let defaults = settings(&defaults());
    assert_eq!(missing.devices, defaults.devices);
    assert_eq!(missing.keyboard, defaults.keyboard);
}

// Unsupported keys, logging once, and missing schemas.

/// A made-up key, so that no ticket that honours a key takes the example away.
const TEST_UNSUPPORTED: &[(&str, &str)] = &[(MARCO_GENERAL, "test-style")];

fn unsupported_problems(reading: &Reading) -> Vec<Problem> {
    translate_with(reading, TEST_UNSUPPORTED).problems
}

#[test]
fn an_unsupported_key_at_its_default_is_not_reported() {
    let mut reading = defaults();
    reading.insert(
        MARCO_GENERAL,
        "test-style",
        string("plain"),
        string("plain"),
    );
    assert_eq!(unsupported_problems(&reading), []);
}

#[test]
fn an_unsupported_key_set_away_from_its_default_is_reported() {
    let mut reading = defaults();
    reading.insert(
        MARCO_GENERAL,
        "test-style",
        string("fancy"),
        string("plain"),
    );
    assert_eq!(
        unsupported_problems(&reading),
        [Problem::Unsupported {
            schema: MARCO_GENERAL.into(),
            key: "test-style".into(),
            value: "'fancy'".into(),
        }]
    );
}

#[test]
fn an_unsupported_key_that_isnt_installed_is_not_reported() {
    assert_eq!(unsupported_problems(&defaults()), []);
}

#[test]
fn an_unsupported_key_is_logged_once_whatever_its_values() {
    let unsupported = |value: &str| Problem::Unsupported {
        schema: MARCO_GENERAL.into(),
        key: "test-style".into(),
        value: value.into(),
    };
    let mut logged = Logged::default();
    assert_eq!(
        logged.first_time(vec![unsupported("'fancy'")]),
        [unsupported("'fancy'")]
    );
    assert_eq!(logged.first_time(vec![unsupported("'fancier'")]), []);
}

#[test]
fn a_bad_value_is_logged_once_and_a_new_bad_value_again() {
    let mut logged = Logged::default();
    let fifty = out_of_range(MARCO_GENERAL, "num-workspaces", "50", "36");
    let sixty = out_of_range(MARCO_GENERAL, "num-workspaces", "60", "36");
    assert_eq!(
        logged.first_time(vec![fifty.clone()]),
        std::slice::from_ref(&fifty)
    );
    assert_eq!(logged.first_time(vec![fifty]), []);
    assert_eq!(logged.first_time(vec![sixty.clone()]), [sixty]);
}

#[test]
fn every_unsupported_key_is_in_a_schema_caminito_reads() {
    for (schema, key) in UNSUPPORTED {
        assert!(
            SCHEMAS.iter().any(|s| s.id == *schema),
            "{schema} {key} isn't read"
        );
    }
}

#[test]
fn a_missing_optional_schema_is_reported_and_its_keys_are_defaults() {
    let mut reading = Reading::default();
    for (key, value) in [
        ("num-workspaces", Value::Int(4)),
        ("theme", string("Menta")),
    ] {
        reading.insert(MARCO_GENERAL, key, value.clone(), value);
    }
    reading.set_missing(MOUSE);
    let Translation { settings, problems } = translate(&reading);
    assert_eq!(problems, [Problem::MissingSchema(MOUSE.into())]);
    assert_eq!(settings.mouse.cursor_size, 24);
}

#[test]
fn missing_marco_schemas_are_an_error_naming_them() {
    let installed = |id: &str| id != MARCO_WORKSPACE_NAMES && id != MARCO_GENERAL;
    let err = check_schemas(installed).unwrap_err();
    assert_eq!(err.0, [MARCO_GENERAL, MARCO_WORKSPACE_NAMES]);
    assert_eq!(
        err.to_string(),
        "Marco's GSettings schemas org.mate.Marco.general, org.mate.Marco.workspace-names \
         aren't installed; Caminito reads Marco's settings, so install marco"
    );
}

#[test]
fn missing_optional_schemas_are_not_an_error() {
    let installed = |id: &str| id.starts_with("org.mate.Marco");
    assert_eq!(check_schemas(installed), Ok(()));
}

#[test]
fn a_bad_value_falls_back_to_the_schemas_own_default() {
    let mut reading = defaults();
    reading.insert(MOUSE, "cursor-size", Value::Int(0), Value::Int(32));
    reading.insert(MARCO_GENERAL, "theme", string(""), string("Traditional"));
    let Translation { settings, problems } = translate(&reading);
    assert_eq!(settings.mouse.cursor_size, 32);
    assert_eq!(settings.frames.theme, "Traditional");
    assert_eq!(
        problems,
        [
            invalid(MARCO_GENERAL, "theme", "''", "'Traditional'"),
            invalid(MOUSE, "cursor-size", "0", "32"),
        ]
    );
}

// Night Light (137).

#[test]
fn night_light_is_off_on_the_sun_schedule_by_default() {
    assert_eq!(
        settings(&defaults()).night_light,
        NightLightSettings {
            enabled: false,
            schedule: Schedule::Sun,
            noon_temperature: 5500,
            night_temperature: 3400,
            constant_temperature: 4000,
            paused_until: 0,
            last_location: None,
        }
    );
}

#[test]
fn night_lights_keys_are_read() {
    let mut reading = defaults();
    let mut set = |key: &str, value: Value| reading.insert(CAMINITO, key, value.clone(), value);
    set("night-light-enabled", Value::Bool(true));
    set("night-light-schedule", string("constant"));
    set("night-light-noon-temperature", Value::Uint(6000));
    set("night-light-night-temperature", Value::Uint(2700));
    set("night-light-constant-temperature", Value::Uint(3500));
    set("night-light-paused-until", Value::Int64(1_800_000_000));
    set(
        "night-light-last-location",
        Value::MaybePair(Some((52.37, 4.89))),
    );
    assert_eq!(
        settings(&reading).night_light,
        NightLightSettings {
            enabled: true,
            schedule: Schedule::Constant,
            noon_temperature: 6000,
            night_temperature: 2700,
            constant_temperature: 3500,
            paused_until: 1_800_000_000,
            last_location: Some((52.37, 4.89)),
        }
    );
}

#[test]
fn a_night_light_temperature_out_of_range_is_clamped() {
    let reading = with(
        CAMINITO,
        "night-light-constant-temperature",
        Value::Uint(500),
    );
    let (settings, problem) = problem(&reading);
    assert_eq!(settings.night_light.constant_temperature, 1000);
    assert_eq!(
        problem,
        out_of_range(CAMINITO, "night-light-constant-temperature", "500", "1000")
    );
}

#[test]
fn an_unknown_night_light_schedule_is_the_sun() {
    let reading = with(CAMINITO, "night-light-schedule", string("moon"));
    let (settings, problem) = problem(&reading);
    assert_eq!(settings.night_light.schedule, Schedule::Sun);
    assert_eq!(
        problem,
        invalid(CAMINITO, "night-light-schedule", "'moon'", "'sun'")
    );
}

// Hot corners (CMN-183).

#[test]
fn every_hot_corner_is_off_by_default() {
    assert_eq!(settings(&defaults()).hot_corners, HotCorners::default());
    assert_eq!(
        settings(&Reading::default()).hot_corners,
        HotCorners {
            top_left: HotCornerAction::None,
            top_right: HotCornerAction::None,
            bottom_left: HotCornerAction::None,
            bottom_right: HotCornerAction::None,
        }
    );
}

#[test]
fn the_hot_corners_keys_are_read() {
    let mut reading = defaults();
    let mut set = |key: &str, value: &str| {
        let value = string(value);
        reading.insert(CAMINITO, key, value, string("none"));
    };
    set("hot-corner-top-left", "overview");
    set("hot-corner-top-right", "show-desktop");
    set("hot-corner-bottom-left", "lock");
    set("hot-corner-bottom-right", "application-overview");
    assert_eq!(
        settings(&reading).hot_corners,
        HotCorners {
            top_left: HotCornerAction::Overview,
            top_right: HotCornerAction::ShowDesktop,
            bottom_left: HotCornerAction::Lock,
            bottom_right: HotCornerAction::ApplicationOverview,
        }
    );
}

#[test]
fn an_unknown_hot_corner_action_is_none() {
    let reading = with(CAMINITO, "hot-corner-top-right", string("cube"));
    let (settings, problem) = problem(&reading);
    assert_eq!(settings.hot_corners.top_right, HotCornerAction::None);
    assert_eq!(
        problem,
        invalid(CAMINITO, "hot-corner-top-right", "'cube'", "'none'")
    );
}
