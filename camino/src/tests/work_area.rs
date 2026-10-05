//! The work area (64): each output less its layer surfaces' exclusive zones and XWayland
//! windows' struts, as maximised windows, the debug dump and X clients see it. The tests
//! of a window beside a panel run with an xdg toplevel and an XWayland window (45). Only an
//! X client sets struts or reads the root's work-area properties, so those tests map
//! XWayland windows only.

use camino_test_client::layer_shell::LayerSpec;
use camino_test_client::x11::Names;
use insta::assert_snapshot;
use smithay::utils::{Logical, Rectangle};

use super::fixture::{
    ClientId, ClientWindow, Fixture, KEY_F10, KEY_LEFTALT, Kind, WindowSpec, on_both_kinds,
};

const BAR_COLOR: u32 = 0xff00ff00;

/// Each output's work area from the dump, one per line.
fn work_areas(f: &Fixture) -> String {
    let dump: serde_json::Value = serde_json::from_str(&f.state.camino.state_dump()).unwrap();
    let outputs = dump["outputs"].as_array().unwrap();
    let lines = outputs.iter().map(|output| {
        let area = &output["work_area"];
        format!(
            "{}: {},{} {}x{}",
            output["name"].as_str().unwrap(),
            area["x"],
            area["y"],
            area["width"],
            area["height"],
        )
    });
    lines.collect::<Vec<_>>().join("\n")
}

/// Maximises the focused `window` with Alt+F10 if it isn't maximised yet, has its client
/// draw at the size it's asked for, which is its frame's in a work area 1920 wide and
/// `area_height` high, and returns its frame.
#[track_caller]
fn maximised_frame(
    f: &mut Fixture,
    window: ClientWindow,
    area_height: i32,
) -> Rectangle<i32, Logical> {
    if !f.state.camino.policy.is_maximized(f.pid(window)) {
        f.chord(&[KEY_LEFTALT, KEY_F10]);
    }
    let e = f.frame_extents(window);
    let size = (1920 - e.left - e.right, area_height - e.top - e.bottom);
    f.draw_at_asked_size(window, size);
    f.visible_frame(window)
}

fn rect(x: i32, y: i32, w: i32, h: i32) -> Rectangle<i32, Logical> {
    Rectangle::new((x, y).into(), (w, h).into())
}

/// The root's work-area properties, one per line.
fn root_work_areas(f: &mut Fixture, x: ClientId) -> String {
    f.x11_call(x, move |x| {
        let mut names = Names::new(x);
        let lines = [
            names.property(x, x.root(), "_NET_WORKAREA")?,
            names.property(x, x.root(), "_GTK_WORKAREAS_D0")?,
        ];
        Ok::<_, x11rb::errors::ReplyError>(lines.join("\n"))
    })
    .unwrap()
}

/// A dock that sets `_NET_WM_STRUT_PARTIAL` to `partial`.
fn dock(partial: [u32; 12]) -> WindowSpec {
    WindowSpec {
        types: vec!["_NET_WM_WINDOW_TYPE_DOCK"],
        cardinals: vec![("_NET_WM_STRUT_PARTIAL", partial.to_vec())],
        position: Some((0, 0)),
        ..WindowSpec::default()
    }
}

on_both_kinds! {
    fn a_top_bars_exclusive_zone_shrinks_a_maximised_window_and_moves_it_down(kind: Kind) {
        let (mut f, client) = Fixture::connected(kind);
        let panel = f.connect(Kind::Xdg);
        let window = f.map(client, WindowSpec::default());
        assert_eq!(maximised_frame(&mut f, window, 1080), rect(0, 0, 1920, 1080));

        let bar = f.map_layer(panel, LayerSpec::top_bar(28), BAR_COLOR);

        assert_eq!(maximised_frame(&mut f, window, 1052), rect(0, 28, 1920, 1052));
        assert_eq!(work_areas(&f), "headless-1: 0,28 1920x1052");

        // Unmapping it gives the space back.
        f.client(panel).unmap_layer_surface(bar);
        f.roundtrip(panel);

        assert_eq!(maximised_frame(&mut f, window, 1080), rect(0, 0, 1920, 1080));
        assert_eq!(work_areas(&f), "headless-1: 0,0 1920x1080");
    }
}

on_both_kinds! {
    fn a_zone_on_one_output_leaves_the_others_work_area_whole(kind: Kind) {
        let (mut f, client) = Fixture::connected(kind);
        f.add_output((1280, 1024), 1.0);
        let panel = f.connect(Kind::Xdg);
        f.roundtrip(panel);
        f.client(panel).bind_outputs();
        f.roundtrip(panel);
        let window = f.map(client, WindowSpec::default());
        let whole = maximised_frame(&mut f, window, 1080);

        f.map_layer(
            panel,
            LayerSpec {
                output: Some(1),
                ..LayerSpec::top_bar(28)
            },
            BAR_COLOR,
        );
        f.sync(window);

        assert_eq!(f.visible_frame(window), whole);
        assert_eq!(
            work_areas(&f),
            "headless-1: 0,0 1920x1080\nheadless-2: 1920,28 1280x996"
        );
    }
}

on_both_kinds! {
    fn a_window_under_a_new_panel_is_not_moved_and_a_new_one_is_placed_below_it(kind: Kind) {
        let (mut f, client) = Fixture::connected(kind);
        let panel = f.connect(Kind::Xdg);
        let under = f.map(client, WindowSpec { size: (1800, 900), ..WindowSpec::default() });
        let before = f.visible_frame(under);

        f.map_layer(panel, LayerSpec::top_bar(200), BAR_COLOR);
        f.sync(under);
        let new = f.map(client, WindowSpec { size: (1800, 800), ..WindowSpec::default() });

        assert_eq!(f.visible_frame(under), before);
        assert!(before.loc.y < 200, "the first window is under the panel");
        // Nothing fits beside the first window, so the new one cascades from the work
        // area's corner.
        assert_eq!(f.visible_frame(new).loc, (0, 200).into());
    }
}

#[test]
fn layer_surfaces_without_an_exclusive_zone_reserve_nothing() {
    let (mut f, panel) = Fixture::connected(Kind::Xdg);

    f.map_layer(
        panel,
        LayerSpec {
            exclusive_zone: 0,
            ..LayerSpec::top_bar(28)
        },
        BAR_COLOR,
    );

    assert_snapshot!(work_areas(&f), @"headless-1: 0,0 1920x1080");
}

#[test]
fn a_dock_with_a_partial_strut_reserves_its_edge_until_it_unmaps() {
    let (mut f, x) = Fixture::connected(Kind::X11);
    let window = f.map(x, WindowSpec::default());
    assert_eq!(
        maximised_frame(&mut f, window, 1080),
        rect(0, 0, 1920, 1080)
    );

    let top = f.map(x, dock([0, 0, 30, 0, 0, 0, 0, 0, 0, 1919, 0, 0]));

    assert_snapshot!(work_areas(&f), @"headless-1: 0,30 1920x1050");
    assert_eq!(
        maximised_frame(&mut f, window, 1050),
        rect(0, 30, 1920, 1050)
    );

    // The strut changes: now 40 along the bottom.
    let id = top.x11();
    f.x11_call(x, move |x| {
        x.set_cardinals(
            id,
            "_NET_WM_STRUT_PARTIAL",
            &[0, 0, 0, 40, 0, 0, 0, 0, 0, 0, 0, 1919],
        )
    })
    .unwrap();
    f.dispatch_until(|f| work_areas(f) == "headless-1: 0,0 1920x1040");
    assert_eq!(
        maximised_frame(&mut f, window, 1040),
        rect(0, 0, 1920, 1040)
    );

    f.x11_call(x, move |x| x.unmap(id)).unwrap();
    f.dispatch_until(|f| work_areas(f) == "headless-1: 0,0 1920x1080");
    assert_eq!(
        maximised_frame(&mut f, window, 1080),
        rect(0, 0, 1920, 1080)
    );
}

#[test]
fn a_strut_reserves_on_the_output_its_span_is_on() {
    let mut f = Fixture::new();
    f.add_output((1920, 1080), 1.0);
    f.add_output((1280, 1024), 1.0);
    let x = f.connect(Kind::X11);

    // 50 from the right edge of the X screen, which is the second output's.
    f.map(x, dock([0, 50, 0, 0, 0, 0, 0, 1023, 0, 0, 0, 0]));

    assert_snapshot!(work_areas(&f), @"
    headless-1: 0,0 1920x1080
    headless-2: 1920,0 1230x1024
    ");
}

#[test]
fn a_strut_along_part_of_an_edge_leaves_the_largest_free_rectangle_as_in_marco() {
    let (mut f, x) = Fixture::connected(Kind::X11);

    // 30 along the top of the left half: below it there's more than beside it.
    f.map(x, dock([0, 0, 30, 0, 0, 0, 0, 0, 0, 959, 0, 0]));
    assert_snapshot!(work_areas(&f), @"headless-1: 0,30 1920x1050");

    // 600 along the top of the left quarter: beside it there's more than below it.
    f.map(x, dock([0, 0, 600, 0, 0, 0, 0, 0, 0, 479, 0, 0]));
    assert_snapshot!(work_areas(&f), @"headless-1: 480,30 1440x1050");
}

#[test]
fn a_strut_is_reserved_at_the_x11_scale_as_soon_as_the_scale_changes() {
    let mut f = Fixture::new();
    let output = f.add_output((1920, 1080), 2.0);
    let x = f.connect(Kind::X11);
    // 60 of the X screen's pixels, which are 30 logical ones at the X11 scale of 2.
    let window = f.map(x, dock([0, 0, 60, 0, 0, 0, 0, 0, 0, 1919, 0, 0]));
    assert_snapshot!(work_areas(&f), @"headless-1: 0,30 960x510");

    f.set_output_scale(&output, 1.0);
    f.sync(window);

    // The same 60 pixels are 60 logical ones now.
    assert_snapshot!(work_areas(&f), @"headless-1: 0,60 1920x1020");
    assert_snapshot!(root_work_areas(&mut f, x), @"
    _NET_WORKAREA(CARDINAL) = 0, 60, 1920, 1020, 0, 60, 1920, 1020, 0, 60, 1920, 1020, 0, 60, 1920, 1020
    _GTK_WORKAREAS_D0(CARDINAL) = 0, 60, 1920, 1020
    ");
}

#[test]
fn the_roots_work_areas_are_less_a_layer_surfaces_zone_at_x11_scales_1_and_2() {
    let mut f = Fixture::new();
    let output = f.add_output((1920, 1080), 1.0);
    let x = f.connect(Kind::X11);
    let panel = f.connect(Kind::Xdg);

    f.map_layer(panel, LayerSpec::top_bar(28), BAR_COLOR);

    assert_snapshot!(root_work_areas(&mut f, x), @"
    _NET_WORKAREA(CARDINAL) = 0, 28, 1920, 1052, 0, 28, 1920, 1052, 0, 28, 1920, 1052, 0, 28, 1920, 1052
    _GTK_WORKAREAS_D0(CARDINAL) = 0, 28, 1920, 1052
    ");

    f.set_output_scale(&output, 2.0);
    f.roundtrip(panel);

    // The output is 960 × 540 logical pixels now, and the bar's 28 are 56 in client space.
    assert_snapshot!(root_work_areas(&mut f, x), @"
    _NET_WORKAREA(CARDINAL) = 0, 56, 1920, 1024, 0, 56, 1920, 1024, 0, 56, 1920, 1024, 0, 56, 1920, 1024
    _GTK_WORKAREAS_D0(CARDINAL) = 0, 56, 1920, 1024
    ");
}

#[test]
fn the_roots_work_areas_are_less_a_zone_on_one_of_two_outputs() {
    let mut f = Fixture::new();
    f.add_output((1920, 1080), 1.0);
    f.add_output((1280, 1024), 1.0);
    let x = f.connect(Kind::X11);
    let panel = f.connect(Kind::Xdg);

    f.map_layer(panel, LayerSpec::top_bar(28), BAR_COLOR);

    // `_NET_WORKAREA` is the largest rectangle of the X screen clear of the bar, as in
    // Marco: the whole of the screen below it.
    assert_snapshot!(root_work_areas(&mut f, x), @"
    _NET_WORKAREA(CARDINAL) = 0, 28, 3200, 1052, 0, 28, 3200, 1052, 0, 28, 3200, 1052, 0, 28, 3200, 1052
    _GTK_WORKAREAS_D0(CARDINAL) = 0, 28, 1920, 1052, 1920, 0, 1280, 1024
    ");
}
