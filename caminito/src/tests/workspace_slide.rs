//! The Workspace slide (233, 234): a change of the output's active workspace shows the old
//! workspace's windows leaving and the new one's arriving, for 250 ms of Caminito's clock,
//! read from the debug dump and from the output's pixels. Policy has switched from the
//! start.

use std::time::Duration;

use caminito_test_client::format_input;
use caminito_test_client::layer_shell::{KeyboardInteractivity, Layer, LayerAnchor, LayerSpec};
use smithay::utils::{Logical, Point, Rectangle};

use super::fixture::{
    BTN_LEFT, ClientId, ClientWindow, Fixture, KEY_A, KEY_DOWN, KEY_LEFT, KEY_LEFTALT,
    KEY_LEFTCTRL, KEY_LEFTMETA, KEY_LEFTSHIFT, KEY_RIGHT, KEY_UP, Kind, WINDOW_COLOR, WindowSpec,
    on_both_kinds,
};
use crate::backend::headless::argb8888;
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
    let on_output = rect.intersection(output());
    middle(on_output.unwrap_or_else(|| panic!("some of {rect:?} is on the output")))
}

/// The output's pixel at the global point `at`, as `0xRRGGBB`.
fn pixel(f: &mut Fixture, at: Point<i32, Logical>) -> u32 {
    let output = f.state.caminito.outputs[0].clone();
    let pixel = f
        .state
        .backend
        .headless_mut()
        .pixel(&output, at.to_physical(1));
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

#[test]
fn a_second_switch_ends_the_running_slide_and_starts_its_own() {
    let (mut f, id) = desk(Kind::Xdg, two_rows());
    let first = map_on(&mut f, id, 0, BIG);
    map_on(&mut f, id, 1, BIG);
    let last = map_on(&mut f, id, 3, BIG);

    f.chord(&[KEY_LEFTCTRL, KEY_LEFTALT, KEY_RIGHT]);
    f.advance_clock(PART_WAY);
    f.chord(&[KEY_LEFTCTRL, KEY_LEFTALT, KEY_DOWN]);
    f.advance_clock(PART_WAY);

    let slide = slide(&f);
    assert_eq!((slide.from, slide.to), (1, 3));
    assert_eq!(slide.offset.x, 0);
    assert_eq!(
        slide.drawn(&f, last),
        moved(f.visible_frame(last), slide.offset)
    );
    // The first workspace's window, which the first slide was moving out, is gone.
    let first = f.pid(first).0;
    assert!(
        slide.windows.iter().all(|(id, _)| *id != first),
        "{slide:?}"
    );
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
