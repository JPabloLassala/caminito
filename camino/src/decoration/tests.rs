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
