//! The primary output (81, ADR 0014): which output it is with no choice made, a choice made
//! over `org.mate.Camino`, the stand-in while the chosen monitor is away, and what follows
//! it: where a new window opens, and XWayland's RandR primary. The debug dump and the bus
//! show it. The monitors are the developer's (50), as in `output_lifecycle`, and a laptop's
//! panel.

use std::sync::mpsc;
use std::thread;

use camino_test_client::dbus::camino_proxy;
use camino_test_client::output_management::HeadChange;
use smithay::output::Output;

use super::fixture::{Fixture, Kind, WindowSpec, on_both_kinds};

/// The AOC G2490W1G4: its EDID's size and preferred mode. It's 1920 logical pixels wide.
const AOC: ((i32, i32), (i32, i32)) = ((527, 296), (1920, 1080));
/// The LG HDR 4K, 2259 logical pixels wide at its default scale.
const LG: ((i32, i32), (i32, i32)) = ((600, 340), (3840, 2160));
/// A 15.6" laptop's panel.
const PANEL: ((i32, i32), (i32, i32)) = ((344, 194), (1920, 1080));

fn plug(f: &mut Fixture, (mm, mode): ((i32, i32), (i32, i32))) -> Output {
    f.plug(mm, mode)
}

/// The primary output's name, from the dump.
fn primary(f: &Fixture) -> Option<String> {
    let dump: serde_json::Value = serde_json::from_str(&f.state.camino.state_dump()).unwrap();
    dump["primary_output"].as_str().map(str::to_owned)
}

/// The AOC as `headless-1` on the left and the LG as `headless-2` on its right, with no
/// choice made.
fn aoc_and_lg() -> (Fixture, Output, Output) {
    let mut f = Fixture::new();
    let aoc = plug(&mut f, AOC);
    let lg = plug(&mut f, LG);
    (f, aoc, lg)
}

/// Waits on another thread for `org.mate.Camino` at `bus` to signal that the primary
/// output changed, once, and returns a handle to what it changed to. It returns once the
/// thread listens.
fn next_signalled(f: &mut Fixture, bus: &str) -> thread::JoinHandle<String> {
    let bus = bus.to_owned();
    let (listening, is_listening) = mpsc::channel();
    let listener = thread::spawn(move || {
        let proxy = camino_proxy(&bus).unwrap();
        // Reading it has the proxy listen for changes before this goes on, and zbus's
        // stream of changes starts with the value read.
        let was = proxy.primary_output().unwrap();
        let mut changes = proxy.receive_primary_output_changed();
        assert_eq!(changes.next().unwrap().get().unwrap(), was);
        listening.send(()).unwrap();
        changes.next().unwrap().get().unwrap()
    });
    f.dispatch_until(|_| is_listening.try_recv().is_ok());
    listener
}

/// What a listener from `next_signalled` heard.
fn signalled(f: &mut Fixture, listener: thread::JoinHandle<String>) -> String {
    f.dispatch_until(|_| listener.is_finished());
    listener.join().unwrap()
}

#[test]
fn with_no_outputs_there_is_no_primary_output() {
    let f = Fixture::new();

    assert_eq!(primary(&f), None);
}

#[test]
fn with_no_choice_made_the_primary_output_is_the_leftmost() {
    let (mut f, _aoc, lg) = aoc_and_lg();
    assert_eq!(primary(&f).as_deref(), Some("headless-1"));

    // The AOC, still at its default place, goes to the LG's right.
    f.set_output_position(&lg, (0, 0));

    assert_eq!(primary(&f).as_deref(), Some("headless-2"));
}

#[test]
fn with_no_choice_made_the_primary_output_is_the_built_in_panel() {
    let mut f = Fixture::new();
    plug(&mut f, AOC);
    let (mm, mode) = PANEL;
    f.plug_internal(mm, mode);

    assert_eq!(primary(&f).as_deref(), Some("eDP-2"));
}

#[test]
fn a_choice_set_over_dbus_is_read_back_and_signalled() {
    let (mut f, _aoc, _lg) = aoc_and_lg();
    let bus = f.start_dbus();
    let read = |f: &mut Fixture| {
        let bus = bus.clone();
        f.dbus_call(move || camino_proxy(&bus).unwrap().primary_output().unwrap())
    };
    assert_eq!(read(&mut f), "headless-1");
    let listener = next_signalled(&mut f, &bus);

    let chooser = bus.clone();
    f.dbus_call(move || {
        let proxy = camino_proxy(&chooser).unwrap();
        proxy.set_primary_output("headless-2").unwrap();
    });

    assert_eq!(signalled(&mut f, listener), "headless-2");
    assert_eq!(read(&mut f), "headless-2");
    assert_eq!(primary(&f).as_deref(), Some("headless-2"));
}

#[test]
fn an_output_camino_doesnt_show_cant_be_chosen() {
    let (mut f, _aoc, _lg) = aoc_and_lg();
    let client = f.connect(Kind::Xdg);
    f.client(client).bind_output_management();
    f.roundtrip(client);
    f.client(client)
        .apply_outputs(&[HeadChange::of("headless-2").disable()]);
    f.roundtrip(client);
    let bus = f.start_dbus();

    for name in ["headless-2", "DP-7", ""] {
        let bus = bus.clone();
        let refused = f.dbus_call(move || {
            let proxy = camino_proxy(&bus).unwrap();
            proxy.set_primary_output(name).unwrap_err().to_string()
        });
        assert!(
            refused.contains("isn't an enabled output"),
            "{name:?} is refused: {refused}"
        );
    }
    assert_eq!(primary(&f).as_deref(), Some("headless-1"));
}

#[test]
fn unplugging_the_primary_output_puts_the_stand_in_in_and_plugging_it_back_returns_the_choice() {
    let (mut f, _aoc, lg) = aoc_and_lg();
    f.choose_primary_output(&lg);
    let bus = f.start_dbus();

    let listener = next_signalled(&mut f, &bus);
    f.unplug(&lg);
    assert_eq!(signalled(&mut f, listener), "headless-1");
    assert_eq!(primary(&f).as_deref(), Some("headless-1"));

    let listener = next_signalled(&mut f, &bus);
    f.replug(&lg);
    assert_eq!(signalled(&mut f, listener), "headless-2");
    assert_eq!(primary(&f).as_deref(), Some("headless-2"));
}

#[test]
fn the_built_in_panel_stands_in_before_the_leftmost_output() {
    let (mut f, _aoc, lg) = aoc_and_lg();
    let (mm, mode) = PANEL;
    f.plug_internal(mm, mode);
    f.choose_primary_output(&lg);

    f.unplug(&lg);

    assert_eq!(primary(&f).as_deref(), Some("eDP-3"));
}

#[test]
fn disabling_the_primary_output_puts_the_stand_in_in_and_enabling_it_returns_the_choice() {
    let (mut f, _aoc, lg) = aoc_and_lg();
    f.choose_primary_output(&lg);
    let client = f.connect(Kind::Xdg);
    f.client(client).bind_output_management();
    f.roundtrip(client);

    f.client(client)
        .apply_outputs(&[HeadChange::of("headless-2").disable()]);
    f.roundtrip(client);
    assert_eq!(primary(&f).as_deref(), Some("headless-1"));

    f.client(client)
        .apply_outputs(&[HeadChange::of("headless-2").enable()]);
    f.roundtrip(client);
    assert_eq!(primary(&f).as_deref(), Some("headless-2"));
}

#[test]
fn a_choice_made_while_the_stand_in_is_in_replaces_the_one_that_is_away() {
    let mut f = Fixture::new();
    plug(&mut f, AOC);
    let lg = plug(&mut f, LG);
    let third = plug(&mut f, AOC);
    f.choose_primary_output(&lg);
    f.unplug(&lg);

    f.choose_primary_output(&third);
    f.replug(&lg);

    assert_eq!(primary(&f), Some(third.name()));
}

on_both_kinds! {
    fn a_new_window_with_nothing_to_go_by_opens_on_the_primary_output(kind: Kind) {
        let (mut f, _aoc, lg) = aoc_and_lg();
        f.choose_primary_output(&lg);
        let client = f.connect(kind);

        let window = f.map(client, WindowSpec::default());

        let geometry = f.geometry(window);
        assert!(geometry.loc.x >= 1920, "the window is on the LG: {geometry:?}");
    }
}

/// RandR is X11's, so this connects an X client only.
#[test]
fn xwaylands_randr_primary_is_the_primary_output_and_follows_it() {
    let (mut f, _aoc, lg) = aoc_and_lg();
    let x = f.connect(Kind::X11);
    let randr_primary = |f: &mut Fixture| f.x11_call(x, |x| x.randr_primary()).unwrap();
    f.dispatch_until_mut(|f| randr_primary(f).as_deref() == Some("headless-1"));

    f.choose_primary_output(&lg);
    f.dispatch_until_mut(|f| randr_primary(f).as_deref() == Some("headless-2"));

    // The stand-in is X11's primary too.
    f.unplug(&lg);
    f.dispatch_until_mut(|f| randr_primary(f).as_deref() == Some("headless-1"));
}
