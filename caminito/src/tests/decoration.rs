//! SSD frames (07, 30): decoration-mode negotiation, frame extents in placement and
//! maximise, when strips are drawn again, and the scale they're drawn at. These run with
//! xdg toplevels only: the negotiation is xdg-decoration's and KDE's, and frames.rs runs
//! what a frame does on both kinds (45).

use caminito_policy::{FrameState, PolicyWindow};
use caminito_test_client::{DecorationMode, KdeMode, format_configures};
use caminito_theme::{ButtonStates, Frame, FrameFlags, FrameType, Strip, Variant};
use insta::assert_snapshot;
use smithay::utils::{FrameExtents, Point, Rectangle, Size};

use super::fixture::{
    BTN_LEFT, ClientWindow, Decoration, Fixture, KEY_LEFTALT, Kind, WINDOW_COLOR, WindowSpec,
};
use crate::backend::headless::argb8888;
use crate::scene::CLEAR_COLOR;
use crate::settings::Value;
use crate::settings::translate::MARCO_GENERAL;

/// A window of `size` that asks for a frame.
fn ssd(size: (i32, i32)) -> WindowSpec {
    WindowSpec {
        size,
        ..WindowSpec::default()
    }
}

fn configures(f: &mut Fixture, window: ClientWindow) -> String {
    format_configures(&f.client(window.client()).window(window.xdg()).configures)
}

#[test]
fn xdg_decoration_grants_server_side_when_asked() {
    let (mut f, id) = Fixture::connected(Kind::Xdg);
    let window = f.map(id, ssd((200, 100)));

    assert!(f.mapped(window).is_ssd());
    assert_snapshot!(configures(&mut f, window));
}

#[test]
fn xdg_decoration_grants_client_side_when_asked() {
    let (mut f, id) = Fixture::connected(Kind::Xdg);
    let window = f.map(
        id,
        WindowSpec {
            decoration: Decoration::Client,
            ..ssd((200, 100))
        },
    );

    assert!(!f.mapped(window).is_ssd());
    assert_snapshot!(configures(&mut f, window));
}

#[test]
fn xdg_decoration_answers_server_side_to_a_client_without_a_preference() {
    let (mut f, id) = Fixture::connected(Kind::Xdg);
    let window = f.map(
        id,
        WindowSpec {
            decoration: Decoration::NoPreference,
            ..ssd((200, 100))
        },
    );

    assert!(f.mapped(window).is_ssd());
    assert_snapshot!(configures(&mut f, window));
}

#[test]
fn a_mapped_window_changes_mode_when_it_asks_and_when_it_unsets_its_mode() {
    let (mut f, id) = Fixture::connected(Kind::Xdg);
    let window = f.map(
        id,
        WindowSpec {
            decoration: Decoration::Client,
            ..ssd((200, 100))
        },
    );
    let xdg = window.xdg();

    f.client(id)
        .request_decoration(xdg, Some(DecorationMode::ServerSide));
    f.roundtrip(id);
    f.client(id)
        .ack_and_commit_buffer(xdg, (200, 100), WINDOW_COLOR);
    f.roundtrip(id);
    assert!(f.mapped(window).is_ssd());

    f.client(id)
        .request_decoration(xdg, Some(DecorationMode::ClientSide));
    f.roundtrip(id);
    f.client(id)
        .ack_and_commit_buffer(xdg, (200, 100), WINDOW_COLOR);
    f.roundtrip(id);
    assert!(!f.mapped(window).is_ssd());

    f.client(id).request_decoration(xdg, None);
    f.roundtrip(id);
    f.client(id)
        .ack_and_commit_buffer(xdg, (200, 100), WINDOW_COLOR);
    f.roundtrip(id);
    assert!(f.mapped(window).is_ssd());

    assert_snapshot!(configures(&mut f, window));
}

#[test]
fn a_window_without_a_decoration_object_stays_client_side() {
    let (mut f, id) = Fixture::connected(Kind::Xdg);
    let window = f.map(id, WindowSpec::plain((200, 100)));

    assert!(!f.mapped(window).is_ssd());
}

#[test]
fn kde_server_decoration_defaults_to_server_side_and_grants_what_is_asked() {
    let (mut f, id) = Fixture::connected(Kind::Xdg);
    // KDE's protocol is what's under test, so the window asks through it by hand.
    let xdg = f.client(id).create_window();
    f.client(id).kde_request_decoration(xdg, None);
    f.client(id).commit(xdg);
    f.roundtrip(id);
    f.client(id)
        .ack_and_commit_buffer(xdg, (200, 100), WINDOW_COLOR);
    f.roundtrip(id);
    let window = ClientWindow::Xdg(id, xdg);

    assert_eq!(f.client(id).state.kde_default_mode, Some(KdeMode::Server));
    assert_eq!(f.client(id).window(xdg).kde_modes, [KdeMode::Server]);
    assert!(f.mapped(window).is_ssd());

    f.client(id)
        .kde_request_decoration(xdg, Some(KdeMode::Client));
    f.roundtrip(id);
    assert!(!f.mapped(window).is_ssd());

    f.client(id)
        .kde_request_decoration(xdg, Some(KdeMode::Server));
    f.roundtrip(id);
    assert!(f.mapped(window).is_ssd());
    assert_eq!(
        f.client(id).window(xdg).kde_modes,
        [KdeMode::Server, KdeMode::Client, KdeMode::Server]
    );
}

#[test]
fn a_frame_is_placed_where_a_window_of_the_frames_size_would_be() {
    let (mut f, id) = Fixture::connected(Kind::Xdg);
    let window = f.map(id, ssd((200, 100)));
    let e = f.frame_extents(window);
    assert!(e.top > 0, "the frame has a titlebar");
    let geometry = f.geometry(window);

    let (mut csd, csd_id) = Fixture::connected(Kind::Xdg);
    let size = (200 + e.left + e.right, 100 + e.top + e.bottom);
    let csd_window = csd.map(csd_id, WindowSpec::plain(size));
    let frame = csd.geometry(csd_window);

    assert_eq!(
        geometry,
        Rectangle::new(
            frame.loc + Point::from((e.left, e.top)),
            Size::from((200, 100))
        )
    );
}

#[test]
fn maximising_fits_the_maximised_frame_in_the_work_area() {
    let (mut f, id) = Fixture::connected(Kind::Xdg);
    let window = f.map(id, ssd((200, 100)));
    let FrameExtents {
        left,
        right,
        top,
        bottom,
        ..
    } = f.mapped(window).frame_extents(FrameState {
        maximized: true,
        focused: true,
        ..FrameState::default()
    });

    f.client(id).set_maximized(window.xdg());
    f.roundtrip(id);
    let size = (1920 - left - right, 1080 - top - bottom);
    f.client(id)
        .ack_and_commit_buffer(window.xdg(), size, WINDOW_COLOR);
    f.roundtrip(id);

    assert_eq!(
        f.geometry(window),
        Rectangle::new((left, top).into(), size.into())
    );
    assert_snapshot!(configures(&mut f, window));
}

#[test]
fn strips_are_drawn_again_only_when_an_input_changes() {
    let (mut f, id) = Fixture::connected(Kind::Xdg);
    let window = f.map(id, ssd((200, 100)));
    let xdg = window.xdg();
    let renders = |f: &Fixture| f.mapped(window).frame().renders();
    assert_eq!(renders(&f), 1);

    // Nothing changed.
    f.client(id).commit(xdg);
    f.roundtrip(id);
    assert_eq!(renders(&f), 1);

    f.client(id).set_title(xdg, "A new title");
    f.client(id).commit(xdg);
    f.roundtrip(id);
    assert_eq!(renders(&f), 2);

    f.client(id)
        .ack_and_commit_buffer(xdg, (300, 150), WINDOW_COLOR);
    f.roundtrip(id);
    assert_eq!(renders(&f), 3);

    // Another window takes the focus.
    f.map(id, WindowSpec::plain((200, 100)));
    assert_eq!(renders(&f), 4);
}

/// The scale an SSD window's strips are drawn at, on one output at `scale`.
fn strip_scale_on(scale: f64) -> i32 {
    let mut f = Fixture::new();
    f.add_output((1920, 1080), scale);
    let id = f.connect(Kind::Xdg);
    let window = f.map(id, ssd((200, 100)));
    f.mapped(window).frame().scale()
}

#[test]
fn strips_are_drawn_at_the_output_scale_rounded_up() {
    assert_eq!(strip_scale_on(1.0), 1);
    assert_eq!(strip_scale_on(1.5), 2);
    assert_eq!(strip_scale_on(2.0), 2);
}

#[test]
fn a_window_across_two_outputs_is_drawn_at_the_larger_scale() {
    let mut f = Fixture::new();
    f.add_output((1920, 1080), 1.0);
    f.add_output((1920, 1080), 1.5);
    let id = f.connect(Kind::Xdg);
    f.client(id).bind_input();
    let window = f.map(id, ssd((400, 300)));
    assert_eq!(f.mapped(window).frame().scale(), 1);

    // Alt+drag it to straddle the outputs' shared edge.
    let start = f.geometry(window);
    let (x, y) = (start.loc.x + 10, start.loc.y + 10);
    f.pointer_move(f64::from(x), f64::from(y));
    f.key(KEY_LEFTALT, true);
    f.pointer_button(BTN_LEFT, true);
    f.pointer_move(f64::from(1920 - 200 + 10), f64::from(y));
    f.pointer_button(BTN_LEFT, false);
    f.key(KEY_LEFTALT, false);
    let moved = f.geometry(window);
    assert!(moved.loc.x < 1920 && moved.loc.x + moved.size.w > 1920);
    assert_eq!(f.mapped(window).frame().scale(), 2);

    // And wholly back onto the first.
    f.key(KEY_LEFTALT, true);
    f.pointer_button(BTN_LEFT, true);
    f.pointer_move(f64::from(x), f64::from(y));
    f.pointer_button(BTN_LEFT, false);
    f.key(KEY_LEFTALT, false);
    assert_eq!(f.mapped(window).frame().scale(), 1);
}

#[test]
fn maximising_before_mapping_configures_the_size_inside_the_frame() {
    let (mut f, id) = Fixture::connected(Kind::Xdg);
    let window = f.client(id).create_window();
    f.client(id)
        .request_decoration(window, Some(DecorationMode::ServerSide));
    f.client(id).set_maximized(window);
    f.client(id).commit(window);
    f.roundtrip(id);

    assert_snapshot!(format_configures(&f.client(id).window(window).configures));
}

#[test]
fn a_framed_window_is_drawn_with_the_themes_strips_around_it() {
    let (mut f, id) = Fixture::connected(Kind::Xdg);
    let output = f.state.caminito.outputs[0].clone();
    let window = f.map(
        id,
        WindowSpec {
            title: Some("Frame"),
            ..ssd((300, 200))
        },
    );
    let geometry = f.geometry(window);
    let pixels = f.state.backend.headless_mut().pixels(&output).unwrap();
    let clear = argb8888(CLEAR_COLOR);

    // The strips caminito-theme draws for a focused normal window that can do everything.
    let caminito = &f.state.caminito;
    let frame_context = &caminito.frame_context;
    let flags = FrameFlags {
        allows_delete: true,
        allows_menu: true,
        allows_minimize: true,
        allows_maximize: true,
        allows_vertical_resize: true,
        allows_horizontal_resize: true,
        has_focus: true,
        allows_shade: true,
        allows_move: true,
        ..FrameFlags::default()
    };
    let title = frame_context.title(FrameType::Normal, &flags, "Frame");
    let frame = Frame {
        frame_type: FrameType::Normal,
        flags,
        client_width: 300,
        client_height: 200,
        title: &title,
        button_layout: frame_context.button_layout(),
        button_states: ButtonStates::default(),
        mini_icon: None,
        icon: None,
        colors: caminito.palette.palette().variant(Variant::Default),
    };
    let theme = frame_context.theme();
    let frame_geometry = theme.frame_geometry(
        FrameType::Normal,
        title.text_height,
        &flags,
        300,
        200,
        frame_context.button_layout(),
    );

    for strip in Strip::ALL {
        let mut expected = theme.render_strip(&frame, strip, 1).unwrap();
        let rect = frame_geometry.strip_rect(strip);
        let total = frame_geometry.borders.total;
        let stride = expected.stride() as usize;
        let (width, height) = (expected.width(), expected.height());
        let data = expected.data().unwrap();
        for y in 0..height {
            for x in 0..width {
                let i = y as usize * stride + x as usize * 4;
                let src = u32::from_ne_bytes(data[i..i + 4].try_into().unwrap());
                let want = over(src, clear);
                let gx = geometry.loc.x - total.left + rect.x + x;
                let gy = geometry.loc.y - total.top + rect.y + y;
                let got = pixels[(gy * 1920 + gx) as usize];
                assert!(
                    close(got, want),
                    "{strip:?} at ({x}, {y}): drew {got:08x}, the theme drew {want:08x}"
                );
            }
        }
    }
}

/// Premultiplied `src` over opaque `dst`.
fn over(src: u32, dst: u32) -> u32 {
    let alpha = src >> 24;
    (0..4).fold(0, |pixel, i| {
        let shift = i * 8;
        let s = (src >> shift) & 0xff;
        let d = (dst >> shift) & 0xff;
        pixel | (s + (d * (255 - alpha) + 127) / 255).min(255) << shift
    })
}

/// Equal but for rounding in each channel.
fn close(a: u32, b: u32) -> bool {
    (0..4).all(|i| {
        let channel = |p: u32| ((p >> (i * 8)) & 0xff) as i32;
        (channel(a) - channel(b)).abs() <= 1
    })
}

#[test]
fn a_new_theme_font_or_button_layout_applies_to_open_windows() {
    let (mut f, id) = Fixture::connected(Kind::Xdg);
    let window = f.map(id, ssd((200, 100)));
    let mapped = |f: &Fixture| (f.mapped(window).frame().renders(), f.frame_extents(window));
    let (_, menta) = mapped(&f);

    f.set_setting(MARCO_GENERAL, "theme", Value::Str("Atlanta".into()));
    f.dispatch_until(|f| mapped(f).0 == 2);
    assert_eq!(f.state.caminito.frame_context.theme().name, "Atlanta");
    let (_, atlanta) = mapped(&f);
    assert_ne!(atlanta, menta);

    f.set_setting(
        MARCO_GENERAL,
        "titlebar-font",
        Value::Str("Sans Bold 30".into()),
    );
    f.dispatch_until(|f| mapped(f).0 == 3);
    assert!(mapped(&f).1.top > atlanta.top);

    f.set_setting(MARCO_GENERAL, "button-layout", Value::Str("close:".into()));
    f.dispatch_until(|f| mapped(f).0 == 4);
}

#[test]
fn a_theme_that_fails_to_load_falls_back_to_the_built_in_one() {
    let (mut f, id) = Fixture::connected(Kind::Xdg);
    let window = f.map(id, ssd((200, 100)));

    f.set_setting(MARCO_GENERAL, "theme", Value::Str("NoSuchTheme".into()));
    f.dispatch_until(|f| f.state.caminito.frame_context.theme().name == "Caminito");

    assert!(f.mapped(window).frame().strips().count() > 0);
}
