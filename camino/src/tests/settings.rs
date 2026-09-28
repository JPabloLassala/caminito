//! The settings thread under Camino: the settings read before anything maps, and live
//! `num-workspaces` changes (29).

use camino_test_client::ToplevelState;

use super::fixture::{Fixture, KEY_LEFTALT, KEY_LEFTCTRL, KEY_RIGHT};
use crate::settings::Value;
use crate::settings::translate::{MARCO_GENERAL, MARCO_WORKSPACE_NAMES};

fn switch_right(f: &mut Fixture) {
    f.chord(&[KEY_LEFTCTRL, KEY_LEFTALT, KEY_RIGHT]);
}

#[test]
fn a_window_maps_only_after_the_settings_are_read() {
    let mut f = Fixture::with_settings(vec![
        (MARCO_GENERAL, "num-workspaces".into(), Value::Int(6)),
        (
            MARCO_WORKSPACE_NAMES,
            "name-6".into(),
            Value::Str("Last".into()),
        ),
    ]);
    // Nothing has been dispatched: the settings came with the state.
    assert_eq!(f.state.camino.policy.workspace_count(), 6);
    assert_eq!(f.state.camino.policy.workspace_names()[5], "Last");

    f.add_output((1920, 1080), 1.0);
    let id = f.add_client();
    f.roundtrip(id);
    for _ in 0..5 {
        switch_right(&mut f);
    }
    f.map_window(id, (400, 300));

    assert_eq!(f.state.camino.policy.active_workspace(), 5);
    assert_eq!(f.state.camino.policy.stacking(5).len(), 1);
}

#[test]
fn raising_num_workspaces_live_adds_workspaces() {
    let (mut f, _) = Fixture::connected();
    assert_eq!(f.state.camino.policy.workspace_count(), 4);

    f.set_setting(MARCO_GENERAL, "num-workspaces", Value::Int(6));
    f.dispatch_until(|f| f.state.camino.policy.workspace_count() == 6);

    for _ in 0..8 {
        switch_right(&mut f);
    }
    assert_eq!(f.state.camino.policy.active_workspace(), 5);
}

#[test]
fn lowering_num_workspaces_live_moves_windows_and_the_active_workspace_as_marco_does() {
    let (mut f, id) = Fixture::connected();
    let kept = f.map_window(id, (400, 300));
    for _ in 0..3 {
        switch_right(&mut f);
    }
    let moved = f.map_window(id, (400, 300));
    let kept_id = f.policy_id(id, kept).unwrap();
    let moved_id = f.policy_id(id, moved).unwrap();
    assert_eq!(f.state.camino.policy.workspace_of(moved_id), Some(3));

    f.set_setting(MARCO_GENERAL, "num-workspaces", Value::Int(2));
    f.dispatch_until(|f| f.state.camino.policy.workspace_count() == 2);
    f.roundtrip(id);

    let policy = &f.state.camino.policy;
    assert_eq!(policy.active_workspace(), 1);
    assert_eq!(policy.workspace_of(moved_id), Some(1));
    assert_eq!(policy.workspace_of(kept_id), Some(0));
    let configure = f
        .client(id)
        .window(moved)
        .configures
        .last()
        .unwrap()
        .clone();
    assert!(configure.states.contains(&ToplevelState::Activated));
}
