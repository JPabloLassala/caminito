//! Saved layouts (83, ADR 0014): a layout is saved when `org.mate.Camino` is asked, per set
//! of connected monitors, and restored when Camino next has that set, as the debug dump
//! shows. A restart is a new fixture that keeps its layouts in the same directory. The
//! headless backend has no monitors before the first is plugged in, so a start with two
//! monitors is the second one's plugging in; `output::saved`'s own tests have the monitors
//! with EDID serials. The monitors are the developer's (50), as in `output_lifecycle`.

use std::path::Path;

use camino_test_client::dbus::camino_proxy;
use camino_test_client::output_management::HeadChange;
use insta::assert_snapshot;
use smithay::output::Output;

use super::fixture::{ClientId, Fixture, Kind, Logs};
use crate::settings::Value;
use crate::settings::translate::{INTERFACE, WindowScalingFactor};

/// The AOC G2490W1G4: its EDID's size and preferred mode. Its default scale is 1.
const AOC: ((i32, i32), (i32, i32)) = ((527, 296), (1920, 1080));
/// The LG HDR 4K, whose default scale is 1.7.
const LG: ((i32, i32), (i32, i32)) = ((600, 340), (3840, 2160));

fn plug(f: &mut Fixture, (mm, mode): ((i32, i32), (i32, i32))) -> Output {
    f.plug(mm, mode)
}

fn dump(f: &Fixture) -> serde_json::Value {
    serde_json::from_str(&f.state.camino.state_dump()).unwrap()
}

/// Each output's name, place, logical size and scale, one per line, then the primary
/// output's name, from the dump.
fn layout(f: &Fixture) -> String {
    let dump = dump(f);
    let outputs = dump["outputs"].as_array().unwrap().iter().map(|output| {
        let g = &output["geometry"];
        format!(
            "{} at {},{} {}x{} scale {}",
            output["name"].as_str().unwrap(),
            g["x"],
            g["y"],
            g["width"],
            g["height"],
            output["scale"]
        )
    });
    let primary = format!("primary {}", dump["primary_output"].as_str().unwrap());
    outputs.chain([primary]).collect::<Vec<_>>().join("\n")
}

/// Camino keeping its layouts in `dir`, with the AOC as `headless-1` and the LG as
/// `headless-2`.
fn start_with_aoc_and_lg(dir: &Path) -> (Fixture, Output, Output) {
    let mut f = Fixture::with_saved_layouts(dir);
    let aoc = plug(&mut f, AOC);
    let lg = plug(&mut f, LG);
    (f, aoc, lg)
}

/// A client of output management.
fn output_manager(f: &mut Fixture) -> ClientId {
    let client = f.connect(Kind::Xdg);
    f.client(client).bind_output_management();
    f.roundtrip(client);
    client
}

/// Applies a configuration over output management, as `wlr-randr` and camino-displays do.
fn apply(f: &mut Fixture, client: ClientId, changes: &[HeadChange]) {
    f.client(client).apply_outputs(changes);
    f.roundtrip(client);
}

/// Puts the LG on the left at a scale of 2 and the AOC on its right, and makes the LG the
/// primary output: not what Camino gives the two by default.
fn change_layout(f: &mut Fixture, lg: &Output) {
    let client = output_manager(f);
    let lg_left = HeadChange::of("headless-2").position((0, 0)).scale(2.0);
    let aoc_right = HeadChange::of("headless-1").position((1920, 0));
    apply(f, client, &[lg_left, aoc_right]);
    f.choose_primary_output(lg);
}

/// Asks `org.mate.Camino` to save the layout, and returns why it didn't, if it didn't.
fn save(f: &mut Fixture) -> Result<(), String> {
    let bus = f.start_dbus();
    f.dbus_call(move || {
        let proxy = camino_proxy(&bus).unwrap();
        proxy.save_layout().map_err(|err| err.to_string())
    })
}

/// Camino with the changed layout of the AOC and the LG saved in `dir`, and gone.
fn save_changed_layout(dir: &Path) {
    let (mut f, _aoc, lg) = start_with_aoc_and_lg(dir);
    change_layout(&mut f, &lg);
    save(&mut f).unwrap();
}

#[test]
fn a_layout_saved_for_two_monitors_is_restored_when_camino_starts_with_those_two() {
    let dir = tempfile::tempdir().unwrap();
    save_changed_layout(dir.path());

    let (f, _aoc, _lg) = start_with_aoc_and_lg(dir.path());

    assert_snapshot!(layout(&f), @"
    headless-1 at 1920,0 1920x1080 scale 1.0
    headless-2 at 0,0 1920x1080 scale 2.0
    primary headless-2
    ");
}

#[test]
fn a_different_set_of_monitors_gets_the_defaults() {
    let dir = tempfile::tempdir().unwrap();
    save_changed_layout(dir.path());

    // A laptop's panel where the AOC was. Headless monitors have no EDID serial, so
    // they're known by their connectors.
    let mut f = Fixture::with_saved_layouts(dir.path());
    f.plug_internal((344, 194), (1920, 1080));
    plug(&mut f, LG);

    assert_snapshot!(layout(&f), @"
    eDP-1 at 0,0 1920x1080 scale 1.0
    headless-2 at 1920,0 2259x1271 scale 1.7
    primary eDP-1
    ");
}

#[test]
fn plugging_the_second_monitor_back_restores_the_layout_and_the_primary_output() {
    let dir = tempfile::tempdir().unwrap();
    let (mut f, _aoc, lg) = start_with_aoc_and_lg(dir.path());
    change_layout(&mut f, &lg);
    save(&mut f).unwrap();

    f.unplug(&lg);
    assert_snapshot!(layout(&f), @"
    headless-1 at 0,0 1920x1080 scale 1.0
    primary headless-1
    ");

    f.replug(&lg);
    assert_snapshot!(layout(&f), @"
    headless-1 at 1920,0 1920x1080 scale 1.0
    headless-2 at 0,0 1920x1080 scale 2.0
    primary headless-2
    ");
}

#[test]
fn an_applied_configuration_that_wasnt_saved_is_gone_after_a_restart() {
    let dir = tempfile::tempdir().unwrap();
    let (mut f, _aoc, lg) = start_with_aoc_and_lg(dir.path());
    change_layout(&mut f, &lg);
    drop(f);

    let (f, _aoc, _lg) = start_with_aoc_and_lg(dir.path());

    assert_snapshot!(layout(&f), @"
    headless-1 at 0,0 1920x1080 scale 1.0
    headless-2 at 1920,0 2259x1271 scale 1.7
    primary headless-1
    ");
}

#[test]
fn a_configuration_applied_after_the_layout_was_saved_isnt_in_it() {
    let dir = tempfile::tempdir().unwrap();
    let (mut f, _aoc, lg) = start_with_aoc_and_lg(dir.path());
    change_layout(&mut f, &lg);
    save(&mut f).unwrap();
    let client = output_manager(&mut f);
    apply(&mut f, client, &[HeadChange::of("headless-2").scale(1.5)]);
    drop(f);

    let (f, _aoc, _lg) = start_with_aoc_and_lg(dir.path());

    assert_snapshot!(layout(&f), @"
    headless-1 at 1920,0 1920x1080 scale 1.0
    headless-2 at 0,0 1920x1080 scale 2.0
    primary headless-2
    ");
}

#[test]
fn a_saved_mode_that_the_monitor_no_longer_has_leaves_a_contiguous_layout() {
    let dir = tempfile::tempdir().unwrap();
    let mut f = Fixture::with_saved_layouts(dir.path());
    plug(&mut f, AOC);
    f.plug_with_modes(LG.0, &[LG.1, (1920, 1080)]);
    let client = output_manager(&mut f);
    // The LG in its smaller mode on the left, and the AOC at its right edge.
    let lg_left = HeadChange::of("headless-2")
        .mode((1920, 1080))
        .scale(1.0)
        .position((0, 0));
    let aoc_right = HeadChange::of("headless-1").position((1920, 0));
    apply(&mut f, client, &[lg_left, aoc_right]);
    save(&mut f).unwrap();
    drop(f);

    // The LG lists only its 4K mode now, which is twice the size at the saved scale: the
    // AOC, saved where the LG now is, is moved off it by the shortest way, to above it.
    let (f, _aoc, _lg) = start_with_aoc_and_lg(dir.path());

    assert_snapshot!(layout(&f), @"
    headless-1 at 1920,0 1920x1080 scale 1.0
    headless-2 at 0,1080 3840x2160 scale 1.0
    primary headless-2
    ");
}

#[test]
fn a_saved_mode_that_the_backend_cant_show_leaves_the_rest_of_the_layout_restored() {
    let dir = tempfile::tempdir().unwrap();
    let mut f = Fixture::with_saved_layouts(dir.path());
    f.plug_with_modes(LG.0, &[LG.1, (1920, 1080)]);
    f.plug(AOC.0, AOC.1);
    let client = output_manager(&mut f);
    let lg_smaller = HeadChange::of("headless-1").mode((1920, 1080)).scale(1.0);
    let aoc_scaled = HeadChange::of("headless-2").scale(1.25);
    apply(&mut f, client, &[lg_smaller, aoc_scaled]);
    save(&mut f).unwrap();
    drop(f);

    // The LG is plugged in first, so that its mode is refused before the AOC's plugging
    // in restores the layout.
    let mut f = Fixture::with_saved_layouts(dir.path());
    let lg = f.plug_with_modes(LG.0, &[LG.1, (1920, 1080)]);
    f.refuse_mode(&lg, (1920, 1080));
    f.plug(AOC.0, AOC.1);

    // The LG stays in its 4K mode at the saved scale, and the AOC, at its saved scale and
    // saved where the LG now is, is moved to above it.
    assert_snapshot!(layout(&f), @"
    headless-1 at 0,864 3840x2160 scale 1.0
    headless-2 at 1920,0 1536x864 scale 1.25
    primary headless-1
    ");
}

#[test]
fn a_restored_scale_counts_as_set_so_window_scaling_factor_leaves_it() {
    let dir = tempfile::tempdir().unwrap();
    let (mut f, _aoc, _lg) = start_with_aoc_and_lg(dir.path());
    let client = output_manager(&mut f);
    apply(&mut f, client, &[HeadChange::of("headless-2").scale(1.5)]);
    save(&mut f).unwrap();
    drop(f);
    let (mut f, _aoc, _lg) = start_with_aoc_and_lg(dir.path());

    f.set_setting(INTERFACE, "window-scaling-factor", Value::Int(2));
    // No output changes, so there's only the setting's arrival to wait for.
    let forced = WindowScalingFactor::Forced(2);
    f.dispatch_until(|f| f.state.camino.settings.interface.window_scaling_factor == forced);

    // The AOC's scale was saved too, though it was Camino's default.
    assert_snapshot!(layout(&f), @"
    headless-1 at 0,0 1920x1080 scale 1.0
    headless-2 at 1920,0 2560x1440 scale 1.5
    primary headless-1
    ");
}

#[test]
fn a_disabled_monitor_is_saved_as_disabled() {
    let dir = tempfile::tempdir().unwrap();
    let (mut f, _aoc, _lg) = start_with_aoc_and_lg(dir.path());
    let client = output_manager(&mut f);
    apply(&mut f, client, &[HeadChange::of("headless-1").disable()]);
    save(&mut f).unwrap();
    drop(f);

    let (mut f, _aoc, _lg) = start_with_aoc_and_lg(dir.path());

    assert_snapshot!(layout(&f), @"
    headless-2 at 0,0 2259x1271 scale 1.7
    primary headless-2
    ");
    // The AOC is still there for a client to enable.
    let client = output_manager(&mut f);
    apply(&mut f, client, &[HeadChange::of("headless-1").enable()]);
    assert!(layout(&f).contains("headless-1 at 2259,0"));
}

#[test]
fn each_set_of_monitors_has_its_own_saved_layout_and_saving_again_replaces_it() {
    let dir = tempfile::tempdir().unwrap();
    let (mut f, _aoc, lg) = start_with_aoc_and_lg(dir.path());
    change_layout(&mut f, &lg);
    save(&mut f).unwrap();
    let client = output_manager(&mut f);
    apply(&mut f, client, &[HeadChange::of("headless-2").scale(1.5)]);
    save(&mut f).unwrap();
    f.unplug(&lg);
    apply(&mut f, client, &[HeadChange::of("headless-1").scale(1.25)]);
    save(&mut f).unwrap();
    drop(f);

    let mut f = Fixture::with_saved_layouts(dir.path());
    plug(&mut f, AOC);
    assert_snapshot!(layout(&f), @"
    headless-1 at 0,0 1536x864 scale 1.25
    primary headless-1
    ");

    plug(&mut f, LG);
    assert_snapshot!(layout(&f), @"
    headless-1 at 2560,0 1920x1080 scale 1.0
    headless-2 at 0,0 2560x1440 scale 1.5
    primary headless-2
    ");
}

#[test]
fn a_layout_isnt_saved_where_theres_no_place_to_keep_it() {
    let mut f = Fixture::new();
    plug(&mut f, AOC);

    let refused = save(&mut f).unwrap_err();

    assert!(refused.contains("XDG_CONFIG_HOME"), "{refused}");
}

#[test]
fn a_file_that_cant_be_read_is_warned_of_and_replaced_by_the_next_layout_saved() {
    let dir = tempfile::tempdir().unwrap();
    let file = dir.path().join("layouts.json");
    std::fs::write(&file, "<monitors version=\"2\">").unwrap();
    let logs = Logs::capture();

    let (mut f, _aoc, lg) = start_with_aoc_and_lg(dir.path());
    assert!(
        logs.text().contains("can't restore the saved layouts"),
        "{}",
        logs.text()
    );
    change_layout(&mut f, &lg);
    save(&mut f).unwrap();
    drop(f);

    let (f, _aoc, _lg) = start_with_aoc_and_lg(dir.path());
    assert!(layout(&f).ends_with("primary headless-2"), "{}", layout(&f));
}
