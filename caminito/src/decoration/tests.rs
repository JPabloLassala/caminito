use caminito_policy::{FrameState, Functions, WindowType};
use caminito_theme::{Palette, ThemeDirs, Variant};
use smithay::utils::{FrameExtents, Logical, Rectangle};

use super::{FrameContent, FrameContext, IconSource, WindowFacts, WindowFrame};
use crate::settings::translate::{Reading, translate};

/// A frame context with `theme` from the themes vendored for the goldens.
fn context(theme: &str) -> FrameContext {
    let mut settings = translate(&Reading::default()).settings;
    settings.frames.theme = theme.into();
    let dirs = ThemeDirs::new(vec![
        concat!(env!("CARGO_MANIFEST_DIR"), "/../tests").into(),
    ]);
    FrameContext::new(&settings, dirs)
}

fn facts(window_type: WindowType) -> WindowFacts {
    WindowFacts {
        window_type,
        wants_frame: true,
    }
}

/// A window that resizes both ways, not focused: the state policy asks for the extents of
/// a normal window in.
const UNFOCUSED: FrameState = FrameState {
    focused: false,
    ..FOCUSED
};

/// The functions policy sends with `state`, for a window that has the others.
fn functions(state: FrameState) -> Functions {
    Functions {
        minimize: true,
        maximize: true,
        resize_horizontally: state.resizes_horizontally,
        resize_vertically: state.resizes_vertically,
        fullscreen: true,
        shade: !state.border_only,
        border_only: state.border_only,
    }
}

fn frame(context: &FrameContext, facts: WindowFacts) -> WindowFrame {
    let mut frame = WindowFrame::default();
    frame.set_facts(facts, context);
    frame
}

#[test]
fn an_eos_maximised_utility_frame_has_side_borders_only_while_unfocused() {
    let context = context("eOS");
    let frame = frame(&context, facts(WindowType::Utility));
    let maximized = |focused| {
        frame.extents(FrameState {
            maximized: true,
            focused,
            ..UNFOCUSED
        })
    };

    let (focused, unfocused) = (maximized(true), maximized(false));

    assert_eq!((focused.left, focused.right, focused.bottom), (0, 0, 0));
    assert_eq!(
        (unfocused.left, unfocused.right, unfocused.bottom),
        (1, 1, 1)
    );
    assert_eq!(focused.top, unfocused.top);
}

#[test]
fn a_pending_window_is_framed_as_it_will_be_once_mapped() {
    let context = context("eOS");
    let facts = facts(WindowType::Utility);
    let frame = frame(&context, facts);

    for (maximized, focused, shaded) in [(false, true, false), (true, false, false)] {
        let state = FrameState {
            maximized,
            focused,
            shaded,
            ..UNFOCUSED
        };
        assert_eq!(context.extents(&facts, state), frame.extents(state));
    }
}

#[test]
fn a_csd_window_and_a_dock_have_no_frame() {
    let context = context("Menta");
    let csd = frame(
        &context,
        WindowFacts {
            wants_frame: false,
            ..facts(WindowType::Normal)
        },
    );
    let dock = frame(&context, facts(WindowType::Dock));
    let normal = frame(&context, facts(WindowType::Normal));

    for frame in [&csd, &dock] {
        assert!(!frame.is_ssd());
        assert_eq!(frame.extents(UNFOCUSED), FrameExtents::default());
    }
    assert!(normal.is_ssd());
    assert!(normal.extents(UNFOCUSED).top > 0);
}

#[test]
fn a_frame_thats_a_border_only_is_measured_and_drawn_attached() {
    let mut context = context("Menta");
    let mut frame = frame(&context, facts(WindowType::ModalDialog));
    let attached = FrameState {
        border_only: true,
        resizes_horizontally: false,
        resizes_vertically: false,
        ..FOCUSED
    };

    assert!(frame.extents(attached).top < frame.extents(FOCUSED).top);

    draw(&mut frame, &mut context, attached, 1);
    assert_eq!(
        frame.frame_type(),
        Some(caminito_theme::FrameType::Attached)
    );
    draw(&mut frame, &mut context, FOCUSED, 1);
    assert_eq!(
        frame.frame_type(),
        Some(caminito_theme::FrameType::ModalDialog)
    );
}

#[test]
fn every_state_policy_can_ask_for_has_the_extents_its_style_gives() {
    let context = context("eOS");
    let facts = facts(WindowType::Utility);
    let frame = frame(&context, facts);
    for index in 0..1 << 8 {
        let bit = |n: usize| index & (1 << n) != 0;
        let state = FrameState {
            maximized: bit(0),
            focused: bit(1),
            shaded: bit(2),
            tiled_left: bit(3),
            tiled_right: bit(4) && !bit(3),
            border_only: bit(5),
            resizes_horizontally: bit(6),
            resizes_vertically: bit(7),
        };
        assert_eq!(context.extents(&facts, state), frame.extents(state));
    }
}

/// The ARGB32 word at the middle of `surface`.
fn center(surface: &caminito_theme::cairo::ImageSurface) -> u32 {
    let (x, y) = (surface.width() / 2, surface.height() / 2);
    let offset = (y * surface.stride() + x * 4) as usize;
    let mut pixel = 0;
    surface
        .with_data(|data| pixel = u32::from_ne_bytes(data[offset..offset + 4].try_into().unwrap()))
        .unwrap();
    pixel
}

#[test]
fn net_wm_icon_takes_the_smallest_image_at_least_as_big_and_premultiplies_it() {
    let mut value = vec![2, 2];
    value.extend([0xff00_ff00; 4]);
    value.extend([32, 32]);
    // Red at half opacity.
    value.extend([0x80ff_0000; 32 * 32]);
    value.extend([64, 64]);
    value.extend([0xff00_00ff; 64 * 64]);
    let icon = super::NetWmIcon::parse(&value).unwrap();

    let drawn = super::icon_from_net_wm_icon(&icon, 16).unwrap();

    assert_eq!((drawn.width(), drawn.height()), (16, 16));
    assert_eq!(center(&drawn), 0x8080_0000);
}

#[test]
fn net_wm_icon_leaves_out_a_truncated_image() {
    let mut value = vec![4, 4];
    value.extend([0xffff_ffff; 16]);
    value.extend([16, 16]);
    value.extend([0; 10]);
    let icon = super::NetWmIcon::parse(&value).unwrap();

    let drawn = super::icon_from_net_wm_icon(&icon, 16).unwrap();
    assert_eq!(center(&drawn), 0xffff_ffff);

    assert!(super::NetWmIcon::parse(&[16, 16, 0]).is_none());
    assert!(super::NetWmIcon::parse(&[]).is_none());
}

#[test]
fn a_side_tiled_frame_is_measured_in_the_themes_tiled_styles() {
    // Menta's tiled geometries are its maximised one with a border of 1 on the inner side.
    let context = context("Menta");
    let frame = frame(&context, facts(WindowType::Normal));
    let sides = |state: FrameState| {
        let extents = frame.extents(state);
        (extents.left, extents.right, extents.bottom)
    };
    let focused = FrameState {
        focused: true,
        ..UNFOCUSED
    };

    let left = sides(FrameState {
        tiled_left: true,
        ..focused
    });
    let right = sides(FrameState {
        tiled_right: true,
        ..focused
    });

    assert_eq!((left, right), ((0, 1, 0), (1, 0, 0)));
    assert_ne!(
        sides(focused),
        left,
        "a floating frame has borders of its own"
    );
}

/// Draws `frame` around a window 400 by 300, at the integer `scale`.
fn draw(frame: &mut WindowFrame, context: &mut FrameContext, state: FrameState, scale: i32) {
    draw_sized(frame, context, state, scale, (400, 300), "a window");
}

fn draw_sized(
    frame: &mut WindowFrame,
    context: &mut FrameContext,
    state: FrameState,
    scale: i32,
    size: (i32, i32),
    title: &str,
) {
    let palette = Palette::neutral();
    let content = FrameContent {
        size: size.into(),
        title: title.into(),
        icon: IconSource::AppId(None),
        scale,
        flashing: false,
        above: false,
        stuck: false,
    };
    let colors = palette.variant(Variant::Default);
    frame.refresh(context, colors, state, functions(state), content);
}

/// `visible` grown by this much on each side.
fn grown(
    visible: Rectangle<i32, Logical>,
    (left, top, right, bottom): (i32, i32, i32, i32),
) -> Rectangle<i32, Logical> {
    Rectangle::new(
        (visible.loc.x - left, visible.loc.y - top).into(),
        (visible.size.w + left + right, visible.size.h + top + bottom).into(),
    )
}

const FOCUSED: FrameState = FrameState {
    maximized: false,
    focused: true,
    shaded: false,
    tiled_left: false,
    tiled_right: false,
    border_only: false,
    resizes_horizontally: true,
    resizes_vertically: true,
};

#[test]
fn a_frame_of_each_type_has_marcos_shadow_for_its_focus_at_scales_1_and_1_5_and_2() {
    let mut context = context("Menta");
    for window_type in [
        WindowType::Normal,
        WindowType::Dialog,
        WindowType::ModalDialog,
        WindowType::Menu,
        WindowType::Utility,
    ] {
        for focused in [true, false] {
            // Marco's offsets: the large shadow for the focus, the medium one without.
            let reach = if focused {
                (15, 15, 21, 21)
            } else {
                (9, 7, 9, 11)
            };
            for scale in [1.0_f64, 1.5, 2.0] {
                // A frame is drawn at its output's scale rounded up (13), and so is its
                // shadow.
                let drawn = scale.ceil() as i32;
                let mut frame = frame(&context, facts(window_type));
                let state = FrameState { focused, ..FOCUSED };
                draw(&mut frame, &mut context, state, drawn);

                let visible = frame.visible().unwrap();
                let extents = frame.extents(state);
                assert_eq!(
                    visible,
                    grown(
                        Rectangle::from_size((400, 300).into()),
                        (extents.left, extents.top, extents.right, extents.bottom)
                    )
                );
                let shadow = frame.shadow_rect().unwrap();
                let what = format!("{window_type:?}, focused {focused}, at {scale}");
                assert_eq!(shadow, grown(visible, reach), "{what}");

                // The bands are the shadow without the frame's rows between its corners.
                let bands: Vec<_> = frame.shadow().collect();
                assert!(!bands.is_empty(), "{what}");
                for (rect, _) in &bands {
                    assert!(shadow.contains_rect(*rect), "{what}");
                }
                let area: i32 = bands.iter().map(|(r, _)| r.size.w * r.size.h).sum();
                assert!(area >= shadow.size.w * shadow.size.h - visible.size.w * visible.size.h);
                assert!(area < shadow.size.w * shadow.size.h);
            }
        }
    }
}

#[test]
fn a_shaded_frames_shadow_follows_its_titlebar() {
    let mut context = context("Menta");
    let mut frame = frame(&context, facts(WindowType::Normal));
    let state = FrameState {
        shaded: true,
        ..FOCUSED
    };
    draw(&mut frame, &mut context, state, 1);

    let visible = frame.visible().unwrap();
    assert!(
        visible.size.h < 100,
        "the frame is its titlebar: {visible:?}"
    );
    assert_eq!(frame.shadow_rect(), Some(grown(visible, (15, 15, 21, 21))));
}

#[test]
fn a_maximised_or_side_tiled_frame_and_a_csd_window_have_no_shadow() {
    let mut context = context("Menta");
    for state in [
        FrameState {
            maximized: true,
            ..FOCUSED
        },
        FrameState {
            tiled_left: true,
            ..FOCUSED
        },
        FrameState {
            tiled_right: true,
            ..FOCUSED
        },
    ] {
        let mut frame = frame(&context, facts(WindowType::Normal));
        draw(&mut frame, &mut context, state, 1);
        assert!(frame.visible().is_some(), "{state:?} has a frame");
        assert_eq!(frame.shadow_rect(), None, "{state:?}");
        assert_eq!(frame.shadow().count(), 0);
    }

    let csd = WindowFacts {
        wants_frame: false,
        ..facts(WindowType::Normal)
    };
    let mut frame = frame(&context, csd);
    draw(&mut frame, &mut context, FOCUSED, 1);
    assert_eq!(frame.shadow_rect(), None);
}

#[test]
fn a_frame_of_the_same_size_draws_no_new_shadow() {
    let mut context = context("Menta");
    let mut frame = frame(&context, facts(WindowType::Normal));
    draw(&mut frame, &mut context, FOCUSED, 1);
    assert_eq!((frame.renders(), frame.shadow_renders()), (1, 1));

    // A new title draws the strips again, and the shadow is the one it had.
    draw_sized(&mut frame, &mut context, FOCUSED, 1, (400, 300), "another");
    assert_eq!((frame.renders(), frame.shadow_renders()), (2, 1));

    draw_sized(&mut frame, &mut context, FOCUSED, 1, (401, 300), "another");
    assert_eq!(frame.shadow_renders(), 2, "a new size");
    draw_sized(&mut frame, &mut context, FOCUSED, 2, (401, 300), "another");
    assert_eq!(frame.shadow_renders(), 3, "a new scale");
    let unfocused = FrameState {
        focused: false,
        ..FOCUSED
    };
    draw_sized(
        &mut frame,
        &mut context,
        unfocused,
        2,
        (401, 300),
        "another",
    );
    assert_eq!(frame.shadow_renders(), 4, "the other focus");
}

#[test]
fn frames_lose_their_shadows_with_the_compositing_manager_off() {
    let mut settings = translate(&Reading::default()).settings;
    settings.frames.theme = "Menta".into();
    let mut context = context("Menta");
    let mut frame = frame(&context, facts(WindowType::Normal));
    draw(&mut frame, &mut context, FOCUSED, 1);
    assert!(frame.shadow_rect().is_some());

    settings.frames.shadows = false;
    assert!(context.settings_changed(&settings));
    draw(&mut frame, &mut context, FOCUSED, 1);
    assert_eq!(frame.shadow_rect(), None);

    settings.frames.shadows = true;
    assert!(context.settings_changed(&settings));
    draw(&mut frame, &mut context, FOCUSED, 1);
    assert!(frame.shadow_rect().is_some());
}
