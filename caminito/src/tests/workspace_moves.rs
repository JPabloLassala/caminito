//! Up, down and the previous workspace, and `wrap-style` (150), from their bindings with
//! Marco's defaults, on both kinds of window.

use super::fixture::{
    ClientId, Fixture, KEY_A, KEY_DOWN, KEY_LEFT, KEY_LEFTALT, KEY_LEFTCTRL, KEY_LEFTMETA,
    KEY_LEFTSHIFT, KEY_UP, Kind, WindowSpec, on_both_kinds,
};
use crate::settings::Value;
use crate::settings::translate::{CAMINITO, MARCO_GENERAL, MARCO_GLOBAL_KEYBINDINGS};
use crate::state::SEAT;
use caminito_policy::WrapStyle;

/// Four workspaces in two rows, `switch-to-workspace-prev` on Super+A (it's unbound by
/// default), one output, and a client of `kind`.
///
/// ```text
/// 0 1
/// 2 3
/// ```
fn two_rows(kind: Kind) -> (Fixture, ClientId) {
    let mut f = Fixture::with_settings(vec![
        (CAMINITO, "workspace-layout-rows".into(), Value::Int(2)),
        (
            MARCO_GLOBAL_KEYBINDINGS,
            "switch-to-workspace-prev".into(),
            Value::Str("<Super>a".into()),
        ),
    ]);
    f.add_output((1920, 1080), 1.0);
    let id = f.connect(kind);
    (f, id)
}

fn active(f: &Fixture) -> usize {
    f.state.caminito.policy.active_workspace()
}

on_both_kinds! {
    fn the_switches_up_down_and_back_run_from_their_bindings(kind: Kind) {
        let (mut f, id) = two_rows(kind);
        f.map(id, WindowSpec::plain((400, 300)));

        f.chord(&[KEY_LEFTCTRL, KEY_LEFTALT, KEY_DOWN]);
        assert_eq!(active(&f), 2);
        f.chord(&[KEY_LEFTCTRL, KEY_LEFTALT, KEY_UP]);
        assert_eq!(active(&f), 0);
        f.chord(&[KEY_LEFTMETA, KEY_A]);
        assert_eq!(active(&f), 2);
        f.chord(&[KEY_LEFTMETA, KEY_A]);
        assert_eq!(active(&f), 0);
    }
}

on_both_kinds! {
    fn move_to_workspace_down_and_up_carry_the_focused_window(kind: Kind) {
        let (mut f, id) = two_rows(kind);
        let a = f.map(id, WindowSpec::plain((400, 300)));
        let b = f.map(id, WindowSpec::plain((400, 300)));
        let (pa, pb) = (f.pid(a), f.pid(b));
        assert_eq!(f.state.caminito.policy.focus(SEAT), Some(pb));

        f.chord(&[KEY_LEFTCTRL, KEY_LEFTSHIFT, KEY_LEFTALT, KEY_DOWN]);
        let policy = &f.state.caminito.policy;
        assert_eq!(policy.active_workspace(), 2);
        assert_eq!(policy.workspace_of(pb), Some(2));
        assert_eq!(policy.workspace_of(pa), Some(0));
        assert_eq!(policy.focus(SEAT), Some(pb));

        f.chord(&[KEY_LEFTCTRL, KEY_LEFTSHIFT, KEY_LEFTALT, KEY_UP]);
        let policy = &f.state.caminito.policy;
        assert_eq!(policy.active_workspace(), 0);
        assert_eq!(policy.workspace_of(pb), Some(0));
        assert_eq!(policy.focus(SEAT), Some(pb));
    }
}

#[test]
fn a_wrap_style_change_applies_at_once() {
    let (mut f, _) = Fixture::connected(Kind::Xdg);
    f.chord(&[KEY_LEFTCTRL, KEY_LEFTALT, KEY_LEFT]);
    assert_eq!(active(&f), 0);

    f.set_setting(MARCO_GENERAL, "wrap-style", Value::Str("toroidal".into()));
    f.dispatch_until(|f| f.state.caminito.settings.policy.wrap_style == WrapStyle::Toroidal);
    f.chord(&[KEY_LEFTCTRL, KEY_LEFTALT, KEY_LEFT]);
    assert_eq!(active(&f), 3);

    f.set_setting(MARCO_GENERAL, "wrap-style", Value::Str("classic".into()));
    f.dispatch_until(|f| f.state.caminito.settings.policy.wrap_style == WrapStyle::Classic);
    // In one row, classic takes up to the column before.
    f.chord(&[KEY_LEFTCTRL, KEY_LEFTALT, KEY_UP]);
    assert_eq!(active(&f), 2);
}
