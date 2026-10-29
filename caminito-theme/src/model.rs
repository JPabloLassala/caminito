// SPDX-FileCopyrightText: 2001 Havoc Pennington
// SPDX-FileCopyrightText: 2025-2026 Juan Pablo Lassala
// SPDX-License-Identifier: GPL-3.0-or-later AND GPL-2.0-or-later

//! What a loaded theme is made of, mirroring Marco's `theme.h`.
//!
//! Everything named in a theme is shared by reference, as in Marco: a style holds its
//! geometry and parent, and an `include` holds the list it draws.

use std::collections::{BTreeSet, HashMap};
use std::path::PathBuf;
use std::sync::Arc;

use crate::color::{ColorSpec, GtkState};
use crate::expr::{Constants, Expr};
use crate::image::ImageCache;

/// A loaded, validated theme.
#[derive(Debug, Clone)]
pub struct Theme {
    /// The directory name the theme was looked up by.
    pub name: String,
    /// The file it was loaded from.
    pub path: PathBuf,
    /// The format version in Marco's encoding, `1000 * major + minor`. It's the file's
    /// major version, raised by a `version` check on the root element.
    pub format_version: u32,
    pub info: ThemeInfo,
    pub constants: Constants,
    pub geometries: HashMap<String, Arc<FrameLayout>>,
    pub draw_ops: HashMap<String, Arc<DrawOpList>>,
    pub styles: HashMap<String, Arc<FrameStyle>>,
    pub style_sets: HashMap<String, Arc<FrameStyleSet>>,
    /// The `<window>` table, indexed by [`FrameType`]. Validation guarantees every type
    /// but `attached` has one.
    pub(crate) window_style_sets: [Option<Arc<FrameStyleSet>>; FrameType::ALL.len()],
    /// The theme's images, loaded when they're first drawn at a scale.
    pub(crate) images: Arc<ImageCache>,
}

impl Theme {
    /// The style set for a frame type. `attached` falls back to `border`
    /// (`theme.c:5731`).
    pub fn style_set(&self, frame_type: FrameType) -> &Arc<FrameStyleSet> {
        let assigned = |frame_type: FrameType| self.window_style_sets[frame_type as usize].as_ref();
        assigned(frame_type)
            .or_else(|| {
                (frame_type == FrameType::Attached)
                    .then(|| assigned(FrameType::Border))
                    .flatten()
            })
            .or_else(|| assigned(FrameType::Normal))
            .expect("validation requires a style set for every frame type but attached")
    }

    /// The names of the GTK colours the theme's `gtk:custom` specs read, which the palette
    /// helper is asked to look up.
    pub fn custom_color_names(&self) -> BTreeSet<String> {
        let mut names = BTreeSet::new();
        let mut add = |color: &ColorSpec| color.custom_names(&mut names);
        // Every list is either named here or a style's own; includes and tiles only
        // refer to named ones.
        let style_lists = self.styles.values().flat_map(|style| {
            let pieces = style.pieces.iter().flatten();
            let buttons = style.buttons.iter().flatten().flatten();
            pieces.chain(buttons)
        });
        for list in self.draw_ops.values().chain(style_lists) {
            for op in &list.ops {
                op.colors().for_each(&mut add);
            }
        }
        for style in self.styles.values() {
            style.window_background.iter().for_each(&mut add);
        }
        names
    }

    /// The frame style Marco uses for a window (`theme_get_style`).
    pub fn frame_style(
        &self,
        frame_type: FrameType,
        state: FrameState,
        resize: FrameResize,
        focus: FrameFocus,
    ) -> &Arc<FrameStyle> {
        self.style_set(frame_type).style(state, resize, focus)
    }
}

/// The five `<info>` children, all of which Marco requires.
#[derive(Debug, Clone, Default, PartialEq)]
pub struct ThemeInfo {
    pub name: String,
    pub author: String,
    pub copyright: String,
    pub date: String,
    pub description: String,
}

/// A `<frame_geometry>`, Marco's `MetaFrameLayout`, with every field validated.
#[derive(Debug, Clone, PartialEq)]
pub struct FrameLayout {
    pub left_width: i32,
    pub right_width: i32,
    pub bottom_height: i32,
    pub title_border: Border,
    pub button_border: Border,
    pub title_vertical_pad: i32,
    pub left_titlebar_edge: i32,
    pub right_titlebar_edge: i32,
    pub button_sizing: ButtonSizing,
    pub title_scale: f64,
    pub has_title: bool,
    pub hide_buttons: bool,
    pub top_left_radius: u32,
    pub top_right_radius: u32,
    pub bottom_left_radius: u32,
    pub bottom_right_radius: u32,
}

#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
pub struct Border {
    pub top: i32,
    pub bottom: i32,
    pub left: i32,
    pub right: i32,
}

#[derive(Debug, Clone, Copy, PartialEq)]
pub enum ButtonSizing {
    /// `<aspect_ratio name="button">`: the width is the height divided by this.
    Aspect(f64),
    /// `button_width` and `button_height`.
    Fixed { width: i32, height: i32 },
}

/// A `<draw_ops>` list.
#[derive(Debug, Clone, Default, PartialEq)]
pub struct DrawOpList {
    pub ops: Vec<DrawOp>,
}

/// The rectangle most draw ops take.
#[derive(Debug, Clone, PartialEq)]
pub struct DrawRect {
    pub x: Expr,
    pub y: Expr,
    pub width: Expr,
    pub height: Expr,
}

/// One drawing operation (`MetaDrawOp`).
#[derive(Debug, Clone, PartialEq)]
pub enum DrawOp {
    Line {
        color: ColorSpec,
        x1: Expr,
        y1: Expr,
        /// `None` when the theme wrote the same text as `x1`, as Marco records it.
        x2: Option<Expr>,
        /// `None` when the theme wrote the same text as `y1`.
        y2: Option<Expr>,
        width: i32,
        dash_on_length: i32,
        dash_off_length: i32,
    },
    Rectangle {
        color: ColorSpec,
        rect: DrawRect,
        filled: bool,
    },
    Arc {
        color: ColorSpec,
        rect: DrawRect,
        filled: bool,
        /// `start_angle` as written, or `from` converted as Marco converts it.
        start_angle: f64,
        /// `extent_angle` as written, or `to` converted as Marco converts it.
        extent_angle: f64,
    },
    Clip {
        rect: DrawRect,
    },
    Tint {
        color: ColorSpec,
        alpha: AlphaGradient,
        rect: DrawRect,
    },
    Gradient {
        kind: GradientType,
        colors: Vec<ColorSpec>,
        alpha: Option<AlphaGradient>,
        rect: DrawRect,
    },
    Image {
        source: ImageSource,
        colorize: Option<ColorSpec>,
        alpha: Option<AlphaGradient>,
        rect: DrawRect,
        fill: ImageFill,
    },
    GtkArrow {
        state: GtkState,
        shadow: GtkShadow,
        arrow: GtkArrow,
        filled: bool,
        rect: DrawRect,
    },
    GtkBox {
        state: GtkState,
        shadow: GtkShadow,
        rect: DrawRect,
    },
    GtkVline {
        state: GtkState,
        x: Expr,
        y1: Expr,
        y2: Expr,
    },
    Icon {
        alpha: Option<AlphaGradient>,
        rect: DrawRect,
        fill: ImageFill,
    },
    Title {
        color: ColorSpec,
        x: Expr,
        y: Expr,
        ellipsize_width: Option<Expr>,
    },
    /// `<include>`, with the list resolved.
    Include {
        list: Arc<DrawOpList>,
        rect: DrawRect,
    },
    Tile {
        list: Arc<DrawOpList>,
        rect: DrawRect,
        tile_xoffset: Expr,
        tile_yoffset: Expr,
        tile_width: Expr,
        tile_height: Expr,
    },
}

impl DrawOp {
    /// The colours the op draws with.
    fn colors(&self) -> impl Iterator<Item = &ColorSpec> {
        let (one, many): (Option<&ColorSpec>, &[ColorSpec]) = match self {
            DrawOp::Line { color, .. }
            | DrawOp::Rectangle { color, .. }
            | DrawOp::Arc { color, .. }
            | DrawOp::Tint { color, .. }
            | DrawOp::Title { color, .. } => (Some(color), &[]),
            DrawOp::Gradient { colors, .. } => (None, colors),
            DrawOp::Image { colorize, .. } => (colorize.as_ref(), &[]),
            DrawOp::Clip { .. }
            | DrawOp::GtkArrow { .. }
            | DrawOp::GtkBox { .. }
            | DrawOp::GtkVline { .. }
            | DrawOp::Icon { .. }
            | DrawOp::Include { .. }
            | DrawOp::Tile { .. } => (None, &[]),
        };
        one.into_iter().chain(many)
    }
}

/// The alphas of an `alpha` attribute, `0.0:0.5:1.0` stored as bytes (`v * 255`,
/// truncated) as Marco stores them.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct AlphaGradient(pub Vec<u8>);

#[derive(Debug, Clone, PartialEq)]
pub enum ImageSource {
    /// A file in the theme's directory, which exists.
    File(PathBuf),
    /// `theme:icon-name`, from the icon theme. Caminito parses and ignores these (07).
    IconTheme(String),
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum GradientType {
    Vertical,
    Horizontal,
    Diagonal,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum ImageFill {
    Scale,
    Tile,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum GtkShadow {
    None,
    In,
    Out,
    EtchedIn,
    EtchedOut,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum GtkArrow {
    Up,
    Down,
    Left,
    Right,
    None,
}

/// A `<frame_style>`.
#[derive(Debug, Clone, PartialEq)]
pub struct FrameStyle {
    pub parent: Option<Arc<FrameStyle>>,
    pub geometry: Arc<FrameLayout>,
    /// `background`, which no surveyed theme uses.
    pub window_background: Option<ColorSpec>,
    /// `alpha`, 255 unless the theme sets it with a background.
    pub window_background_alpha: u8,
    pub(crate) pieces: [Option<Arc<DrawOpList>>; FramePiece::ALL.len()],
    pub(crate) buttons: [[Option<Arc<DrawOpList>>; ButtonState::ALL.len()]; ButtonType::ALL.len()],
}

impl FrameStyle {
    /// A piece's draw ops, from this style or the nearest parent that has it.
    pub fn piece(&self, piece: FramePiece) -> Option<&Arc<DrawOpList>> {
        self.ancestors()
            .find_map(|style| style.pieces[piece as usize].as_ref())
    }

    /// A button's draw ops, with Marco's fallbacks (`get_button`): up the parents, then
    /// single backgrounds to the outer ones, outer backgrounds to the middle one, and
    /// prelight to normal.
    pub fn button(&self, button: ButtonType, state: ButtonState) -> Option<&Arc<DrawOpList>> {
        if let Some(ops) = self.own_button(button, state) {
            return Some(ops);
        }
        let fallback = match button {
            ButtonType::LeftSingleBackground => Some(ButtonType::LeftLeftBackground),
            ButtonType::RightSingleBackground => Some(ButtonType::RightRightBackground),
            ButtonType::LeftLeftBackground | ButtonType::LeftRightBackground => {
                Some(ButtonType::LeftMiddleBackground)
            }
            ButtonType::RightLeftBackground | ButtonType::RightRightBackground => {
                Some(ButtonType::RightMiddleBackground)
            }
            _ => None,
        };
        if let Some(fallback) = fallback {
            return self.button(fallback, state);
        }
        if state == ButtonState::Prelight {
            return self.button(button, ButtonState::Normal);
        }
        None
    }

    /// A button's draw ops from this style or the nearest parent that has them, with
    /// none of [`FrameStyle::button`]'s fallbacks.
    pub(crate) fn own_button(
        &self,
        button: ButtonType,
        state: ButtonState,
    ) -> Option<&Arc<DrawOpList>> {
        self.ancestors()
            .find_map(|style| style.buttons[button as usize][state as usize].as_ref())
    }

    fn ancestors(&self) -> impl Iterator<Item = &FrameStyle> {
        std::iter::successors(Some(self), |style| style.parent.as_deref())
    }
}

/// A `<frame_style_set>`: which style each window state gets.
#[derive(Debug, Clone, Default, PartialEq)]
pub struct FrameStyleSet {
    pub parent: Option<Arc<FrameStyleSet>>,
    pub(crate) normal: [[Option<Arc<FrameStyle>>; FrameFocus::ALL.len()]; FrameResize::ALL.len()],
    pub(crate) shaded: [[Option<Arc<FrameStyle>>; FrameFocus::ALL.len()]; FrameResize::ALL.len()],
    pub(crate) maximized: [Option<Arc<FrameStyle>>; FrameFocus::ALL.len()],
    pub(crate) tiled_left: [Option<Arc<FrameStyle>>; FrameFocus::ALL.len()],
    pub(crate) tiled_right: [Option<Arc<FrameStyle>>; FrameFocus::ALL.len()],
    pub(crate) maximized_and_shaded: [Option<Arc<FrameStyle>>; FrameFocus::ALL.len()],
    pub(crate) tiled_left_and_shaded: [Option<Arc<FrameStyle>>; FrameFocus::ALL.len()],
    pub(crate) tiled_right_and_shaded: [Option<Arc<FrameStyle>>; FrameFocus::ALL.len()],
}

impl FrameStyleSet {
    /// The style for a state. Validation guarantees one exists for every state.
    pub fn style(
        &self,
        state: FrameState,
        resize: FrameResize,
        focus: FrameFocus,
    ) -> &Arc<FrameStyle> {
        self.lookup(state, resize, focus)
            .expect("validation requires a style for every state")
    }

    /// Marco's `get_style`: `normal` and `shaded` fall back to the parent and then to
    /// `resize="both"`, and the tiled states to `normal` or `shaded`.
    pub(crate) fn lookup(
        &self,
        state: FrameState,
        resize: FrameResize,
        focus: FrameFocus,
    ) -> Option<&Arc<FrameStyle>> {
        let focus_index = focus as usize;
        match state {
            FrameState::Normal | FrameState::Shaded => {
                let table = if state == FrameState::Shaded {
                    &self.shaded
                } else {
                    &self.normal
                };
                table[resize as usize][focus_index]
                    .as_ref()
                    .or_else(|| {
                        self.parent
                            .as_ref()
                            .and_then(|parent| parent.lookup(state, resize, focus))
                    })
                    .or_else(|| {
                        (resize != FrameResize::Both)
                            .then(|| self.lookup(state, FrameResize::Both, focus))
                            .flatten()
                    })
            }
            _ => {
                let styles = match state {
                    FrameState::Maximized => &self.maximized,
                    FrameState::TiledLeft => &self.tiled_left,
                    FrameState::TiledRight => &self.tiled_right,
                    FrameState::MaximizedAndShaded => &self.maximized_and_shaded,
                    FrameState::TiledLeftAndShaded => &self.tiled_left_and_shaded,
                    FrameState::TiledRightAndShaded => &self.tiled_right_and_shaded,
                    FrameState::Normal | FrameState::Shaded => unreachable!(),
                };
                styles[focus_index]
                    .as_ref()
                    .or_else(|| match state {
                        FrameState::TiledLeft | FrameState::TiledRight => {
                            self.lookup(FrameState::Normal, resize, focus)
                        }
                        FrameState::TiledLeftAndShaded | FrameState::TiledRightAndShaded => {
                            self.lookup(FrameState::Shaded, resize, focus)
                        }
                        _ => None,
                    })
                    .or_else(|| {
                        self.parent
                            .as_ref()
                            .and_then(|parent| parent.lookup(state, resize, focus))
                    })
            }
        }
    }
}

/// Declares an enum of names Marco reads from theme files, with `ALL` in Marco's order
/// and the name of each.
macro_rules! named_enum {
    ($(#[$meta:meta])* $name:ident { $($variant:ident = $text:literal,)* }) => {
        $(#[$meta])*
        #[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
        pub enum $name {
            $($variant,)*
        }

        impl $name {
            pub const ALL: [$name; [$($text),*].len()] = [$($name::$variant),*];

            pub fn name(self) -> &'static str {
                match self {
                    $($name::$variant => $text,)*
                }
            }

            pub fn from_name(name: &str) -> Option<$name> {
                $name::ALL.into_iter().find(|value| value.name() == name)
            }
        }
    };
}

named_enum! {
    /// The kinds of window Marco draws frames for.
    FrameType {
        Normal = "normal",
        Dialog = "dialog",
        ModalDialog = "modal_dialog",
        Utility = "utility",
        Menu = "menu",
        Border = "border",
        Attached = "attached",
    }
}

named_enum! {
    FrameState {
        Normal = "normal",
        Maximized = "maximized",
        TiledLeft = "tiled_left",
        TiledRight = "tiled_right",
        Shaded = "shaded",
        MaximizedAndShaded = "maximized_and_shaded",
        TiledLeftAndShaded = "tiled_left_and_shaded",
        TiledRightAndShaded = "tiled_right_and_shaded",
    }
}

named_enum! {
    FrameResize {
        None = "none",
        Vertical = "vertical",
        Horizontal = "horizontal",
        Both = "both",
    }
}

named_enum! {
    FrameFocus {
        No = "no",
        Yes = "yes",
    }
}

named_enum! {
    /// The parts of a frame, in the order Marco draws them.
    FramePiece {
        EntireBackground = "entire_background",
        Titlebar = "titlebar",
        TitlebarMiddle = "titlebar_middle",
        LeftTitlebarEdge = "left_titlebar_edge",
        RightTitlebarEdge = "right_titlebar_edge",
        TopTitlebarEdge = "top_titlebar_edge",
        BottomTitlebarEdge = "bottom_titlebar_edge",
        Title = "title",
        LeftEdge = "left_edge",
        RightEdge = "right_edge",
        BottomEdge = "bottom_edge",
        Overlay = "overlay",
    }
}

named_enum! {
    /// Buttons and button backgrounds, in Marco's order: the backgrounds first, since
    /// they are drawn first.
    ButtonType {
        LeftLeftBackground = "left_left_background",
        LeftMiddleBackground = "left_middle_background",
        LeftRightBackground = "left_right_background",
        LeftSingleBackground = "left_single_background",
        RightLeftBackground = "right_left_background",
        RightMiddleBackground = "right_middle_background",
        RightRightBackground = "right_right_background",
        RightSingleBackground = "right_single_background",
        Close = "close",
        Maximize = "maximize",
        Minimize = "minimize",
        Menu = "menu",
        Appmenu = "appmenu",
        Shade = "shade",
        Above = "above",
        Stick = "stick",
        Unshade = "unshade",
        Unabove = "unabove",
        Unstick = "unstick",
    }
}

impl ButtonType {
    /// The first format version with this button (`theme.c:7304`).
    pub fn earliest_version(self) -> u32 {
        match self {
            ButtonType::Shade
            | ButtonType::Above
            | ButtonType::Stick
            | ButtonType::Unshade
            | ButtonType::Unabove
            | ButtonType::Unstick => 2000,
            ButtonType::LeftSingleBackground | ButtonType::RightSingleBackground => 3003,
            ButtonType::Appmenu => 3005,
            _ => 1000,
        }
    }
}

named_enum! {
    ButtonState {
        Normal = "normal",
        Pressed = "pressed",
        Prelight = "prelight",
    }
}
