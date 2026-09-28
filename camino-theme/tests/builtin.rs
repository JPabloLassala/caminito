//! The theme compiled into Camino, for when the configured one fails (07, 16).

use camino_theme::{
    ButtonState, ButtonType, FrameFocus, FrameResize, FrameState, FrameType, Theme, ThemeDirs,
    ThemeError,
};

#[test]
fn the_builtin_theme_loads_with_no_theme_directories() {
    let no_dirs = ThemeDirs::new(Vec::new());
    assert!(matches!(
        Theme::load("Menta", &no_dirs),
        Err(ThemeError::NotFound { .. })
    ));

    let theme = Theme::builtin();
    assert_eq!(theme.name, "Camino");
    for frame_type in FrameType::ALL {
        for state in FrameState::ALL {
            for focus in FrameFocus::ALL {
                let style = theme.frame_style(frame_type, state, FrameResize::Both, focus);
                for button in [
                    ButtonType::Close,
                    ButtonType::Maximize,
                    ButtonType::Minimize,
                ] {
                    assert!(style.button(button, ButtonState::Prelight).is_some());
                }
            }
        }
    }
}
