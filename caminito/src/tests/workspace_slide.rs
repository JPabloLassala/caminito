//! The Workspace slide (233 to 236): a change of an output's active workspace shows the old
//! workspace's windows leaving and the new one's arriving, for 250 ms of Caminito's clock,
//! read from the debug dump and from the output's pixels. Policy has switched from the
//! start.

use std::time::Duration;

use caminito_test_client::format_input;
use caminito_test_client::layer_shell::{KeyboardInteractivity, Layer, LayerAnchor, LayerSpec};
use smithay::output::Output;
use smithay::utils::{Logical, Point, Rectangle};

use super::fixture::{
    BTN_LEFT, ClientId, ClientWindow, Fixture, KEY_A, KEY_DOWN, KEY_LEFT, KEY_LEFTALT,
    KEY_LEFTCTRL, KEY_LEFTMETA, KEY_LEFTSHIFT, KEY_RIGHT, KEY_UP, Kind, WINDOW_COLOR, WindowSpec,
    on_both_kinds,
};
use crate::backend::headless::argb8888;
use crate::output::{output_geometry, output_id};
use crate::scene::{CLEAR_COLOR, SceneLayer};
use crate::settings::Value as Setting;
use crate::settings::translate::{CAMINITO, INTERFACE, MARCO_GENERAL, MARCO_GLOBAL_KEYBINDINGS};
use crate::state::SEAT;
use crate::workspace_slide::LENGTH;

/// The window's colour, as `0xRRGGBB`.
const WINDOW: u32 = WINDOW_COLOR & 0xff_ffff;

/// The panel's bar, in a colour that's neither the window's nor the clear colour's.
const BAR_COLOR: u32 = 0xff00_00ff;
const BAR_HEIGHT: i32 = 30;

/// The output, which a window of `BIG` nearly fills.
const OUTPUT: (i32, i32) = (800, 600);

/// A window so wide and so high beside the output that some of it is on the output for
/// the first 110 ms of its way out, whichever way it goes, so a slow run between the
/// switch and the frame doesn't fail the tests that read it part-way.
const BIG: (i32, i32) = (700, 500);

/// How far into the slide the tests read it.
const PART_WAY: Duration = Duration::from_millis(50);

type Settings = Vec<(&'static str, String, Setting)>;

/// One output with `workspace-slide` on, unless `settings` has it, and a client of `kind`.
/// The fixture has the key off, for the tests written when a switch was a cut.
fn desk(kind: Kind, mut settings: Settings) -> (Fixture, ClientId) {
    if !settings.iter().any(|(_, key, _)| key == "workspace-slide") {
        settings.push((CAMINITO, "workspace-slide".into(), Setting::Bool(true)));
    }
    let mut f = Fixture::with_settings(settings);
    f.add_output(OUTPUT, 1.0);
    let id = f.connect(kind);
    (f, id)
}

/// Marco's four workspaces in two rows:
///
/// ```text
/// 0 1
/// 2 3
/// ```
fn two_rows() -> Settings {
    vec![(CAMINITO, "workspace-layout-rows".into(), Setting::Int(2))]
}

/// Maps a window of `size` on `workspace`, which isn't the active one unless it's the
/// first.
fn map_on(f: &mut Fixture, id: ClientId, workspace: usize, size: (i32, i32)) -> ClientWindow {
    let window = f.map(id, WindowSpec::plain(size));
    let pid = f.pid(window);
    f.state.caminito.policy.move_to_workspace(pid, workspace);
    f.dispatch();
    window
}

/// A running slide, as the debug dump has it.
#[derive(Debug)]
struct Sliding {
    output: String,
    from: u64,
    to: u64,
    offset: Point<i32, Logical>,
    /// Each window it moves, by policy's id, with where its frame is drawn.
    windows: Vec<(u64, Rectangle<i32, Logical>)>,
    /// The windows it leaves in their places, by policy's id.
    still: Vec<u64>,
}

impl Sliding {
    /// Where the slide draws `window`'s frame.
    #[track_caller]
    fn drawn(&self, f: &Fixture, window: ClientWindow) -> Rectangle<i32, Logical> {
        let pid = f.pid(window).0;
        let mut windows = self.windows.iter();
        let found = windows.find(|(id, _)| *id == pid);
        found.unwrap_or_else(|| panic!("{self:?} moves {pid}")).1
    }
}

/// The slides that run, from the debug dump's `workspace_slides`, which isn't there while
/// none does.
fn slides(f: &Fixture) -> Vec<Sliding> {
    let dump: serde_json::Value = serde_json::from_str(&f.state.caminito.state_dump()).unwrap();
    let int = |value: &serde_json::Value| value.as_i64().unwrap() as i32;
    let Some(slides) = dump["workspace_slides"].as_array() else {
        return Vec::new();
    };
    let slides = slides.iter().map(|slide| Sliding {
        output: slide["output"].as_str().unwrap().to_owned(),
        from: slide["from"].as_u64().unwrap(),
        to: slide["to"].as_u64().unwrap(),
        offset: (int(&slide["offset"]["x"]), int(&slide["offset"]["y"])).into(),
        windows: slide["windows"]
            .as_array()
            .unwrap()
            .iter()
            .map(|window| {
                let rect = &window["rect"];
                let rect = Rectangle::new(
                    (int(&rect["x"]), int(&rect["y"])).into(),
                    (int(&rect["width"]), int(&rect["height"])).into(),
                );
                (window["id"].as_u64().unwrap(), rect)
            })
            .collect(),
        still: slide["still"]
            .as_array()
            .unwrap()
            .iter()
            .map(|id| id.as_u64().unwrap())
            .collect(),
    });
    slides.collect()
}

/// The one slide that runs.
#[track_caller]
fn slide(f: &Fixture) -> Sliding {
    let mut slides = slides(f);
    assert_eq!(slides.len(), 1, "one slide runs: {slides:?}");
    slides.remove(0)
}

fn active(f: &Fixture) -> usize {
    f.state.caminito.policy.active_workspace()
}

/// The output, from its corner.
fn output() -> Rectangle<i32, Logical> {
    Rectangle::from_size(OUTPUT.into())
}

/// The middle of `rect`.
fn middle(rect: Rectangle<i32, Logical>) -> Point<i32, Logical> {
    rect.loc + Point::from((rect.size.w / 2, rect.size.h / 2))
}

/// The middle of the part of `rect` that's on the output.
#[track_caller]
fn middle_on_output(rect: Rectangle<i32, Logical>) -> Point<i32, Logical> {
    middle_within(rect, output())
}

/// The middle of the part of `rect` that's in `area`.
#[track_caller]
fn middle_within(
    rect: Rectangle<i32, Logical>,
    area: Rectangle<i32, Logical>,
) -> Point<i32, Logical> {
    let within = rect.intersection(area);
    middle(within.unwrap_or_else(|| panic!("some of {rect:?} is in {area:?}")))
}

/// The first output's pixel at the global point `at`, as `0xRRGGBB`.
fn pixel(f: &mut Fixture, at: Point<i32, Logical>) -> u32 {
    let output = f.state.caminito.outputs[0].clone();
    pixel_on(f, &output, at)
}

/// `output`'s pixel at the global point `at`, as `0xRRGGBB`.
fn pixel_on(f: &mut Fixture, output: &Output, at: Point<i32, Logical>) -> u32 {
    let local = at - output_geometry(output).loc;
    let pixel = f
        .state
        .backend
        .headless_mut()
        .pixel(output, local.to_physical(1));
    pixel.expect("the output is drawn") & 0xff_ffff
}

/// Where nothing is drawn, as `0xRRGGBB`.
fn clear() -> u32 {
    argb8888(CLEAR_COLOR) & 0xff_ffff
}

/// `rect`, moved by `by`.
fn moved(rect: Rectangle<i32, Logical>, by: Point<i32, Logical>) -> Rectangle<i32, Logical> {
    Rectangle::new(rect.loc + by, rect.size)
}

/// Maps the panel's bar along the output's bottom, on the top layer.
fn map_bar(f: &mut Fixture) {
    let panel = f.connect(Kind::Xdg);
    f.client(panel).bind_outputs();
    let bar = LayerSpec {
        layer: Layer::Top,
        namespace: "panel",
        output: None,
        anchor: LayerAnchor::Bottom | LayerAnchor::Left | LayerAnchor::Right,
        size: (0, BAR_HEIGHT as u32),
        exclusive_zone: BAR_HEIGHT,
        keyboard: KeyboardInteractivity::None,
    };
    f.map_layer(panel, bar, BAR_COLOR);
}

/// The spot on the bar that's `x` from the output's left.
fn bar_at(x: i32) -> Point<i32, Logical> {
    Point::from((x, OUTPUT.1 - BAR_HEIGHT / 2))
}

on_both_kinds! {
    fn a_switch_each_way_slides_the_way_of_the_grid(kind: Kind) {
        let (mut f, id) = desk(kind, two_rows());
        let windows: Vec<ClientWindow> = (0..4).map(|w| map_on(&mut f, id, w, BIG)).collect();
        // Round the grid, each with the distance between the two workspaces' places.
        let ways = [
            (KEY_RIGHT, 0, 1, (OUTPUT.0, 0)),
            (KEY_DOWN, 1, 3, (0, OUTPUT.1)),
            (KEY_LEFT, 3, 2, (-OUTPUT.0, 0)),
            (KEY_UP, 2, 0, (0, -OUTPUT.1)),
        ];
        for (key, from, to, distance) in ways {
            let distance = Point::<i32, Logical>::from(distance);
            let (old, new) = (windows[from], windows[to]);
            f.chord(&[KEY_LEFTCTRL, KEY_LEFTALT, key]);
            assert_eq!(active(&f), to, "policy has switched at once");

            f.advance_clock(PART_WAY);
            let slide = slide(&f);
            assert_eq!((slide.from, slide.to), (from as u64, to as u64));
            assert_eq!(slide.output, "headless-1");
            // The new workspace's windows are on their way in from where the key points,
            // and the old one's are the whole output ahead of them.
            let offset = slide.offset;
            assert_eq!(offset.x.signum(), distance.x.signum(), "{slide:?}");
            assert_eq!(offset.y.signum(), distance.y.signum(), "{slide:?}");
            assert!(offset.x.abs() < OUTPUT.0 && offset.y.abs() < OUTPUT.1, "{slide:?}");
            let (new_frame, old_frame) = (f.visible_frame(new), f.visible_frame(old));
            assert_eq!(slide.drawn(&f, new), moved(new_frame, offset));
            assert_eq!(slide.drawn(&f, old), moved(old_frame, offset - distance));
            assert_eq!(slide.windows.len(), 2, "{slide:?}");

            f.advance_clock(LENGTH - PART_WAY);
            assert!(slides(&f).is_empty(), "the slide is over");
            assert_eq!(pixel(&mut f, middle(new_frame)), WINDOW);
        }
    }
}

on_both_kinds! {
    fn part_way_both_workspaces_windows_are_drawn_and_the_panel_stays(kind: Kind) {
        let (mut f, id) = desk(kind, two_rows());
        map_bar(&mut f);
        let on_bar = bar_at(OUTPUT.0 / 2);
        let old = map_on(&mut f, id, 0, BIG);
        let new = map_on(&mut f, id, 1, (300, 200));
        let (old_frame, new_frame) = (f.visible_frame(old), f.visible_frame(new));
        // A spot the old workspace's window covers in its place, and the new one's doesn't.
        let old_alone = old_frame.loc + old_frame.size.to_point() - Point::from((50, 50));
        assert!(!new_frame.contains(old_alone));
        f.wait_for_pixel(old_alone.x, old_alone.y, WINDOW);

        f.chord(&[KEY_LEFTCTRL, KEY_LEFTALT, KEY_RIGHT]);
        f.advance_clock(PART_WAY);
        let slide = slide(&f);
        let leaving = middle_on_output(slide.drawn(&f, old));
        let arriving = middle_on_output(slide.drawn(&f, new));
        assert_eq!(pixel(&mut f, leaving), WINDOW, "the old one is on its way out");
        assert_eq!(pixel(&mut f, arriving), WINDOW, "the new one is on its way in");
        assert_eq!(pixel(&mut f, on_bar), BAR_COLOR & 0xff_ffff);

        f.advance_clock(LENGTH - PART_WAY);
        assert_eq!(pixel(&mut f, middle(new_frame)), WINDOW, "the new one is in its place");
        assert_eq!(pixel(&mut f, old_alone), clear(), "the old one is gone");
        assert_eq!(pixel(&mut f, on_bar), BAR_COLOR & 0xff_ffff);
    }
}

// From here on the slide's windows are a Wayland client's: the slide is the Scene's, which
// draws both kinds of window the same way, as the two tests above show.

/// Six workspaces in two rows, with `switch-to-workspace-6` on Super+A:
///
/// ```text
/// 0 1 2
/// 3 4 5
/// ```
fn three_columns() -> Settings {
    let mut settings = two_rows();
    settings.push((MARCO_GENERAL, "num-workspaces".into(), Setting::Int(6)));
    settings.push((
        MARCO_GLOBAL_KEYBINDINGS,
        "switch-to-workspace-6".into(),
        Setting::Str("<Super>a".into()),
    ));
    settings
}

#[test]
fn a_jump_is_one_straight_line_with_no_workspace_between() {
    let (mut f, id) = desk(Kind::Xdg, three_columns());
    let old = map_on(&mut f, id, 0, BIG);
    let between = map_on(&mut f, id, 1, BIG);
    let new = map_on(&mut f, id, 5, BIG);

    f.chord(&[KEY_LEFTMETA, KEY_A]);
    assert_eq!(active(&f), 5);
    f.advance_clock(PART_WAY);

    let slide = slide(&f);
    assert_eq!((slide.from, slide.to), (0, 5));
    // Two columns and a row away: twice the output's width for each of its height.
    let offset = slide.offset;
    assert!(0 < offset.y && offset.y < OUTPUT.1, "{slide:?}");
    let on_the_line = 2 * OUTPUT.0 * offset.y / OUTPUT.1;
    assert!((offset.x - on_the_line).abs() <= 3, "{slide:?}");
    let distance = Point::from((2 * OUTPUT.0, OUTPUT.1));
    assert_eq!(slide.drawn(&f, new), moved(f.visible_frame(new), offset));
    assert_eq!(
        slide.drawn(&f, old),
        moved(f.visible_frame(old), offset - distance)
    );
    let between = f.pid(between).0;
    assert!(
        slide.windows.iter().all(|(id, _)| *id != between),
        "{slide:?}"
    );

    f.advance_clock(LENGTH - PART_WAY);
    assert!(slides(&f).is_empty());
}

#[test]
fn a_wrap_slides_one_cell_the_way_of_the_key() {
    let mut settings = two_rows();
    settings.push((
        MARCO_GENERAL,
        "wrap-style".into(),
        Setting::Str("toroidal".into()),
    ));
    let (mut f, id) = desk(Kind::Xdg, settings);
    let old = map_on(&mut f, id, 0, BIG);
    let new = map_on(&mut f, id, 1, BIG);

    // Left of the first workspace is the end of its row, which is to its right in the grid.
    f.chord(&[KEY_LEFTCTRL, KEY_LEFTALT, KEY_LEFT]);
    assert_eq!(active(&f), 1);
    f.advance_clock(PART_WAY);

    let slide = slide(&f);
    assert_eq!((slide.from, slide.to), (0, 1));
    let offset = slide.offset;
    assert!(-OUTPUT.0 < offset.x && offset.x < 0, "{slide:?}");
    assert_eq!(offset.y, 0);
    let distance = Point::from((-OUTPUT.0, 0));
    assert_eq!(slide.drawn(&f, new), moved(f.visible_frame(new), offset));
    assert_eq!(
        slide.drawn(&f, old),
        moved(f.visible_frame(old), offset - distance)
    );
}

#[test]
fn a_pagers_switch_slides() {
    let (mut f, id) = desk(Kind::Xdg, two_rows());
    map_on(&mut f, id, 0, BIG);
    let pager = f.connect(Kind::Xdg);
    f.client(pager).bind_outputs();
    f.client(pager).bind_workspaces();
    f.roundtrip(pager);

    f.client(pager).activate_workspace(2);
    f.client(pager).commit_workspaces();
    f.roundtrip(pager);
    assert_eq!(active(&f), 2);
    f.advance_clock(PART_WAY);

    let slide = slide(&f);
    assert_eq!((slide.from, slide.to), (0, 2));
    assert!(0 < slide.offset.y && slide.offset.y < OUTPUT.1, "{slide:?}");
}

#[test]
fn activating_a_window_that_isnt_showing_slides() {
    let (mut f, id) = desk(Kind::Xdg, two_rows());
    map_on(&mut f, id, 0, BIG);
    let spec = WindowSpec {
        title: Some("Pluma"),
        ..WindowSpec::plain(BIG)
    };
    let pluma = f.map(id, spec);
    let pid = f.pid(pluma);
    f.state.caminito.policy.move_to_workspace(pid, 1);
    let panel = f.connect(Kind::Xdg);
    f.client(panel).bind_foreign_toplevels();
    f.roundtrip(panel);
    let index = f.client(panel).foreign_toplevels().titled("Pluma");

    f.client(panel).activate_toplevel(index);
    f.roundtrip(panel);
    assert_eq!(active(&f), 1);
    assert_eq!(f.state.caminito.policy.focus(SEAT), Some(pid));
    f.advance_clock(PART_WAY);

    let slide = slide(&f);
    assert_eq!((slide.from, slide.to), (0, 1));
    assert_eq!(
        slide.drawn(&f, pluma),
        moved(f.visible_frame(pluma), slide.offset)
    );
}

// What a client is sent is read from a Wayland client, as `input.rs` reads it.
#[test]
fn keys_and_clicks_part_way_reach_the_new_workspaces_window_at_its_own_place() {
    let (mut f, id) = desk(Kind::Xdg, two_rows());
    f.client(id).bind_input();
    let old = map_on(&mut f, id, 0, BIG);
    let new = map_on(&mut f, id, 1, BIG);
    // The client's second window.
    let surface = "window 1";
    f.pointer_move(f64::from(OUTPUT.0 - 1), f64::from(OUTPUT.1 - 1));
    f.roundtrip(id);
    f.client(id).state.input.clear();

    f.chord(&[KEY_LEFTCTRL, KEY_LEFTALT, KEY_RIGHT]);
    f.advance_clock(PART_WAY);
    assert!(f.has_keyboard(new));
    f.key(KEY_A, true);
    f.key(KEY_A, false);
    f.roundtrip(id);
    let input = format_input(&f.client(id).state.input);
    assert!(
        input.contains(&format!("keyboard enters {surface}")),
        "{input}"
    );
    assert!(input.contains(&format!("key {KEY_A} pressed")), "{input}");

    // A spot in the new window's own place, where the slide still draws the old one.
    let spot = f.geometry(new).loc + Point::from((20, 100));
    let slide = slide(&f);
    assert!(!slide.drawn(&f, new).contains(spot), "{slide:?}");
    assert!(slide.drawn(&f, old).contains(spot), "{slide:?}");
    f.client(id).state.input.clear();
    f.pointer_move(f64::from(spot.x), f64::from(spot.y));
    f.pointer_button(BTN_LEFT, true);
    f.pointer_button(BTN_LEFT, false);
    f.roundtrip(id);
    let input = format_input(&f.client(id).state.input);
    assert!(
        input.contains(&format!("pointer enters {surface} at 20, 100")),
        "{input}"
    );
    assert!(
        input.contains(&format!("button {BTN_LEFT:#x} pressed")),
        "{input}"
    );
}

/// Switches to the workspace on the right and checks that the frame after is the new
/// workspace at rest.
fn assert_a_cut(mut f: Fixture, id: ClientId) {
    let old = map_on(&mut f, id, 0, BIG);
    let new = map_on(&mut f, id, 1, (300, 200));
    let (old_frame, new_frame) = (f.visible_frame(old), f.visible_frame(new));
    let old_alone = old_frame.loc + old_frame.size.to_point() - Point::from((50, 50));
    f.wait_for_pixel(old_alone.x, old_alone.y, WINDOW);

    f.chord(&[KEY_LEFTCTRL, KEY_LEFTALT, KEY_RIGHT]);

    assert_eq!(active(&f), 1);
    assert!(slides(&f).is_empty(), "nothing slides");
    assert_eq!(
        pixel(&mut f, old_alone),
        clear(),
        "the old one is gone at once"
    );
    assert_eq!(pixel(&mut f, middle(new_frame)), WINDOW);
}

#[test]
fn with_its_key_or_one_of_the_three_others_against_it_a_switch_is_a_cut() {
    for (schema, key, value) in [
        (CAMINITO, "workspace-slide", false),
        (MARCO_GENERAL, "compositing-manager", false),
        (MARCO_GENERAL, "reduced-resources", true),
        (INTERFACE, "enable-animations", false),
    ] {
        let mut settings = two_rows();
        settings.push((schema, key.into(), Setting::Bool(value)));
        let (f, id) = desk(Kind::Xdg, settings);

        assert_a_cut(f, id);
    }
}

#[test]
fn the_key_turned_off_makes_the_next_switch_a_cut() {
    let (mut f, id) = desk(Kind::Xdg, two_rows());
    f.set_setting(CAMINITO, "workspace-slide", Setting::Bool(false));
    f.dispatch_until(|f| !f.state.caminito.settings.workspace_slide);

    assert_a_cut(f, id);
}

#[test]
fn the_workspace_popup_is_drawn_over_the_sliding_windows() {
    let (mut f, id) = desk(Kind::Xdg, two_rows());
    map_on(&mut f, id, 0, BIG);
    map_on(&mut f, id, 1, BIG);

    // The binding's modifiers stay down, which holds the popup open.
    f.key(KEY_LEFTCTRL, true);
    f.key(KEY_LEFTALT, true);
    f.key(KEY_RIGHT, true);
    f.key(KEY_RIGHT, false);
    f.advance_clock(PART_WAY);

    assert_eq!(slide(&f).windows.len(), 2);
    let output = f.state.caminito.outputs[0].clone();
    let layers = f.state.scene_layers(&output);
    let first = |layer| layers.iter().position(|l| *l == layer);
    let popup = first(SceneLayer::WorkspacePopup).expect("the popup is drawn");
    let windows = first(SceneLayer::Windows).expect("the windows are drawn");
    assert!(popup < windows, "{layers:?}");
}

// A second switch part-way retargets the slide (235). The clock is stopped, so the dump
// before a switch and the one after it are read at the same time.

/// Checks that `after`, read straight after a switch, draws each window that `before`
/// moved where `before` drew it.
#[track_caller]
fn assert_no_jump(before: &Sliding, after: &Sliding) {
    for window in &before.windows {
        assert!(
            after.windows.contains(window),
            "{window:?} is where it was: {after:?}"
        );
    }
}

/// Checks that each window `slide` draws on the output is in the output's pixels, and
/// that there are `count` of them at the least.
#[track_caller]
fn assert_drawn_on_the_output(f: &mut Fixture, slide: &Sliding, count: usize) {
    let on_output = slide
        .windows
        .iter()
        .filter_map(|(_, rect)| rect.intersection(output()));
    let on_output: Vec<_> = on_output.collect();
    assert!(on_output.len() >= count, "{slide:?}");
    for rect in on_output {
        assert_eq!(pixel(f, middle(rect)), WINDOW, "{rect:?} of {slide:?}");
    }
}

#[test]
fn three_switches_each_part_way_through_the_last_are_one_movement() {
    // Marco's four workspaces in one row.
    let (mut f, id) = desk(Kind::Xdg, Vec::new());
    let windows: Vec<ClientWindow> = (0..4).map(|w| map_on(&mut f, id, w, BIG)).collect();
    f.stop_clock();

    let mut before: Option<Sliding> = None;
    for to in 1..=3 {
        f.chord(&[KEY_LEFTCTRL, KEY_LEFTALT, KEY_RIGHT]);
        assert_eq!(active(&f), to);
        let after = slide(&f);
        assert_eq!((after.from, after.to), (to as u64 - 1, to as u64));
        // Each workspace so far is at its cell's place, a whole output from the next.
        assert_eq!(after.windows.len(), to + 1, "{after:?}");
        for (workspace, &window) in windows[..=to].iter().enumerate() {
            let cells = workspace as i32 - to as i32;
            let place = after.offset + Point::from((cells * OUTPUT.0, 0));
            assert_eq!(
                after.drawn(&f, window),
                moved(f.visible_frame(window), place),
                "workspace {workspace} of {after:?}"
            );
        }
        if let Some(before) = &before {
            assert_no_jump(before, &after);
        }

        f.advance_clock(PART_WAY);
        let part_way = slide(&f);
        assert!(
            0 < part_way.offset.x && part_way.offset.x < after.offset.x,
            "{part_way:?} is nearer than {after:?}"
        );
        // The workspace that leaves and the one before it, or the one that arrives.
        assert_drawn_on_the_output(&mut f, &part_way, 2);
        before = Some(part_way);
    }

    f.advance_clock(LENGTH);
    assert!(slides(&f).is_empty(), "the slide is over");
    let last = f.visible_frame(windows[3]);
    assert_eq!(pixel(&mut f, middle(last)), WINDOW);
}

#[test]
fn a_workspace_passed_over_is_drawn_as_it_goes_by() {
    let (mut f, id) = desk(Kind::Xdg, Vec::new());
    let windows: Vec<ClientWindow> = (0..3).map(|w| map_on(&mut f, id, w, BIG)).collect();
    f.stop_clock();
    // So soon that the first workspace is still on the output after the second switch.
    let soon = Duration::from_millis(20);

    f.chord(&[KEY_LEFTCTRL, KEY_LEFTALT, KEY_RIGHT]);
    f.advance_clock(soon);
    f.chord(&[KEY_LEFTCTRL, KEY_LEFTALT, KEY_RIGHT]);
    f.advance_clock(soon);

    let slide = slide(&f);
    assert_eq!((slide.from, slide.to), (1, 2));
    let (passed, left) = (slide.drawn(&f, windows[0]), slide.drawn(&f, windows[1]));
    // A spot of the first workspace's window that the second one's, on its right, isn't
    // over.
    let spot = middle_on_output(passed);
    assert!(!left.contains(spot), "{slide:?}");
    assert_eq!(pixel(&mut f, spot), WINDOW, "{slide:?}");
}

#[test]
fn a_second_switch_another_way_bends_the_movement() {
    let (mut f, id) = desk(Kind::Xdg, two_rows());
    let first = map_on(&mut f, id, 0, BIG);
    let second = map_on(&mut f, id, 1, BIG);
    let last = map_on(&mut f, id, 3, BIG);
    f.stop_clock();

    f.chord(&[KEY_LEFTCTRL, KEY_LEFTALT, KEY_RIGHT]);
    f.advance_clock(PART_WAY);
    let right = slide(&f);
    f.chord(&[KEY_LEFTCTRL, KEY_LEFTALT, KEY_DOWN]);

    let down = slide(&f);
    assert_eq!((down.from, down.to), (1, 3));
    assert_no_jump(&right, &down);
    assert_eq!(down.offset, right.offset + Point::from((0, OUTPUT.1)));
    // The first workspace is up and to the left of the last, and the second is above it.
    for (window, place) in [
        (first, (-OUTPUT.0, -OUTPUT.1)),
        (second, (0, -OUTPUT.1)),
        (last, (0, 0)),
    ] {
        assert_eq!(
            down.drawn(&f, window),
            moved(f.visible_frame(window), down.offset + Point::from(place))
        );
    }

    f.advance_clock(LENGTH);
    assert!(slides(&f).is_empty());
}

#[test]
fn a_switch_back_part_way_returns_the_windows_from_where_they_are() {
    let (mut f, id) = desk(Kind::Xdg, Vec::new());
    let first = map_on(&mut f, id, 0, SMALL);
    let second = map_on(&mut f, id, 1, BIG);
    let (first_frame, second_frame) = (f.visible_frame(first), f.visible_frame(second));
    // A spot the second workspace's window covers in its place, and the first one's doesn't.
    let second_alone = second_frame.loc + second_frame.size.to_point() - Point::from((50, 50));
    assert!(!first_frame.contains(second_alone));
    f.stop_clock();

    f.chord(&[KEY_LEFTCTRL, KEY_LEFTALT, KEY_RIGHT]);
    f.advance_clock(PART_WAY);
    let out = slide(&f);
    f.chord(&[KEY_LEFTCTRL, KEY_LEFTALT, KEY_LEFT]);
    assert_eq!(active(&f), 0);

    let back = slide(&f);
    assert_eq!((back.from, back.to), (1, 0));
    assert_no_jump(&out, &back);
    assert_eq!(back.windows.len(), 2, "{back:?}");
    // The first workspace's window comes back from the left, where it had got to.
    assert!(-OUTPUT.0 < back.offset.x && back.offset.x < 0, "{back:?}");
    assert_eq!(back.drawn(&f, first), moved(first_frame, back.offset));

    f.advance_clock(PART_WAY);
    let nearer = slide(&f);
    assert!(
        back.offset.x < nearer.offset.x && nearer.offset.x < 0,
        "{nearer:?} is nearer than {back:?}"
    );

    f.advance_clock(LENGTH);
    assert!(slides(&f).is_empty());
    assert_eq!(pixel(&mut f, middle(first_frame)), WINDOW);
    assert_eq!(pixel(&mut f, second_alone), clear(), "the second is gone");
}

#[test]
fn the_session_lock_ends_a_slide_at_once() {
    let (mut f, id) = desk(Kind::Xdg, Vec::new());
    let old = map_on(&mut f, id, 0, BIG);
    let new = map_on(&mut f, id, 1, SMALL);
    let (old_frame, new_frame) = (f.visible_frame(old), f.visible_frame(new));
    let old_alone = old_frame.loc + old_frame.size.to_point() - Point::from((50, 50));
    assert!(!new_frame.contains(old_alone));
    let locker = f.connect(Kind::Xdg);
    // The slide doesn't end for the time that passes.
    f.stop_clock();
    f.chord(&[KEY_LEFTCTRL, KEY_LEFTALT, KEY_RIGHT]);
    f.advance_clock(PART_WAY);
    assert_eq!(slides(&f).len(), 1);

    let lock = f.client(locker).lock_session();
    f.roundtrip(locker);
    assert_ne!(f.lock_state(), "unlocked");
    assert!(slides(&f).is_empty(), "the lock ended it");

    f.client(locker).unlock_and_destroy(lock);
    f.roundtrip(locker);
    assert_eq!(f.lock_state(), "unlocked");
    assert!(slides(&f).is_empty());
    let at = middle(new_frame);
    f.wait_for_pixel(at.x, at.y, WINDOW);
    assert_eq!(pixel(&mut f, old_alone), clear(), "the old one is gone");
}

#[test]
fn the_key_turned_off_part_way_lets_the_running_slide_finish() {
    let (mut f, id) = desk(Kind::Xdg, Vec::new());
    map_on(&mut f, id, 0, BIG);
    map_on(&mut f, id, 1, BIG);
    f.stop_clock();
    f.chord(&[KEY_LEFTCTRL, KEY_LEFTALT, KEY_RIGHT]);
    f.advance_clock(PART_WAY);
    let before = slide(&f);

    f.set_setting(CAMINITO, "workspace-slide", Setting::Bool(false));
    f.dispatch_until(|f| !f.state.caminito.settings.workspace_slide);

    let after = slide(&f);
    assert_eq!(after.windows, before.windows);
    f.advance_clock(LENGTH - PART_WAY);
    assert!(slides(&f).is_empty());
}

#[test]
fn a_slide_ends_with_its_output() {
    let (mut f, id) = desk(Kind::Xdg, Vec::new());
    map_on(&mut f, id, 0, BIG);
    map_on(&mut f, id, 1, BIG);
    let output = f.state.caminito.outputs[0].clone();
    f.stop_clock();
    f.chord(&[KEY_LEFTCTRL, KEY_LEFTALT, KEY_RIGHT]);
    f.advance_clock(PART_WAY);
    assert_eq!(slides(&f).len(), 1);

    f.unplug(&output);
    f.dispatch();

    // The dump has a slide by its output, so with none it has no slide to show.
    assert_eq!(f.state.caminito.workspace_slides.deadline(), None);
}

// The windows that are the same on both workspaces stay still (234).

/// A window so small that one dragged `RIGHT` is clear of the band the others slide down
/// in.
const SMALL: (i32, i32) = (300, 200);

/// How far `drag_right` takes a window.
const RIGHT: i32 = 400;

/// Presses the left button inside `window`, starts its move as its client asks for one,
/// and moves the pointer `RIGHT` with the button still down.
#[track_caller]
fn drag_right(f: &mut Fixture, window: ClientWindow) {
    let id = window.client();
    let press = (f.geometry(window).loc + Point::from((50, 10))).to_f64();
    f.pointer_move(press.x, press.y);
    f.pointer_button(BTN_LEFT, true);
    f.roundtrip(id);
    let serial = f.client(id).state.last_press_serial;
    f.client(id)
        .start_move(window.xdg(), serial.expect("the client saw the press"));
    f.roundtrip(id);
    f.pointer_move(press.x + f64::from(RIGHT), press.y);
    assert_eq!(f.state.caminito.policy.grab(SEAT), Some(f.pid(window)));
}

/// Checks that the slide moves `old` out and `new` in, the workspace below's, and leaves
/// `still` alone in its place, where it's drawn.
#[track_caller]
fn assert_only_stays(f: &mut Fixture, still: ClientWindow, old: ClientWindow, new: ClientWindow) {
    let slide = slide(f);
    let distance = Point::from((0, OUTPUT.1));
    let offset = slide.offset;
    assert!(0 < offset.y && offset.y < OUTPUT.1, "{slide:?}");
    assert_eq!(slide.still, [f.pid(still).0], "{slide:?}");
    assert_eq!(slide.windows.len(), 2, "{slide:?}");
    assert_eq!(slide.drawn(f, new), moved(f.visible_frame(new), offset));
    assert_eq!(
        slide.drawn(f, old),
        moved(f.visible_frame(old), offset - distance)
    );
    // It's to the right of the band the two others slide in.
    let frame = f.visible_frame(still);
    let band = f.visible_frame(old).merge(f.visible_frame(new));
    assert!(frame.loc.x > band.loc.x + band.size.w + 40, "{frame:?}");
    assert_eq!(pixel(f, middle(frame)), WINDOW, "it's drawn in its place");
    let beside = Point::from((frame.loc.x - 20, frame.loc.y + frame.size.h / 2));
    assert_eq!(pixel(f, beside), clear(), "and nowhere else");
}

#[test]
fn a_sticky_window_stays_while_the_others_slide() {
    let (mut f, id) = desk(Kind::Xdg, two_rows());
    f.client(id).bind_input();
    let old = map_on(&mut f, id, 0, SMALL);
    let new = map_on(&mut f, id, 2, SMALL);
    let sticky = f.map(id, WindowSpec::plain(SMALL));
    drag_right(&mut f, sticky);
    f.pointer_button(BTN_LEFT, false);
    let pid = f.pid(sticky);
    f.state.caminito.policy.set_sticky(pid, true);
    f.dispatch();
    let frame = f.visible_frame(sticky);

    f.chord(&[KEY_LEFTCTRL, KEY_LEFTALT, KEY_DOWN]);
    f.advance_clock(PART_WAY);

    assert_eq!(f.visible_frame(sticky), frame);
    assert_only_stays(&mut f, sticky, old, new);
}

#[test]
fn the_window_a_binding_carries_stays_while_the_others_slide() {
    let (mut f, id) = desk(Kind::Xdg, two_rows());
    f.client(id).bind_input();
    let old = map_on(&mut f, id, 0, SMALL);
    let new = map_on(&mut f, id, 2, SMALL);
    let carried = f.map(id, WindowSpec::plain(SMALL));
    drag_right(&mut f, carried);
    f.pointer_button(BTN_LEFT, false);
    let pid = f.pid(carried);
    assert_eq!(f.state.caminito.policy.focus(SEAT), Some(pid));
    let frame = f.visible_frame(carried);

    // `move-to-workspace-down`.
    f.chord(&[KEY_LEFTCTRL, KEY_LEFTSHIFT, KEY_LEFTALT, KEY_DOWN]);
    assert_eq!(active(&f), 2);
    assert_eq!(f.state.caminito.policy.workspace_of(pid), Some(2));
    f.advance_clock(PART_WAY);

    assert_eq!(f.visible_frame(carried), frame);
    assert_only_stays(&mut f, carried, old, new);
}

#[test]
fn a_window_being_dragged_stays_under_the_pointer_while_the_others_slide() {
    let (mut f, id) = desk(Kind::Xdg, two_rows());
    f.client(id).bind_input();
    let old = map_on(&mut f, id, 0, SMALL);
    let new = map_on(&mut f, id, 2, SMALL);
    let dragged = f.map(id, WindowSpec::plain(SMALL));
    drag_right(&mut f, dragged);
    let pid = f.pid(dragged);
    let frame = f.visible_frame(dragged);

    f.chord(&[KEY_LEFTCTRL, KEY_LEFTALT, KEY_DOWN]);
    assert_eq!(active(&f), 2);
    assert_eq!(f.state.caminito.policy.workspace_of(pid), Some(2));
    f.advance_clock(PART_WAY);

    assert_eq!(f.visible_frame(dragged), frame);
    assert_only_stays(&mut f, dragged, old, new);
    // The drag goes on, part-way through the slide.
    assert_eq!(f.state.caminito.policy.grab(SEAT), Some(pid));
    f.pointer_move_by(-40., 20.);
    assert_eq!(f.visible_frame(dragged), moved(frame, (-40, 20).into()));
    f.pointer_button(BTN_LEFT, false);
    assert_eq!(f.state.caminito.policy.grab(SEAT), None);
    assert_eq!(f.visible_frame(dragged), moved(frame, (-40, 20).into()));
}

#[test]
fn a_fullscreen_window_slides_and_the_panel_shows() {
    let (mut f, id) = desk(Kind::Xdg, two_rows());
    map_bar(&mut f);
    let full = f.map(id, WindowSpec::plain(SMALL));
    let pid = f.pid(full);
    f.client(id).set_fullscreen(full.xdg(), None);
    f.dispatch_until(|f| f.state.caminito.policy.is_fullscreen(pid));
    f.draw_at_asked_size(full, OUTPUT);
    let frame = f.visible_frame(full);
    assert_eq!(frame, output());
    // Focused, it covers the panel.
    let (left, right) = (bar_at(OUTPUT.0 / 4), bar_at(OUTPUT.0 - 5));
    f.wait_for_pixel(left.x, left.y, WINDOW);

    // To the empty workspace on the right: it leaves below the panel, as a fullscreen
    // window that isn't focused is.
    f.chord(&[KEY_LEFTCTRL, KEY_LEFTALT, KEY_RIGHT]);
    f.advance_clock(PART_WAY);
    let out = slide(&f);
    let distance = Point::from((OUTPUT.0, 0));
    assert!(0 < out.offset.x && out.offset.x < OUTPUT.0, "{out:?}");
    assert_eq!(out.drawn(&f, full), moved(frame, out.offset - distance));
    assert!(out.still.is_empty(), "{out:?}");
    let above_bar = Point::from((10, OUTPUT.1 / 2));
    assert_eq!(pixel(&mut f, above_bar), WINDOW, "it's on its way out");
    assert_eq!(pixel(&mut f, left), BAR_COLOR & 0xff_ffff);
    assert_eq!(pixel(&mut f, right), BAR_COLOR & 0xff_ffff);
    f.advance_clock(LENGTH - PART_WAY);
    assert_eq!(pixel(&mut f, above_bar), clear());

    // And back: focused again, it comes in over the panel, which shows where it isn't yet.
    f.chord(&[KEY_LEFTCTRL, KEY_LEFTALT, KEY_LEFT]);
    f.advance_clock(PART_WAY);
    let back = slide(&f);
    assert!(-OUTPUT.0 < back.offset.x && back.offset.x < 0, "{back:?}");
    assert_eq!(back.drawn(&f, full), moved(frame, back.offset));
    assert_eq!(pixel(&mut f, left), WINDOW, "it's on its way in");
    assert_eq!(pixel(&mut f, right), BAR_COLOR & 0xff_ffff);
    f.advance_clock(LENGTH - PART_WAY);
    assert_eq!(pixel(&mut f, right), WINDOW);
}

// Two outputs (236): the left one of `OUTPUT`, and one of another size on its right.

/// The output on the right, wider and higher than the first.
const WIDE: (i32, i32) = (1024, 768);

/// A window that slides in a band along the top of an output, clear of what's put below
/// it, and so wide that some of it is on the other output part-way in from there.
const LONG: (i32, i32) = (600, 200);

/// Where a `LONG` window is put on the left output and on the right one.
const TOP_LEFT: (i32, i32) = (50, 50);
const TOP_RIGHT: (i32, i32) = (OUTPUT.0 + 100, 50);

/// Where a `SMALL` window is across both outputs below that band, a third of it on the
/// left one, so the right one is its Main output.
const ACROSS: (i32, i32) = (OUTPUT.0 - 100, 380);

/// Where a `SMALL` window is on the right output alone, below the band and clear of
/// `ACROSS`.
const LOW_RIGHT: (i32, i32) = (OUTPUT.0 + 600, 500);

/// Two outputs side by side, the left one the primary, with `workspaces-per-output` as
/// `per_output` says, `workspace-slide` on and a Wayland client.
fn two_outputs(per_output: bool, mut settings: Settings) -> (Fixture, ClientId, Output, Output) {
    let key = "workspaces-per-output".into();
    settings.push((CAMINITO, key, Setting::Bool(per_output)));
    let (mut f, id) = desk(Kind::Xdg, settings);
    let left = f.state.caminito.outputs[0].clone();
    let right = f.add_output(WIDE, 1.0);
    f.choose_primary_output(&left);
    assert_eq!(output_geometry(&right).loc, (OUTPUT.0, 0).into());
    (f, id, left, right)
}

/// Maps a window of `size`, drags it so that its geometry is at the global point `at`,
/// and puts it on `workspace`. A drag can leave it across two outputs.
fn put_at(
    f: &mut Fixture,
    id: ClientId,
    at: (i32, i32),
    workspace: usize,
    size: (i32, i32),
) -> ClientWindow {
    let window = f.map(id, WindowSpec::plain(size));
    let pid = f.pid(window);
    let from = f.geometry(window).loc.to_f64();
    let to = Point::<i32, Logical>::from(at).to_f64();
    let policy = &mut f.state.caminito.policy;
    assert!(policy.begin_move(SEAT, pid, from));
    policy.grab_motion(SEAT, to, false);
    policy.end_grab(SEAT);
    policy.move_to_workspace(pid, workspace);
    f.dispatch();
    assert_eq!(f.geometry(window).loc, at.into(), "it's where it was put");
    window
}

/// Switches `output` alone to `workspace`, as the debug interface does.
#[track_caller]
fn switch_output(f: &mut Fixture, output: &Output, workspace: usize) {
    let caminito = &mut f.state.caminito;
    assert!(caminito.switch_output_workspace(&output.name(), workspace));
    f.dispatch();
}

/// Each output's active workspace.
fn active_on(f: &Fixture, outputs: [&Output; 2]) -> [usize; 2] {
    let policy = &f.state.caminito.policy;
    outputs.map(|output| policy.active_workspace_on(output_id(output)))
}

/// Checks that nothing slides, and that nothing waits for a slide to end.
#[track_caller]
fn assert_no_slide(f: &Fixture) {
    assert!(slides(f).is_empty(), "nothing slides: {:?}", slides(f));
    assert_eq!(f.state.caminito.workspace_slides.deadline(), None);
}

#[test]
fn a_switch_of_one_output_slides_it_alone_and_shows_nothing_on_the_other() {
    let (mut f, id, left, right) = two_outputs(true, Vec::new());
    let old = put_at(&mut f, id, TOP_LEFT, 0, LONG);
    let new = put_at(&mut f, id, TOP_LEFT, 1, LONG);
    let other = put_at(&mut f, id, LOW_RIGHT, 0, SMALL);
    let on_other = middle(f.visible_frame(other));
    let right_area = output_geometry(&right);
    let local = on_other - right_area.loc;
    f.wait_for_pixel_on(&right, local.x, local.y, WINDOW);
    f.stop_clock();

    switch_output(&mut f, &left, 1);
    assert_eq!(active_on(&f, [&left, &right]), [1, 0]);
    f.advance_clock(PART_WAY);
    f.draw(&right);

    let slide = slide(&f);
    assert_eq!(slide.output, left.name());
    assert_eq!((slide.from, slide.to), (0, 1));
    assert!(0 < slide.offset.x && slide.offset.x < OUTPUT.0, "{slide:?}");
    assert_eq!(slide.windows.len(), 2, "{slide:?}");
    assert!(slide.still.is_empty(), "{slide:?}");
    assert_eq!(slide.drawn(&f, old).loc.y, f.visible_frame(old).loc.y);
    // The new workspace's window comes in from the right, over where the other output is,
    // and is drawn on its own output alone.
    let arriving = slide.drawn(&f, new);
    let over_the_other = middle_within(arriving, right_area);
    assert_eq!(pixel(&mut f, middle_on_output(arriving)), WINDOW);
    assert_eq!(pixel_on(&mut f, &right, over_the_other), clear());
    assert_eq!(pixel_on(&mut f, &right, on_other), WINDOW);

    f.advance_clock(LENGTH - PART_WAY);
    f.draw(&right);
    assert_no_slide(&f);
    assert_eq!(pixel_on(&mut f, &right, over_the_other), clear());
    assert_eq!(pixel_on(&mut f, &right, on_other), WINDOW);
}

#[test]
fn a_switch_towards_the_other_output_sends_nothing_onto_it() {
    let (mut f, id, left, right) = two_outputs(true, Vec::new());
    put_at(&mut f, id, TOP_LEFT, 0, LONG);
    let old = put_at(&mut f, id, TOP_LEFT, 1, LONG);
    let other = put_at(&mut f, id, LOW_RIGHT, 0, SMALL);
    let on_other = middle(f.visible_frame(other));
    let right_area = output_geometry(&right);
    switch_output(&mut f, &left, 1);
    f.advance_clock(LENGTH);
    f.stop_clock();

    // Back to the first workspace, on the left in the grid: the second one's window
    // leaves to the right.
    switch_output(&mut f, &left, 0);
    f.advance_clock(PART_WAY);
    f.draw(&right);

    let slide = slide(&f);
    assert_eq!((slide.from, slide.to), (1, 0));
    let leaving = slide.drawn(&f, old);
    let over_the_other = middle_within(leaving, right_area);
    assert_eq!(pixel(&mut f, middle_on_output(leaving)), WINDOW);
    assert_eq!(pixel_on(&mut f, &right, over_the_other), clear());
    assert_eq!(pixel_on(&mut f, &right, on_other), WINDOW);
}

#[test]
fn a_straddling_window_of_the_other_output_stays_drawn_on_both_through_the_slide() {
    let (mut f, id, left, right) = two_outputs(true, Vec::new());
    let old = put_at(&mut f, id, TOP_LEFT, 0, LONG);
    let new = put_at(&mut f, id, TOP_LEFT, 1, LONG);
    let across = put_at(&mut f, id, ACROSS, 0, SMALL);
    let pid = f.pid(across);
    assert_eq!(
        f.state.caminito.policy.output_showing(pid),
        Some(output_id(&right))
    );
    let frame = f.visible_frame(across);
    let on_left = middle_within(frame, output());
    let on_right = middle_within(frame, output_geometry(&right));
    f.wait_for_pixel(on_left.x, on_left.y, WINDOW);
    f.stop_clock();

    switch_output(&mut f, &left, 1);

    // As the slide starts, part-way and when it's over.
    for step in [Duration::ZERO, PART_WAY, LENGTH] {
        f.advance_clock(step);
        f.draw(&right);
        assert!(f.state.caminito.policy.is_showing(pid));
        assert_eq!(f.visible_frame(across), frame);
        assert_eq!(pixel(&mut f, on_left), WINDOW, "after {step:?}");
        assert_eq!(pixel_on(&mut f, &right, on_right), WINDOW, "after {step:?}");
        if step < LENGTH {
            // The two others slide by in their band above it, and it's none of theirs.
            let slide = slide(&f);
            assert_eq!(slide.windows.len(), 2, "{slide:?}");
            slide.drawn(&f, old);
            slide.drawn(&f, new);
            assert!(slide.still.is_empty(), "{slide:?}");
        }
    }
    assert_no_slide(&f);
}

#[test]
fn a_sliding_window_that_straddles_is_at_its_place_on_the_other_output() {
    let (mut f, id, left, right) = two_outputs(true, Vec::new());
    put_at(&mut f, id, TOP_LEFT, 0, SMALL);
    // Most of it on the left output, its Main output, and the rest on the right one.
    let at = (OUTPUT.0 - 200, 380);
    let new = put_at(&mut f, id, at, 1, SMALL);
    let frame = f.visible_frame(new);
    let on_right = middle_within(frame, output_geometry(&right));
    f.draw(&right);
    assert_eq!(
        pixel_on(&mut f, &right, on_right),
        clear(),
        "it doesn't show"
    );
    f.stop_clock();

    switch_output(&mut f, &left, 1);
    f.advance_clock(PART_WAY);
    f.draw(&right);

    let out = slide(&f);
    let arriving = out.drawn(&f, new);
    assert_eq!(arriving, moved(frame, out.offset));
    // Its own output draws it on its way in. The other draws its part at its place, and
    // nothing of it further right, where its own output has it now.
    let further = Point::from((arriving.loc.x + arriving.size.w - 20, on_right.y));
    assert!(!frame.contains(further), "{out:?}");
    assert_eq!(pixel_on(&mut f, &right, on_right), WINDOW);
    assert_eq!(pixel_on(&mut f, &right, further), clear());

    // And back: it leaves on its own output, and the other doesn't draw its part.
    f.advance_clock(LENGTH);
    switch_output(&mut f, &left, 0);
    f.advance_clock(PART_WAY);
    f.draw(&right);
    let back = slide(&f);
    // The slide has it over the other output by now, which draws it nowhere.
    let leaving = back.drawn(&f, new);
    assert_eq!(pixel_on(&mut f, &right, middle(leaving)), clear());
    assert_eq!(pixel_on(&mut f, &right, on_right), clear());
}

#[test]
fn under_spanning_workspaces_each_output_slides_its_own_windows_by_its_own_size() {
    let (mut f, id, left, right) = two_outputs(false, two_rows());
    // Each workspace's window on the left output, and its window on the right one.
    let windows: Vec<[ClientWindow; 2]> = [0, 1, 3]
        .into_iter()
        .map(|w| [TOP_LEFT, TOP_RIGHT].map(|at| put_at(&mut f, id, at, w, LONG)))
        .collect();
    let outputs = [&left, &right];
    f.stop_clock();

    // Right, and then down.
    let ways = [
        (KEY_RIGHT, 0, 1, [(OUTPUT.0, 0), (WIDE.0, 0)]),
        (KEY_DOWN, 1, 2, [(0, OUTPUT.1), (0, WIDE.1)]),
    ];
    for (key, from, to, distances) in ways {
        f.chord(&[KEY_LEFTCTRL, KEY_LEFTALT, key]);
        let at_once = slides(&f);
        f.advance_clock(LENGTH / 2);
        let half_way = slides(&f);
        assert_eq!(at_once.len(), 2, "both outputs slide: {at_once:?}");
        assert_eq!(half_way.len(), 2, "both outputs slide: {half_way:?}");

        for (index, output) in outputs.into_iter().enumerate() {
            let distance = Point::<i32, Logical>::from(distances[index]);
            let (old, new) = (windows[from][index], windows[to][index]);
            let (old_frame, new_frame) = (f.visible_frame(old), f.visible_frame(new));
            assert_eq!(at_once[index].output, output.name());
            // The whole of its own width or height at the start.
            assert_eq!(at_once[index].offset, distance);
            // Half the time leaves an eighth of the way.
            let slide = &half_way[index];
            let offset = Point::from((distance.x / 8, distance.y / 8));
            assert_eq!(slide.offset, offset, "{slide:?}");
            assert_eq!(slide.windows.len(), 2, "its own windows alone: {slide:?}");
            assert_eq!(slide.drawn(&f, new), moved(new_frame, offset));
            assert_eq!(slide.drawn(&f, old), moved(old_frame, offset - distance));
            let arriving = middle_within(slide.drawn(&f, new), output_geometry(output));
            assert_eq!(pixel_on(&mut f, output, arriving), WINDOW);
        }

        f.advance_clock(LENGTH / 2);
        assert_no_slide(&f);
    }
}

#[test]
fn turning_per_output_workspaces_on_or_off_and_an_output_coming_or_going_slide_nothing() {
    let (mut f, id, left, right) = two_outputs(true, Vec::new());
    put_at(&mut f, id, TOP_LEFT, 0, LONG);
    let first = put_at(&mut f, id, LOW_RIGHT, 0, SMALL);
    let second = put_at(&mut f, id, TOP_RIGHT, 1, LONG);
    let (first_frame, second_frame) = (f.visible_frame(first), f.visible_frame(second));
    switch_output(&mut f, &right, 1);
    f.advance_clock(LENGTH);
    assert_no_slide(&f);
    // A slide that started from here on wouldn't end for the time that passes.
    f.stop_clock();
    assert_eq!(pixel_on(&mut f, &right, middle(second_frame)), WINDOW);

    // Off: the right output takes the primary output's workspace, at once.
    f.set_setting(CAMINITO, "workspaces-per-output", Setting::Bool(false));
    f.dispatch_until(|f| !f.state.caminito.policy.workspaces_per_output());
    assert_eq!(active_on(&f, [&left, &right]), [0, 0]);
    assert_no_slide(&f);
    assert_eq!(pixel_on(&mut f, &right, middle(first_frame)), WINDOW);
    assert_eq!(pixel_on(&mut f, &right, middle(second_frame)), clear());

    f.set_setting(CAMINITO, "workspaces-per-output", Setting::Bool(true));
    f.dispatch_until(|f| f.state.caminito.policy.workspaces_per_output());
    assert_no_slide(&f);

    // An output that appears.
    let third = f.add_output((640, 480), 1.0);
    f.dispatch();
    assert_no_slide(&f);
    switch_output(&mut f, &third, 1);
    assert_eq!(slide(&f).output, third.name(), "it slides as the others do");
    f.advance_clock(LENGTH);

    // And one that goes, from another workspace: the left output takes its windows.
    switch_output(&mut f, &right, 1);
    f.advance_clock(LENGTH);
    assert_eq!(active_on(&f, [&left, &right]), [0, 1]);
    f.unplug(&right);
    assert_eq!(
        f.state.caminito.policy.output_showing(f.pid(first)),
        Some(output_id(&left))
    );
    assert_no_slide(&f);
    f.unplug(&third);
    assert_no_slide(&f);
}

#[test]
fn dynamic_workspaces_removing_the_workspace_an_output_shows_slides_it_to_the_first() {
    let settings = vec![
        (
            MARCO_GENERAL,
            "dynamic-workspaces".into(),
            Setting::Bool(true),
        ),
        (CAMINITO, "workspace-slide".into(), Setting::Bool(true)),
    ];
    let mut f = Fixture::with_marco_master_settings(settings);
    f.add_output(OUTPUT, 1.0);
    let id = f.connect(Kind::Xdg);
    let first = f.map(id, WindowSpec::plain(BIG));
    let second = map_on(&mut f, id, 1, SMALL);
    let frame = f.visible_frame(first);
    // The empty workspace after the two.
    f.state.caminito.policy.switch_workspace(2);
    f.dispatch();
    f.advance_clock(LENGTH);
    assert_no_slide(&f);
    f.stop_clock();

    // The second workspace's one window goes, and the empty one after it with it.
    f.client(id).destroy_window(second.xdg());
    f.roundtrip(id);
    assert_eq!(f.state.caminito.policy.workspace_count(), 2);
    assert_eq!(active(&f), 0);

    // Two columns to the left.
    let slide = slide(&f);
    assert_eq!((slide.from, slide.to), (2, 0));
    assert_eq!(slide.offset, Point::from((-2 * OUTPUT.0, 0)));
    assert_eq!(slide.windows.len(), 1, "{slide:?}");
    assert_eq!(slide.drawn(&f, first), moved(frame, slide.offset));
    f.advance_clock(LENGTH);
    assert_no_slide(&f);
    assert_eq!(pixel(&mut f, middle(frame)), WINDOW);
}
