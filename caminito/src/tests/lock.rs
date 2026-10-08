//! The session lock (12): ext-session-lock with a test locker, and what the locked state
//! keeps from every other client. The keys and the pointer are checked as a Wayland client
//! sees them, so the windows are xdg toplevels (45); the Xwayland keyboard grabs are X's.

use caminito_test_client::dbus::caminito_proxy;
use caminito_test_client::layer_shell::{Layer, LayerSpec};
use caminito_test_client::session_lock::{LockId, LockSurfaceId};
use caminito_test_client::{
    Anchor, ConstraintAdjustment, Gravity, InputEvent, Positioner, SurfaceRef, format_input,
};
use insta::assert_snapshot;
use smithay::utils::Point;
use x11rb::protocol::Event;
use x11rb::protocol::xproto::GrabStatus;

use super::fixture::{
    BTN_LEFT, ClientId, ClientWindow, Fixture, KEY_A, KEY_F4, KEY_F9, KEY_LEFTALT, KEY_LEFTCTRL,
    KEY_LEFTSHIFT, KEY_MUTE, KEY_RIGHT, Kind, WINDOW_COLOR, WindowSpec,
};
use crate::scene::SceneLayer;
use crate::settings::Value;
use crate::settings::translate::KEYBOARD_XKB;

/// The lock background, as `wait_for_pixel` reads it.
const LOCK_BACKGROUND: u32 = 0x000000;
/// What the test locker fills its lock surfaces with.
const LOCKER_COLOR: u32 = 0xff12_3456;
const RED: u32 = 0xffff_0000;

/// Asks for the session lock as `locker`, and returns once it has its answer.
fn lock(f: &mut Fixture, locker: ClientId) -> LockId {
    let lock = f.client(locker).lock_session();
    f.roundtrip(locker);
    lock
}

/// Locks the session as `locker`, a client that listens to the keyboard and the pointer,
/// and shows a lock surface on the first output.
fn lock_with_a_surface(f: &mut Fixture, locker: ClientId) -> (LockId, LockSurfaceId) {
    f.client(locker).bind_input();
    f.client(locker).bind_outputs();
    let lock = lock(f, locker);
    assert!(f.client(locker).session_lock(lock).locked);
    let surface = f.client(locker).create_lock_surface(lock, 0);
    f.roundtrip(locker);
    f.client(locker).map_lock_surface(surface, LOCKER_COLOR);
    f.roundtrip(locker);
    (lock, surface)
}

/// A client that listens to the keyboard and the pointer, with a window that has the
/// keyboard and the pointer over it, and a second client to lock with.
fn a_focused_window_and_a_locker() -> (Fixture, ClientWindow, ClientId) {
    let (mut f, client) = Fixture::connected(Kind::Xdg);
    f.client(client).bind_input();
    let window = f.map(client, WindowSpec::plain((400, 300)));
    let inside = f.geometry(window).loc + Point::from((10, 10));
    f.pointer_move(f64::from(inside.x), f64::from(inside.y));
    f.roundtrip(client);
    assert!(f.has_keyboard(window));
    f.client(client).state.input.clear();
    let locker = f.connect(Kind::Xdg);
    (f, window, locker)
}

#[test]
fn a_lock_gets_locked_and_the_session_is_behind_the_lock_background() {
    let (mut f, client) = Fixture::connected(Kind::Xdg);
    let window = f.map(client, WindowSpec::plain((400, 300)));
    let inside = f.geometry(window).loc + Point::from((10, 10));
    let locker = f.connect(Kind::Xdg);
    assert_eq!(f.lock_state(), "unlocked");

    let lock = lock(&mut f, locker);

    assert!(f.client(locker).session_lock(lock).locked);
    assert_eq!(f.lock_state(), "locked");
    f.wait_for_pixel(inside.x, inside.y, LOCK_BACKGROUND);
}

#[test]
fn no_other_client_has_the_keyboard_or_the_pointer_once_locked() {
    let (mut f, window, locker) = a_focused_window_and_a_locker();
    let client = window.client();

    lock(&mut f, locker);
    f.roundtrip(client);
    assert!(!f.has_keyboard(window));
    assert_snapshot!(format_input(&f.client(client).state.input), @r"
    pointer leaves window 0
    keyboard leaves window 0
    ");

    f.client(client).state.input.clear();
    let inside = f.geometry(window).loc + Point::from((20, 20));
    f.click(f64::from(inside.x), f64::from(inside.y));
    f.chord(&[KEY_A]);
    f.roundtrip(client);
    assert_eq!(f.client(client).state.input, []);
    assert!(!f.has_keyboard(window));
}

#[test]
fn a_window_mapped_while_locked_gets_neither_the_keyboard_nor_the_pointer() {
    let (mut f, _, locker) = a_focused_window_and_a_locker();
    lock(&mut f, locker);
    let late = f.connect(Kind::Xdg);
    f.client(late).bind_input();

    let window = f.map(late, WindowSpec::plain((400, 300)));
    let inside = f.geometry(window).loc + Point::from((10, 10));
    f.click(f64::from(inside.x), f64::from(inside.y));
    f.chord(&[KEY_A]);
    f.roundtrip(late);

    assert_eq!(f.client(late).state.input, []);
}

#[test]
fn the_locker_shows_its_lock_surface_and_has_the_keyboard_and_the_pointer_on_it() {
    let (mut f, window, locker) = a_focused_window_and_a_locker();
    // Away from the cursor.
    let inside = f.geometry(window).loc + Point::from((200, 200));

    let (_, surface) = lock_with_a_surface(&mut f, locker);

    assert_eq!(
        f.client(locker).lock_surface(surface).configures,
        [(1920, 1080)]
    );
    f.wait_for_pixel(inside.x, inside.y, LOCKER_COLOR & 0xff_ffff);
    f.chord(&[KEY_A]);
    f.pointer_button(BTN_LEFT, true);
    f.pointer_button(BTN_LEFT, false);
    f.roundtrip(locker);
    assert_snapshot!(format_input(&f.client(locker).state.input), @r"
    keyboard enters lock surface 0
    pointer enters lock surface 0 at 168, 69
    key 30 pressed
    key 30 released
    button 0x110 pressed
    button 0x110 released
    ");
}

#[test]
fn the_lock_surface_follows_its_outputs_size() {
    let (mut f, _, locker) = a_focused_window_and_a_locker();
    let (_, surface) = lock_with_a_surface(&mut f, locker);
    let output = f.state.caminito.outputs[0].clone();

    f.set_output_scale(&output, 2.0);
    f.roundtrip(locker);

    assert_eq!(
        f.client(locker).lock_surface(surface).configures,
        [(1920, 1080), (960, 540)]
    );
}

#[test]
fn unlock_and_destroy_gives_the_focus_back_to_the_window_that_had_it() {
    let (mut f, window, locker) = a_focused_window_and_a_locker();
    let client = window.client();
    let other = f.map(client, WindowSpec::plain((200, 200)));
    let focused = f.pid(window);
    let now = f.state.caminito.now();
    f.state
        .caminito
        .policy
        .activate(crate::state::SEAT, focused, now);
    f.roundtrip(client);
    assert!(f.has_keyboard(window) && !f.has_keyboard(other));
    let (lock, _) = lock_with_a_surface(&mut f, locker);
    f.roundtrip(client);
    f.client(client).state.input.clear();

    f.client(locker).unlock_and_destroy(lock);
    f.roundtrip(locker);
    f.roundtrip(client);

    assert_eq!(f.lock_state(), "unlocked");
    assert!(f.has_keyboard(window));
    let entered = InputEvent::KeyboardEnter {
        surface: SurfaceRef::Window(window.xdg()),
    };
    assert!(f.client(client).state.input.contains(&entered));
    // Away from the cursor.
    let inside = f.geometry(window).loc + Point::from((200, 200));
    f.wait_for_pixel(inside.x, inside.y, WINDOW_COLOR & 0xff_ffff);
}

#[test]
fn a_locker_that_is_killed_leaves_the_session_locked() {
    let (mut f, window, locker) = a_focused_window_and_a_locker();
    // Away from the cursor.
    let inside = f.geometry(window).loc + Point::from((200, 200));
    lock_with_a_surface(&mut f, locker);

    f.disconnect(locker);

    assert_eq!(f.lock_state(), "locker-lost");
    assert!(!f.has_keyboard(window));
    f.wait_for_pixel(inside.x, inside.y, LOCK_BACKGROUND);
}

#[test]
fn a_second_clients_lock_is_refused_once_the_locker_is_gone() {
    let (mut f, window, locker) = a_focused_window_and_a_locker();
    lock_with_a_surface(&mut f, locker);
    f.disconnect(locker);
    let second = f.connect(Kind::Xdg);

    let lock = lock(&mut f, second);

    let answer = f.client(second).session_lock(lock);
    assert!(answer.finished && !answer.locked);
    assert_eq!(f.lock_state(), "locker-lost");
    assert!(!f.has_keyboard(window));
}

/// 126 spawns the locker; until then a test names the process itself.
#[test]
fn a_locker_caminito_spawned_takes_the_lock_over_once_the_locker_is_gone() {
    let (mut f, _, locker) = a_focused_window_and_a_locker();
    lock_with_a_surface(&mut f, locker);
    f.disconnect(locker);
    f.state.caminito.lock.spawned_locker = Some(4242);
    let other = f.connect_as_process(4243);
    let spawned = f.connect_as_process(4242);

    let refused = lock(&mut f, other);
    let taken = lock(&mut f, spawned);

    assert!(f.client(other).session_lock(refused).finished);
    assert!(f.client(spawned).session_lock(taken).locked);
    assert_eq!(f.lock_state(), "locked");
}

#[test]
fn a_second_lock_is_refused_while_a_locker_holds_the_lock() {
    let (mut f, _, locker) = a_focused_window_and_a_locker();
    let held = lock(&mut f, locker);
    let second = f.connect(Kind::Xdg);

    let lock = lock(&mut f, second);

    let answer = f.client(second).session_lock(lock);
    assert!(answer.finished && !answer.locked);
    // The first is still the one that unlocks.
    f.client(locker).unlock_and_destroy(held);
    f.roundtrip(locker);
    assert_eq!(f.lock_state(), "unlocked");
}

#[test]
fn a_binding_does_nothing_while_locked() {
    let (mut f, window, locker) = a_focused_window_and_a_locker();
    let client = window.client();
    let (lock, _) = lock_with_a_surface(&mut f, locker);

    f.chord(&[KEY_LEFTALT, KEY_F4]);
    f.chord(&[KEY_LEFTCTRL, KEY_LEFTALT, KEY_RIGHT]);
    f.roundtrip(client);

    assert!(!f.client(client).window(window.xdg()).close_requested);
    assert_eq!(f.state.caminito.policy.active_workspace(), 0);
    // And does again after the unlock.
    f.client(locker).unlock_and_destroy(lock);
    f.roundtrip(locker);
    f.chord(&[KEY_LEFTALT, KEY_F4]);
    f.roundtrip(client);
    assert!(f.client(client).window(window.xdg()).close_requested);
}

#[test]
fn a_grabbed_accelerator_does_nothing_while_locked() {
    let (mut f, _, locker) = a_focused_window_and_a_locker();
    let bus = f.start_dbus();
    let (caminito, mut activations) = f.dbus_call(move || {
        let caminito = caminito_proxy(&bus).unwrap();
        let activations = caminito.receive_accelerator_activated().unwrap();
        (caminito, activations)
    });
    let actions = f.dbus_call(move || {
        caminito
            .grab_accelerators(&["XF86AudioMute", "F9"])
            .unwrap()
    });
    let (lock, _) = lock_with_a_surface(&mut f, locker);

    f.chord(&[KEY_MUTE]);
    f.client(locker).unlock_and_destroy(lock);
    f.roundtrip(locker);
    // F9 is signalled first: nothing was while the session was locked.
    f.chord(&[KEY_F9]);

    let first = f.dbus_call(move || activations.next().unwrap().args().unwrap().action);
    assert_eq!(first, actions[1]);
}

#[test]
fn a_layout_switch_works_while_locked() {
    let strings =
        |values: &[&str]| Value::StrList(values.iter().map(|&value| value.into()).collect());
    let mut f = Fixture::with_settings(vec![
        (KEYBOARD_XKB, "layouts".into(), strings(&["us", "es"])),
        (
            KEYBOARD_XKB,
            "options".into(),
            strings(&["grp\tgrp:alt_shift_toggle"]),
        ),
    ]);
    f.add_output((1920, 1080), 1.0);
    let locker = f.connect(Kind::Xdg);
    lock_with_a_surface(&mut f, locker);
    assert_eq!(f.client_state(locker).group, 0);

    f.chord(&[KEY_LEFTALT, KEY_LEFTSHIFT]);
    f.roundtrip(locker);

    assert_eq!(f.client_state(locker).group, 1);
}

/// A menu's grab would keep the keyboard and the pointer, and the menu would be there, open
/// and without them, after the unlock. One asked for under the lock isn't granted.
#[test]
fn an_open_menu_is_dismissed_as_the_session_locks_and_none_opens_under_the_lock() {
    let (mut f, window, locker) = a_focused_window_and_a_locker();
    let client = window.client();
    f.pointer_button(BTN_LEFT, true);
    f.pointer_button(BTN_LEFT, false);
    f.roundtrip(client);
    let press = f.client(client).state.last_press_serial.unwrap();
    let place = || Positioner {
        size: (300, 200),
        anchor_rect: (10, 20, 30, 40),
        anchor: Anchor::BottomRight,
        gravity: Gravity::BottomRight,
        constraint_adjustment: ConstraintAdjustment::None,
    };
    let menu = f.client(client).create_popup(window.xdg(), place());
    f.client(client).grab_popup(menu, press);
    f.roundtrip(client);
    f.client(client).map_popup(menu, RED);
    f.roundtrip(client);
    assert!(!f.client(client).popup(menu).done);

    lock_with_a_surface(&mut f, locker);
    let late = f.client(client).create_popup(window.xdg(), place());
    f.client(client).grab_popup(late, press);
    f.roundtrip(client);

    assert!(f.client(client).popup(menu).done);
    assert!(f.client(client).popup(late).done);
    f.chord(&[KEY_A]);
    f.roundtrip(locker);
    let key = InputEvent::Key {
        key: KEY_A,
        pressed: true,
    };
    assert!(f.client(locker).state.input.contains(&key));
}

#[test]
fn nothing_of_the_session_is_in_the_scene_while_locked() {
    let (mut f, _, locker) = a_focused_window_and_a_locker();
    let panel = f.connect(Kind::Xdg);
    f.map_layer(panel, LayerSpec::floating(Layer::Overlay, (200, 100)), RED);
    let x = f.connect(Kind::X11);
    f.map_override_redirect(x, (500, 500), (100, 100), RED);
    let output = f.state.caminito.outputs[0].clone();

    lock_with_a_surface(&mut f, locker);

    let mut layers = f.state.scene_layers(&output);
    layers.dedup();
    assert_eq!(layers, [SceneLayer::Cursor, SceneLayer::Lock]);
}

#[test]
fn an_xwayland_keyboard_grab_that_is_held_is_released_as_the_session_locks() {
    let (mut f, x) = Fixture::connected(Kind::X11);
    let toplevel = f.map(x, WindowSpec::default());
    let menu = f.map_override_redirect(x, (500, 500), (100, 100), RED);
    let status = f.x11_call(x, move |x| x.grab_keyboard(menu)).unwrap();
    assert_eq!(status, GrabStatus::SUCCESS);
    f.dispatch_until(|f| f.override_redirect_with_keyboard() == Some(menu));
    let locker = f.connect(Kind::Xdg);

    let (lock, _) = lock_with_a_surface(&mut f, locker);
    assert_eq!(f.override_redirect_with_keyboard(), None);
    f.x11_call(x, |x| x.events()).unwrap();
    f.chord(&[KEY_A]);
    f.roundtrip(locker);

    let events = f.x11_call(x, |x| x.events()).unwrap();
    assert!(
        !events
            .iter()
            .any(|event| matches!(event, Event::KeyPress(_))),
        "{events:?}"
    );
    let key = InputEvent::Key {
        key: KEY_A,
        pressed: true,
    };
    assert!(f.client(locker).state.input.contains(&key));
    // The grab is gone, not set aside: the unlock gives the keyboard to the window.
    f.client(locker).unlock_and_destroy(lock);
    f.roundtrip(locker);
    f.dispatch_until(|f| f.has_keyboard(toplevel));
}

#[test]
fn an_xwayland_keyboard_grab_asked_for_while_locked_is_refused() {
    let (mut f, x) = Fixture::connected(Kind::X11);
    let toplevel = f.map(x, WindowSpec::default());
    let menu = f.map_override_redirect(x, (500, 500), (100, 100), RED);
    let locker = f.connect(Kind::Xdg);
    let (lock, _) = lock_with_a_surface(&mut f, locker);

    // X grants the grab; it's Xwayland's request for the keyboard that's refused. Xwayland
    // has made it by the time it answers the client's next request.
    let status = f.x11_call(x, move |x| x.grab_keyboard(menu)).unwrap();
    assert_eq!(status, GrabStatus::SUCCESS);
    f.x11_call(x, |x| x.events()).unwrap();
    f.sync(toplevel);
    f.chord(&[KEY_A]);
    f.roundtrip(locker);

    assert_eq!(f.override_redirect_with_keyboard(), None);
    let events = f.x11_call(x, |x| x.events()).unwrap();
    assert!(
        !events
            .iter()
            .any(|event| matches!(event, Event::KeyPress(_))),
        "{events:?}"
    );
    let key = InputEvent::Key {
        key: KEY_A,
        pressed: true,
    };
    assert!(f.client(locker).state.input.contains(&key));
    // It wasn't set aside for the unlock either.
    f.client(locker).unlock_and_destroy(lock);
    f.roundtrip(locker);
    f.dispatch_until(|f| f.has_keyboard(toplevel));
    assert_eq!(f.override_redirect_with_keyboard(), None);
}
