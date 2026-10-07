//! Marco's `button-layout` setting, parsed as `prefs.c`'s `button_layout_handler` does.

use crate::model::ButtonType;

/// What a titlebar button does (`MetaButtonFunction`).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum ButtonFunction {
    Menu,
    /// Parsed, and shown only for a window that allows an app menu, which in practice
    /// none does.
    Appmenu,
    Minimize,
    Maximize,
    Close,
    Shade,
    Above,
    Stick,
    Unshade,
    Unabove,
    Unstick,
}

impl ButtonFunction {
    pub const ALL: [ButtonFunction; 11] = [
        ButtonFunction::Menu,
        ButtonFunction::Appmenu,
        ButtonFunction::Minimize,
        ButtonFunction::Maximize,
        ButtonFunction::Close,
        ButtonFunction::Shade,
        ButtonFunction::Above,
        ButtonFunction::Stick,
        ButtonFunction::Unshade,
        ButtonFunction::Unabove,
        ButtonFunction::Unstick,
    ];

    /// A name a layout can use. The opposites have none: a toggle brings its own.
    fn from_layout_name(name: &str) -> Option<ButtonFunction> {
        Some(match name {
            "menu" => ButtonFunction::Menu,
            "appmenu" => ButtonFunction::Appmenu,
            "minimize" => ButtonFunction::Minimize,
            "maximize" => ButtonFunction::Maximize,
            "close" => ButtonFunction::Close,
            "shade" => ButtonFunction::Shade,
            "above" => ButtonFunction::Above,
            "stick" => ButtonFunction::Stick,
            _ => return None,
        })
    }

    /// The button that undoes a toggle, and the reverse (`button_opposite_function`).
    pub fn opposite(self) -> Option<ButtonFunction> {
        Some(match self {
            ButtonFunction::Shade => ButtonFunction::Unshade,
            ButtonFunction::Unshade => ButtonFunction::Shade,
            ButtonFunction::Above => ButtonFunction::Unabove,
            ButtonFunction::Unabove => ButtonFunction::Above,
            ButtonFunction::Stick => ButtonFunction::Unstick,
            ButtonFunction::Unstick => ButtonFunction::Stick,
            _ => return None,
        })
    }

    /// The button the theme draws for it (`map_button_function_to_type`).
    pub fn button_type(self) -> ButtonType {
        match self {
            ButtonFunction::Menu => ButtonType::Menu,
            ButtonFunction::Appmenu => ButtonType::Appmenu,
            ButtonFunction::Minimize => ButtonType::Minimize,
            ButtonFunction::Maximize => ButtonType::Maximize,
            ButtonFunction::Close => ButtonType::Close,
            ButtonFunction::Shade => ButtonType::Shade,
            ButtonFunction::Above => ButtonType::Above,
            ButtonFunction::Stick => ButtonType::Stick,
            ButtonFunction::Unshade => ButtonType::Unshade,
            ButtonFunction::Unabove => ButtonType::Unabove,
            ButtonFunction::Unstick => ButtonType::Unstick,
        }
    }
}

/// One entry of a side of the layout.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct LayoutButton {
    pub function: ButtonFunction,
    /// Whether a spacer, three quarters of a button wide, follows it.
    pub spacer: bool,
}

/// The buttons on each side of the titlebar, from the left.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ButtonLayout {
    pub left: Vec<LayoutButton>,
    pub right: Vec<LayoutButton>,
}

impl Default for ButtonLayout {
    /// Marco's default, `menu:minimize,maximize,close`.
    fn default() -> Self {
        ButtonLayout::parse("menu:minimize,maximize,close")
    }
}

impl ButtonLayout {
    /// Parses a `button-layout` string: the left side's buttons and the right side's,
    /// separated by a colon, each a comma-separated list of names and `spacer`s. Unknown
    /// and repeated names are ignored, as Marco ignores them. Marco keeps the previous
    /// layout's side when a side is missing; here it is empty. Marco also mirrors the
    /// layout for right-to-left languages, which is the caller's to do.
    pub fn parse(layout: &str) -> ButtonLayout {
        let mut sides = layout.splitn(2, ':');
        ButtonLayout {
            left: sides.next().map(parse_side).unwrap_or_default(),
            right: sides.next().map(parse_side).unwrap_or_default(),
        }
    }
}

fn parse_side(side: &str) -> Vec<LayoutButton> {
    let mut buttons: Vec<LayoutButton> = Vec::new();
    for name in side.split(',') {
        if name == "spacer" {
            // Marco then marks the button before that too if "spacer" had an opposite,
            // which it never does.
            if let Some(last) = buttons.last_mut() {
                last.spacer = true;
            }
            continue;
        }
        let Some(function) = ButtonFunction::from_layout_name(name) else {
            continue;
        };
        if buttons.iter().any(|button| button.function == function) {
            continue;
        }
        buttons.push(LayoutButton {
            function,
            spacer: false,
        });
        if let Some(opposite) = function.opposite() {
            buttons.push(LayoutButton {
                function: opposite,
                spacer: false,
            });
        }
    }
    buttons
}

#[cfg(test)]
mod tests {
    use super::*;

    use ButtonFunction::*;

    fn side(buttons: &[(ButtonFunction, bool)]) -> Vec<LayoutButton> {
        buttons
            .iter()
            .map(|&(function, spacer)| LayoutButton { function, spacer })
            .collect()
    }

    #[test]
    fn marcos_default_layout() {
        let layout = ButtonLayout::parse("menu:minimize,maximize,close");
        assert_eq!(layout.left, side(&[(Menu, false)]));
        assert_eq!(
            layout.right,
            side(&[(Minimize, false), (Maximize, false), (Close, false)])
        );
        assert_eq!(ButtonLayout::default(), layout);
    }

    #[test]
    fn a_toggle_is_followed_by_its_opposite() {
        let layout = ButtonLayout::parse("shade,above,stick:close");
        assert_eq!(
            layout.left,
            side(&[
                (Shade, false),
                (Unshade, false),
                (Above, false),
                (Unabove, false),
                (Stick, false),
                (Unstick, false),
            ])
        );
    }

    #[test]
    fn a_spacer_follows_the_button_before_it() {
        let layout = ButtonLayout::parse("menu,spacer,appmenu:minimize,spacer,spacer,close");
        assert_eq!(layout.left, side(&[(Menu, true), (Appmenu, false)]));
        assert_eq!(layout.right, side(&[(Minimize, true), (Close, false)]));
    }

    #[test]
    fn a_leading_spacer_is_ignored() {
        let layout = ButtonLayout::parse("spacer,menu:");
        assert_eq!(layout.left, side(&[(Menu, false)]));
    }

    // Marco marks the opposite, which only shows when the window is toggled, so the
    // spacer after an untoggled toggle is lost.
    #[test]
    fn a_spacer_after_a_toggle_follows_its_opposite() {
        let layout = ButtonLayout::parse("shade,spacer,menu:");
        assert_eq!(
            layout.left,
            side(&[(Shade, false), (Unshade, true), (Menu, false)])
        );
    }

    #[test]
    fn unknown_and_repeated_names_are_ignored() {
        let layout = ButtonLayout::parse("menu,help,unshade,menu, close:close,Close");
        assert_eq!(layout.left, side(&[(Menu, false)]));
        assert_eq!(layout.right, side(&[(Close, false)]));
    }

    #[test]
    fn a_side_may_repeat_the_other_sides_buttons() {
        let layout = ButtonLayout::parse("close:close");
        assert_eq!(layout.left, side(&[(Close, false)]));
        assert_eq!(layout.right, side(&[(Close, false)]));
    }

    // Marco keeps the previous layout's missing side; Caminito has no previous layout.
    #[test]
    fn a_missing_side_is_empty() {
        assert_eq!(ButtonLayout::parse("menu").right, []);
        assert_eq!(ButtonLayout::parse("").left, []);
        assert_eq!(ButtonLayout::parse(":close").left, []);
    }

    #[test]
    fn only_the_first_colon_separates_the_sides() {
        let layout = ButtonLayout::parse("menu:minimize:close");
        assert_eq!(layout.right, side(&[]));
        let layout = ButtonLayout::parse("menu:minimize,close:x");
        assert_eq!(layout.right, side(&[(Minimize, false)]));
    }
}
