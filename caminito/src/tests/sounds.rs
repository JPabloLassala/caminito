//! The workspace switch sounds (170): each switch of the active workspace plays Marco's
//! sound for the way it went in the Workspace grid, from the sound theme, while
//! `org.mate.sound event-sounds` is on. The tests' player records what Caminito asks for.

use super::fixture::{
    Fixture, KEY_DOWN, KEY_LEFT, KEY_LEFTALT, KEY_LEFTCTRL, KEY_RIGHT, KEY_UP, Kind,
};
use crate::settings::Value;
use crate::settings::translate::{CAMINITO, MARCO_GENERAL, SOUND};

/// Four workspaces in two rows, the `mate` sound theme, `event-sounds` as given, and one
/// output.
///
/// ```text
/// 0 1
/// 2 3
/// ```
fn two_rows(event_sounds: bool) -> Fixture {
    let mut f = Fixture::with_settings(vec![
        (CAMINITO, "workspace-layout-rows".into(), Value::Int(2)),
        (SOUND, "theme-name".into(), Value::Str("mate".into())),
        (SOUND, "event-sounds".into(), Value::Bool(event_sounds)),
    ]);
    f.add_output((1920, 1080), 1.0);
    f
}

/// Ctrl+Alt and `arrow`, Marco's default for a switch that way.
fn switch(f: &mut Fixture, arrow: u32) {
    f.chord(&[KEY_LEFTCTRL, KEY_LEFTALT, arrow]);
    f.dispatch();
}

/// The switch sounds of these ways from the `mate` sound theme, as the player is asked for
/// them.
fn mate(ways: &[&str]) -> Vec<(String, String)> {
    ways.iter()
        .map(|way| ("mate".into(), format!("desktop-switch-{way}")))
        .collect()
}

#[test]
fn a_switch_plays_the_sound_of_the_way_it_went() {
    let mut f = two_rows(true);
    assert_eq!(f.sounds.take(), [], "nothing plays as Caminito starts");

    switch(&mut f, KEY_RIGHT);
    assert_eq!(f.sounds.take(), mate(&["right"]));
    switch(&mut f, KEY_DOWN);
    assert_eq!(f.sounds.take(), mate(&["down"]));
    switch(&mut f, KEY_LEFT);
    assert_eq!(f.sounds.take(), mate(&["left"]));
    switch(&mut f, KEY_UP);
    assert_eq!(f.sounds.take(), mate(&["up"]));
}

#[test]
fn no_switch_plays_a_sound_with_event_sounds_off() {
    let mut f = two_rows(false);

    for arrow in [KEY_RIGHT, KEY_DOWN, KEY_LEFT, KEY_UP] {
        switch(&mut f, arrow);
    }

    assert_eq!(f.state.caminito.policy.active_workspace(), 0);
    assert_eq!(f.sounds.take(), []);
}

#[test]
fn a_switch_that_goes_nowhere_plays_nothing() {
    let mut f = two_rows(true);

    switch(&mut f, KEY_UP);
    switch(&mut f, KEY_LEFT);

    assert_eq!(f.sounds.take(), []);
}

#[test]
fn a_switch_from_the_pager_plays_its_sound_too() {
    let mut f = two_rows(true);
    let pager = f.connect(Kind::Xdg);
    f.client(pager).bind_outputs();
    f.client(pager).bind_workspaces();
    f.roundtrip(pager);

    f.client(pager).activate_workspace(3);
    f.client(pager).commit_workspaces();
    f.roundtrip(pager);
    assert_eq!(f.state.caminito.policy.active_workspace(), 3);
    assert_eq!(f.sounds.take(), mate(&["right"]));

    f.client(pager).activate_workspace(1);
    f.client(pager).commit_workspaces();
    f.roundtrip(pager);
    assert_eq!(f.sounds.take(), mate(&["up"]));
}

#[test]
fn removing_the_active_workspace_plays_the_switch_to_the_last_left() {
    let mut f = two_rows(true);
    switch(&mut f, KEY_RIGHT);
    switch(&mut f, KEY_DOWN);
    f.sounds.take();

    f.set_setting(MARCO_GENERAL, "num-workspaces", Value::Int(2));
    f.dispatch_until(|f| f.state.caminito.policy.workspace_count() == 2);
    f.dispatch();

    assert_eq!(f.state.caminito.policy.active_workspace(), 1);
    assert_eq!(f.sounds.take(), mate(&["up"]));
}

#[test]
fn a_change_of_the_sound_keys_applies_to_the_next_switch() {
    let mut f = two_rows(true);

    f.set_setting(SOUND, "theme-name", Value::Str("freedesktop".into()));
    f.dispatch_until(|f| f.state.caminito.settings.sound.theme_name == "freedesktop");
    assert_eq!(f.sounds.take(), [], "a change plays nothing itself");
    switch(&mut f, KEY_RIGHT);
    assert_eq!(
        f.sounds.take(),
        [("freedesktop".into(), "desktop-switch-right".into())]
    );

    f.set_setting(SOUND, "event-sounds", Value::Bool(false));
    f.dispatch_until(|f| !f.state.caminito.settings.sound.event_sounds);
    switch(&mut f, KEY_LEFT);
    assert_eq!(f.sounds.take(), []);

    f.set_setting(SOUND, "event-sounds", Value::Bool(true));
    f.dispatch_until(|f| f.state.caminito.settings.sound.event_sounds);
    switch(&mut f, KEY_RIGHT);
    assert_eq!(
        f.sounds.take(),
        [("freedesktop".into(), "desktop-switch-right".into())]
    );
}
