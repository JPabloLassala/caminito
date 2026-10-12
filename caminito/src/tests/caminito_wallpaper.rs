//! The Wallpaper dialog against Caminito (212): its client of output management and its
//! page, without its window. The sidebar has each output Caminito shows, left to right, by
//! the identity the drawer knows it by, and follows a monitor plugged in and out.

use caminito_wallpaper::monitors::Monitors;
use caminito_wallpaper::page::{Output, Page};
use caminito_wallpaper::{Memory, Store};

use super::fixture::Fixture;

/// The AOC G2490W1G4: its EDID's size and preferred mode.
const AOC: ((i32, i32), (i32, i32)) = ((527, 296), (1920, 1080));
/// The LG HDR 4K.
const LG: ((i32, i32), (i32, i32)) = ((600, 340), (3840, 2160));

/// Opens the dialog's connection on `f`'s Caminito, and waits for the outputs.
fn open(f: &mut Fixture) -> Monitors {
    let mut monitors = Monitors::over(f.connect_stream()).unwrap();
    wait_for_change(f, &mut monitors);
    monitors
}

fn wait_for_change(f: &mut Fixture, monitors: &mut Monitors) {
    f.dispatch_until_mut(|_| monitors.dispatch().unwrap());
}

/// Each output of the page as the sidebar has it: connector, title, place, size and the
/// identity's path element.
fn sidebar(outputs: &[Output]) -> String {
    let line = |output: &Output| {
        let (x, y, (w, h)) = (output.x, output.y, output.size);
        format!(
            "{} {:?} at {x},{y} {w}x{h} {}",
            output.connector,
            output.title,
            output.identity.path_element()
        )
    };
    outputs.iter().map(line).collect::<Vec<_>>().join("\n")
}

#[test]
fn the_sidebar_has_each_output_left_to_right_and_follows_a_hot_plug() {
    let mut f = Fixture::new();
    let _aoc = f.plug(AOC.0, AOC.1);
    let lg = f.plug(LG.0, LG.1);
    let mut monitors = open(&mut f);
    let mut store = Memory::default();

    let page = Page::new(&mut store, monitors.outputs(), Some("headless-1"));
    insta::assert_snapshot!(sidebar(page.outputs()), @r#"
    headless-1 "Caminito Headless" at 0,0 1920x1080 connector-headless_2D1
    headless-2 "Caminito Headless" at 1920,0 3840x2160 connector-headless_2D2
    "#);
    assert_eq!(page.selected(), 0);
    assert!(page.has_switch());
    // Each is seen, as the drawer would see it.
    assert_eq!(store.seen().len(), 2);

    f.unplug(&lg);
    wait_for_change(&mut f, &mut monitors);
    let page = Page::new(&mut store, monitors.outputs(), Some("headless-1"));
    assert_eq!(page.outputs().len(), 1);
    assert!(!page.has_switch());

    let _internal = f.plug_internal((300, 190), (2560, 1600));
    wait_for_change(&mut f, &mut monitors);
    let page = Page::new(&mut store, monitors.outputs(), Some("headless-1"));
    insta::assert_snapshot!(sidebar(page.outputs()), @r#"
    headless-1 "Caminito Headless" at 0,0 1920x1080 connector-headless_2D1
    eDP-3 "Built-in display" at 1920,0 2560x1600 connector-eDP_2D3
    "#);
}
