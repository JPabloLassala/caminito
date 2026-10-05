//! Outputs coming and going (51): the place and scale Camino gives them, keeping the
//! layout contiguous as they come, go and change size (82), and `window-scaling-factor`,
//! as the debug dump shows them. Plugged outputs are the developer's monitors (50): an AOC at 1.0 and an
//! LG 4K at 1.70.

use camino_test_client::x11::XSetting;
use insta::assert_snapshot;
use smithay::output::Output;

use super::fixture::{Fixture, Kind, WindowSpec, on_both_kinds};
use crate::settings::Value;
use crate::settings::translate::INTERFACE;

/// The AOC G2490W1G4: its EDID's size and preferred mode.
const AOC: ((i32, i32), (i32, i32)) = ((527, 296), (1920, 1080));
/// The LG HDR 4K.
const LG: ((i32, i32), (i32, i32)) = ((600, 340), (3840, 2160));

fn plug(f: &mut Fixture, (mm, mode): ((i32, i32), (i32, i32))) -> Output {
    f.plug(mm, mode)
}

/// Each output's name, place, logical size and scale, one per line, from the dump.
fn layout(f: &Fixture) -> String {
    dump(f)["outputs"]
        .as_array()
        .unwrap()
        .iter()
        .map(|output| {
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
        })
        .collect::<Vec<_>>()
        .join("\n")
}

fn x11_scale(f: &Fixture) -> u64 {
    dump(f)["x11_scale"].as_u64().unwrap()
}

fn dump(f: &Fixture) -> serde_json::Value {
    serde_json::from_str(&f.state.camino.state_dump()).unwrap()
}

#[test]
fn outputs_plugged_in_go_left_to_right_at_their_default_scales() {
    let mut f = Fixture::new();
    plug(&mut f, AOC);
    plug(&mut f, LG);

    assert_snapshot!(layout(&f), @"
    headless-1 at 0,0 1920x1080 scale 1.0
    headless-2 at 1920,0 2259x1271 scale 1.7
    ");
    assert_eq!(x11_scale(&f), 2);
}

on_both_kinds! {
    fn unplugging_the_left_output_moves_its_windows_and_the_other_to_the_origin(kind: Kind) {
        let mut f = Fixture::new();
        let left = plug(&mut f, AOC);
        plug(&mut f, LG);
        let client = f.connect(kind);
        let window = f.map(client, WindowSpec::default());
        assert!(f.geometry(window).loc.x < 1920, "the window is on the left output");

        f.unplug(&left);
        f.sync(window);

        assert_eq!(layout(&f), "headless-2 at 0,0 2259x1271 scale 1.7");
        let geometry = f.geometry(window);
        assert!(
            smithay::utils::Rectangle::new((0, 0).into(), (2259, 1271).into())
                .contains_rect(geometry),
            "the window is on the remaining output: {geometry:?}"
        );
    }
}

/// Output enter and leave are Wayland surface events, and Xwayland owns the surfaces of
/// XWayland windows, so this maps an xdg toplevel only.
#[test]
fn a_window_leaves_an_output_that_is_unplugged_and_enters_the_one_it_moves_to() {
    let mut f = Fixture::new();
    let left = plug(&mut f, AOC);
    plug(&mut f, LG);
    let client = f.connect(Kind::Xdg);
    f.client(client).bind_outputs();
    f.roundtrip(client);
    let window = f.map(client, WindowSpec::default());

    f.unplug(&left);
    f.roundtrip(client);

    assert_snapshot!(f.client(client).window(window.xdg()).output_events.join("\n"), @"
    enter headless-1
    leave headless-1
    enter headless-2
    buffer scale 2
    ");
}

#[test]
fn unplugging_the_middle_output_of_three_closes_the_gap() {
    let mut f = Fixture::new();
    plug(&mut f, AOC);
    let middle = plug(&mut f, LG);
    plug(&mut f, AOC);
    assert_snapshot!(layout(&f), @"
    headless-1 at 0,0 1920x1080 scale 1.0
    headless-2 at 1920,0 2259x1271 scale 1.7
    headless-3 at 4179,0 1920x1080 scale 1.0
    ");

    f.unplug(&middle);

    assert_snapshot!(layout(&f), @"
    headless-1 at 0,0 1920x1080 scale 1.0
    headless-3 at 1920,0 1920x1080 scale 1.0
    ");
}

#[test]
fn a_scale_change_that_grows_an_output_pushes_its_right_hand_neighbour_along() {
    let mut f = Fixture::new();
    let lg = plug(&mut f, LG);
    plug(&mut f, AOC);
    assert_snapshot!(layout(&f), @"
    headless-1 at 0,0 2259x1271 scale 1.7
    headless-2 at 2259,0 1920x1080 scale 1.0
    ");

    f.set_output_scale(&lg, 1.0);

    assert_snapshot!(layout(&f), @"
    headless-1 at 0,0 3840x2160 scale 1.0
    headless-2 at 3840,0 1920x1080 scale 1.0
    ");
}

#[test]
fn an_output_below_one_that_shrinks_stays_under_it() {
    let mut f = Fixture::new();
    let aoc = plug(&mut f, AOC);
    let lg = plug(&mut f, LG);
    f.set_output_position(&lg, (0, 1080));

    f.set_output_scale(&aoc, 2.0);

    assert_snapshot!(layout(&f), @"
    headless-1 at 0,0 960x540 scale 2.0
    headless-2 at 0,540 2259x1271 scale 1.7
    ");
}

#[test]
fn an_output_placed_away_from_the_others_is_docked_to_them() {
    let mut f = Fixture::new();
    plug(&mut f, AOC);
    let right = plug(&mut f, LG);

    f.set_output_position(&right, (2500, 100));

    assert_snapshot!(layout(&f), @"
    headless-1 at 0,0 1920x1080 scale 1.0
    headless-2 at 1920,100 2259x1271 scale 1.7
    ");
}

#[test]
fn an_output_placed_on_top_of_another_is_moved_off_it() {
    let mut f = Fixture::new();
    plug(&mut f, AOC);
    let right = plug(&mut f, LG);

    f.set_output_position(&right, (1000, 100));

    assert_snapshot!(layout(&f), @"
    headless-1 at 0,0 1920x1080 scale 1.0
    headless-2 at 1920,100 2259x1271 scale 1.7
    ");
}

#[test]
fn the_layouts_top_left_corner_is_at_the_origin() {
    let mut f = Fixture::new();
    let left = plug(&mut f, AOC);
    let right = plug(&mut f, LG);

    // Above the left output's top, so the left one moves down.
    f.set_output_position(&right, (1920, -191));
    assert_snapshot!(layout(&f), @"
    headless-1 at 0,191 1920x1080 scale 1.0
    headless-2 at 1920,0 2259x1271 scale 1.7
    ");

    // And alone, an output is at the origin wherever it was.
    f.unplug(&left);
    assert_snapshot!(layout(&f), @"headless-2 at 0,0 2259x1271 scale 1.7");
}

#[test]
fn an_output_plugged_in_again_is_back_at_its_defaults() {
    let mut f = Fixture::new();
    plug(&mut f, AOC);
    let lg = plug(&mut f, LG);
    f.set_output_scale(&lg, 1.5);
    f.set_output_position(&lg, (0, 1080));

    f.unplug(&lg);
    plug(&mut f, LG);

    assert_snapshot!(layout(&f), @"
    headless-1 at 0,0 1920x1080 scale 1.0
    headless-3 at 1920,0 2259x1271 scale 1.7
    ");
}

#[test]
fn a_new_output_goes_right_of_the_rightmost_at_the_layouts_top() {
    let mut f = Fixture::new();
    plug(&mut f, AOC);
    let lg = plug(&mut f, LG);
    f.set_output_position(&lg, (1920, 500));

    plug(&mut f, AOC);

    assert_snapshot!(layout(&f), @"
    headless-1 at 0,0 1920x1080 scale 1.0
    headless-2 at 1920,500 2259x1271 scale 1.7
    headless-3 at 4179,0 1920x1080 scale 1.0
    ");
}

#[test]
fn a_new_output_that_would_touch_no_other_is_docked() {
    let mut f = Fixture::new();
    plug(&mut f, AOC);
    let lg = plug(&mut f, LG);
    f.set_output_position(&lg, (0, 1080));

    // Right of the LG and at the AOC's top, it would only meet the LG's corner. The
    // nearest place is 100 pixels down the LG's side.
    plug(&mut f, AOC);

    assert_snapshot!(layout(&f), @"
    headless-1 at 0,0 1920x1080 scale 1.0
    headless-2 at 0,1080 2259x1271 scale 1.7
    headless-3 at 2259,100 1920x1080 scale 1.0
    ");
}

#[test]
fn window_scaling_factor_rescales_only_outputs_at_the_default_scale() {
    let mut f = Fixture::new();
    plug(&mut f, LG);
    let aoc = plug(&mut f, AOC);
    f.set_output_scale(&aoc, 1.25);
    assert_eq!(x11_scale(&f), 2);

    f.set_setting(INTERFACE, "window-scaling-factor", Value::Int(1));
    f.dispatch_until(|f| layout(f).starts_with("headless-1 at 0,0 3840x2160"));

    // The LG is at 1 now, and the AOC, still at its default place, moves over.
    assert_snapshot!(layout(&f), @"
    headless-1 at 0,0 3840x2160 scale 1.0
    headless-2 at 3840,0 1536x864 scale 1.25
    ");

    f.set_setting(INTERFACE, "window-scaling-factor", Value::Int(0));
    f.dispatch_until(|f| layout(f).starts_with("headless-1 at 0,0 2259x1271"));
    assert_snapshot!(layout(&f), @"
    headless-1 at 0,0 2259x1271 scale 1.7
    headless-2 at 2259,0 1536x864 scale 1.25
    ");
}

#[test]
fn the_x11_scale_follows_window_scaling_factor() {
    let mut f = Fixture::new();
    plug(&mut f, AOC);
    assert_eq!(x11_scale(&f), 1);

    f.set_setting(INTERFACE, "window-scaling-factor", Value::Int(2));
    f.dispatch_until(|f| x11_scale(f) == 2);

    assert_snapshot!(layout(&f), @"headless-1 at 0,0 960x540 scale 2.0");
}

#[test]
fn a_new_mode_gets_the_default_scale_for_its_size() {
    let mut f = Fixture::new();
    let lg = plug(&mut f, LG);

    f.change_mode(&lg, (1920, 1080));

    assert_snapshot!(layout(&f), @"headless-1 at 0,0 1920x1080 scale 1.0");
}

#[test]
fn xsettings_follow_window_scaling_factor() {
    let mut f = Fixture::new();
    plug(&mut f, AOC);
    let x = f.connect(Kind::X11);
    let scaling_factor = |f: &mut Fixture| {
        let table = f
            .x11_call(x, |x| x.xsettings())
            .unwrap()
            .expect("Camino publishes XSETTINGS");
        table
            .into_iter()
            .find(|(name, _)| name == "Gdk/WindowScalingFactor")
            .map(|(_, value)| value)
    };
    assert_eq!(scaling_factor(&mut f), Some(XSetting::Int(1)));

    f.set_setting(INTERFACE, "window-scaling-factor", Value::Int(2));
    f.dispatch_until_mut(|f| scaling_factor(f) == Some(XSetting::Int(2)));
}
