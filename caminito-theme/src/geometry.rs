//! Where every part of a frame goes, as Marco computes it (`theme.c:514-1100`).

use std::sync::Arc;

use crate::button_layout::{ButtonFunction, ButtonLayout, LayoutButton};
use crate::model::{
    Border, ButtonSizing, FrameFocus, FrameLayout, FrameResize, FrameState, FrameStyle, FrameType,
    Theme,
};

/// A rectangle in logical pixels (`GdkRectangle`).
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
pub struct Rect {
    pub x: i32,
    pub y: i32,
    pub width: i32,
    pub height: i32,
}

/// What a window allows and what state it's in (`MetaFrameFlags`).
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
pub struct FrameFlags {
    pub allows_delete: bool,
    pub allows_menu: bool,
    pub allows_appmenu: bool,
    pub allows_minimize: bool,
    pub allows_maximize: bool,
    pub allows_vertical_resize: bool,
    pub allows_horizontal_resize: bool,
    pub has_focus: bool,
    pub shaded: bool,
    pub stuck: bool,
    pub maximized: bool,
    pub allows_shade: bool,
    pub allows_move: bool,
    pub fullscreen: bool,
    pub is_flashing: bool,
    pub above: bool,
    pub tiled_left: bool,
    pub tiled_right: bool,
}

impl FrameFlags {
    /// The state the frame's style is chosen by (`theme_get_style`). Tiling wins over
    /// maximizing, as in Marco. Marco asserts on a window tiled both ways, or tiled,
    /// maximized and shaded; here the left tile wins, and such a window is tiled and
    /// shaded.
    pub fn state(&self) -> FrameState {
        match (
            self.tiled_left,
            self.tiled_right,
            self.maximized,
            self.shaded,
        ) {
            (true, _, _, false) => FrameState::TiledLeft,
            (true, _, _, true) => FrameState::TiledLeftAndShaded,
            (false, true, _, false) => FrameState::TiledRight,
            (false, true, _, true) => FrameState::TiledRightAndShaded,
            (false, false, true, false) => FrameState::Maximized,
            (false, false, true, true) => FrameState::MaximizedAndShaded,
            (false, false, false, false) => FrameState::Normal,
            (false, false, false, true) => FrameState::Shaded,
        }
    }

    pub fn resize(&self) -> FrameResize {
        match (self.allows_vertical_resize, self.allows_horizontal_resize) {
            (false, false) => FrameResize::None,
            (true, false) => FrameResize::Vertical,
            (false, true) => FrameResize::Horizontal,
            (true, true) => FrameResize::Both,
        }
    }

    /// A flashing frame swaps its focused and unfocused styles.
    pub fn focus(&self) -> FrameFocus {
        if self.has_focus != self.is_flashing {
            FrameFocus::Yes
        } else {
            FrameFocus::No
        }
    }
}

/// A frame's borders around the client (`MetaFrameBorders`).
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
pub struct FrameBorders {
    /// What the theme draws.
    pub visible: Border,
    /// The resize area outside it, on the sides the window can be resized from.
    pub invisible: Border,
    pub total: Border,
}

/// A button's place (`MetaButtonSpace`).
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
pub struct ButtonSpace {
    /// Where it's drawn.
    pub visible: Rect,
    /// Where it takes clicks: the visible rect, extended to the screen's corner for the
    /// outermost button of a maximized or tiled window (Fitts's law).
    pub clickable: Rect,
}

/// Marco's `MAX_MIDDLE_BACKGROUNDS`.
pub const MAX_MIDDLE_BACKGROUNDS: usize = ButtonFunction::ALL.len() - 2;

/// The rects of the backgrounds drawn behind the buttons of each side. A side with one
/// button has a single background; otherwise its first and last buttons have the left
/// and right ones and the others middle ones.
///
/// They're assigned before buttons that don't fit are dropped, as Marco does, so a side
/// that loses buttons can keep backgrounds of the wrong kind and have gaps among its
/// middle ones.
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
pub struct ButtonBackgrounds {
    pub left_left: Option<Rect>,
    pub left_middle: [Option<Rect>; MAX_MIDDLE_BACKGROUNDS],
    pub left_right: Option<Rect>,
    pub left_single: Option<Rect>,
    pub right_left: Option<Rect>,
    pub right_middle: [Option<Rect>; MAX_MIDDLE_BACKGROUNDS],
    pub right_right: Option<Rect>,
    pub right_single: Option<Rect>,
}

/// Which of [`ButtonBackgrounds`]' rects a button fills.
#[derive(Debug, Clone, Copy)]
enum Background {
    LeftLeft,
    LeftMiddle(usize),
    LeftRight,
    LeftSingle,
    RightLeft,
    RightMiddle(usize),
    RightRight,
    RightSingle,
}

impl ButtonBackgrounds {
    fn rect(&mut self, background: Background) -> &mut Option<Rect> {
        match background {
            Background::LeftLeft => &mut self.left_left,
            Background::LeftMiddle(i) => &mut self.left_middle[i],
            Background::LeftRight => &mut self.left_right,
            Background::LeftSingle => &mut self.left_single,
            Background::RightLeft => &mut self.right_left,
            Background::RightMiddle(i) => &mut self.right_middle[i],
            Background::RightRight => &mut self.right_right,
            Background::RightSingle => &mut self.right_single,
        }
    }
}

/// The corner radii the frame is rounded with.
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
pub struct CornerRadii {
    pub top_left: u32,
    pub top_right: u32,
    pub bottom_left: u32,
    pub bottom_right: u32,
}

/// Where every part of a frame goes, in logical pixels from the frame's top left
/// corner, the invisible border included (`MetaFrameGeometry`).
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct FrameGeometry {
    pub borders: FrameBorders,
    pub width: i32,
    pub height: i32,
    /// Empty when the title doesn't fit.
    pub title_rect: Rect,
    pub left_titlebar_edge: i32,
    pub right_titlebar_edge: i32,
    pub top_titlebar_edge: i32,
    pub bottom_titlebar_edge: i32,
    buttons: [Option<ButtonSpace>; ButtonFunction::ALL.len()],
    /// How many of each side's buttons are left once those that don't fit are dropped.
    /// Marco reads the button layout's first that many as the buttons shown, which is
    /// wrong when a toggle's opposite or a dropped button comes before the last.
    pub(crate) n_left_buttons: usize,
    pub(crate) n_right_buttons: usize,
    pub button_backgrounds: ButtonBackgrounds,
    pub corner_radii: CornerRadii,
}

impl FrameGeometry {
    /// A button's place, if the frame shows it: the layout has it, the window allows it,
    /// and it fits.
    pub fn button(&self, function: ButtonFunction) -> Option<ButtonSpace> {
        self.buttons[function as usize]
    }
}

/// The resize area outside the visible frame, on each side a window can be resized
/// from. Marco's layouts are all created with it and no theme can change it.
const INVISIBLE_BORDER: i32 = 10;

/// The order Marco drops buttons in when they don't fit, each tried on the left side
/// first unless noted. Marco never drops the opposites of the toggles, and stops with an
/// error if only they are left; Caminito drops them last.
const DROP_ORDER: [(ButtonFunction, bool); 11] = [
    (ButtonFunction::Above, true),
    (ButtonFunction::Stick, true),
    (ButtonFunction::Shade, true),
    (ButtonFunction::Minimize, true),
    (ButtonFunction::Maximize, true),
    (ButtonFunction::Close, true),
    // The menu is the most useful, so it stays longest where layouts usually put it.
    (ButtonFunction::Menu, false),
    (ButtonFunction::Appmenu, false),
    (ButtonFunction::Unabove, true),
    (ButtonFunction::Unstick, true),
    (ButtonFunction::Unshade, true),
];

/// Marco's C casts of three quarters of a button width.
fn spacer_width(width: i32) -> i32 {
    (0.75 * f64::from(width)) as i32
}

impl Theme {
    /// The style Marco draws a frame with (`theme_get_style`).
    pub fn frame_style_for(&self, frame_type: FrameType, flags: &FrameFlags) -> &Arc<FrameStyle> {
        self.frame_style(frame_type, flags.state(), flags.resize(), flags.focus())
    }

    /// The `<frame_geometry>` of the frame's style.
    fn frame_layout_for(&self, frame_type: FrameType, flags: &FrameFlags) -> &FrameLayout {
        &self.frame_style_for(frame_type, flags).geometry
    }

    /// The factor the frame's style scales the title font by.
    pub fn title_scale(&self, frame_type: FrameType, flags: &FrameFlags) -> f64 {
        self.frame_layout_for(frame_type, flags).title_scale
    }

    /// The frame's borders, given the title's text height ([`crate::Title`]).
    pub fn frame_borders(
        &self,
        frame_type: FrameType,
        text_height: i32,
        flags: &FrameFlags,
    ) -> FrameBorders {
        borders(self.frame_layout_for(frame_type, flags), text_height, flags)
    }

    /// Where every part of the frame goes (`meta_theme_calc_geometry`), given the title's
    /// text height ([`crate::Title`]) and the client's size.
    pub fn frame_geometry(
        &self,
        frame_type: FrameType,
        text_height: i32,
        flags: &FrameFlags,
        client_width: i32,
        client_height: i32,
        button_layout: &ButtonLayout,
    ) -> FrameGeometry {
        let layout = self.frame_layout_for(frame_type, flags);
        let borders = borders(layout, text_height, flags);
        let width = client_width + borders.total.left + borders.total.right;
        let height =
            if flags.shaded { 0 } else { client_height } + borders.total.top + borders.total.bottom;
        let button_border = layout.button_border;
        let title_border = layout.title_border;

        let (button_width, button_height) = match layout.button_sizing {
            ButtonSizing::Aspect(aspect) => {
                let height = borders.visible.top - button_border.top - button_border.bottom;
                ((f64::from(height) / aspect) as i32, height)
            }
            ButtonSizing::Fixed { width, height } => (width, height),
        };

        let shown = |side: &[LayoutButton]| -> Vec<LayoutButton> {
            if layout.hide_buttons {
                return Vec::new();
            }
            side.iter()
                .copied()
                .filter(|button| self.shows(button.function, flags))
                .collect()
        };
        let left = shown(&button_layout.left);
        let right = shown(&button_layout.right);

        let mut left_side = Side::new(&left, |i, n| match i {
            _ if n == 1 => Background::LeftSingle,
            0 => Background::LeftLeft,
            _ if i == n - 1 => Background::LeftRight,
            _ => Background::LeftMiddle(i - 1),
        });
        let mut right_side = Side::new(&right, |i, n| match i {
            _ if n == 1 => Background::RightSingle,
            _ if i == n - 1 => Background::RightRight,
            0 => Background::RightLeft,
            _ => Background::RightMiddle(i - 1),
        });

        // Drop spacers, then buttons, until the buttons fit.
        let space_available = width - layout.left_titlebar_edge - layout.right_titlebar_edge;
        while !left_side.buttons.is_empty() || !right_side.buttons.is_empty() {
            let space_used = left_side.space_used(button_width, button_border)
                + right_side.space_used(button_width, button_border);
            if space_used <= space_available {
                break;
            }
            if left_side.drop_spacer() || right_side.drop_spacer() {
                continue;
            }
            let dropped = DROP_ORDER.iter().any(|&(function, left_first)| {
                let (first, second) = if left_first {
                    (&mut left_side, &mut right_side)
                } else {
                    (&mut right_side, &mut left_side)
                };
                first.drop(function) || second.drop(function)
            });
            assert!(dropped, "every button a side can have is in DROP_ORDER");
        }

        let mut buttons = [None; ButtonFunction::ALL.len()];
        let mut backgrounds = ButtonBackgrounds::default();

        // Centred vertically in the visible titlebar.
        let button_y =
            (borders.visible.top - (button_height + button_border.top + button_border.bottom)) / 2
                + button_border.top
                + borders.invisible.top;
        let place = |x: i32| Rect {
            x,
            y: button_y,
            width: button_width,
            height: button_height,
        };
        // Fitts's law: the outermost button reaches the screen's corner.
        let to_corner = |mut rect: Rect, extra_width: i32| {
            rect.y = 0;
            rect.width += extra_width;
            rect.height = button_y + button_height;
            rect
        };

        // The right side, from its right edge.
        let mut x = width - layout.right_titlebar_edge - borders.invisible.right;
        let n_right = right_side.buttons.len();
        for (i, &(function, background)) in right_side.buttons.iter().enumerate().rev() {
            // Marco leaves the buttons it doesn't get to empty.
            if x < 0 {
                break;
            }
            let mut visible_x = x - button_border.right - button_width;
            if right_side.spacers[i] {
                visible_x -= spacer_width(button_width);
            }
            let visible = place(visible_x);
            let mut clickable = visible;
            if i == n_right - 1 && (flags.maximized || flags.tiled_right) {
                clickable = to_corner(
                    clickable,
                    layout.right_titlebar_edge + layout.right_width + button_border.right,
                );
            }
            buttons[function as usize] = Some(ButtonSpace { visible, clickable });
            *backgrounds.rect(background) = Some(visible);
            x = visible.x - button_border.left;
        }
        let title_right_edge = x - title_border.right;

        // The left side, from its left edge.
        let mut x = layout.left_titlebar_edge + borders.invisible.left;
        for (i, &(function, background)) in left_side.buttons.iter().enumerate() {
            let visible = place(x + button_border.left);
            let mut clickable = visible;
            if i == 0 && (flags.maximized || flags.tiled_left) {
                clickable = to_corner(
                    clickable,
                    layout.left_titlebar_edge + layout.left_width + button_border.left,
                );
                clickable.x = 0;
            }
            buttons[function as usize] = Some(ButtonSpace { visible, clickable });
            *backgrounds.rect(background) = Some(visible);
            x = visible.x + visible.width + button_border.right;
            if left_side.spacers[i] {
                x += spacer_width(button_width);
            }
        }
        // The title fills the titlebar's height between the buttons, or nothing.
        let mut title_rect = Rect {
            x: x + title_border.left,
            y: title_border.top + borders.invisible.top,
            width: 0,
            height: borders.visible.top - title_border.top - title_border.bottom,
        };
        title_rect.width = title_right_edge - title_rect.x;
        if title_rect.width < 0 || title_rect.height < 0 {
            title_rect.width = 0;
            title_rect.height = 0;
        }

        // frames.c only rounds a corner whose borders are large enough to show it.
        let min_size_for_rounding = if flags.shaded { 0 } else { 5 };
        let visible = borders.visible;
        let radius = |a: i32, b: i32, radius: u32| {
            if a + b >= min_size_for_rounding {
                radius
            } else {
                0
            }
        };
        let corner_radii = CornerRadii {
            top_left: radius(visible.top, visible.left, layout.top_left_radius),
            top_right: radius(visible.top, visible.right, layout.top_right_radius),
            bottom_left: radius(visible.bottom, visible.left, layout.bottom_left_radius),
            bottom_right: radius(visible.bottom, visible.right, layout.bottom_right_radius),
        };

        FrameGeometry {
            borders,
            width,
            height,
            title_rect,
            left_titlebar_edge: layout.left_titlebar_edge,
            right_titlebar_edge: layout.right_titlebar_edge,
            top_titlebar_edge: title_border.top,
            bottom_titlebar_edge: title_border.bottom,
            buttons,
            n_left_buttons: left_side.buttons.len(),
            n_right_buttons: right_side.buttons.len(),
            button_backgrounds: backgrounds,
            corner_radii,
        }
    }

    /// Whether the window's flags show a button (`rect_for_function`). Marco means to
    /// show the toggles only from format 2, but compares the format version, which is at
    /// least 1000, with 2, so every theme has them.
    fn shows(&self, function: ButtonFunction, flags: &FrameFlags) -> bool {
        use ButtonFunction::*;
        match function {
            Shade => flags.allows_shade && !flags.shaded,
            Unshade => flags.allows_shade && flags.shaded,
            Above => !flags.above,
            Unabove => flags.above,
            Stick => !flags.stuck,
            Unstick => flags.stuck,
            Menu => flags.allows_menu,
            Appmenu => flags.allows_appmenu,
            Minimize => flags.allows_minimize,
            Maximize => flags.allows_maximize,
            Close => flags.allows_delete,
        }
    }
}

/// One side's buttons while Marco fits them in.
struct Side {
    /// The buttons, with the backgrounds they were given before any was dropped.
    buttons: Vec<(ButtonFunction, Background)>,
    /// Whether a spacer follows each position. Marco doesn't shift these when it drops
    /// a button.
    spacers: Vec<bool>,
    /// The spacers Marco counts, which it drops by clearing the position before the
    /// count, whether or not that position has the spacer.
    n_spacers: usize,
}

impl Side {
    fn new(shown: &[LayoutButton], background: impl Fn(usize, usize) -> Background) -> Side {
        let n = shown.len();
        Side {
            buttons: shown
                .iter()
                .enumerate()
                .map(|(i, button)| (button.function, background(i, n)))
                .collect(),
            spacers: shown.iter().map(|button| button.spacer).collect(),
            n_spacers: shown.iter().filter(|button| button.spacer).count(),
        }
    }

    fn space_used(&self, button_width: i32, button_border: Border) -> i32 {
        let n = self.buttons.len() as i32;
        button_width * n
            + spacer_width(button_width * self.n_spacers as i32)
            + button_border.left * n
            + button_border.right * n
    }

    fn drop_spacer(&mut self) -> bool {
        if self.n_spacers == 0 {
            return false;
        }
        self.n_spacers -= 1;
        self.spacers[self.n_spacers] = false;
        true
    }

    /// Marco's `strip_button`. Each button keeps its background.
    fn drop(&mut self, function: ButtonFunction) -> bool {
        match self.buttons.iter().position(|&(f, _)| f == function) {
            Some(i) => {
                self.buttons.remove(i);
                true
            }
            None => false,
        }
    }
}

/// Marco's `meta_frame_layout_get_borders`.
fn borders(layout: &FrameLayout, text_height: i32, flags: &FrameFlags) -> FrameBorders {
    let mut borders = FrameBorders::default();
    // A fullscreen window has no borders, visible or not.
    if flags.fullscreen {
        return borders;
    }

    let text_height = if layout.has_title { text_height } else { 0 };
    // Marco leaves the button height unset, -1, for aspect sizing, and adds it anyway.
    let button_height = match layout.button_sizing {
        ButtonSizing::Aspect(_) => -1,
        ButtonSizing::Fixed { height, .. } => height,
    };
    let buttons_height = button_height + layout.button_border.top + layout.button_border.bottom;
    let title_height = text_height
        + layout.title_vertical_pad
        + layout.title_border.top
        + layout.title_border.bottom;

    borders.visible = Border {
        top: buttons_height.max(title_height),
        bottom: layout.bottom_height,
        left: layout.left_width,
        right: layout.right_width,
    };
    if flags.allows_horizontal_resize {
        borders.invisible.left = INVISIBLE_BORDER;
        borders.invisible.right = INVISIBLE_BORDER;
    }
    if flags.allows_vertical_resize {
        borders.invisible.top = INVISIBLE_BORDER;
        borders.invisible.bottom = INVISIBLE_BORDER;
    }
    if flags.shaded {
        borders.visible.bottom = 0;
        borders.invisible.bottom = 0;
    }
    let (visible, invisible) = (borders.visible, borders.invisible);
    borders.total = Border {
        top: visible.top + invisible.top,
        bottom: visible.bottom + invisible.bottom,
        left: visible.left + invisible.left,
        right: visible.right + invisible.right,
    };
    borders
}

#[cfg(test)]
mod tests {
    use std::path::Path;

    use super::*;

    use ButtonFunction::*;

    /// A theme whose normal windows use `geometry`'s children, over a geometry with
    /// simple numbers: 20×20 buttons in a 1, 1, 2, 2 border, so 22 wide and a 24 high
    /// titlebar, and edges 5 and 6 wide.
    fn theme_with(attributes: &str, children: &str) -> Theme {
        let buttons: String = [
            "close", "maximize", "minimize", "menu", "shade", "above", "stick", "unshade",
            "unabove", "unstick",
        ]
        .iter()
        .flat_map(|function| {
            ["normal", "pressed"].map(|state| {
                format!(r#"<button function="{function}" state="{state}" draw_ops="empty"/>"#)
            })
        })
        .collect();
        let mut distances = vec![
            ("left_width", 2),
            ("right_width", 3),
            ("bottom_height", 4),
            ("title_vertical_pad", 0),
            ("left_titlebar_edge", 5),
            ("right_titlebar_edge", 6),
            ("button_width", 20),
            ("button_height", 20),
        ];
        distances.retain(|(name, _)| {
            !children.contains(&format!("\"{name}\""))
                && !(children.contains("aspect_ratio") && name.starts_with("button_"))
        });
        let distances: String = distances
            .iter()
            .map(|(name, value)| format!(r#"<distance name="{name}" value="{value}"/>"#))
            .collect();
        let windows: String = [
            "normal",
            "dialog",
            "modal_dialog",
            "menu",
            "utility",
            "border",
        ]
        .iter()
        .map(|kind| format!(r#"<window type="{kind}" style_set="set"/>"#))
        .collect();
        let frames: String = ["normal", "maximized", "shaded", "maximized_and_shaded"]
            .iter()
            .flat_map(|state| {
                ["yes", "no"].map(|focus| {
                    let resize = if state.ends_with("maximized") || state.contains('_') {
                        ""
                    } else {
                        r#" resize="both""#
                    };
                    format!(r#"<frame focus="{focus}" state="{state}"{resize} style="s"/>"#)
                })
            })
            .collect();
        let text = format!(
            r#"<metacity_theme>
<info><name>T</name><author>A</author><copyright>C</copyright><date>D</date><description>X</description></info>
<frame_geometry name="g" {attributes}>
{distances}
<border name="title_border" left="1" right="2" top="3" bottom="4"/>
<border name="button_border" left="1" right="1" top="2" bottom="2"/>
{children}
</frame_geometry>
<draw_ops name="empty"/>
<frame_style name="s" geometry="g">{buttons}</frame_style>
<frame_style_set name="set">{frames}</frame_style_set>
{windows}
</metacity_theme>"#
        );
        Theme::parse(
            text.as_bytes(),
            "T",
            Path::new("/themes/T/metacity-1/metacity-theme-3.xml"),
            3,
        )
        .unwrap_or_else(|e| panic!("{e}"))
    }

    fn theme() -> Theme {
        theme_with("", "")
    }

    fn flags() -> FrameFlags {
        FrameFlags {
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
        }
    }

    fn geometry(
        theme: &Theme,
        flags: &FrameFlags,
        client_width: i32,
        layout: &str,
    ) -> FrameGeometry {
        theme.frame_geometry(
            FrameType::Normal,
            10,
            flags,
            client_width,
            100,
            &ButtonLayout::parse(layout),
        )
    }

    fn border(top: i32, bottom: i32, left: i32, right: i32) -> Border {
        Border {
            top,
            bottom,
            left,
            right,
        }
    }

    fn shown(geometry: &FrameGeometry) -> Vec<ButtonFunction> {
        ButtonFunction::ALL
            .into_iter()
            .filter(|&function| geometry.button(function).is_some())
            .collect()
    }

    #[test]
    fn the_state_comes_from_the_flags_with_tiling_winning() {
        let state = |f: fn(&mut FrameFlags)| {
            let mut flags = FrameFlags::default();
            f(&mut flags);
            flags.state()
        };
        assert_eq!(state(|_| {}), FrameState::Normal);
        assert_eq!(state(|f| f.maximized = true), FrameState::Maximized);
        assert_eq!(state(|f| f.shaded = true), FrameState::Shaded);
        assert_eq!(
            state(|f| (f.maximized, f.shaded) = (true, true)),
            FrameState::MaximizedAndShaded
        );
        assert_eq!(
            state(|f| (f.maximized, f.tiled_left) = (true, true)),
            FrameState::TiledLeft
        );
        assert_eq!(
            state(|f| (f.maximized, f.tiled_right) = (true, true)),
            FrameState::TiledRight
        );
        assert_eq!(
            state(|f| (f.tiled_right, f.shaded) = (true, true)),
            FrameState::TiledRightAndShaded
        );
        // Marco asserts on these.
        assert_eq!(
            state(|f| (f.tiled_left, f.tiled_right) = (true, true)),
            FrameState::TiledLeft
        );
        assert_eq!(
            state(|f| (f.tiled_left, f.maximized, f.shaded) = (true, true, true)),
            FrameState::TiledLeftAndShaded
        );
    }

    #[test]
    fn a_flashing_frame_swaps_its_focus() {
        let mut flags = flags();
        assert_eq!(flags.focus(), FrameFocus::Yes);
        flags.is_flashing = true;
        assert_eq!(flags.focus(), FrameFocus::No);
        flags.has_focus = false;
        assert_eq!(flags.focus(), FrameFocus::Yes);
    }

    #[test]
    fn borders_add_the_invisible_resize_area() {
        let geometry = geometry(&theme(), &flags(), 200, "menu:close");
        assert_eq!(geometry.borders.visible, border(24, 4, 2, 3));
        assert_eq!(geometry.borders.invisible, border(10, 10, 10, 10));
        assert_eq!(geometry.borders.total, border(34, 14, 12, 13));
        assert_eq!((geometry.width, geometry.height), (225, 148));
    }

    #[test]
    fn the_invisible_border_is_only_where_the_window_resizes() {
        let theme = theme();
        let mut flags = flags();
        flags.allows_horizontal_resize = false;
        let borders = theme.frame_borders(FrameType::Normal, 10, &flags);
        assert_eq!(borders.invisible, border(10, 10, 0, 0));
        flags.allows_vertical_resize = false;
        let borders = theme.frame_borders(FrameType::Normal, 10, &flags);
        assert_eq!(borders.invisible, border(0, 0, 0, 0));
    }

    #[test]
    fn a_shaded_frame_has_no_bottom_or_client() {
        let mut flags = flags();
        flags.shaded = true;
        let geometry = geometry(&theme(), &flags, 200, "menu:close");
        assert_eq!(geometry.borders.total, border(34, 0, 12, 13));
        assert_eq!(geometry.height, 34);
    }

    #[test]
    fn a_fullscreen_frame_has_no_borders() {
        let mut flags = flags();
        flags.fullscreen = true;
        let geometry = geometry(&theme(), &flags, 200, "menu:close");
        assert_eq!(geometry.borders, FrameBorders::default());
        assert_eq!((geometry.width, geometry.height), (200, 100));
    }

    #[test]
    fn a_tall_title_sizes_the_titlebar_unless_the_theme_has_none() {
        let theme = theme();
        // 50 plus the title border's 3 and 4.
        let borders = theme.frame_borders(FrameType::Normal, 50, &flags());
        assert_eq!(borders.visible.top, 57);
        let theme = theme_with(r#"has_title="false""#, "");
        let borders = theme.frame_borders(FrameType::Normal, 50, &flags());
        assert_eq!(borders.visible.top, 24);
    }

    #[test]
    fn aspect_sized_buttons_are_as_high_as_the_titlebar() {
        let theme = theme_with("", r#"<aspect_ratio name="button" value="0.7"/>"#);
        // A 17 high titlebar from the title, less the button border's 2 and 2; the
        // width is 13 / 0.7, truncated.
        let geometry = geometry(&theme, &flags(), 200, "menu:close");
        assert_eq!(geometry.borders.visible.top, 17);
        let close = geometry.button(Close).unwrap().visible;
        assert_eq!((close.width, close.height), (18, 13));
    }

    #[test]
    fn buttons_are_placed_from_the_edges_in() {
        let geometry = geometry(&theme(), &flags(), 200, "menu:minimize,close");
        // Right edge 225 - 6 - 10 = 209; close's border right 1.
        let close = geometry.button(Close).unwrap();
        assert_eq!(
            close.visible,
            Rect {
                x: 188,
                y: 12,
                width: 20,
                height: 20
            }
        );
        assert_eq!(close.clickable, close.visible);
        assert_eq!(geometry.button(Minimize).unwrap().visible.x, 166);
        // Left edge 5 + 10; menu's border left 1.
        assert_eq!(geometry.button(Menu).unwrap().visible.x, 16);
        assert_eq!(
            geometry.title_rect,
            Rect {
                x: 38,
                y: 13,
                width: 163 - 38,
                height: 17
            }
        );
        let backgrounds = geometry.button_backgrounds;
        assert_eq!(
            backgrounds.left_single,
            Some(geometry.button(Menu).unwrap().visible)
        );
        assert_eq!(
            backgrounds.right_left,
            Some(geometry.button(Minimize).unwrap().visible)
        );
        assert_eq!(backgrounds.right_right, Some(close.visible));
        assert_eq!(backgrounds.right_single, None);
    }

    #[test]
    fn hidden_buttons_leave_the_titlebar_to_the_title() {
        let geometry = geometry(
            &theme_with(r#"hide_buttons="true""#, ""),
            &flags(),
            200,
            "menu:close",
        );
        assert_eq!(shown(&geometry), []);
        assert_eq!(geometry.title_rect.x, 16);
        assert_eq!(geometry.title_rect.width, 209 - 2 - 16);
    }

    #[test]
    fn a_window_shows_only_the_buttons_it_allows() {
        let mut flags = flags();
        flags.allows_minimize = false;
        flags.allows_shade = false;
        let layout = "menu,appmenu,shade,above,stick:minimize,maximize,close";
        assert_eq!(
            shown(&geometry(&theme(), &flags, 400, layout)),
            [Menu, Maximize, Close, Above, Stick]
        );
        flags.allows_shade = true;
        flags.shaded = true;
        flags.above = true;
        flags.stuck = true;
        flags.allows_appmenu = true;
        assert_eq!(
            shown(&geometry(&theme(), &flags, 400, layout)),
            [Menu, Appmenu, Maximize, Close, Unshade, Unabove, Unstick]
        );
    }

    #[test]
    fn a_spacer_is_three_quarters_of_a_button() {
        let geometry = geometry(&theme(), &flags(), 200, "menu,spacer,minimize:close");
        // 16 + 20 + 1 right border, then 15 of spacer and 1 left border.
        assert_eq!(geometry.button(Minimize).unwrap().visible.x, 53);
    }

    // Four buttons of 22 need 88 of the width less the edges, 25 + client - 11.
    #[test]
    fn buttons_that_dont_fit_are_dropped_in_marcos_order() {
        let theme = theme();
        let layout = "menu:minimize,maximize,close";
        assert_eq!(shown(&geometry(&theme, &flags(), 74, layout)).len(), 4);
        assert_eq!(
            shown(&geometry(&theme, &flags(), 73, layout)),
            [Menu, Maximize, Close]
        );
        assert_eq!(
            shown(&geometry(&theme, &flags(), 30, layout)),
            [Menu, Close]
        );
        assert_eq!(shown(&geometry(&theme, &flags(), 10, layout)), [Menu]);
        assert_eq!(shown(&geometry(&theme, &flags(), 7, layout)), []);
    }

    #[test]
    fn spacers_are_dropped_before_buttons() {
        // 22 + 15 + 22 = 59 against 44 + 14.
        let geometry = geometry(&theme(), &flags(), 44, "menu,spacer:close");
        assert_eq!(shown(&geometry), [Menu, Close]);
        assert_eq!(geometry.title_rect.x, 38);
    }

    // Marco assigns backgrounds before it drops buttons, and a dropped button takes its
    // background with it.
    #[test]
    fn a_dropped_button_leaves_a_gap_in_the_backgrounds() {
        let geometry = geometry(&theme(), &flags(), 95, "menu,shade,above,stick:close");
        assert_eq!(shown(&geometry), [Menu, Close, Shade, Stick]);
        let backgrounds = geometry.button_backgrounds;
        assert_eq!(
            backgrounds.left_left,
            Some(geometry.button(Menu).unwrap().visible)
        );
        assert_eq!(
            backgrounds.left_middle[..2],
            [Some(geometry.button(Shade).unwrap().visible), None]
        );
        assert_eq!(
            backgrounds.left_right,
            Some(geometry.button(Stick).unwrap().visible)
        );
    }

    #[test]
    fn a_title_that_doesnt_fit_is_empty() {
        let geometry = geometry(&theme(), &flags(), 70, "menu:minimize,maximize,close");
        assert_eq!(
            geometry.title_rect,
            Rect {
                x: 38,
                y: 13,
                width: 0,
                height: 0
            }
        );
    }

    #[test]
    fn a_corner_is_rounded_only_where_its_borders_are_five_or_more() {
        let theme = theme_with(
            r#"rounded_top_left="4" rounded_bottom_left="3" rounded_bottom_right="2""#,
            r#"<distance name="left_width" value="1"/><distance name="right_width" value="1"/><distance name="bottom_height" value="3"/>"#,
        );
        assert_eq!(
            geometry(&theme, &flags(), 200, "menu:close").corner_radii,
            CornerRadii {
                top_left: 4,
                top_right: 0,
                bottom_left: 0,
                bottom_right: 0
            }
        );
        // A shaded frame is rounded wherever the theme says.
        let mut flags = flags();
        flags.shaded = true;
        assert_eq!(
            geometry(&theme, &flags, 200, "menu:close").corner_radii,
            CornerRadii {
                top_left: 4,
                top_right: 0,
                bottom_left: 3,
                bottom_right: 2
            }
        );
    }
}
