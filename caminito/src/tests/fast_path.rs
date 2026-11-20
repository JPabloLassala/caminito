//! Fast-path eligibility (CMN-312) through the headless fixture: the debug dump says, for each
//! output, whether its last frame was one opaque fullscreen window that covers it with
//! nothing drawn over it, and why not. What was drawn over it gone, the next frame is
//! eligible again.

use caminito_test_client::layer_shell::{Layer, LayerSpec};
use caminito_test_client::{Anchor, ConstraintAdjustment, Gravity, Positioner};
use smithay::output::Output;

use super::fixture::{
    ClientId, ClientWindow, Fixture, KEY_ESC, KEY_LEFT, KEY_LEFTALT, KEY_LEFTCTRL, KEY_RIGHT, Kind,
    WindowSpec,
};
use super::overview;
use crate::settings::Value as Setting;
use crate::settings::translate::CAMINITO;
use crate::workspace_slide::LENGTH;

const OUTPUT: (i32, i32) = (1920, 1080);

/// The whole of a surface of the output's size, as an opaque region.
const WHOLE: Option<(i32, i32, i32, i32)> = Some((0, 0, OUTPUT.0, OUTPUT.1));

/// One output and a Wayland client, on settings with `initial` set.
fn desk_with(initial: Vec<(&'static str, String, Setting)>) -> (Fixture, ClientId, Output) {
    let mut f = Fixture::with_settings(initial);
    let output = f.add_output(OUTPUT, 1.0);
    let client = f.connect(Kind::Xdg);
    (f, client, output)
}

fn desk() -> (Fixture, ClientId, Output) {
    desk_with(Vec::new())
}

/// The output's eligibility after a frame of it, as the debug dump has it: the window, by
/// policy's id, if it's eligible, or why not and what's drawn over the window.
#[derive(Debug, PartialEq, Eq)]
enum Eligibility {
    Eligible(u64),
    Not(String, Option<String>),
}

fn not(reason: &str) -> Eligibility {
    Eligibility::Not(reason.into(), None)
}

fn drawn_over(over: &str) -> Eligibility {
    Eligibility::Not("drawn-over".into(), Some(over.into()))
}

fn eligibility(f: &mut Fixture, output: &Output) -> Eligibility {
    f.draw(output);
    let dump: serde_json::Value = serde_json::from_str(&f.state.caminito.state_dump()).unwrap();
    let fast_path = &dump["outputs"][0]["fast_path"];
    let text = |name: &str| fast_path[name].as_str().map(str::to_owned);
    if fast_path["eligible"].as_bool().unwrap() {
        return Eligibility::Eligible(fast_path["window"].as_u64().unwrap());
    }
    Eligibility::Not(text("reason").unwrap(), text("over"))
}

/// Maps a window that goes fullscreen and commits a `size` buffer, opaque in `opaque`.
fn fullscreen(
    f: &mut Fixture,
    client: ClientId,
    size: (i32, i32),
    opaque: Option<(i32, i32, i32, i32)>,
) -> ClientWindow {
    let window = f.map(client, WindowSpec::plain((400, 300)));
    let xdg = window.xdg();
    f.client(client).set_fullscreen(xdg, None);
    let pid = f.pid(window);
    f.dispatch_until(|f| f.state.caminito.policy.is_fullscreen(pid));
    f.roundtrip(client);
    f.client(client).set_opaque_region(xdg, opaque);
    f.client(client)
        .ack_and_commit_buffer(xdg, size, 0xff00_80ff);
    f.roundtrip(client);
    window
}

#[test]
fn an_opaque_fullscreen_window_that_covers_its_output_is_eligible() {
    let (mut f, client, output) = desk();
    let window = fullscreen(&mut f, client, OUTPUT, WHOLE);

    assert_eq!(
        eligibility(&mut f, &output),
        Eligibility::Eligible(f.pid(window).0)
    );
}

#[test]
fn an_output_without_a_fullscreen_window_is_not_eligible() {
    let (mut f, client, output) = desk();
    f.map(client, WindowSpec::plain((400, 300)));

    assert_eq!(eligibility(&mut f, &output), not("no-fullscreen-window"));
}

#[test]
fn a_translucent_fullscreen_window_is_not_eligible() {
    let (mut f, client, output) = desk();
    fullscreen(&mut f, client, OUTPUT, Some((0, 0, OUTPUT.0, OUTPUT.1 - 1)));

    assert_eq!(eligibility(&mut f, &output), not("translucent"));
}

#[test]
fn a_fullscreen_window_that_does_not_cover_its_output_is_not_eligible() {
    let (mut f, client, output) = desk();
    fullscreen(&mut f, client, (800, 600), Some((0, 0, 800, 600)));

    assert_eq!(eligibility(&mut f, &output), not("does-not-cover"));
}

#[test]
fn a_popup_over_the_fullscreen_window_takes_eligibility_until_it_goes() {
    let (mut f, client, output) = desk();
    let window = fullscreen(&mut f, client, OUTPUT, WHOLE);
    let positioner = Positioner {
        size: (300, 200),
        anchor_rect: (100, 100, 10, 10),
        anchor: Anchor::BottomRight,
        gravity: Gravity::BottomRight,
        constraint_adjustment: ConstraintAdjustment::None,
    };
    let popup = f.client(client).create_popup(window.xdg(), positioner);
    f.roundtrip(client);
    f.client(client).map_popup(popup, 0xffff_0000);
    f.roundtrip(client);

    assert_eq!(eligibility(&mut f, &output), drawn_over("popup"));

    f.client(client).destroy_popup(popup);
    f.roundtrip(client);

    assert_eq!(
        eligibility(&mut f, &output),
        Eligibility::Eligible(f.pid(window).0)
    );
}

#[test]
fn a_bubble_over_the_fullscreen_window_takes_eligibility_until_it_goes() {
    let (mut f, client, output) = desk();
    let window = fullscreen(&mut f, client, OUTPUT, WHOLE);
    let mut bubble = LayerSpec::floating(Layer::Overlay, (300, 100));
    bubble.namespace = "caminito-notification";
    let bubble = f.map_layer(client, bubble, 0xff20_2020);

    assert_eq!(eligibility(&mut f, &output), drawn_over("overlay"));

    f.client(client).unmap_layer_surface(bubble);
    f.roundtrip(client);

    assert_eq!(
        eligibility(&mut f, &output),
        Eligibility::Eligible(f.pid(window).0)
    );
}

#[test]
fn the_overview_takes_eligibility_until_it_closes() {
    let (mut f, client, output) = desk();
    let window = fullscreen(&mut f, client, OUTPUT, WHOLE);

    overview::open(&mut f);

    assert_eq!(eligibility(&mut f, &output), drawn_over("overview"));

    f.chord(&[KEY_ESC]);
    assert!(!overview::is_open(&f));

    assert_eq!(
        eligibility(&mut f, &output),
        Eligibility::Eligible(f.pid(window).0)
    );
}

#[test]
fn a_workspace_slide_takes_eligibility_until_it_ends() {
    let slide = vec![(CAMINITO, "workspace-slide".into(), Setting::Bool(true))];
    let (mut f, client, output) = desk_with(slide);
    let window = fullscreen(&mut f, client, OUTPUT, WHOLE);
    f.chord(&[KEY_LEFTCTRL, KEY_LEFTALT, KEY_RIGHT]);
    f.advance_clock(LENGTH);
    f.chord(&[KEY_LEFTCTRL, KEY_LEFTALT, KEY_LEFT]);

    assert_eq!(eligibility(&mut f, &output), drawn_over("workspace-slide"));

    f.advance_clock(LENGTH);

    assert_eq!(
        eligibility(&mut f, &output),
        Eligibility::Eligible(f.pid(window).0)
    );
}
