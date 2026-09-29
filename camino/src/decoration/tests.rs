use camino_policy::{FrameState, SizeHints, WindowType};
use camino_theme::ThemeDirs;
use smithay::utils::FrameExtents;

use super::{FrameContext, WindowFacts, WindowFrame};
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
        has_parent: false,
        hints: SizeHints::default(),
        wants_frame: true,
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
            shaded: false,
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
        assert_eq!(
            frame.extents(FrameState::default()),
            FrameExtents::default()
        );
    }
    assert!(normal.is_ssd());
    assert!(normal.extents(FrameState::default()).top > 0);
}

/// The ARGB32 word at the middle of `surface`.
fn center(surface: &camino_theme::cairo::ImageSurface) -> u32 {
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
