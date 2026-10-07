//! Accelerator grabs (98): `GrabAccelerators` and `AcceleratorActivated` on
//! `org.mate.Camino`, with callers on the test binary's private bus. The keys are checked
//! as a Wayland client sees them, so the windows are xdg toplevels only (45).

use std::time::Duration;

use camino_test_client::dbus::{
    AcceleratorActivatedIterator, AcceleratorRepeatedIterator, CaminoProxy, camino_proxy,
    debug_proxy,
};
use camino_test_client::format_input;
use insta::assert_snapshot;

use super::fixture::{
    ClientId, ClientWindow, Fixture, KEY_A, KEY_F4, KEY_F9, KEY_LEFTALT, KEY_LEFTCTRL, KEY_MUTE,
    Kind, Logs, WindowSpec,
};
use crate::settings::Value;
use crate::settings::translate::KEYBOARD;

/// A caller of `org.mate.Camino` on a connection of its own, and the activations and
/// repeats it's signalled.
struct Caller {
    camino: CaminoProxy<'static>,
    activations: Option<AcceleratorActivatedIterator>,
    repeats: Option<AcceleratorRepeatedIterator>,
}

impl Caller {
    /// Connects to Camino at `bus`, listening for activations and repeats from now on.
    fn new(f: &mut Fixture, bus: &str) -> Self {
        let bus = bus.to_owned();
        f.dbus_call(move || {
            let camino = camino_proxy(&bus).unwrap();
            let activations = camino.receive_accelerator_activated().unwrap();
            let repeats = camino.receive_accelerator_repeated().unwrap();
            Self {
                camino,
                activations: Some(activations),
                repeats: Some(repeats),
            }
        })
    }

    /// The caller's unique name on the bus.
    fn name(&self) -> String {
        let connection = self.camino.inner().connection();
        connection.unique_name().unwrap().to_string()
    }

    /// Grabs `accelerators`, and returns their action ids.
    fn grab(&self, f: &mut Fixture, accelerators: &[&'static str]) -> Vec<u32> {
        let camino = self.camino.clone();
        let accelerators = accelerators.to_vec();
        f.dbus_call(move || camino.grab_accelerators(&accelerators).unwrap())
    }

    /// Releases the grabs with these action ids.
    fn ungrab(&self, f: &mut Fixture, actions: &[u32]) {
        let camino = self.camino.clone();
        let actions = actions.to_vec();
        f.dbus_call(move || camino.ungrab_accelerators(&actions).unwrap())
    }

    /// The next activation the caller is signalled: its action id and time. It waits for
    /// one, so a test of a key that mustn't signal presses one that does after it.
    fn next_activation(&mut self, f: &mut Fixture) -> (u32, u32) {
        let mut activations = self.activations.take().unwrap();
        let (activation, activations) = f.dbus_call(move || {
            let signal = activations.next().unwrap();
            let args = signal.args().unwrap();
            ((args.action, args.time), activations)
        });
        self.activations = Some(activations);
        activation
    }

    /// The next repeat the caller is signalled: its action id and time. It waits for one,
    /// as `next_activation` does.
    fn next_repeat(&mut self, f: &mut Fixture) -> (u32, u32) {
        let mut repeats = self.repeats.take().unwrap();
        let (repeat, repeats) = f.dbus_call(move || {
            let signal = repeats.next().unwrap();
            let args = signal.args().unwrap();
            ((args.action, args.time), repeats)
        });
        self.repeats = Some(repeats);
        repeat
    }

    /// Asserts that nothing repeated in the last while. It holds `key`, grabbed as
    /// `action`, until it repeats: a repeat signalled before that would be read first.
    fn assert_next_repeat_is(&mut self, f: &mut Fixture, key: u32, action: u32) {
        f.key(key, true);
        assert_eq!(self.next_repeat(f).0, action);
        f.key(key, false);
    }
}

/// The repeat the tests of what ends the repeats set: the first comes soon, and the second
/// half a second after, so there's time to end them between the two.
const SLOW: (i32, i32) = (30, 2);

/// Long enough for `SLOW`'s second repeat to be signalled, were there one.
const PAST_THE_SECOND_REPEAT: Duration = Duration::from_millis(600);

/// Sets the keyboard's repeat: the milliseconds a key is held before it repeats, and the
/// repeats per second, where 0 turns repeat off.
fn set_repeat(f: &mut Fixture, delay: i32, rate: i32) {
    f.set_setting(KEYBOARD, "delay", Value::Int(delay));
    f.set_setting(KEYBOARD, "rate", Value::Int(rate.max(1)));
    f.set_setting(KEYBOARD, "repeat", Value::Bool(rate > 0));
    f.dispatch_until(|f| {
        let keyboard = &f.state.camino.settings.keyboard;
        (keyboard.repeat_delay, keyboard.repeat_rate) == (delay, rate)
    });
}

/// A fixture on the bus, and a client that records its input, with a window that has the
/// keyboard.
fn with_a_focused_window() -> (Fixture, ClientId, ClientWindow, String) {
    let (mut f, id) = Fixture::connected(Kind::Xdg);
    f.client(id).bind_input();
    let window = f.map(id, WindowSpec::plain((400, 300)));
    f.client(id).state.input.clear();
    let bus = f.start_dbus();
    (f, id, window, bus)
}

#[test]
fn a_grabbed_combination_signals_its_action_once_per_press_and_reaches_no_client() {
    let (mut f, id, _, bus) = with_a_focused_window();
    let mut caller = Caller::new(&mut f, &bus);
    let actions = caller.grab(&mut f, &["XF86AudioMute", "<Control><Alt>a", "F9"]);
    let [mute, ctrl_alt_a, f9] = actions[..] else {
        panic!("three action ids, not {actions:?}");
    };
    assert!(mute != 0 && ctrl_alt_a != 0 && f9 != 0);
    assert!(mute != ctrl_alt_a && ctrl_alt_a != f9 && mute != f9);

    f.chord(&[KEY_MUTE]);
    f.chord(&[KEY_LEFTCTRL, KEY_LEFTALT, KEY_A]);
    f.chord(&[KEY_MUTE]);
    // F9 ends them: nothing else was signalled before it.
    f.chord(&[KEY_F9]);

    let (first, first_time) = caller.next_activation(&mut f);
    let (second, second_time) = caller.next_activation(&mut f);
    let (third, third_time) = caller.next_activation(&mut f);
    assert_eq!([first, second, third], [mute, ctrl_alt_a, mute]);
    assert_eq!(caller.next_activation(&mut f).0, f9);
    // The key events' times, which the fixture makes later each time.
    assert!(first_time < second_time && second_time < third_time);

    f.roundtrip(id);
    // Only the modifiers, which aren't grabbed themselves.
    assert_snapshot!(format_input(&f.client(id).state.input), @r"
    key 29 pressed
    key 56 pressed
    key 56 released
    key 29 released
    ");
}

#[test]
fn a_held_grabbed_key_signals_repeats_at_the_keyboards_delay_and_rate() {
    let (mut f, id, _, bus) = with_a_focused_window();
    set_repeat(&mut f, 30, 100);
    let mut caller = Caller::new(&mut f, &bus);
    let actions = caller.grab(&mut f, &["XF86AudioMute", "F9"]);

    f.key(KEY_MUTE, true);
    let (_, pressed) = caller.next_activation(&mut f);
    let (first, first_time) = caller.next_repeat(&mut f);
    let (second, second_time) = caller.next_repeat(&mut f);
    f.key(KEY_MUTE, false);

    assert_eq!([first, second], [actions[0], actions[0]]);
    // The press's time, then the delay, then the rate's period.
    assert_eq!([first_time, second_time], [pressed + 30, pressed + 40]);
    // The press was signalled once, and no client saw the key.
    f.chord(&[KEY_F9]);
    assert_eq!(caller.next_activation(&mut f).0, actions[1]);
    f.roundtrip(id);
    assert_snapshot!(format_input(&f.client(id).state.input), @"");
}

#[test]
fn a_grabbed_keys_release_ends_its_repeats() {
    let (mut f, _, _, bus) = with_a_focused_window();
    set_repeat(&mut f, SLOW.0, SLOW.1);
    let mut caller = Caller::new(&mut f, &bus);
    let actions = caller.grab(&mut f, &["XF86AudioMute", "F9"]);
    f.key(KEY_MUTE, true);
    assert_eq!(caller.next_repeat(&mut f).0, actions[0]);

    f.key(KEY_MUTE, false);

    f.dispatch_for(PAST_THE_SECOND_REPEAT);
    caller.assert_next_repeat_is(&mut f, KEY_F9, actions[1]);
}

#[test]
fn another_keys_press_ends_the_repeats_of_a_held_grabbed_key() {
    let (mut f, _, _, bus) = with_a_focused_window();
    set_repeat(&mut f, SLOW.0, SLOW.1);
    let mut caller = Caller::new(&mut f, &bus);
    let actions = caller.grab(&mut f, &["XF86AudioMute", "F9"]);
    f.key(KEY_MUTE, true);
    assert_eq!(caller.next_repeat(&mut f).0, actions[0]);

    f.key(KEY_A, true);

    f.dispatch_for(PAST_THE_SECOND_REPEAT);
    f.key(KEY_A, false);
    f.key(KEY_MUTE, false);
    caller.assert_next_repeat_is(&mut f, KEY_F9, actions[1]);
}

#[test]
fn releasing_the_grab_of_a_held_key_ends_its_repeats() {
    let (mut f, _, _, bus) = with_a_focused_window();
    set_repeat(&mut f, SLOW.0, SLOW.1);
    let mut caller = Caller::new(&mut f, &bus);
    let actions = caller.grab(&mut f, &["XF86AudioMute", "F9"]);
    f.key(KEY_MUTE, true);
    assert_eq!(caller.next_repeat(&mut f).0, actions[0]);

    caller.ungrab(&mut f, &actions[..1]);
    // Grabbed again, it has another id, and the key held since before isn't its press.
    let again = caller.grab(&mut f, &["XF86AudioMute"]);
    assert_ne!(again, actions[..1]);

    f.dispatch_for(PAST_THE_SECOND_REPEAT);
    caller.assert_next_repeat_is(&mut f, KEY_F9, actions[1]);
    f.key(KEY_MUTE, false);
}

#[test]
fn a_held_grabbed_key_doesnt_repeat_with_the_keyboards_repeat_off() {
    let (mut f, _, _, bus) = with_a_focused_window();
    set_repeat(&mut f, 30, 0);
    let mut caller = Caller::new(&mut f, &bus);
    let actions = caller.grab(&mut f, &["XF86AudioMute", "F9"]);

    f.key(KEY_MUTE, true);
    // More than three times the delay: a key that repeated would have by now.
    f.dispatch_for(Duration::from_millis(100));
    f.key(KEY_MUTE, false);

    assert_eq!(caller.next_activation(&mut f).0, actions[0]);
    set_repeat(&mut f, 30, 100);
    caller.assert_next_repeat_is(&mut f, KEY_F9, actions[1]);
}

#[test]
fn an_ungrabbed_combination_reaches_the_focused_client_as_before() {
    let (mut f, id, _, bus) = with_a_focused_window();
    let mut caller = Caller::new(&mut f, &bus);
    let actions = caller.grab(&mut f, &["XF86AudioMute", "<Control><Alt>a", "F9"]);
    caller.ungrab(&mut f, &actions[..2]);

    // Neither the keys released, nor a grabbed key with other modifiers held.
    f.chord(&[KEY_MUTE]);
    f.chord(&[KEY_LEFTCTRL, KEY_LEFTALT, KEY_A]);
    f.chord(&[KEY_LEFTALT, KEY_F9]);
    f.chord(&[KEY_F9]);

    assert_eq!(caller.next_activation(&mut f).0, actions[2]);
    f.roundtrip(id);
    assert_snapshot!(format_input(&f.client(id).state.input), @r"
    key 113 pressed
    key 113 released
    key 29 pressed
    key 56 pressed
    key 30 pressed
    key 30 released
    key 56 released
    key 29 released
    key 56 pressed
    key 67 pressed
    key 67 released
    key 56 released
    ");
}

#[test]
fn a_second_callers_grab_of_the_same_combination_is_refused() {
    let (mut f, _, _, bus) = with_a_focused_window();
    let mut first = Caller::new(&mut f, &bus);
    let mut second = Caller::new(&mut f, &bus);
    let [mute] = first.grab(&mut f, &["XF86AudioMute"])[..] else {
        panic!("one action id");
    };

    // However it's written, and among others that are granted.
    let actions = second.grab(&mut f, &["XF86AudioMute", "F9", "<Shift>xf86audiomute"]);
    let [0, f9, shift_mute] = actions[..] else {
        panic!("a refusal and two action ids, not {actions:?}");
    };
    assert!(f9 != 0 && shift_mute != 0);
    // The first caller's grab is the one it had.
    assert_eq!(first.grab(&mut f, &["XF86AudioMute"]), [mute]);

    f.chord(&[KEY_MUTE]);
    f.chord(&[KEY_F9]);
    assert_eq!(first.next_activation(&mut f).0, mute);
    // The second caller hears of its own key, and wasn't told of the first's before it.
    assert_eq!(second.next_activation(&mut f).0, f9);
}

#[test]
fn a_caller_can_only_release_its_own_grabs() {
    let (mut f, _, _, bus) = with_a_focused_window();
    let mut first = Caller::new(&mut f, &bus);
    let second = Caller::new(&mut f, &bus);
    let actions = first.grab(&mut f, &["XF86AudioMute"]);

    second.ungrab(&mut f, &actions);

    f.chord(&[KEY_MUTE]);
    assert_eq!(first.next_activation(&mut f).0, actions[0]);
}

#[test]
fn the_grabs_of_a_caller_that_leaves_the_bus_are_released() {
    let (mut f, id, _, bus) = with_a_focused_window();
    let leaver = Caller::new(&mut f, &bus);
    let grabbed = leaver.grab(&mut f, &["XF86AudioMute"]);
    assert_ne!(grabbed, [0]);
    let other = Caller::new(&mut f, &bus);
    assert_eq!(other.grab(&mut f, &["XF86AudioMute"]), [0]);

    drop(leaver);

    // The bus reports it some time after: until then the combination is still taken.
    f.dispatch_until_mut(|f| {
        let actions = other.grab(f, &["XF86AudioMute"]);
        other.ungrab(f, &actions);
        actions != [0]
    });
    f.chord(&[KEY_MUTE]);
    f.roundtrip(id);
    assert_snapshot!(format_input(&f.client(id).state.input), @r"
    key 113 pressed
    key 113 released
    ");
}

#[test]
fn a_combination_camino_binds_itself_isnt_signalled() {
    let (mut f, id, window, bus) = with_a_focused_window();
    let mut caller = Caller::new(&mut f, &bus);
    let actions = caller.grab(&mut f, &["<Alt>F4", "F9"]);

    f.chord(&[KEY_LEFTALT, KEY_F4]);
    f.chord(&[KEY_F9]);

    // The binding ran, and F9 is the first the caller hears of.
    assert_eq!(caller.next_activation(&mut f).0, actions[1]);
    f.roundtrip(id);
    assert!(f.client(id).window(window.xdg()).close_requested);
}

#[test]
fn an_accelerator_that_doesnt_parse_gets_no_action_and_one_log_line() {
    let (mut f, _, _, bus) = with_a_focused_window();
    let caller = Caller::new(&mut f, &bus);
    let logs = Logs::capture();

    let actions = caller.grab(
        &mut f,
        &["<Control>NoSuchKey", "", "disabled", "<Contrl>t", "F9"],
    );

    let [0, 0, 0, 0, f9] = actions[..] else {
        panic!("four refusals and an action id, not {actions:?}");
    };
    assert_ne!(f9, 0);
    let logs = logs.text();
    let about = |accelerator: &str| {
        let line = format!(
            "can't grab the accelerator {accelerator:?} for {}",
            caller.name()
        );
        logs.matches(&line).count()
    };
    assert_eq!(about("<Control>NoSuchKey"), 1, "{logs}");
    assert_eq!(about("<Contrl>t"), 1, "{logs}");
    // No accelerator isn't a mistake.
    assert_eq!(logs.matches("can't grab").count(), 2, "{logs}");
}

#[test]
fn an_inhibitor_on_the_focused_surface_passes_grabbed_keys_to_the_client() {
    let (mut f, id, window, bus) = with_a_focused_window();
    let caller = Caller::new(&mut f, &bus);
    caller.grab(&mut f, &["XF86AudioMute"]);
    f.client(id).inhibit_shortcuts(window.xdg());
    f.roundtrip(id);

    f.chord(&[KEY_MUTE]);

    f.roundtrip(id);
    assert_snapshot!(format_input(&f.client(id).state.input), @r"
    key 113 pressed
    key 113 released
    ");
}

#[test]
fn the_state_dump_lists_the_grabs_and_their_owners() {
    let (mut f, _, _, bus) = with_a_focused_window();
    let first = Caller::new(&mut f, &bus);
    let second = Caller::new(&mut f, &bus);
    first.grab(&mut f, &["XF86AudioMute", "<Control><Alt>a"]);
    let released = second.grab(&mut f, &["F9", "<Mod4>e"]);
    second.ungrab(&mut f, &released[..1]);

    let dump = f.dbus_call(move || debug_proxy(&bus).unwrap().dump_state().unwrap());

    let dump: serde_json::Value = serde_json::from_str(&dump).unwrap();
    let grabs = serde_json::to_string_pretty(&dump["accelerator_grabs"]).unwrap();
    let grabs = grabs
        .replace(&first.name(), "[first]")
        .replace(&second.name(), "[second]");
    assert_snapshot!(grabs, @r#"
    [
      {
        "accelerator": "XF86AudioMute",
        "action": 1,
        "owner": "[first]"
      },
      {
        "accelerator": "<Control><Alt>a",
        "action": 2,
        "owner": "[first]"
      },
      {
        "accelerator": "<Mod4>e",
        "action": 4,
        "owner": "[second]"
      }
    ]
    "#);
}
