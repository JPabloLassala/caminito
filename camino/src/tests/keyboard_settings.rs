//! `org.mate.peripherals-keyboard` on the seat's keyboard (53): the repeat info clients
//! get, and NumLock restored at start-up and written back. Both reach clients through
//! `wl_keyboard`, which Xwayland follows too, so a Wayland client stands for either kind
//! (45).

use super::fixture::{ClientId, Fixture, KEY_NUMLOCK, Kind, WindowSpec};
use crate::settings::Value;
use crate::settings::translate::KEYBOARD;

/// NumLock's bit in the locked modifiers: Mod2, in the default keymap.
const NUM_LOCK: u32 = 1 << 4;

/// A client with a window, which has the keyboard.
fn focused_client(f: &mut Fixture) -> ClientId {
    f.add_output((1920, 1080), 1.0);
    let id = f.connect(Kind::Xdg);
    f.client(id).bind_input();
    f.map(id, WindowSpec::plain((400, 300)));
    id
}

fn repeat_info(f: &Fixture, id: ClientId) -> Option<(i32, i32)> {
    f.client_state(id).repeat_info
}

#[test]
fn a_client_gets_rate_and_delay_as_its_repeat_info() {
    let (mut f, id) = Fixture::connected(Kind::Xdg);
    f.client(id).bind_input();
    f.roundtrip(id);
    assert_eq!(repeat_info(&f, id), Some((30, 500)));
}

#[test]
fn changing_rate_and_delay_live_changes_the_repeat_info_a_client_gets() {
    let (mut f, id) = Fixture::connected(Kind::Xdg);
    f.client(id).bind_input();
    f.roundtrip(id);

    f.set_setting(KEYBOARD, "rate", Value::Int(50));
    f.set_setting(KEYBOARD, "delay", Value::Int(250));
    f.dispatch_until(|f| repeat_info(f, id) == Some((50, 250)));
}

#[test]
fn turning_repeat_off_live_sends_rate_0() {
    let (mut f, id) = Fixture::connected(Kind::Xdg);
    f.client(id).bind_input();
    f.roundtrip(id);

    f.set_setting(KEYBOARD, "repeat", Value::Bool(false));
    f.dispatch_until(|f| repeat_info(f, id) == Some((0, 500)));
}

#[test]
fn a_remembered_numlock_is_restored_at_start_up() {
    let mut f = Fixture::with_settings(vec![(
        KEYBOARD,
        "numlock-state".into(),
        Value::Str("on".into()),
    )]);
    let id = focused_client(&mut f);
    f.roundtrip(id);
    assert_eq!(f.client_state(id).locked_modifiers, NUM_LOCK);
}

#[test]
fn numlock_isnt_restored_when_remembering_is_off() {
    let mut f = Fixture::with_settings(vec![
        (KEYBOARD, "numlock-state".into(), Value::Str("on".into())),
        (
            KEYBOARD,
            "remember-numlock-state".into(),
            Value::Bool(false),
        ),
    ]);
    let id = focused_client(&mut f);
    f.roundtrip(id);
    assert_eq!(f.client_state(id).locked_modifiers, 0);
}

#[test]
fn numlocks_state_is_written_back_on_each_change() {
    let mut f = Fixture::new();
    let id = focused_client(&mut f);
    assert_eq!(f.state.camino.settings.keyboard.numlock, None);

    f.chord(&[KEY_NUMLOCK]);
    f.dispatch_until(|f| f.state.camino.settings.keyboard.numlock == Some(true));
    f.roundtrip(id);
    assert_eq!(f.client_state(id).locked_modifiers, NUM_LOCK);

    f.chord(&[KEY_NUMLOCK]);
    f.dispatch_until(|f| f.state.camino.settings.keyboard.numlock == Some(false));
}

#[test]
fn numlocks_state_isnt_written_back_when_remembering_is_off() {
    let mut f = Fixture::with_settings(vec![(
        KEYBOARD,
        "remember-numlock-state".into(),
        Value::Bool(false),
    )]);
    let id = focused_client(&mut f);

    f.chord(&[KEY_NUMLOCK]);
    f.roundtrip(id);
    assert_eq!(f.client_state(id).locked_modifiers, NUM_LOCK);
    // A write would come back from the settings thread well within this.
    f.dispatch_for(std::time::Duration::from_millis(200));
    assert_eq!(f.state.camino.settings.keyboard.numlock, None);
}
