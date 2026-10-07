//! wlr-output-management (58): the heads and modes a client sees, and what its
//! configurations change, as the client and the debug dump show them. The outputs are the
//! developer's monitors (50), as in `output_lifecycle`: an AOC at 1.0 and an LG 4K at 1.70.

use caminito_test_client::output_management::{HeadChange, Outcome, format_heads};
use insta::assert_snapshot;
use smithay::utils::Rectangle;

use super::fixture::{ClientId, Fixture, Kind, WindowSpec, on_both_kinds};
use crate::settings::Value;
use crate::settings::translate::INTERFACE;

/// The AOC G2490W1G4's EDID size.
const AOC_MM: (i32, i32) = (527, 296);
/// Its preferred mode, and a smaller one.
const AOC_MODES: [(i32, i32); 2] = [(1920, 1080), (1280, 720)];
/// The LG HDR 4K.
const LG: ((i32, i32), (i32, i32)) = ((600, 340), (3840, 2160));

/// The AOC as `headless-1`, and a client that has seen output management's heads.
fn aoc_and_a_client() -> (Fixture, ClientId) {
    let mut f = Fixture::new();
    f.plug_with_modes(AOC_MM, &AOC_MODES);
    let client = manage_outputs(&mut f);
    (f, client)
}

/// A client that has bound output management and seen the heads.
fn manage_outputs(f: &mut Fixture) -> ClientId {
    let client = f.connect(Kind::Xdg);
    f.client(client).bind_output_management();
    f.roundtrip(client);
    client
}

fn heads(f: &mut Fixture, client: ClientId) -> String {
    format_heads(&f.client(client).output_management().heads)
}

/// The compositor's answer to the client's only configuration so far.
#[track_caller]
fn outcome(f: &mut Fixture, client: ClientId) -> Outcome {
    let outcomes = &f.client(client).output_management().outcomes;
    assert_eq!(outcomes.len(), 1, "one configuration was answered");
    outcomes[0]
}

/// Each output's name, place, logical size and scale, one per line, from the dump.
fn layout(f: &Fixture) -> String {
    let dump: serde_json::Value = serde_json::from_str(&f.state.caminito.state_dump()).unwrap();
    dump["outputs"]
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

#[test]
fn a_client_sees_the_heads_and_modes_of_two_outputs() {
    let mut f = Fixture::new();
    f.plug_with_modes(AOC_MM, &AOC_MODES);
    f.plug(LG.0, LG.1);

    let client = manage_outputs(&mut f);

    // The scale is a `wl_fixed`, so the LG's 1.7 comes as 435/256.
    assert_snapshot!(heads(&mut f, client), @"
    headless-1: Caminito - Headless - headless-1, 527x296 mm, make Caminito, model Headless, serial (empty)
      modes: 1920x1080@60000 preferred, 1280x720@60000
      enabled: 1920x1080@60000 at 0,0, scale 1, transform Normal
      adaptive sync off
    headless-2: Caminito - Headless - headless-2, 600x340 mm, make Caminito, model Headless, serial (empty)
      modes: 3840x2160@60000 preferred
      enabled: 3840x2160@60000 at 1920,0, scale 1.69921875, transform Normal
      adaptive sync off
    ");
}

#[test]
fn a_client_is_told_of_outputs_that_come_and_go() {
    let (mut f, client) = aoc_and_a_client();
    let serial = f.client(client).output_serial();

    let lg = f.plug(LG.0, LG.1);
    f.roundtrip(client);
    assert_snapshot!(heads(&mut f, client), @"
    headless-1: Caminito - Headless - headless-1, 527x296 mm, make Caminito, model Headless, serial (empty)
      modes: 1920x1080@60000 preferred, 1280x720@60000
      enabled: 1920x1080@60000 at 0,0, scale 1, transform Normal
      adaptive sync off
    headless-2: Caminito - Headless - headless-2, 600x340 mm, make Caminito, model Headless, serial (empty)
      modes: 3840x2160@60000 preferred
      enabled: 3840x2160@60000 at 1920,0, scale 1.69921875, transform Normal
      adaptive sync off
    ");
    assert_ne!(f.client(client).output_serial(), serial);

    f.unplug(&lg);
    f.roundtrip(client);
    assert_snapshot!(heads(&mut f, client), @"
    headless-1: Caminito - Headless - headless-1, 527x296 mm, make Caminito, model Headless, serial (empty)
      modes: 1920x1080@60000 preferred, 1280x720@60000
      enabled: 1920x1080@60000 at 0,0, scale 1, transform Normal
      adaptive sync off
    ");
}

#[test]
fn applying_a_configuration_changes_the_scale_and_the_mode() {
    let (mut f, client) = aoc_and_a_client();

    let change = HeadChange::of("headless-1").mode((1280, 720)).scale(1.25);
    f.client(client).apply_outputs(&[change]);
    f.roundtrip(client);

    assert_eq!(outcome(&mut f, client), Outcome::Succeeded);
    assert_snapshot!(layout(&f), @"headless-1 at 0,0 1024x576 scale 1.25");
    // The client is told what the head shows now.
    assert_snapshot!(heads(&mut f, client), @"
    headless-1: Caminito - Headless - headless-1, 527x296 mm, make Caminito, model Headless, serial (empty)
      modes: 1920x1080@60000 preferred, 1280x720@60000
      enabled: 1280x720@60000 at 0,0, scale 1.25, transform Normal
      adaptive sync off
    ");
}

#[test]
fn an_applied_scale_is_set_not_caminito_s_default() {
    let (mut f, client) = aoc_and_a_client();
    let change = HeadChange::of("headless-1").scale(1.25);
    f.client(client).apply_outputs(&[change]);
    f.roundtrip(client);

    // The LG is at Caminito's default, so it takes `window-scaling-factor`'s scale. The AOC
    // keeps what the client set.
    f.plug(LG.0, LG.1);
    f.set_setting(INTERFACE, "window-scaling-factor", Value::Int(1));
    f.dispatch_until(|f| layout(f).contains("3840x2160"));

    assert_snapshot!(layout(&f), @"
    headless-1 at 0,0 1536x864 scale 1.25
    headless-2 at 1536,0 3840x2160 scale 1.0
    ");
}

#[test]
fn applying_a_configuration_places_an_output_against_another() {
    let mut f = Fixture::new();
    f.plug_with_modes(AOC_MM, &AOC_MODES);
    f.plug(LG.0, LG.1);
    let client = manage_outputs(&mut f);

    f.client(client)
        .apply_outputs(&[HeadChange::of("headless-2").position((300, 1080))]);
    f.roundtrip(client);

    assert_eq!(outcome(&mut f, client), Outcome::Succeeded);
    assert_snapshot!(layout(&f), @"
    headless-1 at 0,0 1920x1080 scale 1.0
    headless-2 at 300,1080 2259x1271 scale 1.7
    ");
}

/// As `wlr-randr` and kanshi users write places by hand (82).
#[test]
fn a_configuration_that_isnt_contiguous_succeeds_and_is_healed() {
    let mut f = Fixture::new();
    f.plug_with_modes(AOC_MM, &AOC_MODES);
    f.plug(LG.0, LG.1);
    let client = manage_outputs(&mut f);

    f.client(client)
        .apply_outputs(&[HeadChange::of("headless-2").position((3000, 200))]);
    f.roundtrip(client);

    assert_eq!(outcome(&mut f, client), Outcome::Succeeded);
    assert_snapshot!(layout(&f), @"
    headless-1 at 0,0 1920x1080 scale 1.0
    headless-2 at 1920,200 2259x1271 scale 1.7
    ");
    // The client is told where the output went, not where it asked.
    assert!(heads(&mut f, client).contains("3840x2160@60000 at 1920,200,"));
}

#[test]
fn disabling_the_middle_output_of_three_leaves_the_other_two_touching() {
    let mut f = Fixture::new();
    f.plug_with_modes(AOC_MM, &AOC_MODES);
    f.plug(LG.0, LG.1);
    f.plug(AOC_MM, AOC_MODES[0]);
    let client = manage_outputs(&mut f);

    // The client sends the other two's places back, as `wlr-randr --output A --off` does.
    f.client(client)
        .apply_outputs(&[HeadChange::of("headless-2").disable()]);
    f.roundtrip(client);

    assert_eq!(outcome(&mut f, client), Outcome::Succeeded);
    assert_snapshot!(layout(&f), @"
    headless-1 at 0,0 1920x1080 scale 1.0
    headless-3 at 1920,0 1920x1080 scale 1.0
    ");
}

#[test]
fn an_applied_scale_that_grows_an_output_pushes_its_neighbour_along() {
    let mut f = Fixture::new();
    let lg = f.plug(LG.0, LG.1);
    f.set_output_scale(&lg, 2.0);
    f.plug_with_modes(AOC_MM, &AOC_MODES);
    let client = manage_outputs(&mut f);
    assert_snapshot!(layout(&f), @"
    headless-1 at 0,0 1920x1080 scale 2.0
    headless-2 at 1920,0 1920x1080 scale 1.0
    ");

    // The client sends the AOC's place back as it was, as `wlr-randr` does, which the LG
    // now covers. That's no new place, so the AOC stays on the LG's right, though above
    // the LG would be nearer.
    f.client(client)
        .apply_outputs(&[HeadChange::of("headless-1").scale(1.0)]);
    f.roundtrip(client);

    assert_eq!(outcome(&mut f, client), Outcome::Succeeded);
    assert_snapshot!(layout(&f), @"
    headless-1 at 0,0 3840x2160 scale 1.0
    headless-2 at 3840,0 1920x1080 scale 1.0
    ");
}

#[test]
fn an_applied_mode_that_shrinks_an_output_brings_its_neighbour_along() {
    let mut f = Fixture::new();
    f.plug_with_modes(AOC_MM, &AOC_MODES);
    f.plug(LG.0, LG.1);
    let client = manage_outputs(&mut f);

    f.client(client)
        .apply_outputs(&[HeadChange::of("headless-1").mode((1280, 720))]);
    f.roundtrip(client);

    assert_eq!(outcome(&mut f, client), Outcome::Succeeded);
    assert_snapshot!(layout(&f), @"
    headless-1 at 0,0 1280x720 scale 1.0
    headless-2 at 1280,0 2259x1271 scale 1.7
    ");
}

#[test]
fn a_configuration_with_a_place_of_no_use_fails() {
    let (mut f, client) = aoc_and_a_client();

    f.client(client)
        .apply_outputs(&[HeadChange::of("headless-1").position((i32::MAX, 0))]);
    f.roundtrip(client);

    assert_eq!(outcome(&mut f, client), Outcome::Failed);
}

#[test]
fn a_configuration_that_is_only_tested_changes_nothing() {
    let (mut f, client) = aoc_and_a_client();
    let serial = f.client(client).output_serial();
    let before = (layout(&f), heads(&mut f, client));

    let change = HeadChange::of("headless-1")
        .mode((1280, 720))
        .scale(1.25)
        .position((100, 50));
    f.client(client).test_outputs(&[change]);
    f.roundtrip(client);

    assert_eq!(outcome(&mut f, client), Outcome::Succeeded);
    assert_eq!((layout(&f), heads(&mut f, client)), before);
    assert_eq!(f.client(client).output_serial(), serial);
}

#[test]
fn a_configuration_with_a_stale_serial_is_cancelled() {
    let (mut f, client) = aoc_and_a_client();
    let stale = f.client(client).output_serial();
    f.plug(LG.0, LG.1);
    f.roundtrip(client);
    let before = layout(&f);

    let change = HeadChange::of("headless-1").scale(1.25);
    f.client(client).apply_outputs_at(&[change], stale);
    f.roundtrip(client);

    assert_eq!(outcome(&mut f, client), Outcome::Cancelled);
    assert_eq!(layout(&f), before);
}

on_both_kinds! {
    fn disabling_an_output_removes_it_as_unplugging_does(kind: Kind) {
        let mut f = Fixture::new();
        f.plug_with_modes(AOC_MM, &AOC_MODES);
        f.plug(LG.0, LG.1);
        let client = manage_outputs(&mut f);
        let windows = f.connect(kind);
        let window = f.map(windows, WindowSpec::default());
        assert!(f.geometry(window).loc.x < 1920, "the window is on the left output");

        f.client(client).apply_outputs(&[HeadChange::of("headless-1").disable()]);
        f.roundtrip(client);
        f.sync(window);

        assert_eq!(outcome(&mut f, client), Outcome::Succeeded);
        // The client sent the other output's place back, as `wlr-randr` does, and it
        // goes to the origin all the same (82).
        assert_eq!(layout(&f), "headless-2 at 0,0 2259x1271 scale 1.7");
        let geometry = f.geometry(window);
        assert!(
            Rectangle::new((0, 0).into(), (2259, 1271).into()).contains_rect(geometry),
            "the window is on the remaining output: {geometry:?}"
        );
        assert!(
            heads(&mut f, client).contains("headless-1: ")
                && heads(&mut f, client).contains("\n  disabled\n"),
            "the disabled head stays, as disabled"
        );
    }
}

#[test]
fn an_output_enabled_again_is_at_caminito_s_defaults() {
    let mut f = Fixture::new();
    f.plug_with_modes(AOC_MM, &AOC_MODES);
    f.plug(LG.0, LG.1);
    let client = manage_outputs(&mut f);
    f.client(client)
        .apply_outputs(&[HeadChange::of("headless-1").disable()]);
    f.roundtrip(client);

    f.client(client)
        .apply_outputs(&[HeadChange::of("headless-1").enable()]);
    f.roundtrip(client);

    assert_eq!(
        f.client(client).output_management().outcomes,
        [Outcome::Succeeded, Outcome::Succeeded]
    );
    // In its preferred mode, right of the output that stayed.
    assert_snapshot!(layout(&f), @"
    headless-2 at 0,0 2259x1271 scale 1.7
    headless-1 at 2259,0 1920x1080 scale 1.0
    ");
    assert_snapshot!(heads(&mut f, client), @"
    headless-1: Caminito - Headless - headless-1, 527x296 mm, make Caminito, model Headless, serial (empty)
      modes: 1920x1080@60000 preferred, 1280x720@60000
      enabled: 1920x1080@60000 at 2259,0, scale 1, transform Normal
      adaptive sync off
    headless-2: Caminito - Headless - headless-2, 600x340 mm, make Caminito, model Headless, serial (empty)
      modes: 3840x2160@60000 preferred
      enabled: 3840x2160@60000 at 0,0, scale 1.69921875, transform Normal
      adaptive sync off
    ");
}

#[test]
fn a_disabled_output_that_is_unplugged_is_gone() {
    let mut f = Fixture::new();
    let aoc = f.plug_with_modes(AOC_MM, &AOC_MODES);
    let client = manage_outputs(&mut f);
    f.client(client)
        .apply_outputs(&[HeadChange::of("headless-1").disable()]);
    f.roundtrip(client);

    f.unplug(&aoc);
    f.roundtrip(client);

    assert_eq!(heads(&mut f, client), "");
}

#[test]
fn a_configuration_with_a_mode_the_output_doesnt_have_fails_and_changes_nothing() {
    let mut f = Fixture::new();
    f.plug_with_modes(AOC_MM, &AOC_MODES);
    f.plug(LG.0, LG.1);
    let client = manage_outputs(&mut f);
    let before = layout(&f);

    let changes = [
        HeadChange::of("headless-1").scale(1.25),
        HeadChange::of("headless-2").custom_mode((1234, 567), 60_000),
    ];
    f.client(client).apply_outputs(&changes);
    f.roundtrip(client);

    assert_eq!(outcome(&mut f, client), Outcome::Failed);
    assert_eq!(layout(&f), before);
}

#[test]
fn a_configuration_the_backend_cant_apply_fails_and_changes_nothing() {
    let mut f = Fixture::new();
    f.plug_with_modes(AOC_MM, &AOC_MODES);
    let lg = f.plug_with_modes(LG.0, &[LG.1, (1920, 1080)]);
    let client = manage_outputs(&mut f);
    f.refuse_mode(&lg, (1920, 1080));
    let before = (layout(&f), heads(&mut f, client));

    // The first head's mode is switched before the second's is refused.
    let changes = [
        HeadChange::of("headless-1").mode((1280, 720)).scale(1.25),
        HeadChange::of("headless-2").mode((1920, 1080)),
    ];
    f.client(client).apply_outputs(&changes);
    f.roundtrip(client);

    assert_eq!(outcome(&mut f, client), Outcome::Failed);
    assert_eq!((layout(&f), heads(&mut f, client)), before);
}

#[test]
fn a_custom_mode_that_is_one_of_the_outputs_modes_is_taken() {
    let (mut f, client) = aoc_and_a_client();

    // No refresh rate: any mode of the size.
    let change = HeadChange::of("headless-1").custom_mode((1280, 720), 0);
    f.client(client).apply_outputs(&[change]);
    f.roundtrip(client);

    assert_eq!(outcome(&mut f, client), Outcome::Succeeded);
    assert_snapshot!(layout(&f), @"headless-1 at 0,0 1280x720 scale 1.0");
}

#[test]
fn a_configuration_that_asks_for_adaptive_sync_fails() {
    let (mut f, client) = aoc_and_a_client();

    let change = HeadChange::of("headless-1").adaptive_sync();
    f.client(client).apply_outputs(&[change]);
    f.roundtrip(client);

    assert_eq!(outcome(&mut f, client), Outcome::Failed);
}

#[test]
fn a_configuration_with_a_scale_of_no_use_fails() {
    let (mut f, client) = aoc_and_a_client();

    f.client(client)
        .apply_outputs(&[HeadChange::of("headless-1").scale(0.01)]);
    f.roundtrip(client);

    assert_eq!(outcome(&mut f, client), Outcome::Failed);
}

#[test]
fn a_second_configuration_made_before_the_first_was_applied_is_cancelled() {
    let (mut f, client) = aoc_and_a_client();

    // Both are made at the same serial, and reach Caminito in one dispatch.
    f.client(client)
        .apply_outputs(&[HeadChange::of("headless-1").scale(1.25)]);
    f.client(client)
        .apply_outputs(&[HeadChange::of("headless-1").scale(1.5)]);
    f.roundtrip(client);

    assert_eq!(
        f.client(client).output_management().outcomes,
        [Outcome::Succeeded, Outcome::Cancelled]
    );
    assert_snapshot!(layout(&f), @"headless-1 at 0,0 1536x864 scale 1.25");
}

#[test]
fn a_configuration_that_changes_nothing_still_gives_a_new_serial() {
    let (mut f, client) = aoc_and_a_client();
    let serial = f.client(client).output_serial();

    f.client(client)
        .apply_outputs(&[HeadChange::of("headless-1")]);
    f.roundtrip(client);
    f.client(client)
        .apply_outputs(&[HeadChange::of("headless-1")]);
    f.roundtrip(client);

    assert_ne!(f.client(client).output_serial(), serial);
    assert_eq!(
        f.client(client).output_management().outcomes,
        [Outcome::Succeeded, Outcome::Succeeded]
    );
}

#[test]
fn an_applied_scale_snaps_to_120ths() {
    let (mut f, client) = aoc_and_a_client();

    f.client(client)
        .apply_outputs(&[HeadChange::of("headless-1").scale(1.33)]);
    f.roundtrip(client);

    // 1.33 is 340/256 as a `wl_fixed`, and 159/120 is the nearest 120th to that.
    assert_snapshot!(layout(&f), @"headless-1 at 0,0 1449x815 scale 1.325");
}

/// The preferred scale is a Wayland surface event, and Xwayland owns the surfaces of
/// XWayland windows, so this maps an xdg toplevel only.
#[test]
fn an_applied_scale_is_sent_to_the_windows_on_the_output() {
    let (mut f, client) = aoc_and_a_client();
    let spec = WindowSpec {
        fractional_scale: true,
        ..WindowSpec::default()
    };
    let window = f.map(client, spec);

    f.client(client)
        .apply_outputs(&[HeadChange::of("headless-1").scale(1.5)]);
    f.roundtrip(client);

    assert_snapshot!(f.client(client).window(window.xdg()).output_events.join("\n"), @"
    fractional scale 1
    buffer scale 2
    fractional scale 1.5
    ");
}

#[test]
fn a_second_client_sees_what_the_first_applied() {
    let (mut f, first) = aoc_and_a_client();
    f.client(first)
        .apply_outputs(&[HeadChange::of("headless-1").scale(1.5)]);
    f.roundtrip(first);

    let second = manage_outputs(&mut f);

    assert_eq!(heads(&mut f, second), heads(&mut f, first));
    assert_eq!(
        f.client(second).output_serial(),
        f.client(first).output_serial()
    );
}
