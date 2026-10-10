//! The keyboard path's bindings stage (22), with the bindings read from
//! `org.mate.Marco.global-keybindings` and `window-keybindings` (148). The first tests check
//! the configures and keys a Wayland client sees, so they map xdg toplevels only (45); those
//! of the keys run on both kinds of window.

use caminito_test_client::{ToplevelState, format_input};
use insta::assert_snapshot;

use super::fixture::{
    ClientWindow, Fixture, KEY_A, KEY_D, KEY_F4, KEY_F10, KEY_LEFT, KEY_LEFTALT, KEY_LEFTCTRL,
    KEY_LEFTMETA, KEY_LEFTSHIFT, KEY_Q, KEY_RIGHT, KEY_TAB, Kind, WindowSpec, on_both_kinds,
};
use crate::settings::Value;
use crate::settings::translate::{MARCO_GLOBAL_KEYBINDINGS, MARCO_WINDOW_KEYBINDINGS};
use crate::state::SEAT;

fn is_activated(f: &mut Fixture, window: ClientWindow) -> bool {
    let configure = f
        .client(window.client())
        .window(window.xdg())
        .configures
        .last()
        .unwrap()
        .clone();
    configure.states.contains(&ToplevelState::Activated)
}

#[test]
fn switching_workspaces_unmaps_and_remaps_their_windows() {
    let (mut f, id) = Fixture::connected(Kind::Xdg);
    f.client(id).bind_input();
    f.client(id).bind_outputs();
    f.roundtrip(id);
    let window = f.map(id, WindowSpec::plain((400, 300)));
    let output = f.state.caminito.outputs[0].clone();
    f.client(id).state.input.clear();

    f.chord(&[KEY_LEFTCTRL, KEY_LEFTALT, KEY_RIGHT]);
    f.roundtrip(id);
    assert_eq!(f.state.caminito.policy.active_workspace(), 1);
    assert!(!is_activated(&mut f, window));
    // Only the cursor.
    assert_eq!(f.state.scene_element_count(&output), 1);

    f.chord(&[KEY_LEFTCTRL, KEY_LEFTALT, KEY_LEFT]);
    f.roundtrip(id);
    assert_eq!(f.state.caminito.policy.active_workspace(), 0);
    assert!(is_activated(&mut f, window));

    assert_snapshot!(f.client(id).window(window.xdg()).output_events.join("\n"), @r"
    enter headless-1
    leave headless-1
    enter headless-1
    ");
    // The bound keys never reach the client. The modifiers do while it has the keyboard:
    // it loses it mid-chord on the way out, and gets it back mid-chord on the way in.
    assert_snapshot!(format_input(&f.client(id).state.input), @r"
    key 29 pressed
    key 56 pressed
    keyboard leaves window 0
    keyboard enters window 0
    key 56 released
    key 29 released
    ");
}

#[test]
fn alt_f4_asks_the_focused_window_to_close() {
    let (mut f, id) = Fixture::connected(Kind::Xdg);
    f.client(id).bind_input();
    let window = f.map(id, WindowSpec::plain((400, 300)));

    f.chord(&[KEY_LEFTALT, KEY_F4]);
    f.roundtrip(id);

    assert!(f.client(id).window(window.xdg()).close_requested);
}

#[test]
fn alt_f10_toggles_maximised() {
    let (mut f, id) = Fixture::connected(Kind::Xdg);
    f.client(id).bind_input();
    let window = f.map(id, WindowSpec::plain((400, 300)));
    let policy_id = f.pid(window);

    f.chord(&[KEY_LEFTALT, KEY_F10]);
    f.roundtrip(id);
    assert!(f.state.caminito.policy.is_maximized(policy_id));

    f.chord(&[KEY_LEFTALT, KEY_F10]);
    f.roundtrip(id);
    assert!(!f.state.caminito.policy.is_maximized(policy_id));
}

#[test]
fn alt_tab_focuses_the_previously_focused_window() {
    let (mut f, id) = Fixture::connected(Kind::Xdg);
    f.client(id).bind_input();
    let a = f.map(id, WindowSpec::plain((400, 300)));
    let b = f.map(id, WindowSpec::plain((400, 300)));
    let (pa, pb) = (f.policy_id(a), f.policy_id(b));
    assert_eq!(f.state.caminito.policy.focus(SEAT), pb);

    f.chord(&[KEY_LEFTALT, KEY_TAB]);
    f.roundtrip(id);
    assert_eq!(f.state.caminito.policy.focus(SEAT), pa);
    assert!(is_activated(&mut f, a));

    f.chord(&[KEY_LEFTALT, KEY_TAB]);
    f.roundtrip(id);
    assert_eq!(f.state.caminito.policy.focus(SEAT), pb);
}

#[test]
fn an_inhibitor_on_the_focused_surface_passes_bindings_to_the_client() {
    let (mut f, id) = Fixture::connected(Kind::Xdg);
    f.client(id).bind_input();
    let window = f.map(id, WindowSpec::plain((400, 300)));
    f.client(id).inhibit_shortcuts(window.xdg());
    f.roundtrip(id);
    f.client(id).state.input.clear();

    f.chord(&[KEY_LEFTALT, KEY_F4]);
    f.roundtrip(id);

    assert!(!f.client(id).window(window.xdg()).close_requested);
    assert_snapshot!(format_input(&f.client(id).state.input), @r"
    key 56 pressed
    key 62 pressed
    key 62 released
    key 56 released
    ");
}

// The bindings from the keys (148).

/// The debug dump's bindings, as `(key, combination)`.
fn bindings_in_force(f: &Fixture) -> Vec<(String, String)> {
    let dump: serde_json::Value = serde_json::from_str(&f.state.caminito.state_dump()).unwrap();
    let rows = dump["bindings"].as_array().unwrap().clone();
    rows.iter()
        .map(|row| {
            let field = |name: &str| row[name].as_str().unwrap().to_owned();
            (field("key"), field("combination"))
        })
        .collect()
}

/// Sets a key of `schema` with the test backend, and dispatches until the settings thread
/// has passed it on, which the debug dump shows by `in_force`.
fn set_binding(
    f: &mut Fixture,
    schema: &'static str,
    key: &str,
    value: &str,
    in_force: impl Fn(&[(String, String)]) -> bool,
) {
    f.set_setting(schema, key, Value::Str(value.into()));
    f.dispatch_until(|f| in_force(&bindings_in_force(f)));
}

fn binds(rows: &[(String, String)], key: &str, combination: &str) -> bool {
    rows.iter().any(|(k, c)| k == key && c == combination)
}

fn binds_key(rows: &[(String, String)], key: &str) -> bool {
    rows.iter().any(|(k, _)| k == key)
}

on_both_kinds! {
    fn every_binding_that_worked_before_works_from_its_keys_default(kind: Kind) {
        let (mut f, id) = Fixture::connected(kind);
        let a = f.map(id, WindowSpec::plain((400, 300)));
        let b = f.map(id, WindowSpec::plain((400, 300)));
        let (pa, pb) = (f.pid(a), f.pid(b));

        f.chord(&[KEY_LEFTALT, KEY_TAB]);
        assert_eq!(f.state.caminito.policy.focus(SEAT), Some(pa));
        f.chord(&[KEY_LEFTALT, KEY_LEFTSHIFT, KEY_TAB]);
        assert_eq!(f.state.caminito.policy.focus(SEAT), Some(pb));

        f.chord(&[KEY_LEFTALT, KEY_F10]);
        assert!(f.state.caminito.policy.is_maximized(pb));
        f.chord(&[KEY_LEFTALT, KEY_F10]);
        assert!(!f.state.caminito.policy.is_maximized(pb));

        f.chord(&[KEY_LEFTCTRL, KEY_LEFTALT, KEY_D]);
        assert!(f.state.caminito.policy.is_showing_desktop());
        f.chord(&[KEY_LEFTCTRL, KEY_LEFTALT, KEY_D]);
        assert!(!f.state.caminito.policy.is_showing_desktop());

        f.chord(&[KEY_LEFTCTRL, KEY_LEFTALT, KEY_RIGHT]);
        assert_eq!(f.state.caminito.policy.active_workspace(), 1);
        f.chord(&[KEY_LEFTCTRL, KEY_LEFTALT, KEY_LEFT]);
        assert_eq!(f.state.caminito.policy.active_workspace(), 0);

        f.chord(&[KEY_LEFTALT, KEY_F4]);
        assert!(f.close_requested(b));
        // The panel's two and the screenshots' are checked in `panel.rs` and `commands.rs`.
        let rows = bindings_in_force(&f);
        for (key, combination) in [
            ("panel-main-menu", "<Alt>F1"),
            ("panel-run-dialog", "<Alt>F2"),
            ("run-command-screenshot", "Print"),
            ("run-command-window-screenshot", "<Alt>Print"),
            ("run-command-area-screenshot", "<Shift>Print"),
        ] {
            assert!(binds(&rows, key, combination), "{key}: {rows:?}");
        }
    }
}

on_both_kinds! {
    fn changing_a_key_moves_its_binding_at_once(kind: Kind) {
        let (mut f, id) = Fixture::connected(kind);
        let window = f.map(id, WindowSpec::plain((400, 300)));
        let policy_id = f.pid(window);

        set_binding(&mut f, MARCO_WINDOW_KEYBINDINGS, "toggle-maximized", "<Super>q", |rows| {
            binds(rows, "toggle-maximized", "<Super>q")
        });

        f.chord(&[KEY_LEFTALT, KEY_F10]);
        assert!(!f.state.caminito.policy.is_maximized(policy_id));
        f.chord(&[KEY_LEFTMETA, KEY_Q]);
        assert!(f.state.caminito.policy.is_maximized(policy_id));
    }
}

on_both_kinds! {
    fn toggle_above_and_toggle_on_all_workspaces_toggle_the_focused_window(kind: Kind) {
        let (mut f, id) = Fixture::connected(kind);
        let window = f.map(id, WindowSpec::plain((400, 300)));
        let pid = f.pid(window);
        set_binding(&mut f, MARCO_WINDOW_KEYBINDINGS, "toggle-above", "<Super>a", |rows| {
            binds(rows, "toggle-above", "<Super>a")
        });
        let key = "toggle-on-all-workspaces";
        set_binding(&mut f, MARCO_WINDOW_KEYBINDINGS, key, "<Super>q", |rows| {
            binds(rows, key, "<Super>q")
        });

        f.chord(&[KEY_LEFTMETA, KEY_A]);
        assert!(f.state.caminito.policy.is_above(pid));
        f.chord(&[KEY_LEFTMETA, KEY_Q]);
        assert!(f.state.caminito.policy.is_sticky(pid));
        f.chord(&[KEY_LEFTCTRL, KEY_LEFTALT, KEY_RIGHT]);
        assert!(f.state.caminito.policy.is_showing(pid));

        f.chord(&[KEY_LEFTMETA, KEY_A]);
        f.chord(&[KEY_LEFTMETA, KEY_Q]);
        let policy = &f.state.caminito.policy;
        assert!(!policy.is_above(pid) && !policy.is_sticky(pid));
        assert_eq!(policy.workspace_of(pid), Some(1));
    }
}

on_both_kinds! {
    fn disabled_unbinds_a_key(kind: Kind) {
        let (mut f, id) = Fixture::connected(kind);
        let window = f.map(id, WindowSpec::plain((400, 300)));

        set_binding(&mut f, MARCO_WINDOW_KEYBINDINGS, "close", "disabled", |rows| {
            !binds_key(rows, "close")
        });

        f.chord(&[KEY_LEFTALT, KEY_F4]);
        // A binding that's still there shows the press went through the bindings stage.
        f.chord(&[KEY_LEFTALT, KEY_F10]);
        assert!(f.state.caminito.policy.is_maximized(f.pid(window)));
        assert!(!f.close_requested(window));
    }
}

#[test]
fn a_key_set_to_disabled_reaches_the_focused_client() {
    let (mut f, id) = Fixture::connected(Kind::Xdg);
    f.client(id).bind_input();
    let window = f.map(id, WindowSpec::plain((400, 300)));
    set_binding(
        &mut f,
        MARCO_WINDOW_KEYBINDINGS,
        "close",
        "disabled",
        |rows| !binds_key(rows, "close"),
    );
    f.client(id).state.input.clear();

    f.chord(&[KEY_LEFTALT, KEY_F4]);
    f.roundtrip(id);

    assert!(!f.client(id).window(window.xdg()).close_requested);
    assert_snapshot!(format_input(&f.client(id).state.input), @r"
    key 56 pressed
    key 62 pressed
    key 62 released
    key 56 released
    ");
}

on_both_kinds! {
    fn switch_windows_all_opens_the_window_switcher_over_every_workspaces_windows_once_bound(
        kind: Kind
    ) {
        let (mut f, id) = Fixture::connected(kind);
        let a = f.map(id, WindowSpec::plain((400, 300)));
        let b = f.map(id, WindowSpec::plain((400, 300)));
        let (pa, pb) = (f.pid(a), f.pid(b));
        f.state.caminito.policy.move_to_workspace(pa, 1);
        f.dispatch();

        // Unbound by default, as in Marco.
        f.key(KEY_LEFTMETA, true);
        f.key(KEY_TAB, true);
        f.key(KEY_TAB, false);
        assert!(f.state.caminito.policy.switcher().is_none());
        f.key(KEY_LEFTMETA, false);

        set_binding(&mut f, MARCO_GLOBAL_KEYBINDINGS, "switch-windows-all", "<Super>Tab", |rows| {
            binds(rows, "switch-windows-all", "<Super>Tab")
        });
        f.key(KEY_LEFTMETA, true);
        f.key(KEY_TAB, true);
        f.key(KEY_TAB, false);

        let dump: serde_json::Value =
            serde_json::from_str(&f.state.caminito.state_dump()).unwrap();
        let switcher = &dump["switcher"];
        assert_eq!(switcher["list"], "windows-all", "{switcher}");
        let mut listed: Vec<u64> = switcher["windows"]
            .as_array()
            .unwrap()
            .iter()
            .map(|id| id.as_u64().unwrap())
            .collect();
        listed.sort_unstable();
        assert_eq!(listed, [pa.0, pb.0]);

        // Letting go of Super commits it, to the window on the other workspace.
        f.key(KEY_LEFTMETA, false);
        assert_eq!(f.state.caminito.policy.focus(SEAT), Some(pa));
        assert_eq!(f.state.caminito.policy.active_workspace(), 1);
    }
}

#[test]
fn the_state_dump_lists_the_bindings_in_force_with_their_keys() {
    let (mut f, _) = Fixture::connected(Kind::Xdg);
    set_binding(
        &mut f,
        MARCO_WINDOW_KEYBINDINGS,
        "close",
        "<Mod4>q",
        |rows| binds(rows, "close", "<Super>q"),
    );
    let rows = bindings_in_force(&f);
    let row = |key: &str, combination: &str| (key.to_owned(), combination.to_owned());
    assert_eq!(rows.len(), 24, "{rows:?}");
    assert_eq!(
        rows[0],
        row("switch-to-workspace-left", "<Control><Alt>Left")
    );
    assert!(
        rows.contains(&row("switch-windows", "<Shift><Alt>Tab")),
        "{rows:?}"
    );
    assert_eq!(rows[23], row("close", "<Super>q"));
}
