// SPDX-FileCopyrightText: 2001 Havoc Pennington
// SPDX-FileCopyrightText: 2025-2026 Juan Pablo Lassala
// SPDX-License-Identifier: GPL-3.0-or-later AND GPL-2.0-or-later

//! A port of Marco's `theme-parser.c`: the state machine that turns a theme file's
//! markup into a [`Theme`], with every check Marco makes, so that a theme loads in Caminito
//! exactly when it loads in Marco (07).
//!
//! Marco's `META_THEME_ALLOWS` compares the file's version, which is 1000 or more, with
//! 2, so every feature it gates is allowed in every file. The port keeps that.

use std::collections::HashMap;
use std::path::{Path, PathBuf};
use std::sync::Arc;

use crate::color::{ColorSpec, GtkState};
use crate::error::ThemeError;
use crate::expr::{Constants, Expr};
use crate::markup::{Event, Markup};
use crate::model::*;
use crate::number::{strtod_prefix, strtol_prefix};

/// The newest format Marco reads, 3.5 (`theme-parser.c:42`).
const THEME_VERSION: u32 = 3005;

/// The largest integer Marco accepts in a theme (`theme-parser.c:581`).
const MAX_REASONABLE: i64 = 4096;

impl Theme {
    /// Parses one theme file as Marco would load it: `path` is the file's path, whose
    /// directory holds the theme's images, and `major_version` the number in its name.
    pub fn parse(
        text: &[u8],
        name: &str,
        path: &Path,
        major_version: u32,
    ) -> Result<Theme, ThemeError> {
        let mut parser = Parser::new(name, path, major_version);
        for item in Markup::new(text) {
            let (event, line) = item.map_err(|error| ThemeError::Invalid {
                path: path.to_owned(),
                line: error.line,
                message: error.message,
            })?;
            let result = match event {
                Event::Start { name, attributes } => parser.start(&name, &attributes),
                Event::End { .. } => parser.end(),
                Event::Text(text) => parser.text(&text),
            };
            result.map_err(|failure| failure.at(path, line))?;
        }
        parser.theme.ok_or_else(|| ThemeError::NotFound {
            name: name.to_owned(),
        })
    }
}

/// Why parsing stopped, before the file and line are known.
enum Failure {
    Unsupported(String),
    Invalid(String),
}

impl Failure {
    fn at(self, path: &Path, line: usize) -> ThemeError {
        let path = path.to_owned();
        match self {
            Failure::Unsupported(message) => ThemeError::UnsupportedVersion {
                path,
                line,
                message,
            },
            Failure::Invalid(message) => ThemeError::Invalid {
                path,
                line,
                message,
            },
        }
    }
}

type Parse<T = ()> = Result<T, Failure>;

fn invalid<T>(message: impl Into<String>) -> Parse<T> {
    Err(Failure::Invalid(message.into()))
}

/// What the parser is inside of (`ParseState`).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum State {
    Start,
    Theme,
    Info,
    Name,
    Author,
    Copyright,
    Date,
    Description,
    Constant,
    FrameGeometry,
    Distance,
    Border,
    AspectRatio,
    DrawOps,
    /// Any draw op but a gradient.
    Op,
    Gradient,
    Color,
    FrameStyle,
    Piece,
    Button,
    Shadow,
    Padding,
    FrameStyleSet,
    Frame,
    Window,
    MenuIcon,
    Fallback,
}

/// A `<frame_geometry>` being parsed, with Marco's unset (-1) fields as `None`.
#[derive(Clone)]
struct GeometryDraft {
    left_width: Option<i32>,
    right_width: Option<i32>,
    bottom_height: Option<i32>,
    title_border: Option<Border>,
    button_border: Option<Border>,
    title_vertical_pad: Option<i32>,
    left_titlebar_edge: Option<i32>,
    right_titlebar_edge: Option<i32>,
    sizing: Option<Sizing>,
    button_aspect: f64,
    button_width: Option<i32>,
    button_height: Option<i32>,
    title_scale: f64,
    has_title: bool,
    hide_buttons: bool,
    radii: [u32; 4],
}

#[derive(Clone, Copy, PartialEq)]
enum Sizing {
    Aspect,
    Fixed,
}

impl GeometryDraft {
    fn new() -> Self {
        GeometryDraft {
            left_width: None,
            right_width: None,
            bottom_height: None,
            title_border: None,
            button_border: None,
            title_vertical_pad: None,
            left_titlebar_edge: None,
            right_titlebar_edge: None,
            sizing: None,
            button_aspect: 1.0,
            button_width: None,
            button_height: None,
            title_scale: 1.0,
            has_title: true,
            hide_buttons: false,
            radii: [0; 4],
        }
    }

    fn from_parent(parent: &FrameLayout) -> Self {
        let (sizing, button_aspect, button_width, button_height) = match parent.button_sizing {
            ButtonSizing::Aspect(aspect) => (Sizing::Aspect, aspect, None, None),
            ButtonSizing::Fixed { width, height } => {
                (Sizing::Fixed, 1.0, Some(width), Some(height))
            }
        };
        GeometryDraft {
            left_width: Some(parent.left_width),
            right_width: Some(parent.right_width),
            bottom_height: Some(parent.bottom_height),
            title_border: Some(parent.title_border),
            button_border: Some(parent.button_border),
            title_vertical_pad: Some(parent.title_vertical_pad),
            left_titlebar_edge: Some(parent.left_titlebar_edge),
            right_titlebar_edge: Some(parent.right_titlebar_edge),
            sizing: Some(sizing),
            button_aspect,
            button_width,
            button_height,
            title_scale: parent.title_scale,
            has_title: parent.has_title,
            hide_buttons: parent.hide_buttons,
            radii: [
                parent.top_left_radius,
                parent.top_right_radius,
                parent.bottom_left_radius,
                parent.bottom_right_radius,
            ],
        }
    }

    /// Marco's `meta_frame_layout_validate`, in its order.
    fn validate(self) -> Parse<FrameLayout> {
        fn value(value: Option<i32>, name: &str) -> Parse<i32> {
            value.ok_or_else(|| {
                Failure::Invalid(format!(
                    "frame geometry does not specify \"{name}\" dimension"
                ))
            })
        }
        fn border(border: Option<Border>, name: &str) -> Parse<Border> {
            border.ok_or_else(|| {
                Failure::Invalid(format!(
                    "frame geometry does not specify dimension \"top\" for border \"{name}\""
                ))
            })
        }
        let left_width = value(self.left_width, "left_width")?;
        let right_width = value(self.right_width, "right_width")?;
        let bottom_height = value(self.bottom_height, "bottom_height")?;
        let title_border = border(self.title_border, "title_border")?;
        let title_vertical_pad = value(self.title_vertical_pad, "title_vertical_pad")?;
        let right_titlebar_edge = value(self.right_titlebar_edge, "right_titlebar_edge")?;
        let left_titlebar_edge = value(self.left_titlebar_edge, "left_titlebar_edge")?;
        let button_sizing = match self.sizing {
            Some(Sizing::Aspect) => {
                if !(0.1..=15.0).contains(&self.button_aspect) {
                    return invalid(format!(
                        "Button aspect ratio {} is not reasonable",
                        self.button_aspect
                    ));
                }
                ButtonSizing::Aspect(self.button_aspect)
            }
            Some(Sizing::Fixed) => ButtonSizing::Fixed {
                width: value(self.button_width, "button_width")?,
                height: value(self.button_height, "button_height")?,
            },
            None => return invalid("Frame geometry does not specify size of buttons"),
        };
        let button_border = border(self.button_border, "button_border")?;
        let [
            top_left_radius,
            top_right_radius,
            bottom_left_radius,
            bottom_right_radius,
        ] = self.radii;
        Ok(FrameLayout {
            left_width,
            right_width,
            bottom_height,
            title_border,
            button_border,
            title_vertical_pad,
            left_titlebar_edge,
            right_titlebar_edge,
            button_sizing,
            title_scale: self.title_scale,
            has_title: self.has_title,
            hide_buttons: self.hide_buttons,
            top_left_radius,
            top_right_radius,
            bottom_left_radius,
            bottom_right_radius,
        })
    }
}

/// A `<draw_ops>` list being parsed, named unless it's inside a piece or button.
struct OpListDraft {
    name: Option<String>,
    ops: Vec<DrawOp>,
}

struct GradientDraft {
    kind: GradientType,
    colors: Vec<ColorSpec>,
    alpha: Option<AlphaGradient>,
    rect: DrawRect,
}

/// A theme's `<info>`, which Marco requires in full.
#[derive(Default)]
struct InfoDraft {
    name: Option<String>,
    author: Option<String>,
    copyright: Option<String>,
    date: Option<String>,
    description: Option<String>,
}

/// The theme being parsed.
struct ThemeDraft {
    format_version: u32,
    info: InfoDraft,
    constants: Constants,
    geometries: HashMap<String, Arc<FrameLayout>>,
    draw_ops: HashMap<String, Arc<DrawOpList>>,
    styles: HashMap<String, Arc<FrameStyle>>,
    style_sets: HashMap<String, Arc<FrameStyleSet>>,
    window_style_sets: [Option<Arc<FrameStyleSet>>; FrameType::ALL.len()],
}

struct Parser<'a> {
    theme_name: &'a str,
    path: &'a Path,
    format_version: u32,
    states: Vec<State>,
    required_versions: Vec<u32>,
    skip_level: usize,
    theme: Option<Theme>,
    draft: Option<ThemeDraft>,
    geometry: Option<(String, GeometryDraft)>,
    op_list: Option<OpListDraft>,
    /// An inline list just closed, waiting for its piece, button or menu icon to close.
    finished_list: Option<Arc<DrawOpList>>,
    gradient: Option<GradientDraft>,
    style: Option<(String, FrameStyle)>,
    style_set: Option<(String, FrameStyleSet)>,
    piece: FramePiece,
    button: (ButtonType, ButtonState),
}

impl<'a> Parser<'a> {
    fn new(theme_name: &'a str, path: &'a Path, major_version: u32) -> Self {
        Parser {
            theme_name,
            path,
            format_version: 1000 * major_version,
            states: vec![State::Start],
            required_versions: Vec::new(),
            skip_level: 0,
            theme: None,
            draft: None,
            geometry: None,
            op_list: None,
            finished_list: None,
            gradient: None,
            style: None,
            style_set: None,
            piece: FramePiece::EntireBackground,
            button: (ButtonType::Close, ButtonState::Normal),
        }
    }

    fn state(&self) -> State {
        *self.states.last().expect("the start state is never popped")
    }

    fn push(&mut self, state: State) {
        self.states.push(state);
    }

    fn pop(&mut self) {
        self.states.pop();
    }

    fn required_version(&self) -> u32 {
        self.required_versions
            .last()
            .copied()
            .unwrap_or(self.format_version)
    }

    fn draft(&mut self) -> &mut ThemeDraft {
        self.draft
            .as_mut()
            .expect("elements below the root have a theme")
    }

    fn constants(&self) -> &Constants {
        &self
            .draft
            .as_ref()
            .expect("elements below the root have a theme")
            .constants
    }

    fn start(&mut self, element: &str, attributes: &[(String, String)]) -> Parse {
        if self.skip_level > 0 {
            self.skip_level += 1;
            return Ok(());
        }
        let mut required = self.required_version();
        if let Some((_, version)) = attributes.iter().find(|(name, _)| name == "version") {
            if required < 3000 {
                return invalid(
                    "\"version\" attribute cannot be used in metacity-theme-1.xml or \
                     metacity-theme-2.xml",
                );
            }
            let (satisfied, element_required) = check_version(version)?;
            // An unsatisfied check on the root makes Marco try the theme's older files; on
            // any other element it skips the element and its children.
            if self.state() == State::Start {
                if !satisfied {
                    return Err(Failure::Unsupported(format!(
                        "Theme requires version {version} but latest supported theme version \
                         is {}.{}",
                        THEME_VERSION / 1000,
                        THEME_VERSION % 1000
                    )));
                }
                self.format_version = self.format_version.max(element_required);
            } else if !satisfied {
                self.skip_level = 1;
                return Ok(());
            }
            required = required.max(element_required);
        }
        self.required_versions.push(required);

        match self.state() {
            State::Start => {
                if element != "metacity_theme" {
                    return invalid(format!(
                        "Outermost element in theme must be <metacity_theme> not <{element}>"
                    ));
                }
                self.draft = Some(ThemeDraft {
                    format_version: self.format_version,
                    info: InfoDraft::default(),
                    constants: Constants::default(),
                    geometries: Default::default(),
                    draw_ops: Default::default(),
                    styles: Default::default(),
                    style_sets: Default::default(),
                    window_style_sets: Default::default(),
                });
                self.push(State::Theme);
                Ok(())
            }
            State::Theme => self.toplevel_element(element, attributes),
            State::Info => self.info_element(element, attributes),
            State::Name | State::Author | State::Copyright | State::Date | State::Description => {
                invalid(format!(
                    "Element <{element}> is not allowed inside a name/author/date/description \
                     element"
                ))
            }
            State::Constant => invalid(format!(
                "Element <{element}> is not allowed inside a <constant> element"
            )),
            State::FrameGeometry => self.geometry_element(element, attributes),
            State::Distance | State::Border | State::AspectRatio => invalid(format!(
                "Element <{element}> is not allowed inside a distance/border/aspect_ratio element"
            )),
            State::DrawOps => self.draw_op_element(element, attributes),
            State::Op => invalid(format!(
                "Element <{element}> is not allowed inside a draw operation element"
            )),
            State::Gradient => self.gradient_element(element, attributes),
            State::Color => invalid(format!(
                "Element <{element}> is not allowed inside a <color> element"
            )),
            State::FrameStyle => self.style_element(element, attributes),
            State::Piece => self.inline_draw_ops(element, attributes, "piece"),
            State::Button => self.inline_draw_ops(element, attributes, "button"),
            State::MenuIcon => self.inline_draw_ops(element, attributes, "menu_icon"),
            State::Shadow => not_allowed_below(element, "shadow"),
            State::Padding => not_allowed_below(element, "padding"),
            State::FrameStyleSet => self.style_set_element(element, attributes),
            State::Frame => invalid(format!(
                "Element <{element}> is not allowed inside a <frame> element"
            )),
            State::Window => invalid(format!(
                "Element <{element}> is not allowed inside a <window> element"
            )),
            State::Fallback => invalid(format!(
                "Element <{element}> is not allowed inside a <fallback> element"
            )),
        }
    }

    fn toplevel_element(&mut self, element: &str, attributes: &[(String, String)]) -> Parse {
        match element {
            "info" => {
                check_no_attributes(element, attributes)?;
                self.push(State::Info);
            }
            "constant" => {
                let [name, value] = locate(element, attributes, ["!name", "!value"])?;
                let (name, value) = (name.unwrap(), value.unwrap());
                self.define_constant(name, value)?;
                self.push(State::Constant);
            }
            "frame_geometry" => {
                let [
                    name,
                    parent,
                    has_title,
                    title_scale,
                    top_left,
                    top_right,
                    bottom_left,
                    bottom_right,
                    hide_buttons,
                ] = locate(
                    element,
                    attributes,
                    [
                        "!name",
                        "parent",
                        "has_title",
                        "title_scale",
                        "rounded_top_left",
                        "rounded_top_right",
                        "rounded_bottom_left",
                        "rounded_bottom_right",
                        "hide_buttons",
                    ],
                )?;
                let name = name.unwrap();
                let has_title = has_title.map(parse_boolean).transpose()?;
                let hide_buttons = hide_buttons
                    .map(parse_boolean)
                    .transpose()?
                    .unwrap_or(false);
                let mut radii = [None; 4];
                for (slot, radius) in
                    radii
                        .iter_mut()
                        .zip([top_left, top_right, bottom_left, bottom_right])
                {
                    *slot = radius
                        .map(|radius| self.parse_rounding(radius))
                        .transpose()?;
                }
                let title_scale = title_scale.map(parse_title_scale).transpose()?;
                if self.draft().geometries.contains_key(name) {
                    return invalid(format!("<{element}> name \"{name}\" used a second time"));
                }
                let mut geometry = match parent {
                    Some(parent) => match self.draft().geometries.get(parent) {
                        Some(parent) => GeometryDraft::from_parent(parent),
                        None => {
                            return invalid(format!(
                                "<{element}> parent \"{parent}\" has not been defined"
                            ));
                        }
                    },
                    None => GeometryDraft::new(),
                };
                if let Some(has_title) = has_title {
                    geometry.has_title = has_title;
                }
                // Marco only ever turns hiding on, so a child can't turn it off.
                if hide_buttons {
                    geometry.hide_buttons = true;
                }
                if let Some(title_scale) = title_scale {
                    geometry.title_scale = title_scale;
                }
                for (slot, radius) in geometry.radii.iter_mut().zip(radii) {
                    if let Some(radius) = radius {
                        *slot = radius;
                    }
                }
                self.geometry = Some((name.to_owned(), geometry));
                self.push(State::FrameGeometry);
            }
            "draw_ops" => {
                let [name] = locate(element, attributes, ["!name"])?;
                let name = name.unwrap();
                if self.draft().draw_ops.contains_key(name) {
                    return invalid(format!("<{element}> name \"{name}\" used a second time"));
                }
                self.op_list = Some(OpListDraft {
                    name: Some(name.to_owned()),
                    ops: Vec::new(),
                });
                self.push(State::DrawOps);
            }
            "frame_style" => {
                let [name, parent, geometry, background, alpha] = locate(
                    element,
                    attributes,
                    ["!name", "parent", "geometry", "background", "alpha"],
                )?;
                let name = name.unwrap();
                if self.draft().styles.contains_key(name) {
                    return invalid(format!("<{element}> name \"{name}\" used a second time"));
                }
                let parent = match parent {
                    Some(parent) => match self.draft().styles.get(parent) {
                        Some(parent) => Some(parent.clone()),
                        None => {
                            return invalid(format!(
                                "<{element}> parent \"{parent}\" has not been defined"
                            ));
                        }
                    },
                    None => None,
                };
                let geometry = match geometry {
                    Some(geometry) => match self.draft().geometries.get(geometry) {
                        Some(geometry) => geometry.clone(),
                        None => {
                            return invalid(format!(
                                "<{element}> geometry \"{geometry}\" has not been defined"
                            ));
                        }
                    },
                    None => match &parent {
                        Some(parent) => parent.geometry.clone(),
                        None => {
                            return invalid(format!(
                                "<{element}> must specify either a geometry or a parent that \
                                 has a geometry"
                            ));
                        }
                    },
                };
                let mut window_background = None;
                let mut window_background_alpha = 255;
                if let Some(background) = background {
                    window_background =
                        Some(ColorSpec::parse(background).map_err(Failure::Invalid)?);
                    if let Some(alpha) = alpha {
                        window_background_alpha = parse_alpha(alpha)?.0[0];
                    }
                } else if alpha.is_some() {
                    return invalid(
                        "You must specify a background for an alpha value to be meaningful",
                    );
                }
                self.style = Some((
                    name.to_owned(),
                    FrameStyle {
                        parent,
                        geometry,
                        window_background,
                        window_background_alpha,
                        pieces: Default::default(),
                        buttons: Default::default(),
                    },
                ));
                self.push(State::FrameStyle);
            }
            "frame_style_set" => {
                let [name, parent] = locate(element, attributes, ["!name", "parent"])?;
                let name = name.unwrap();
                if self.draft().style_sets.contains_key(name) {
                    return invalid(format!("<{element}> name \"{name}\" used a second time"));
                }
                let parent = match parent {
                    Some(parent) => match self.draft().style_sets.get(parent) {
                        Some(parent) => Some(parent.clone()),
                        None => {
                            return invalid(format!(
                                "<{element}> parent \"{parent}\" has not been defined"
                            ));
                        }
                    },
                    None => None,
                };
                self.style_set = Some((
                    name.to_owned(),
                    FrameStyleSet {
                        parent,
                        ..Default::default()
                    },
                ));
                self.push(State::FrameStyleSet);
            }
            "window" => {
                let [type_name, style_set] = locate(element, attributes, ["!type", "!style_set"])?;
                let (type_name, style_set_name) = (type_name.unwrap(), style_set.unwrap());
                let frame_type = FrameType::from_name(type_name).filter(|&frame_type| {
                    frame_type != FrameType::Attached || self.required_version() >= 3002
                });
                let Some(frame_type) = frame_type else {
                    return invalid(format!(
                        "Unknown type \"{type_name}\" on <{element}> element"
                    ));
                };
                let Some(style_set) = self.draft().style_sets.get(style_set_name).cloned() else {
                    return invalid(format!(
                        "Unknown style_set \"{style_set_name}\" on <{element}> element"
                    ));
                };
                let slot = &mut self.draft().window_style_sets[frame_type as usize];
                if slot.is_some() {
                    return invalid(format!(
                        "Window type \"{type_name}\" has already been assigned a style set"
                    ));
                }
                *slot = Some(style_set);
                self.push(State::Window);
            }
            // Neither is used any more, but Marco still parses them.
            "menu_icon" => self.push(State::MenuIcon),
            "fallback" => self.push(State::Fallback),
            _ => return not_allowed_below(element, "metacity_theme"),
        }
        Ok(())
    }

    fn define_constant(&mut self, name: &str, value: &str) -> Parse {
        // Marco guesses the type from the value: a number with a point is a float,
        // another number an integer, and anything else a colour.
        let first = value.bytes().next();
        let numeric = matches!(first, Some(b'.' | b'+' | b'-' | b'0'..=b'9'));
        if numeric && value.contains('.') {
            let value = parse_double(value)?;
            check_constant_name(name, self.draft().constants.floats.contains_key(name))?;
            self.draft().constants.floats.insert(name.to_owned(), value);
        } else if numeric {
            let value = self.parse_positive_integer(value)?;
            check_constant_name(name, self.draft().constants.ints.contains_key(name))?;
            self.draft().constants.ints.insert(name.to_owned(), value);
        } else {
            check_constant_name(name, self.draft().constants.colors.contains_key(name))?;
            self.draft()
                .constants
                .colors
                .insert(name.to_owned(), value.to_owned());
        }
        Ok(())
    }

    fn info_element(&mut self, element: &str, attributes: &[(String, String)]) -> Parse {
        let state = match element {
            "name" => State::Name,
            "author" => State::Author,
            "copyright" => State::Copyright,
            "description" => State::Description,
            "date" => State::Date,
            _ => return not_allowed_below(element, "info"),
        };
        check_no_attributes(element, attributes)?;
        self.push(state);
        Ok(())
    }

    fn geometry_element(&mut self, element: &str, attributes: &[(String, String)]) -> Parse {
        match element {
            "distance" => {
                let [name, value] = locate(element, attributes, ["!name", "!value"])?;
                let (name, value) = (name.unwrap(), value.unwrap());
                let value = self.parse_positive_integer(value)?;
                let geometry = &mut self.geometry.as_mut().expect("inside a geometry").1;
                let field = match name {
                    "left_width" => &mut geometry.left_width,
                    "right_width" => &mut geometry.right_width,
                    "bottom_height" => &mut geometry.bottom_height,
                    "title_vertical_pad" => &mut geometry.title_vertical_pad,
                    "right_titlebar_edge" => &mut geometry.right_titlebar_edge,
                    "left_titlebar_edge" => &mut geometry.left_titlebar_edge,
                    "button_width" | "button_height" => {
                        if name == "button_width" {
                            geometry.button_width = Some(value);
                        } else {
                            geometry.button_height = Some(value);
                        }
                        if geometry.sizing == Some(Sizing::Aspect) {
                            return invalid(BOTH_SIZINGS);
                        }
                        geometry.sizing = Some(Sizing::Fixed);
                        self.push(State::Distance);
                        return Ok(());
                    }
                    _ => return invalid(format!("Distance \"{name}\" is unknown")),
                };
                *field = Some(value);
                self.push(State::Distance);
            }
            "border" => {
                let [name, top, bottom, left, right] = locate(
                    element,
                    attributes,
                    ["!name", "!top", "!bottom", "!left", "!right"],
                )?;
                let border = Border {
                    top: self.parse_positive_integer(top.unwrap())?,
                    bottom: self.parse_positive_integer(bottom.unwrap())?,
                    left: self.parse_positive_integer(left.unwrap())?,
                    right: self.parse_positive_integer(right.unwrap())?,
                };
                let geometry = &mut self.geometry.as_mut().expect("inside a geometry").1;
                match name.unwrap() {
                    "title_border" => geometry.title_border = Some(border),
                    "button_border" => geometry.button_border = Some(border),
                    name => return invalid(format!("Border \"{name}\" is unknown")),
                }
                self.push(State::Border);
            }
            "aspect_ratio" => {
                let [name, value] = locate(element, attributes, ["!name", "!value"])?;
                let value = parse_double(value.unwrap())?;
                let geometry = &mut self.geometry.as_mut().expect("inside a geometry").1;
                match name.unwrap() {
                    "button" => {
                        geometry.button_aspect = value;
                        if geometry.sizing.is_some() {
                            return invalid(BOTH_SIZINGS);
                        }
                        geometry.sizing = Some(Sizing::Aspect);
                    }
                    name => return invalid(format!("Aspect ratio \"{name}\" is unknown")),
                }
                self.push(State::AspectRatio);
            }
            _ => return not_allowed_below(element, "frame_geometry"),
        }
        Ok(())
    }

    fn expr(&self, text: &str) -> Expr {
        Expr::parse(text, self.constants())
    }

    fn rect(&self, x: &str, y: &str, width: &str, height: &str) -> DrawRect {
        DrawRect {
            x: self.expr(x),
            y: self.expr(y),
            width: self.expr(width),
            height: self.expr(height),
        }
    }

    /// Marco's `parse_color`: a whole value may name a colour constant.
    fn color(&self, text: &str) -> Parse<ColorSpec> {
        let text = self
            .constants()
            .colors
            .get(text)
            .map_or(text, String::as_str);
        ColorSpec::parse(text).map_err(Failure::Invalid)
    }

    fn append(&mut self, op: DrawOp) {
        self.op_list
            .as_mut()
            .expect("draw ops are inside a list")
            .ops
            .push(op);
    }

    fn draw_op_element(&mut self, element: &str, attributes: &[(String, String)]) -> Parse {
        let op = match element {
            "line" => {
                let [color, x1, y1, x2, y2, dash_on, dash_off, width] = locate(
                    element,
                    attributes,
                    [
                        "!color",
                        "!x1",
                        "!y1",
                        "!x2",
                        "!y2",
                        "dash_on_length",
                        "dash_off_length",
                        "width",
                    ],
                )?;
                let (x1, y1, x2, y2) = (x1.unwrap(), y1.unwrap(), x2.unwrap(), y2.unwrap());
                let dash_on_length = self.optional_integer(dash_on)?;
                let dash_off_length = self.optional_integer(dash_off)?;
                let width = self.optional_integer(width)?;
                DrawOp::Line {
                    color: self.color(color.unwrap())?,
                    x1: self.expr(x1),
                    y1: self.expr(y1),
                    x2: (x2 != x1).then(|| self.expr(x2)),
                    y2: (y2 != y1).then(|| self.expr(y2)),
                    width,
                    dash_on_length,
                    dash_off_length,
                }
            }
            "rectangle" => {
                let [color, x, y, width, height, filled] = locate(
                    element,
                    attributes,
                    ["!color", "!x", "!y", "!width", "!height", "filled"],
                )?;
                let filled = filled.map(parse_boolean).transpose()?.unwrap_or(false);
                DrawOp::Rectangle {
                    color: self.color(color.unwrap())?,
                    rect: self.rect(x.unwrap(), y.unwrap(), width.unwrap(), height.unwrap()),
                    filled,
                }
            }
            "arc" => {
                let [
                    color,
                    x,
                    y,
                    width,
                    height,
                    filled,
                    start_angle,
                    extent_angle,
                    from,
                    to,
                ] = locate(
                    element,
                    attributes,
                    [
                        "!color",
                        "!x",
                        "!y",
                        "!width",
                        "!height",
                        "filled",
                        "start_angle",
                        "extent_angle",
                        "from",
                        "to",
                    ],
                )?;
                if start_angle.is_none() && from.is_none() {
                    return invalid(format!(
                        "No \"start_angle\" or \"from\" attribute on element <{element}>"
                    ));
                }
                if extent_angle.is_none() && to.is_none() {
                    return invalid(format!(
                        "No \"extent_angle\" or \"to\" attribute on element <{element}>"
                    ));
                }
                // v2's from and to are degrees clockwise from up, which Marco converts
                // this way.
                let start_angle = match (start_angle, from) {
                    (Some(start_angle), _) => parse_angle(start_angle)?,
                    (None, from) => (180.0 - parse_angle(from.unwrap())?) / 360.0,
                };
                let extent_angle = match (extent_angle, to) {
                    (Some(extent_angle), _) => parse_angle(extent_angle)?,
                    (None, to) => ((180.0 - parse_angle(to.unwrap())?) / 360.0) - start_angle,
                };
                let filled = filled.map(parse_boolean).transpose()?.unwrap_or(false);
                DrawOp::Arc {
                    color: self.color(color.unwrap())?,
                    rect: self.rect(x.unwrap(), y.unwrap(), width.unwrap(), height.unwrap()),
                    filled,
                    start_angle,
                    extent_angle,
                }
            }
            "clip" => {
                let [x, y, width, height] =
                    locate(element, attributes, ["!x", "!y", "!width", "!height"])?;
                DrawOp::Clip {
                    rect: self.rect(x.unwrap(), y.unwrap(), width.unwrap(), height.unwrap()),
                }
            }
            "tint" => {
                let [color, x, y, width, height, alpha] = locate(
                    element,
                    attributes,
                    ["!color", "!x", "!y", "!width", "!height", "!alpha"],
                )?;
                let alpha = parse_alpha(alpha.unwrap())?;
                DrawOp::Tint {
                    color: self.color(color.unwrap())?,
                    alpha,
                    rect: self.rect(x.unwrap(), y.unwrap(), width.unwrap(), height.unwrap()),
                }
            }
            "gradient" => {
                let [kind, x, y, width, height, alpha] = locate(
                    element,
                    attributes,
                    ["!type", "!x", "!y", "!width", "!height", "alpha"],
                )?;
                let kind_name = kind.unwrap();
                let kind = match kind_name {
                    "vertical" => GradientType::Vertical,
                    "horizontal" => GradientType::Horizontal,
                    "diagonal" => GradientType::Diagonal,
                    _ => {
                        return invalid(format!(
                            "Did not understand value \"{kind_name}\" for type of gradient"
                        ));
                    }
                };
                let alpha = alpha.map(parse_alpha).transpose()?;
                // The op is appended when the element closes, with its colours.
                self.gradient = Some(GradientDraft {
                    kind,
                    colors: Vec::new(),
                    alpha,
                    rect: self.rect(x.unwrap(), y.unwrap(), width.unwrap(), height.unwrap()),
                });
                self.push(State::Gradient);
                return Ok(());
            }
            "image" => {
                let [x, y, width, height, alpha, filename, colorize, fill_type] = locate(
                    element,
                    attributes,
                    [
                        "!x",
                        "!y",
                        "!width",
                        "!height",
                        "alpha",
                        "!filename",
                        "colorize",
                        "fill_type",
                    ],
                )?;
                let fill = parse_fill(element, fill_type)?;
                let source = self.image_source(filename.unwrap())?;
                let colorize = colorize.map(|colorize| self.color(colorize)).transpose()?;
                let alpha = alpha.map(parse_alpha).transpose()?;
                DrawOp::Image {
                    source,
                    colorize,
                    alpha,
                    rect: self.rect(x.unwrap(), y.unwrap(), width.unwrap(), height.unwrap()),
                    fill,
                }
            }
            "gtk_arrow" => {
                let [state, shadow, arrow, x, y, width, height, filled] = locate(
                    element,
                    attributes,
                    [
                        "!state", "!shadow", "!arrow", "!x", "!y", "!width", "!height", "filled",
                    ],
                )?;
                let filled = filled.map(parse_boolean).transpose()?.unwrap_or(true);
                let state = parse_gtk_state(element, state.unwrap())?;
                let shadow = parse_shadow(element, shadow.unwrap())?;
                let arrow_name = arrow.unwrap();
                let arrow = match arrow_name {
                    "up" => GtkArrow::Up,
                    "down" => GtkArrow::Down,
                    "left" => GtkArrow::Left,
                    "right" => GtkArrow::Right,
                    "none" => GtkArrow::None,
                    _ => {
                        return invalid(format!(
                            "Did not understand arrow \"{arrow_name}\" for <{element}> element"
                        ));
                    }
                };
                DrawOp::GtkArrow {
                    state,
                    shadow,
                    arrow,
                    filled,
                    rect: self.rect(x.unwrap(), y.unwrap(), width.unwrap(), height.unwrap()),
                }
            }
            "gtk_box" => {
                let [state, shadow, x, y, width, height] = locate(
                    element,
                    attributes,
                    ["!state", "!shadow", "!x", "!y", "!width", "!height"],
                )?;
                DrawOp::GtkBox {
                    state: parse_gtk_state(element, state.unwrap())?,
                    shadow: parse_shadow(element, shadow.unwrap())?,
                    rect: self.rect(x.unwrap(), y.unwrap(), width.unwrap(), height.unwrap()),
                }
            }
            "gtk_vline" => {
                let [state, x, y1, y2] =
                    locate(element, attributes, ["!state", "!x", "!y1", "!y2"])?;
                DrawOp::GtkVline {
                    state: parse_gtk_state(element, state.unwrap())?,
                    x: self.expr(x.unwrap()),
                    y1: self.expr(y1.unwrap()),
                    y2: self.expr(y2.unwrap()),
                }
            }
            "icon" => {
                let [x, y, width, height, alpha, fill_type] = locate(
                    element,
                    attributes,
                    ["!x", "!y", "!width", "!height", "alpha", "fill_type"],
                )?;
                let fill = parse_fill(element, fill_type)?;
                let alpha = alpha.map(parse_alpha).transpose()?;
                DrawOp::Icon {
                    alpha,
                    rect: self.rect(x.unwrap(), y.unwrap(), width.unwrap(), height.unwrap()),
                    fill,
                }
            }
            "title" => {
                let [color, x, y, ellipsize_width] = locate(
                    element,
                    attributes,
                    ["!color", "!x", "!y", "ellipsize_width"],
                )?;
                // Marco's message for an attribute from a newer version.
                if ellipsize_width.is_some() && self.required_version() < 3001 {
                    return invalid(format!(
                        "No \"ellipsize_width\" attribute on element <{element}>"
                    ));
                }
                DrawOp::Title {
                    color: self.color(color.unwrap())?,
                    x: self.expr(x.unwrap()),
                    y: self.expr(y.unwrap()),
                    ellipsize_width: ellipsize_width.map(|width| self.expr(width)),
                }
            }
            "include" => {
                let [x, y, width, height, name] =
                    locate(element, attributes, ["x", "y", "width", "height", "!name"])?;
                DrawOp::Include {
                    list: self.included_list(name.unwrap())?,
                    rect: self.rect(
                        x.unwrap_or("0"),
                        y.unwrap_or("0"),
                        width.unwrap_or("width"),
                        height.unwrap_or("height"),
                    ),
                }
            }
            "tile" => {
                let [
                    x,
                    y,
                    width,
                    height,
                    name,
                    xoffset,
                    yoffset,
                    tile_width,
                    tile_height,
                ] = locate(
                    element,
                    attributes,
                    [
                        "x",
                        "y",
                        "width",
                        "height",
                        "!name",
                        "tile_xoffset",
                        "tile_yoffset",
                        "!tile_width",
                        "!tile_height",
                    ],
                )?;
                DrawOp::Tile {
                    list: self.included_list(name.unwrap())?,
                    rect: self.rect(
                        x.unwrap_or("0"),
                        y.unwrap_or("0"),
                        width.unwrap_or("width"),
                        height.unwrap_or("height"),
                    ),
                    tile_xoffset: self.expr(xoffset.unwrap_or("0")),
                    tile_yoffset: self.expr(yoffset.unwrap_or("0")),
                    tile_width: self.expr(tile_width.unwrap()),
                    tile_height: self.expr(tile_height.unwrap()),
                }
            }
            _ => return not_allowed_below(element, "draw_ops"),
        };
        self.append(op);
        self.push(State::Op);
        Ok(())
    }

    fn optional_integer(&self, text: Option<&str>) -> Parse<i32> {
        text.map_or(Ok(0), |text| self.parse_positive_integer(text))
    }

    /// Marco loads images when it parses them, and a missing one fails the theme.
    /// Decoding them is the drawing ticket's (27).
    fn image_source(&self, filename: &str) -> Parse<ImageSource> {
        if let Some(icon) = filename.strip_prefix("theme:") {
            return Ok(ImageSource::IconTheme(icon.to_owned()));
        }
        let dir = self.path.parent().unwrap_or(Path::new(""));
        // g_build_filename joins an absolute filename onto the directory too.
        let path: PathBuf = dir.join(filename.trim_start_matches('/'));
        if !path.is_file() {
            return invalid(format!(
                "Failed to load image \"{filename}\": {} is not a file",
                path.display()
            ));
        }
        Ok(ImageSource::File(path))
    }

    fn included_list(&mut self, name: &str) -> Parse<Arc<DrawOpList>> {
        let current = self.op_list.as_ref().and_then(|list| list.name.as_deref());
        // Marco registers a list when it opens, so it finds the list being defined and
        // reports the cycle. Lists defined earlier can't contain this one.
        if current == Some(name) {
            return invalid(format!(
                "Including draw_ops \"{name}\" here would create a circular reference"
            ));
        }
        match self.draft().draw_ops.get(name) {
            Some(list) => Ok(list.clone()),
            None => invalid(format!("No <draw_ops> called \"{name}\" has been defined")),
        }
    }

    fn gradient_element(&mut self, element: &str, attributes: &[(String, String)]) -> Parse {
        if element != "color" {
            return not_allowed_below(element, "gradient");
        }
        let [value] = locate(element, attributes, ["!value"])?;
        let color = self.color(value.unwrap())?;
        self.gradient
            .as_mut()
            .expect("inside a gradient")
            .colors
            .push(color);
        self.push(State::Color);
        Ok(())
    }

    fn named_list(&mut self, name: Option<&str>) -> Parse<Option<Arc<DrawOpList>>> {
        let Some(name) = name else {
            return Ok(None);
        };
        match self.draft().draw_ops.get(name) {
            Some(list) => Ok(Some(list.clone())),
            None => invalid(format!(
                "No <draw_ops> with the name \"{name}\" has been defined"
            )),
        }
    }

    fn style_element(&mut self, element: &str, attributes: &[(String, String)]) -> Parse {
        match element {
            "piece" => {
                let [position, draw_ops] = locate(element, attributes, ["!position", "draw_ops"])?;
                let position = position.unwrap();
                let Some(piece) = FramePiece::from_name(position) else {
                    return invalid(format!("Unknown position \"{position}\" for frame piece"));
                };
                let style = &self.style.as_ref().expect("inside a style").1;
                if style.pieces[piece as usize].is_some() {
                    return invalid(format!(
                        "Frame style already has a piece at position {position}"
                    ));
                }
                self.piece = piece;
                self.finished_list = self.named_list(draw_ops)?;
                self.push(State::Piece);
            }
            "button" => {
                let [function, state, draw_ops] =
                    locate(element, attributes, ["!function", "!state", "draw_ops"])?;
                let (function, state) = (function.unwrap(), state.unwrap());
                let Some(button) = ButtonType::from_name(function) else {
                    return invalid(format!("Unknown function \"{function}\" for button"));
                };
                let required = self.required_version();
                if button.earliest_version() > required {
                    return invalid(format!(
                        "Button function \"{function}\" does not exist in this version ({required}, \
                         need {})",
                        button.earliest_version()
                    ));
                }
                let Some(button_state) = ButtonState::from_name(state) else {
                    return invalid(format!("Unknown state \"{state}\" for button"));
                };
                let style = &self.style.as_ref().expect("inside a style").1;
                if style.buttons[button as usize][button_state as usize].is_some() {
                    return invalid(format!(
                        "Frame style already has a button for function {function} state {state}"
                    ));
                }
                self.button = (button, button_state);
                self.finished_list = self.named_list(draw_ops)?;
                self.push(State::Button);
            }
            "shadow" => self.push(State::Shadow),
            "padding" => self.push(State::Padding),
            _ => return not_allowed_below(element, "frame_style"),
        }
        Ok(())
    }

    /// A `<draw_ops>` inside a piece, button or menu icon.
    fn inline_draw_ops(
        &mut self,
        element: &str,
        attributes: &[(String, String)],
        parent: &str,
    ) -> Parse {
        if element != "draw_ops" {
            return not_allowed_below(element, parent);
        }
        if self.finished_list.is_some() {
            return invalid(format!(
                "Can't have a two draw_ops for a <{parent}> element (theme specified a draw_ops \
                 attribute and also a <draw_ops> element, or specified two elements)"
            ));
        }
        check_no_attributes(element, attributes)?;
        self.op_list = Some(OpListDraft {
            name: None,
            ops: Vec::new(),
        });
        self.push(State::DrawOps);
        Ok(())
    }

    fn style_set_element(&mut self, element: &str, attributes: &[(String, String)]) -> Parse {
        if element != "frame" {
            return not_allowed_below(element, "frame_style_set");
        }
        let [focus, state, resize, style] = locate(
            element,
            attributes,
            ["!focus", "!state", "resize", "!style"],
        )?;
        let (focus_name, state_name, style_name) = (focus.unwrap(), state.unwrap(), style.unwrap());
        let Some(focus) = FrameFocus::from_name(focus_name) else {
            return invalid(format!(
                "\"{focus_name}\" is not a valid value for focus attribute"
            ));
        };
        // Marco's messages for a bad state or resize quote the focus attribute.
        let Some(state) = FrameState::from_name(state_name) else {
            return invalid(format!(
                "\"{focus_name}\" is not a valid value for state attribute"
            ));
        };
        let Some(style) = self.draft().styles.get(style_name).cloned() else {
            return invalid(format!(
                "A style called \"{style_name}\" has not been defined"
            ));
        };
        let parse_resize = |resize: &str| {
            FrameResize::from_name(resize).ok_or_else(|| {
                Failure::Invalid(format!(
                    "\"{focus_name}\" is not a valid value for resize attribute"
                ))
            })
        };
        let resize = match state {
            FrameState::Normal => match resize {
                Some(resize) => parse_resize(resize)?,
                None => {
                    return invalid(format!("No \"resize\" attribute on element <{element}>"));
                }
            },
            // Since v2 a shaded frame may omit resize, meaning both.
            FrameState::Shaded => resize.map_or(Ok(FrameResize::Both), parse_resize)?,
            _ => {
                if resize.is_some() {
                    return invalid(format!(
                        "Should not have \"resize\" attribute on <{element}> element for \
                         maximized states"
                    ));
                }
                FrameResize::Both
            }
        };
        let set = &mut self.style_set.as_mut().expect("inside a style set").1;
        let focus_index = focus as usize;
        let slot = match state {
            FrameState::Normal => &mut set.normal[resize as usize][focus_index],
            FrameState::Shaded => &mut set.shaded[resize as usize][focus_index],
            FrameState::Maximized => &mut set.maximized[focus_index],
            FrameState::TiledLeft => &mut set.tiled_left[focus_index],
            FrameState::TiledRight => &mut set.tiled_right[focus_index],
            FrameState::MaximizedAndShaded => &mut set.maximized_and_shaded[focus_index],
            FrameState::TiledLeftAndShaded => &mut set.tiled_left_and_shaded[focus_index],
            FrameState::TiledRightAndShaded => &mut set.tiled_right_and_shaded[focus_index],
        };
        if slot.is_some() {
            let message = match state {
                FrameState::Normal | FrameState::Shaded => format!(
                    "Style has already been specified for state {state_name} resize {} focus \
                     {focus_name}",
                    resize.name()
                ),
                _ => format!(
                    "Style has already been specified for state {state_name} focus {focus_name}"
                ),
            };
            return invalid(message);
        }
        *slot = Some(style);
        self.push(State::Frame);
        Ok(())
    }

    fn end(&mut self) -> Parse {
        if self.skip_level > 0 {
            self.skip_level -= 1;
            return Ok(());
        }
        let state = self.state();
        self.pop();
        match state {
            State::Start => {}
            State::Theme => {
                let draft = self.draft.take().expect("the root made a theme");
                self.theme = Some(self.validate_theme(draft)?);
            }
            State::FrameGeometry => {
                let (name, geometry) = self.geometry.take().expect("inside a geometry");
                let geometry = geometry.validate()?;
                self.draft().geometries.insert(name, Arc::new(geometry));
            }
            State::DrawOps => {
                let list = self.op_list.take().expect("inside a list");
                let ops = Arc::new(DrawOpList { ops: list.ops });
                match list.name {
                    Some(name) => {
                        self.draft().draw_ops.insert(name, ops);
                    }
                    None => self.finished_list = Some(ops),
                }
            }
            State::Gradient => {
                let gradient = self.gradient.take().expect("inside a gradient");
                if gradient.colors.len() < 2 {
                    return invalid("Gradients should have at least two colors");
                }
                self.append(DrawOp::Gradient {
                    kind: gradient.kind,
                    colors: gradient.colors,
                    alpha: gradient.alpha,
                    rect: gradient.rect,
                });
            }
            State::FrameStyle => {
                let (name, style) = self.style.take().expect("inside a style");
                validate_style(&style, self.required_version())?;
                self.draft().styles.insert(name, Arc::new(style));
            }
            State::Piece => {
                let Some(list) = self.finished_list.take() else {
                    return invalid("No draw_ops provided for frame piece");
                };
                let style = &mut self.style.as_mut().expect("inside a style").1;
                style.pieces[self.piece as usize] = Some(list);
            }
            State::Button => {
                let Some(list) = self.finished_list.take() else {
                    return invalid("No draw_ops provided for button");
                };
                let (button, state) = self.button;
                let style = &mut self.style.as_mut().expect("inside a style").1;
                style.buttons[button as usize][state as usize] = Some(list);
            }
            State::MenuIcon => self.finished_list = None,
            State::FrameStyleSet => {
                let (name, set) = self.style_set.take().expect("inside a style set");
                validate_style_set(&set)?;
                self.draft().style_sets.insert(name, Arc::new(set));
            }
            _ => {}
        }
        self.required_versions.pop();
        Ok(())
    }

    fn text(&mut self, text: &str) -> Parse {
        // g_ascii_isspace, which includes vertical tab and form feed.
        if self.skip_level > 0 || text.bytes().all(|c| c.is_ascii_whitespace() || c == 0x0b) {
            return Ok(());
        }
        let (field, name) = {
            let state = self.state();
            let info = &mut self.draft().info;
            match state {
                State::Name => (&mut info.name, "name"),
                State::Author => (&mut info.author, "author"),
                State::Copyright => (&mut info.copyright, "copyright"),
                State::Date => (&mut info.date, "date"),
                State::Description => (&mut info.description, "description"),
                state => {
                    return invalid(format!(
                        "No text is allowed inside element <{}>",
                        state.element_name()
                    ));
                }
            }
        };
        if field.is_some() {
            return invalid(format!("<{name}> specified twice for this theme"));
        }
        *field = Some(text.to_owned());
        Ok(())
    }

    /// Marco's `meta_theme_validate`.
    fn validate_theme(&self, draft: ThemeDraft) -> Parse<Theme> {
        let theme_name = self.theme_name;
        let required = |field: Option<String>, name: &str| {
            field.ok_or_else(|| {
                Failure::Invalid(format!("No <{name}> set for theme \"{theme_name}\""))
            })
        };
        let InfoDraft {
            name,
            author,
            copyright,
            date,
            description,
        } = draft.info;
        let name = required(name, "name")?;
        let author = required(author, "author")?;
        let date = required(date, "date")?;
        let description = required(description, "description")?;
        let copyright = required(copyright, "copyright")?;
        for frame_type in FrameType::ALL {
            if frame_type != FrameType::Attached
                && draft.window_style_sets[frame_type as usize].is_none()
            {
                let type_name = frame_type.name();
                return invalid(format!(
                    "No frame style set for window type \"{type_name}\" in theme \
                     \"{theme_name}\", add a <window type=\"{type_name}\" \
                     style_set=\"whatever\"/> element"
                ));
            }
        }
        Ok(Theme {
            name: theme_name.to_owned(),
            path: self.path.to_owned(),
            format_version: draft.format_version,
            info: ThemeInfo {
                name,
                author,
                copyright,
                date,
                description,
            },
            constants: draft.constants,
            geometries: draft.geometries,
            draw_ops: draft.draw_ops,
            styles: draft.styles,
            style_sets: draft.style_sets,
            window_style_sets: draft.window_style_sets,
            images: Default::default(),
        })
    }

    /// Marco's `parse_positive_integer`, which also takes an integer constant.
    fn parse_positive_integer(&self, text: &str) -> Parse<i32> {
        let value = match self.constants().ints.get(text) {
            Some(&value) => i64::from(value),
            None => {
                let Some((value, len)) = strtol_prefix(text) else {
                    return invalid(format!("Could not parse \"{text}\" as an integer"));
                };
                if len != text.len() {
                    return invalid(format!(
                        "Did not understand trailing characters \"{}\" in string \"{text}\"",
                        &text[len..]
                    ));
                }
                value
            }
        };
        if value < 0 {
            return invalid(format!("Integer {value} must be positive"));
        }
        if value > MAX_REASONABLE {
            return invalid(format!(
                "Integer {value} is too large, current max is {MAX_REASONABLE}"
            ));
        }
        Ok(value as i32)
    }

    /// A corner radius: a boolean, where true is 5 pixels, or an integer.
    fn parse_rounding(&self, text: &str) -> Parse<u32> {
        match text {
            "true" => Ok(5),
            "false" => Ok(0),
            _ => self.parse_positive_integer(text).map(|value| value as u32),
        }
    }
}

const BOTH_SIZINGS: &str =
    "Cannot specify both \"button_width\"/\"button_height\" and \"aspect_ratio\" for buttons";

fn not_allowed_below(element: &str, parent: &str) -> Parse {
    invalid(format!(
        "Element <{element}> is not allowed below <{parent}>"
    ))
}

impl State {
    /// The element this state is inside, for Marco's "No text is allowed" message.
    fn element_name(self) -> &'static str {
        match self {
            State::Start | State::Theme => "metacity_theme",
            State::Info => "info",
            State::Name => "name",
            State::Author => "author",
            State::Copyright => "copyright",
            State::Date => "date",
            State::Description => "description",
            State::Constant => "constant",
            State::FrameGeometry => "frame_geometry",
            State::Distance => "distance",
            State::Border => "border",
            State::AspectRatio => "aspect_ratio",
            State::DrawOps => "draw_ops",
            State::Op => "draw operation",
            State::Gradient => "gradient",
            State::Color => "color",
            State::FrameStyle => "frame_style",
            State::Piece => "piece",
            State::Button => "button",
            State::Shadow => "shadow",
            State::Padding => "padding",
            State::FrameStyleSet => "frame_style_set",
            State::Frame => "frame",
            State::Window => "window",
            State::MenuIcon => "menu_icon",
            State::Fallback => "fallback",
        }
    }
}

/// Marco's `locate_attributes`: the values of the named attributes, in order. A leading
/// `!` marks a required one. `version` may appear on any element, and anything else not
/// named is an error.
fn locate<'v, const N: usize>(
    element: &str,
    attributes: &'v [(String, String)],
    names: [&str; N],
) -> Parse<[Option<&'v str>; N]> {
    let mut values = [None; N];
    for (name, value) in attributes {
        if name == "version" {
            continue;
        }
        let Some(index) = names
            .iter()
            .position(|wanted| wanted.trim_start_matches('!') == name)
        else {
            return invalid(format!(
                "Attribute \"{name}\" is invalid on <{element}> element in this context"
            ));
        };
        if values[index].is_some() {
            return invalid(format!(
                "Attribute \"{name}\" repeated twice on the same <{element}> element"
            ));
        }
        values[index] = Some(value.as_str());
    }
    for (wanted, value) in names.iter().zip(&values) {
        if let Some(name) = wanted.strip_prefix('!')
            && value.is_none()
        {
            return invalid(format!("No \"{name}\" attribute on element <{element}>"));
        }
    }
    Ok(values)
}

/// Marco's `check_no_attributes`, which allows only a leading `version`.
fn check_no_attributes(element: &str, attributes: &[(String, String)]) -> Parse {
    let skip = usize::from(
        attributes
            .first()
            .is_some_and(|(name, _)| name == "version"),
    );
    if attributes.len() > skip {
        // Marco names the first attribute, whichever one is wrong.
        return invalid(format!(
            "Attribute \"{}\" is invalid on <{element}> element in this context",
            attributes[0].0
        ));
    }
    Ok(())
}

/// Marco's `check_version`: whether Marco's 3.5 satisfies a `version` attribute, and the
/// minimum version it implies.
fn check_version(text: &str) -> Parse<(bool, u32)> {
    let bad = || Failure::Invalid(format!("Bad version specification '{text}'"));
    let spec = text.trim_matches(|c: char| c.is_ascii_whitespace());
    let (comparison, rest) = ["<=", ">=", "<", ">"]
        .into_iter()
        .find_map(|op| spec.strip_prefix(op).map(|rest| (op, rest)))
        .ok_or_else(bad)?;
    let rest = rest.trim_start_matches(|c: char| c.is_ascii_whitespace());
    let (major, minor) = match rest.split_once('.') {
        Some((major, minor)) => (major, Some(minor)),
        None => (rest, None),
    };
    let digits = |text: &str| !text.is_empty() && text.bytes().all(|c| c.is_ascii_digit());
    if !digits(major) || minor.is_some_and(|minor| !digits(minor)) {
        return Err(bad());
    }
    // atoi, which overflows as C's does on absurd versions.
    let number = |text: &str| text.parse::<u32>().unwrap_or(u32::MAX);
    let version = number(major)
        .saturating_mul(1000)
        .saturating_add(minor.map_or(0, number));
    Ok(match comparison {
        "<=" => (THEME_VERSION <= version, 0),
        "<" => (THEME_VERSION < version, 0),
        ">=" => (THEME_VERSION >= version, version),
        _ => (THEME_VERSION > version, version.saturating_add(1)),
    })
}

fn check_constant_name(name: &str, defined: bool) -> Parse {
    if !name.starts_with(|c: char| c.is_ascii_uppercase()) {
        return invalid(format!(
            "User-defined constants must begin with a capital letter; \"{name}\" does not"
        ));
    }
    if defined {
        return invalid(format!("Constant \"{name}\" has already been defined"));
    }
    Ok(())
}

/// Marco's `parse_double`, which must use the whole string.
fn parse_double(text: &str) -> Parse<f64> {
    let Some((value, len)) = strtod_prefix(text) else {
        return invalid(format!(
            "Could not parse \"{text}\" as a floating point number"
        ));
    };
    if len != text.len() {
        return invalid(format!(
            "Did not understand trailing characters \"{}\" in string \"{text}\"",
            &text[len..]
        ));
    }
    Ok(value)
}

/// Marco's `parse_angle`, whose bounds are written 1e6 where 1e-6 was meant.
fn parse_angle(text: &str) -> Parse<f64> {
    let value = parse_double(text)?;
    if value < -1e6 || value > 360.0 + 1e6 {
        return invalid(format!(
            "Angle must be between 0.0 and 360.0, was {value}\n"
        ));
    }
    Ok(value)
}

fn parse_boolean(text: &str) -> Parse<bool> {
    match text {
        "true" => Ok(true),
        "false" => Ok(false),
        _ => invalid(format!(
            "Boolean values must be \"true\" or \"false\" not \"{text}\""
        )),
    }
}

/// Pango's named scales, with Pango's own constants.
fn parse_title_scale(text: &str) -> Parse<f64> {
    Ok(match text {
        "xx-small" => 0.5787037037037,
        "x-small" => 0.6944444444444,
        "small" => 0.8333333333333,
        "medium" => 1.0,
        "large" => 1.2,
        "x-large" => 1.44,
        "xx-large" => 1.728,
        _ => {
            return invalid(format!(
                "Invalid title scale \"{text}\" (must be one of \
                 xx-small,x-small,small,medium,large,x-large,xx-large)\n"
            ));
        }
    })
}

/// Marco's `parse_alpha`: colon-separated alphas from 0 to 1.
fn parse_alpha(text: &str) -> Parse<AlphaGradient> {
    if text.is_empty() {
        return invalid(format!(
            "Could not parse \"{text}\" as a floating point number"
        ));
    }
    let mut alphas = Vec::new();
    for part in text.split(':') {
        let value = parse_double(part)?;
        // Written as Marco's comparisons, which a NaN passes.
        if value < -1e-6 || value > 1.0 + 1e-6 {
            return invalid(format!(
                "Alpha must be between 0.0 (invisible) and 1.0 (fully opaque), was {value}\n"
            ));
        }
        alphas.push((value * 255.0) as u8);
    }
    Ok(AlphaGradient(alphas))
}

fn parse_fill(element: &str, fill_type: Option<&str>) -> Parse<ImageFill> {
    match fill_type {
        None | Some("scale") => Ok(ImageFill::Scale),
        Some("tile") => Ok(ImageFill::Tile),
        Some(fill_type) => invalid(format!(
            "Did not understand fill type \"{fill_type}\" for <{element}> element"
        )),
    }
}

fn parse_gtk_state(element: &str, text: &str) -> Parse<GtkState> {
    GtkState::from_name(text).ok_or_else(|| {
        Failure::Invalid(format!(
            "Did not understand state \"{text}\" for <{element}> element"
        ))
    })
}

fn parse_shadow(element: &str, text: &str) -> Parse<GtkShadow> {
    Ok(match text {
        "none" => GtkShadow::None,
        "in" => GtkShadow::In,
        "out" => GtkShadow::Out,
        "etched_in" => GtkShadow::EtchedIn,
        "etched_out" => GtkShadow::EtchedOut,
        _ => {
            return invalid(format!(
                "Did not understand shadow \"{text}\" for <{element}> element"
            ));
        }
    })
}

/// Marco's `meta_frame_style_validate`: every button that exists in the element's
/// version must resolve in every state, positional backgrounds aside.
fn validate_style(style: &FrameStyle, version: u32) -> Parse {
    for button in ButtonType::ALL {
        if (button as usize) < ButtonType::Close as usize {
            continue;
        }
        for state in ButtonState::ALL {
            if style.button(button, state).is_none() && button.earliest_version() <= version {
                return invalid(format!(
                    "<button function=\"{}\" state=\"{}\" draw_ops=\"whatever\"/> must be \
                     specified for this frame style",
                    button.name(),
                    state.name()
                ));
            }
        }
    }
    Ok(())
}

/// Marco's `meta_frame_style_set_validate`.
fn validate_style_set(set: &FrameStyleSet) -> Parse {
    let missing = |state: FrameState, resize: FrameResize, focus: FrameFocus| {
        invalid(format!(
            "Missing <frame state=\"{}\" resize=\"{}\" focus=\"{}\" style=\"whatever\"/>",
            state.name(),
            resize.name(),
            focus.name()
        ))
    };
    for resize in FrameResize::ALL {
        for focus in FrameFocus::ALL {
            if set.lookup(FrameState::Normal, resize, focus).is_none() {
                return missing(FrameState::Normal, resize, focus);
            }
        }
    }
    for state in [
        FrameState::Shaded,
        FrameState::Maximized,
        FrameState::MaximizedAndShaded,
    ] {
        for focus in FrameFocus::ALL {
            if set.lookup(state, FrameResize::None, focus).is_none() {
                return missing(state, FrameResize::None, focus);
            }
        }
    }
    Ok(())
}

#[cfg(test)]
mod tests;
