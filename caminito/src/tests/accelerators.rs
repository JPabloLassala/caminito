//! Accelerator grabs (98): `GrabAccelerators` and `AcceleratorActivated` on
//! `org.mate.Caminito`, with callers on the test binary's private bus. The keys are checked
//! as a Wayland client sees them, so the windows are xdg toplevels only (45). A grab can be
//! of a mouse button too (185, ADR 0028), and its tests are of what the caller is signalled
//! and what an xdg client sees: what keeps a button's press from Caminito is the one check
//! for a grab and a binding, and `button_bindings` runs it on both kinds of window.

use std::time::Duration;

use caminito_test_client::dbus::{
    AcceleratorActivatedIterator, AcceleratorRepeatedIterator, CaminitoProxy, caminito_proxy,
    debug_proxy,
};
use caminito_test_client::format_input;
use insta::assert_snapshot;
use smithay::utils::Point;

use super::button_bindings::{BTN_EXTRA, BTN_SIDE, buttons_seen, press};
use super::fixture::{
    BTN_LEFT, BTN_MIDDLE, ClientId, ClientWindow, Fixture, KEY_1, KEY_A, KEY_ESC, KEY_F4, KEY_F9,
    KEY_KP1, KEY_LEFTALT, KEY_LEFTCTRL, KEY_LEFTMETA, KEY_LEFTSHIFT, KEY_MUTE, KEY_NUMLOCK, Kind,
    Logs, WindowSpec,
};
use crate::settings::Value;
use crate::settings::translate::{KEYBOARD, MARCO_WINDOW_KEYBINDINGS};
use crate::state::SEAT;

/// A caller of `org.mate.Caminito` on a connection of its own, and the activations and
/// repeats it's signalled.
pub(super) struct Caller {
    caminito: CaminitoProxy<'static>,
    activations: Option<AcceleratorActivatedIterator>,
    repeats: Option<AcceleratorRepeatedIterator>,
}

impl Caller {
    /// Connects to Caminito at `bus`, listening for activations and repeats from now on.
    pub(super) fn new(f: &mut Fixture, bus: &str) -> Self {
        let bus = bus.to_owned();
        f.dbus_call(move || {
            let caminito = caminito_proxy(&bus).unwrap();
            let activations = caminito.receive_accelerator_activated().unwrap();
            let repeats = caminito.receive_accelerator_repeated().unwrap();
            Self {
                caminito,
                activations: Some(activations),
                repeats: Some(repeats),
            }
        })
    }

    /// The caller's unique name on the bus.
    fn name(&self) -> String {
        let connection = self.caminito.inner().connection();
        connection.unique_name().unwrap().to_string()
    }

    /// Grabs `accelerators`, and returns their action ids.
    pub(super) fn grab(&self, f: &mut Fixture, accelerators: &[&'static str]) -> Vec<u32> {
        let caminito = self.caminito.clone();
        let accelerators = accelerators.to_vec();
        f.dbus_call(move || caminito.grab_accelerators(&accelerators).unwrap())
    }

    /// Releases the grabs with these action ids.
    fn ungrab(&self, f: &mut Fixture, actions: &[u32]) {
        let caminito = self.caminito.clone();
        let actions = actions.to_vec();
        f.dbus_call(move || caminito.ungrab_accelerators(&actions).unwrap())
    }

    /// The next activation the caller is signalled: its action id and time. It waits for
    /// one, so a test of a key that mustn't signal presses one that does after it.
    pub(super) fn next_activation(&mut self, f: &mut Fixture) -> (u32, u32) {
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
        f.advance_clock(repeat_delay(f));
        assert_eq!(self.next_repeat(f).0, action);
        f.key(key, false);
    }
}

/// The delay of the repeat the tests set, which no test's real time comes near, so only
/// `advance_clock` brings a repeat.
const DELAY: Duration = Duration::from_secs(20);

/// The period of the repeat the tests set.
const PERIOD: Duration = Duration::from_secs(1);

/// Sets the keyboard's repeat to `DELAY` and `PERIOD`.
fn set_slow_repeat(f: &mut Fixture) {
    set_repeat(
        f,
        DELAY.as_millis() as i32,
        (1000 / PERIOD.as_millis()) as i32,
    );
}

/// The delay of the keyboard's repeat, as it's set now.
fn repeat_delay(f: &Fixture) -> Duration {
    let delay = f.state.caminito.settings.keyboard.repeat_delay;
    Duration::from_millis(delay.try_into().unwrap())
}

/// Sets the keyboard's repeat: the milliseconds a key is held before it repeats, and the
/// repeats per second, where 0 turns repeat off.
fn set_repeat(f: &mut Fixture, delay: i32, rate: i32) {
    f.set_setting(KEYBOARD, "delay", Value::Int(delay));
    f.set_setting(KEYBOARD, "rate", Value::Int(rate.max(1)));
    f.set_setting(KEYBOARD, "repeat", Value::Bool(rate > 0));
    f.dispatch_until(|f| {
        let keyboard = &f.state.caminito.settings.keyboard;
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

/// Keyboard Shortcuts writes the keysym the press typed, with Shift beside it: Shift and 1
/// is `<Shift>exclam` (100). msd's X11 grabs find the key by that keysym.
#[test]
fn a_combination_named_by_the_keysym_its_press_types_signals_its_action() {
    let (mut f, id, _, bus) = with_a_focused_window();
    let mut caller = Caller::new(&mut f, &bus);
    let actions = caller.grab(&mut f, &["<Shift>exclam", "F9"]);

    f.chord(&[KEY_LEFTSHIFT, KEY_1]);
    // Without Shift the key types another keysym, and isn't the combination.
    f.chord(&[KEY_1]);
    f.chord(&[KEY_F9]);

    assert_eq!(caller.next_activation(&mut f).0, actions[0]);
    assert_eq!(caller.next_activation(&mut f).0, actions[1]);
    f.roundtrip(id);
    assert_snapshot!(format_input(&f.client(id).state.input), @r"
    key 42 pressed
    key 42 released
    key 2 pressed
    key 2 released
    ");
}

/// msd's X11 `match_key` doesn't take Shift for consumed by the keysym it types, so
/// `exclam` with no Shift named is no press at all.
#[test]
fn a_typed_keysym_without_the_modifier_that_types_it_is_no_press() {
    let (mut f, id, _, bus) = with_a_focused_window();
    let mut caller = Caller::new(&mut f, &bus);
    let actions = caller.grab(&mut f, &["exclam", "<Shift>1", "F9"]);

    f.chord(&[KEY_LEFTSHIFT, KEY_1]);
    f.chord(&[KEY_F9]);

    // The key's first level names it still, and `exclam` wasn't signalled before it.
    assert_eq!(caller.next_activation(&mut f).0, actions[1]);
    assert_eq!(caller.next_activation(&mut f).0, actions[2]);
    f.roundtrip(id);
    assert_snapshot!(format_input(&f.client(id).state.input), @r"
    key 42 pressed
    key 42 released
    ");
}

/// With NumLock on, Keyboard Shortcuts writes a keypad key as the digit it types.
#[test]
fn a_keypad_key_is_named_by_what_it_types_with_numlock_on() {
    let (mut f, _, _, bus) = with_a_focused_window();
    let mut caller = Caller::new(&mut f, &bus);
    let actions = caller.grab(&mut f, &["KP_1", "KP_End", "F9"]);

    f.chord(&[KEY_KP1]);
    f.chord(&[KEY_NUMLOCK]);
    f.chord(&[KEY_KP1]);
    f.chord(&[KEY_F9]);

    // Its first level, `KP_End`, names it with NumLock off and on: the grab made first
    // has the press that both name.
    assert_eq!(caller.next_activation(&mut f).0, actions[1]);
    assert_eq!(caller.next_activation(&mut f).0, actions[0]);
    assert_eq!(caller.next_activation(&mut f).0, actions[2]);
}

#[test]
fn a_held_grabbed_key_signals_repeats_at_the_keyboards_delay_and_rate() {
    let (mut f, id, _, bus) = with_a_focused_window();
    set_slow_repeat(&mut f);
    let mut caller = Caller::new(&mut f, &bus);
    let actions = caller.grab(&mut f, &["XF86AudioMute", "F9"]);

    f.key(KEY_MUTE, true);
    let (_, pressed) = caller.next_activation(&mut f);
    f.advance_clock(DELAY);
    let (first, first_time) = caller.next_repeat(&mut f);
    f.advance_clock(PERIOD);
    let (second, second_time) = caller.next_repeat(&mut f);
    f.advance_clock(PERIOD);
    let (third, third_time) = caller.next_repeat(&mut f);
    f.key(KEY_MUTE, false);

    assert_eq!([first, second, third], [actions[0]; 3]);
    // The press's time, then the delay, then the rate's period.
    assert_eq!(
        [first_time, second_time, third_time],
        [20_000, 21_000, 22_000].map(|after| pressed + after)
    );
    // There were three repeats and no more, the press was signalled once, and no client
    // saw the key.
    caller.assert_next_repeat_is(&mut f, KEY_F9, actions[1]);
    assert_eq!(caller.next_activation(&mut f).0, actions[1]);
    f.roundtrip(id);
    assert_snapshot!(format_input(&f.client(id).state.input), @"");
}

/// A repeat that's late isn't made up for (176): after a suspend, the key repeats one time.
#[test]
fn a_held_grabbed_key_signals_one_repeat_for_many_periods_passed_at_once() {
    let (mut f, _, _, bus) = with_a_focused_window();
    set_slow_repeat(&mut f);
    let mut caller = Caller::new(&mut f, &bus);
    let actions = caller.grab(&mut f, &["XF86AudioMute", "F9"]);

    f.key(KEY_MUTE, true);
    f.advance_clock(DELAY + 10 * PERIOD);
    let (first, _) = caller.next_repeat(&mut f);
    // The next is a period after that one.
    f.advance_clock(PERIOD);
    let (second, _) = caller.next_repeat(&mut f);
    f.key(KEY_MUTE, false);

    assert_eq!([first, second], [actions[0]; 2]);
    caller.assert_next_repeat_is(&mut f, KEY_F9, actions[1]);
}

#[test]
fn a_grabbed_keys_release_ends_its_repeats() {
    let (mut f, _, _, bus) = with_a_focused_window();
    set_slow_repeat(&mut f);
    let mut caller = Caller::new(&mut f, &bus);
    let actions = caller.grab(&mut f, &["XF86AudioMute", "F9"]);
    f.key(KEY_MUTE, true);
    f.advance_clock(DELAY);
    assert_eq!(caller.next_repeat(&mut f).0, actions[0]);

    f.key(KEY_MUTE, false);

    f.advance_clock(PERIOD);
    caller.assert_next_repeat_is(&mut f, KEY_F9, actions[1]);
}

#[test]
fn another_keys_press_ends_the_repeats_of_a_held_grabbed_key() {
    let (mut f, _, _, bus) = with_a_focused_window();
    set_slow_repeat(&mut f);
    let mut caller = Caller::new(&mut f, &bus);
    let actions = caller.grab(&mut f, &["XF86AudioMute", "F9"]);
    f.key(KEY_MUTE, true);
    f.advance_clock(DELAY);
    assert_eq!(caller.next_repeat(&mut f).0, actions[0]);

    f.key(KEY_A, true);

    f.advance_clock(PERIOD);
    f.key(KEY_A, false);
    f.key(KEY_MUTE, false);
    caller.assert_next_repeat_is(&mut f, KEY_F9, actions[1]);
}

#[test]
fn releasing_the_grab_of_a_held_key_ends_its_repeats() {
    let (mut f, _, _, bus) = with_a_focused_window();
    set_slow_repeat(&mut f);
    let mut caller = Caller::new(&mut f, &bus);
    let actions = caller.grab(&mut f, &["XF86AudioMute", "F9"]);
    f.key(KEY_MUTE, true);
    f.advance_clock(DELAY);
    assert_eq!(caller.next_repeat(&mut f).0, actions[0]);

    caller.ungrab(&mut f, &actions[..1]);
    // Grabbed again, it has another id, and the key held since before isn't its press.
    let again = caller.grab(&mut f, &["XF86AudioMute"]);
    assert_ne!(again, actions[..1]);

    f.advance_clock(PERIOD);
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
    // The delay, and then ten of the periods the rate had before repeat went off: a key
    // that repeated would have by now.
    f.advance_clock(Duration::from_millis(30));
    f.advance_clock(Duration::from_millis(100));
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
    // Ctrl, as Alt+F9 is `minimize`'s (149).
    f.chord(&[KEY_LEFTCTRL, KEY_F9]);
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
    key 29 pressed
    key 67 pressed
    key 67 released
    key 29 released
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
fn a_combination_caminito_binds_itself_isnt_signalled() {
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

/// The grabs follow the keys (148): a combination Caminito stops binding is its grab's from
/// then on, and one it starts binding isn't any more.
#[test]
fn a_grab_follows_the_combinations_caminito_binds_as_the_keys_change() {
    let (mut f, id, window, bus) = with_a_focused_window();
    let mut caller = Caller::new(&mut f, &bus);
    let actions = caller.grab(&mut f, &["<Alt>F4", "F9", "XF86AudioMute"]);
    let [alt_f4, f9, mute] = actions[..] else {
        panic!("three action ids, not {actions:?}");
    };

    f.set_setting(MARCO_WINDOW_KEYBINDINGS, "close", Value::Str("F9".into()));
    f.dispatch_until(|f| {
        f.state
            .caminito
            .state_dump()
            .contains("\"combination\": \"F9\"")
    });
    f.chord(&[KEY_F9]);
    f.chord(&[KEY_LEFTALT, KEY_F4]);
    f.chord(&[KEY_MUTE]);

    // F9 closed the window instead of reaching its grab, and Alt+F4 reached its grab.
    assert_eq!(caller.next_activation(&mut f).0, alt_f4);
    assert_eq!(caller.next_activation(&mut f).0, mute);
    f.roundtrip(id);
    assert!(f.client(id).window(window.xdg()).close_requested);

    f.set_setting(
        MARCO_WINDOW_KEYBINDINGS,
        "close",
        Value::Str("disabled".into()),
    );
    f.dispatch_until(|f| !f.state.caminito.state_dump().contains("\"key\": \"close\""));
    f.chord(&[KEY_F9]);
    assert_eq!(caller.next_activation(&mut f).0, f9);
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

// Mouse buttons (185, ADR 0028).

/// `with_a_focused_window`, with the pointer inside the window.
fn with_the_pointer_in_a_focused_window() -> (Fixture, ClientId, ClientWindow, String) {
    let (mut f, id, window, bus) = with_a_focused_window();
    let inside = (f.geometry(window).loc + Point::from((200, 150))).to_f64();
    f.pointer_move(inside.x, inside.y);
    buttons_seen(&mut f, id);
    (f, id, window, bus)
}

#[test]
fn a_grabbed_button_signals_its_action_once_per_press_and_reaches_no_client() {
    let (mut f, id, _, bus) = with_the_pointer_in_a_focused_window();
    let mut caller = Caller::new(&mut f, &bus);
    let actions = caller.grab(&mut f, &["<Super>Button8", "Button9", "F9"]);
    let [super_back, forward, f9] = actions[..] else {
        panic!("three action ids, not {actions:?}");
    };
    assert!(super_back != 0 && forward != 0 && f9 != 0);
    assert!(super_back != forward && forward != f9 && super_back != f9);

    press(&mut f, &[KEY_LEFTMETA], BTN_SIDE);
    press(&mut f, &[], BTN_EXTRA);
    press(&mut f, &[KEY_LEFTMETA], BTN_SIDE);
    // F9 ends them: nothing else was signalled before it.
    f.chord(&[KEY_F9]);

    let (first, first_time) = caller.next_activation(&mut f);
    let (second, second_time) = caller.next_activation(&mut f);
    let (third, third_time) = caller.next_activation(&mut f);
    assert_eq!([first, second, third], [super_back, forward, super_back]);
    assert_eq!(caller.next_activation(&mut f).0, f9);
    // The button events' times, which the fixture makes later each time.
    assert!(first_time < second_time && second_time < third_time);
    // Neither the presses nor the releases.
    assert_eq!(buttons_seen(&mut f, id), []);
}

#[test]
fn a_grabbed_button_with_other_modifiers_or_released_reaches_the_client_as_before() {
    let (mut f, id, _, bus) = with_the_pointer_in_a_focused_window();
    let mut caller = Caller::new(&mut f, &bus);
    let actions = caller.grab(&mut f, &["<Super>Button8", "Button9", "F9"]);
    caller.ungrab(&mut f, &actions[1..2]);

    // The grabbed button without its modifier, another with it, and the one released.
    press(&mut f, &[], BTN_SIDE);
    press(&mut f, &[KEY_LEFTMETA], BTN_EXTRA);
    press(&mut f, &[], BTN_EXTRA);
    f.chord(&[KEY_F9]);

    assert_eq!(caller.next_activation(&mut f).0, actions[2]);
    assert_eq!(
        buttons_seen(&mut f, id),
        [
            (BTN_SIDE, true),
            (BTN_SIDE, false),
            (BTN_EXTRA, true),
            (BTN_EXTRA, false),
            (BTN_EXTRA, true),
            (BTN_EXTRA, false),
        ]
    );
}

#[test]
fn a_second_callers_grab_of_a_taken_button_is_refused() {
    let (mut f, _, _, bus) = with_the_pointer_in_a_focused_window();
    let mut first = Caller::new(&mut f, &bus);
    let mut second = Caller::new(&mut f, &bus);
    let [back] = first.grab(&mut f, &["<Super>Button8"])[..] else {
        panic!("one action id");
    };
    assert_ne!(back, 0);

    // However it's written, and among others that are granted: the same button with other
    // modifiers, and the key whose name is the button's number.
    let actions = second.grab(&mut f, &["<mod4>button8", "F9", "Button8", "<Super>8"]);
    let [0, f9, bare_back, super_8] = actions[..] else {
        panic!("a refusal and three action ids, not {actions:?}");
    };
    assert!(f9 != 0 && bare_back != 0 && super_8 != 0);
    // The first caller's grab is the one it had.
    assert_eq!(first.grab(&mut f, &["<Super>Button8"]), [back]);

    press(&mut f, &[KEY_LEFTMETA], BTN_SIDE);
    f.chord(&[KEY_F9]);
    assert_eq!(first.next_activation(&mut f).0, back);
    // The second caller hears of its own key, and wasn't told of the first's button before
    // it: the signal goes to the grabber alone.
    assert_eq!(second.next_activation(&mut f).0, f9);
}

#[test]
fn a_buttons_grab_goes_when_its_caller_leaves_the_bus() {
    let (mut f, id, _, bus) = with_the_pointer_in_a_focused_window();
    let leaver = Caller::new(&mut f, &bus);
    let grabbed = leaver.grab(&mut f, &["Button9"]);
    assert_ne!(grabbed, [0]);
    let other = Caller::new(&mut f, &bus);
    assert_eq!(other.grab(&mut f, &["Button9"]), [0]);

    drop(leaver);

    // The bus reports it some time after: until then the button is still taken.
    f.dispatch_until_mut(|f| {
        let actions = other.grab(f, &["Button9"]);
        other.ungrab(f, &actions);
        actions != [0]
    });
    press(&mut f, &[], BTN_EXTRA);
    assert_eq!(
        buttons_seen(&mut f, id),
        [(BTN_EXTRA, true), (BTN_EXTRA, false)]
    );
}

/// The grabs follow the keys (148), for a button as for a key.
#[test]
fn a_button_caminito_binds_itself_isnt_signalled_and_is_once_caminito_stops_binding_it() {
    let (mut f, id, window, bus) = with_the_pointer_in_a_focused_window();
    let bound = |f: &Fixture| {
        let dump = f.state.caminito.state_dump();
        dump.contains("\"combination\": \"<Super>Button8\"")
    };
    let value = Value::Str("<Super>Button8".into());
    f.set_setting(MARCO_WINDOW_KEYBINDINGS, "toggle-maximized", value);
    f.dispatch_until(bound);
    let mut caller = Caller::new(&mut f, &bus);
    // It's granted, as a key Caminito binds is, and waits for the binding to go.
    let actions = caller.grab(&mut f, &["<Super>Button8", "F9"]);
    let [back, f9] = actions[..] else {
        panic!("two action ids, not {actions:?}");
    };
    assert!(back != 0 && f9 != 0);
    let is_maximized = |f: &Fixture| f.state.caminito.policy.is_maximized(f.pid(window));

    press(&mut f, &[KEY_LEFTMETA], BTN_SIDE);
    f.chord(&[KEY_F9]);

    // The binding ran, and F9 is the first the caller hears of.
    assert!(is_maximized(&f));
    assert_eq!(caller.next_activation(&mut f).0, f9);

    let value = Value::Str("disabled".into());
    f.set_setting(MARCO_WINDOW_KEYBINDINGS, "toggle-maximized", value);
    f.dispatch_until(|f| !bound(f));
    press(&mut f, &[KEY_LEFTMETA], BTN_SIDE);

    assert_eq!(caller.next_activation(&mut f).0, back);
    assert!(is_maximized(&f));
    assert_eq!(buttons_seen(&mut f, id), []);
}

#[test]
fn a_grabbed_button_isnt_signalled_while_the_session_is_locked() {
    let (mut f, _, _, bus) = with_the_pointer_in_a_focused_window();
    let mut caller = Caller::new(&mut f, &bus);
    let actions = caller.grab(&mut f, &["Button9", "<Super>Button8"]);
    let locker = f.connect(Kind::Xdg);
    let lock = f.client(locker).lock_session();
    f.roundtrip(locker);
    assert_eq!(f.lock_state(), "locked");

    press(&mut f, &[], BTN_EXTRA);

    f.client(locker).unlock_and_destroy(lock);
    f.roundtrip(locker);
    assert_eq!(f.lock_state(), "unlocked");
    // The other button is the first the caller hears of.
    press(&mut f, &[KEY_LEFTMETA], BTN_SIDE);
    assert_eq!(caller.next_activation(&mut f).0, actions[1]);
}

#[test]
fn a_grabbed_button_isnt_signalled_during_a_move() {
    let (mut f, _, _, bus) = with_the_pointer_in_a_focused_window();
    let mut caller = Caller::new(&mut f, &bus);
    let actions = caller.grab(&mut f, &["<Alt>Button9", "<Alt>Button8"]);

    // Alt and the left button move the window (162).
    f.key(KEY_LEFTALT, true);
    f.pointer_button(BTN_LEFT, true);
    assert!(f.state.caminito.policy.grab(SEAT).is_some());
    f.pointer_button(BTN_EXTRA, true);
    f.pointer_button(BTN_EXTRA, false);
    assert!(f.state.caminito.policy.grab(SEAT).is_some());
    f.pointer_button(BTN_LEFT, false);

    // Once it's over a button is signalled, and the other is the first the caller hears of.
    f.pointer_button(BTN_SIDE, true);
    f.pointer_button(BTN_SIDE, false);
    f.key(KEY_LEFTALT, false);
    assert_eq!(caller.next_activation(&mut f).0, actions[1]);
}

#[test]
fn a_modifier_click_wins_over_a_window_and_the_grab_has_the_press_over_the_desktop() {
    let (mut f, id, _, bus) = with_the_pointer_in_a_focused_window();
    let mut caller = Caller::new(&mut f, &bus);
    let actions = caller.grab(&mut f, &["<Alt>Button2", "F9"]);
    let window_menu_is_open = |f: &Fixture| {
        let dump: serde_json::Value = serde_json::from_str(&f.state.caminito.state_dump()).unwrap();
        !dump["window_menu"].is_null()
    };

    // Alt and the middle button open the window's menu by default (162).
    press(&mut f, &[KEY_LEFTALT], BTN_MIDDLE);
    assert!(window_menu_is_open(&f));
    f.chord(&[KEY_ESC]);
    assert!(!window_menu_is_open(&f));
    f.chord(&[KEY_F9]);
    assert_eq!(caller.next_activation(&mut f).0, actions[1]);

    f.pointer_move(1900., 1000.);
    press(&mut f, &[KEY_LEFTALT], BTN_MIDDLE);
    assert!(!window_menu_is_open(&f));
    assert_eq!(caller.next_activation(&mut f).0, actions[0]);
    assert_eq!(buttons_seen(&mut f, id), []);
}

#[test]
fn an_inhibitor_on_the_focused_surface_passes_a_grabbed_button_to_the_client() {
    let (mut f, id, window, bus) = with_the_pointer_in_a_focused_window();
    let mut caller = Caller::new(&mut f, &bus);
    let actions = caller.grab(&mut f, &["<Super>Button8", "F9"]);
    f.client(id).inhibit_shortcuts(window.xdg());
    f.roundtrip(id);

    press(&mut f, &[KEY_LEFTMETA], BTN_SIDE);

    assert_eq!(
        buttons_seen(&mut f, id),
        [(BTN_SIDE, true), (BTN_SIDE, false)]
    );
    // Nothing was signalled: another client's window has the focus now, and no inhibitor,
    // and F9 is the first the caller hears of.
    let other = f.connect(Kind::Xdg);
    f.map(other, WindowSpec::plain((200, 200)));
    f.chord(&[KEY_F9]);
    assert_eq!(caller.next_activation(&mut f).0, actions[1]);
}

#[test]
fn a_button_that_doesnt_parse_gets_no_action_and_one_log_line() {
    let (mut f, _, _, bus) = with_a_focused_window();
    let caller = Caller::new(&mut f, &bus);
    let logs = Logs::capture();

    // The wheel's, the left button with no modifier, and one past the kernel's.
    let actions = caller.grab(
        &mut f,
        &["Button4", "Button1", "<Super>Button21", "Button9"],
    );

    let [0, 0, 0, forward] = actions[..] else {
        panic!("three refusals and an action id, not {actions:?}");
    };
    assert_ne!(forward, 0);
    let logs = logs.text();
    let about = |accelerator: &str| {
        let line = format!(
            "can't grab the accelerator {accelerator:?} for {}",
            caller.name()
        );
        logs.matches(&line).count()
    };
    assert_eq!(about("Button4"), 1, "{logs}");
    assert_eq!(about("Button1"), 1, "{logs}");
    assert_eq!(about("<Super>Button21"), 1, "{logs}");
    assert_eq!(logs.matches("can't grab").count(), 3, "{logs}");
}

#[test]
fn the_state_dump_lists_the_button_grabs_and_their_owners() {
    let (mut f, _, _, bus) = with_a_focused_window();
    let first = Caller::new(&mut f, &bus);
    let second = Caller::new(&mut f, &bus);
    first.grab(&mut f, &["<Super>Button8", "XF86AudioMute"]);
    let released = second.grab(&mut f, &["Button2", "<mod4><ctl>button9"]);
    second.ungrab(&mut f, &released[..1]);

    let dump = f.dbus_call(move || debug_proxy(&bus).unwrap().dump_state().unwrap());

    let dump: serde_json::Value = serde_json::from_str(&dump).unwrap();
    let grabs = serde_json::to_string_pretty(&dump["accelerator_grabs"]).unwrap();
    let grabs = grabs
        .replace(&first.name(), "[first]")
        .replace(&second.name(), "[second]");
    // As each grabber wrote it.
    assert_snapshot!(grabs, @r#"
    [
      {
        "accelerator": "<Super>Button8",
        "action": 1,
        "owner": "[first]"
      },
      {
        "accelerator": "XF86AudioMute",
        "action": 2,
        "owner": "[first]"
      },
      {
        "accelerator": "<mod4><ctl>button9",
        "action": 4,
        "owner": "[second]"
      }
    ]
    "#);
}
