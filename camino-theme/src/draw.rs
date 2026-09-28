//! Drawing a frame as Marco draws it (`theme.c:3868-4586`, `4736-5230`): the draw ops,
//! the pieces and the buttons, onto a cairo surface at an integer scale.
//!
//! Each op makes the cairo calls Marco makes, in the same order, so cairo rasterizes
//! them the same way, including Marco's half-pixel emulation of X lines. The
//! differences are the `gtk_*` ops, which Marco hands to GTK and Camino approximates
//! (07): `gtk_box` is a rect filled with the background with a 1px dark border,
//! `gtk_arrow` a triangle and `gtk_vline` a 1px line, both in the text colour.
//!
//! A cairo error stops the drawing where it happens. In Marco the context goes into an
//! error state and ignores everything after, so the result is the same.

use std::f64::consts::PI;
use std::sync::Arc;

use pangocairo::cairo::{self, Context, Format, ImageSurface, LineCap, LinearGradient, Surface};
use pangocairo::pango;

use crate::button_layout::{ButtonFunction, ButtonLayout};
use crate::color::{ColorComponent, ColorSpec, GtkState, Rgba};
use crate::expr::{Expr, ExprEnv};
use crate::geometry::{FrameFlags, FrameGeometry, MAX_MIDDLE_BACKGROUNDS, Rect};
use crate::image::{ImageCache, Pixels, surface_at_size};
use crate::model::{
    AlphaGradient, ButtonState, ButtonType, DrawOp, DrawOpList, DrawRect, FramePiece, FrameStyle,
    FrameType, GradientType, GtkArrow, ImageSource, Theme,
};
use crate::palette::PaletteColors;
use crate::title::Title;

/// Everything a frame's pixels depend on besides the theme and the scale.
#[derive(Debug, Clone, Copy)]
pub struct Frame<'a> {
    pub frame_type: FrameType,
    pub flags: FrameFlags,
    pub client_width: i32,
    pub client_height: i32,
    pub title: &'a Title,
    pub button_layout: &'a ButtonLayout,
    pub button_states: ButtonStates,
    /// The window's icons, which the `icon` op draws: the mini icon where it's big
    /// enough, the full one otherwise. [`surface_from_pixbuf`](crate::surface_from_pixbuf)
    /// converts a pixbuf as Marco does.
    pub mini_icon: Option<&'a ImageSurface>,
    pub icon: Option<&'a ImageSurface>,
    /// The palette variant the window gets.
    pub colors: &'a PaletteColors,
}

/// The state each button is drawn in. The backgrounds take the state of the button on
/// them.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct ButtonStates([ButtonState; ButtonFunction::ALL.len()]);

impl Default for ButtonStates {
    fn default() -> Self {
        ButtonStates([ButtonState::Normal; ButtonFunction::ALL.len()])
    }
}

impl ButtonStates {
    pub fn get(&self, function: ButtonFunction) -> ButtonState {
        self.0[function as usize]
    }

    pub fn set(&mut self, function: ButtonFunction, state: ButtonState) {
        self.0[function as usize] = state;
    }
}

/// One of the four pieces a frame is drawn in for the compositor (07).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum Strip {
    /// The titlebar and the top border, the full width of the visible frame.
    Top,
    /// The left border beside the client.
    Left,
    /// The right border beside the client.
    Right,
    /// The bottom border, the full width of the visible frame.
    Bottom,
}

impl Strip {
    pub const ALL: [Strip; 4] = [Strip::Top, Strip::Left, Strip::Right, Strip::Bottom];
}

impl FrameGeometry {
    /// The visible frame: the whole frame without its invisible resize border.
    pub fn visible_rect(&self) -> Rect {
        let invisible = self.borders.invisible;
        Rect {
            x: invisible.left,
            y: invisible.top,
            width: self.width - invisible.left - invisible.right,
            height: self.height - invisible.top - invisible.bottom,
        }
    }

    /// Where a strip is, in logical pixels from the frame's top left corner.
    pub fn strip_rect(&self, strip: Strip) -> Rect {
        let visible = self.visible_rect();
        let borders = self.borders.visible;
        let side_height = visible.height - borders.top - borders.bottom;
        match strip {
            Strip::Top => Rect {
                height: borders.top,
                ..visible
            },
            Strip::Bottom => Rect {
                y: visible.y + visible.height - borders.bottom,
                height: borders.bottom,
                ..visible
            },
            Strip::Left => Rect {
                x: visible.x,
                y: visible.y + borders.top,
                width: borders.left,
                height: side_height,
            },
            Strip::Right => Rect {
                x: visible.x + visible.width - borders.right,
                y: visible.y + borders.top,
                width: borders.right,
                height: side_height,
            },
        }
    }

    /// The pixels the rounded corners cut out of the visible frame, in device pixels
    /// from the frame's top left corner at `scale`: one row each, as `frames.c`'s
    /// `get_visible_region` builds its shape.
    pub fn corner_cutouts(&self, scale: i32) -> Vec<Rect> {
        let visible = self.visible_rect();
        let frame = Rect {
            x: visible.x * scale,
            y: visible.y * scale,
            width: visible.width * scale,
            height: visible.height * scale,
        };
        let radii = self.corner_radii;
        let mut rows = Vec::new();
        for (radius, right, bottom) in [
            (radii.top_left, false, false),
            (radii.top_right, true, false),
            (radii.bottom_left, false, true),
            (radii.bottom_right, true, true),
        ] {
            let corner = radius as i32 * scale;
            // Marco's radius is a float, and the mysterious square root is its own.
            let radius = f64::from((f64::from(corner).sqrt() + f64::from(corner)) as f32);
            for i in 0..corner {
                let from_edge = radius - (f64::from(i) + 0.5);
                let width = (0.5 + radius - (radius * radius - from_edge * from_edge).sqrt())
                    .floor() as i32;
                rows.push(Rect {
                    x: if right {
                        frame.x + frame.width - width
                    } else {
                        frame.x
                    },
                    y: if bottom {
                        frame.y + frame.height - i - 1
                    } else {
                        frame.y + i
                    },
                    width,
                    height: 1,
                });
            }
        }
        rows
    }
}

impl Theme {
    /// The whole frame as Marco's `meta_theme_draw_frame` draws it, client area
    /// included, at `scale` device pixels per logical pixel.
    pub fn render_frame(&self, frame: &Frame, scale: i32) -> Result<ImageSurface, cairo::Error> {
        let geometry = self.geometry_of(frame);
        let whole = Rect {
            x: 0,
            y: 0,
            width: geometry.width,
            height: geometry.height,
        };
        self.render(frame, &geometry, whole, scale)
    }

    /// One strip of the frame: the frame drawn clipped to the strip's rect, at `scale`,
    /// with the pixels the rounded corners cut out made transparent.
    pub fn render_strip(
        &self,
        frame: &Frame,
        strip: Strip,
        scale: i32,
    ) -> Result<ImageSurface, cairo::Error> {
        let geometry = self.geometry_of(frame);
        let rect = geometry.strip_rect(strip);
        let mut surface = self.render(frame, &geometry, rect, scale)?;
        let (width, height) = (surface.width(), surface.height());
        // An empty strip, such as a shaded frame's bottom, has no pixels to cut.
        if width == 0 || height == 0 {
            return Ok(surface);
        }
        let stride = surface.stride() as usize;
        let mut data = surface.data().map_err(|_| cairo::Error::SurfaceFinished)?;
        for cutout in geometry.corner_cutouts(scale) {
            let x0 = (cutout.x - rect.x * scale).clamp(0, width);
            let x1 = (cutout.x + cutout.width - rect.x * scale).clamp(0, width);
            let y = cutout.y - rect.y * scale;
            if (0..height).contains(&y) && x0 < x1 {
                let row = y as usize * stride;
                data[row + x0 as usize * 4..row + x1 as usize * 4].fill(0);
            }
        }
        drop(data);
        Ok(surface)
    }

    fn geometry_of(&self, frame: &Frame) -> FrameGeometry {
        self.frame_geometry(
            frame.frame_type,
            frame.title.text_height,
            &frame.flags,
            frame.client_width,
            frame.client_height,
            frame.button_layout,
        )
    }

    /// Draws the part of the frame in `rect` onto a new surface of that size.
    fn render(
        &self,
        frame: &Frame,
        geometry: &FrameGeometry,
        rect: Rect,
        scale: i32,
    ) -> Result<ImageSurface, cairo::Error> {
        let surface =
            ImageSurface::create(Format::ARgb32, rect.width * scale, rect.height * scale)?;
        // A GTK window at scale 2 draws on a surface with a device scale of 2.
        surface.set_device_scale(f64::from(scale), f64::from(scale));
        surface.set_device_offset(-f64::from(rect.x * scale), -f64::from(rect.y * scale));
        {
            let cr = Context::new(&surface)?;
            rectangle(&cr, rect);
            cr.clip();
            let style = self.frame_style_for(frame.frame_type, &frame.flags);
            let info = DrawInfo::new(frame, geometry, &self.images, scale);
            // Marco's context ignores what follows an error; so does this.
            let _ = draw_frame_style(&cr, style, &info, frame);
        }
        surface.flush();
        Ok(surface)
    }
}

/// What the ops draw with besides their own arguments (`MetaDrawInfo`).
struct DrawInfo<'a> {
    colors: &'a PaletteColors,
    mini_icon: Option<&'a ImageSurface>,
    icon: Option<&'a ImageSurface>,
    title: &'a pango::Layout,
    title_width: i32,
    title_height: i32,
    geometry: &'a FrameGeometry,
    images: &'a ImageCache,
    /// The scale theme images and icons are drawn at, which Marco reads from the root
    /// window.
    scale: i32,
}

impl<'a> DrawInfo<'a> {
    fn new(
        frame: &Frame<'a>,
        geometry: &'a FrameGeometry,
        images: &'a ImageCache,
        scale: i32,
    ) -> DrawInfo<'a> {
        let layout = &frame.title.layout;
        let (_, extents) = layout.pixel_extents();
        DrawInfo {
            colors: frame.colors,
            mini_icon: frame.mini_icon,
            icon: frame.icon,
            title: layout,
            title_width: extents.width(),
            title_height: extents.height(),
            geometry,
            images,
            scale,
        }
    }

    /// Marco's `fill_env`.
    fn env(&self, rect: Rect) -> ExprEnv {
        let borders = self.geometry.borders.visible;
        ExprEnv {
            rect,
            object_width: None,
            object_height: None,
            left_width: borders.left,
            right_width: borders.right,
            top_height: borders.top,
            bottom_height: borders.bottom,
            mini_icon_width: self.mini_icon.map_or(0, |icon| icon.width()),
            mini_icon_height: self.mini_icon.map_or(0, |icon| icon.height()),
            icon_width: self.icon.map_or(0, |icon| icon.width()),
            icon_height: self.icon.map_or(0, |icon| icon.height()),
            title_width: self.title_width,
            title_height: self.title_height,
            frame_x_center: self.geometry.width / 2 - rect.x,
            frame_y_center: self.geometry.height / 2 - rect.y,
        }
    }

    fn color(&self, spec: &ColorSpec) -> Rgba {
        spec.render(self.colors)
    }
}

fn rectangle(cr: &Context, rect: Rect) {
    cr.rectangle(
        f64::from(rect.x),
        f64::from(rect.y),
        f64::from(rect.width),
        f64::from(rect.height),
    );
}

fn set_source_color(cr: &Context, color: Rgba) {
    cr.set_source_rgba(color.red, color.green, color.blue, color.alpha);
}

/// `gdk_cairo_get_clip_rectangle`: whether anything can still be drawn.
fn has_clip(cr: &Context) -> Result<bool, cairo::Error> {
    let (x1, y1, x2, y2) = cr.clip_extents()?;
    Ok(x1 < x2 && y1 < y2)
}

/// An op's rectangle, evaluated in Marco's order.
fn eval_rect(rect: &DrawRect, env: &ExprEnv) -> Rect {
    Rect {
        x: rect.x.x(env),
        y: rect.y.y(env),
        width: rect.width.size(env),
        height: rect.height.size(env),
    }
}

/// Whether an alpha does anything (`needs_alpha`).
fn needs_alpha(alpha: &AlphaGradient) -> bool {
    alpha.0.len() > 1 || alpha.0[0] != 0xff
}

/// `meta_alpha_gradient_spec_get_mask`: one alpha, or a gradient across the unit square.
fn alpha_mask(alpha: &AlphaGradient) -> cairo::Pattern {
    let alphas = &alpha.0;
    if alphas.len() == 1 {
        return (*cairo::SolidPattern::from_rgba(0.0, 0.0, 0.0, f64::from(alphas[0]) / 255.0))
            .clone();
    }
    let pattern = LinearGradient::new(0.0, 0.0, 1.0, 0.0);
    for (i, &a) in alphas.iter().enumerate() {
        pattern.add_color_stop_rgba(stop(i, alphas.len()), 0.0, 0.0, 0.0, f64::from(a) / 255.0);
    }
    (**pattern).clone()
}

/// A gradient stop's offset, which Marco works out in single precision.
fn stop(i: usize, n: usize) -> f64 {
    f64::from(i as f32 / (n - 1) as f32)
}

/// Marco's `meta_draw_op_list_draw_with_style`. A `clip` op replaces the clip for the
/// rest of the list, and the clip stays set on the caller's context afterwards.
fn draw_op_list(
    cr: &Context,
    info: &DrawInfo,
    list: &DrawOpList,
    rect: Rect,
) -> Result<(), cairo::Error> {
    if list.ops.is_empty() {
        return Ok(());
    }
    let mut env = info.env(rect);
    cr.save()?;
    for op in &list.ops {
        if let DrawOp::Clip { rect: clip } = op {
            cr.restore()?;
            rectangle(cr, eval_rect(clip, &env));
            cr.clip();
            cr.save()?;
        } else if has_clip(cr)? {
            cr.save()?;
            cr.set_line_width(1.0);
            let result = draw_op(cr, info, op, rect, &mut env);
            cr.restore()?;
            result?;
        }
    }
    cr.restore()
}

/// Marco's `meta_draw_op_draw_with_env`, inside the save and restore around it.
fn draw_op(
    cr: &Context,
    info: &DrawInfo,
    op: &DrawOp,
    rect: Rect,
    env: &mut ExprEnv,
) -> Result<(), cairo::Error> {
    match op {
        DrawOp::Line {
            color,
            x1,
            y1,
            x2,
            y2,
            width,
            dash_on_length,
            dash_off_length,
        } => {
            set_source_color(cr, info.color(color));
            if *width > 0 {
                cr.set_line_width(f64::from(*width));
            }
            if *dash_on_length > 0 && *dash_off_length > 0 {
                cr.set_dash(
                    &[f64::from(*dash_on_length), f64::from(*dash_off_length)],
                    0.0,
                );
            }
            let x1 = f64::from(x1.x(env));
            let y1 = f64::from(y1.y(env));
            if x2.is_none() && y2.is_none() && *width == 0 {
                cr.rectangle(x1, y1, 1.0, 1.0);
                return cr.fill();
            }
            let x2 = x2.as_ref().map_or(x1, |x2| f64::from(x2.x(env)));
            let y2 = y2.as_ref().map_or(y1, |y2| f64::from(y2.y(env)));
            // X draws a wide horizontal or vertical line as an exact rectangle.
            if (y1 == y2 || x1 == x2) && *width != 0 {
                let offset = if width % 2 != 0 { 0.5 } else { 0.0 };
                if y1 == y2 {
                    cr.move_to(x1, y1 + offset);
                    cr.line_to(x2, y2 + offset);
                } else {
                    cr.move_to(x1 + offset, y1);
                    cr.line_to(x2 + offset, y2);
                }
            } else {
                // X's zero-width lines include both end points; wide ones don't.
                if *width == 0 {
                    cr.set_line_cap(LineCap::Square);
                }
                cr.move_to(x1 + 0.5, y1 + 0.5);
                cr.line_to(x2 + 0.5, y2 + 0.5);
            }
            cr.stroke()
        }
        DrawOp::Rectangle {
            color,
            rect: op_rect,
            filled,
        } => {
            set_source_color(cr, info.color(color));
            let r = eval_rect(op_rect, env);
            if *filled {
                rectangle(cr, r);
                cr.fill()
            } else {
                cr.rectangle(
                    f64::from(r.x) + 0.5,
                    f64::from(r.y) + 0.5,
                    f64::from(r.width),
                    f64::from(r.height),
                );
                cr.stroke()
            }
        }
        DrawOp::Arc {
            color,
            rect: op_rect,
            filled,
            start_angle,
            extent_angle,
        } => {
            set_source_color(cr, info.color(color));
            let r = eval_rect(op_rect, env);
            // From 12 o'clock rather than 3.
            let start = start_angle * (PI / 180.0) - 0.5 * PI;
            let end = start + extent_angle * (PI / 180.0);
            let center_x = f64::from(r.x) + f64::from(r.width) / 2.0 + 0.5;
            let center_y = f64::from(r.y) + f64::from(r.height) / 2.0 + 0.5;
            cr.save()?;
            cr.translate(center_x, center_y);
            cr.scale(f64::from(r.width) / 2.0, f64::from(r.height) / 2.0);
            if *extent_angle >= 0.0 {
                cr.arc(0.0, 0.0, 1.0, start, end);
            } else {
                cr.arc_negative(0.0, 0.0, 1.0, start, end);
            }
            cr.restore()?;
            if *filled {
                cr.line_to(center_x, center_y);
                cr.fill()
            } else {
                cr.stroke()
            }
        }
        DrawOp::Clip { .. } => Ok(()),
        DrawOp::Tint {
            color,
            alpha,
            rect: op_rect,
        } => {
            let r = eval_rect(op_rect, env);
            let color = info.color(color);
            if !needs_alpha(alpha) {
                set_source_color(cr, color);
                rectangle(cr, r);
                return cr.fill();
            }
            // Marco draws it as a pixbuf of the colour with the alpha applied.
            if r.width <= 0 || r.height <= 0 {
                return Ok(());
            }
            let pixels = if let [alpha] = alpha.0[..] {
                Pixels::filled(r.width, r.height, color, alpha)
            } else {
                let mut pixels = Pixels::filled(r.width, r.height, color, 0xff);
                pixels.add_alpha(&alpha.0);
                pixels
            };
            cr.set_source_surface(pixels.to_surface()?, f64::from(r.x), f64::from(r.y))?;
            cr.paint()
        }
        DrawOp::Gradient {
            kind,
            colors,
            alpha,
            rect: op_rect,
        } => {
            let r = eval_rect(op_rect, env);
            draw_gradient(cr, info, *kind, colors, alpha.as_ref(), r)
        }
        DrawOp::Image {
            source,
            colorize,
            alpha,
            rect: op_rect,
            fill,
        } => {
            let scale = info.scale;
            let s = f64::from(scale);
            cr.scale(1.0 / s, 1.0 / s);
            // Marco loads `theme:` images from the icon theme; Camino ignores them (07).
            let ImageSource::File(path) = source else {
                return Ok(());
            };
            let Some(image) = info.images.get(path, scale) else {
                return Ok(());
            };
            env.object_width = Some(image.pixels.width / scale);
            env.object_height = Some(image.pixels.height / scale);
            let width = f64::from(op_rect.width.size(env) * scale);
            let height = f64::from(op_rect.height.size(env) * scale);
            let colorized;
            let pixels = match colorize {
                Some(color) => {
                    colorized = image.pixels.colorize(info.color(color));
                    &colorized
                }
                None => &image.pixels,
            };
            let surface = surface_at_size(
                &pixels.to_surface()?,
                *fill,
                width,
                height,
                image.vertical_stripes,
                image.horizontal_stripes,
            )?;
            let x = f64::from(op_rect.x.x(env) * scale);
            let y = f64::from(op_rect.y.y(env) * scale);
            paint_with_alpha(cr, &surface, x, y, width, height, alpha.as_ref())
        }
        DrawOp::GtkArrow {
            state,
            arrow,
            rect: op_rect,
            ..
        } => {
            let r = eval_rect(op_rect, env);
            let angle = match arrow {
                GtkArrow::Up => 0.0,
                GtkArrow::Right => PI / 2.0,
                GtkArrow::Down => PI,
                GtkArrow::Left => 3.0 * PI / 2.0,
                GtkArrow::None => return Ok(()),
            };
            // GTK draws the theme's arrow icon in a square of the larger side, turned
            // by the angle; this is a triangle in the middle of it.
            let size = f64::from(r.width.max(r.height));
            set_source_color(cr, gtk_color(info, ColorComponent::Fg, *state));
            cr.translate(f64::from(r.x) + size / 2.0, f64::from(r.y) + size / 2.0);
            cr.rotate(angle);
            cr.move_to(-size / 4.0, size / 8.0);
            cr.line_to(size / 4.0, size / 8.0);
            cr.line_to(0.0, -size / 8.0);
            cr.close_path();
            cr.fill()
        }
        DrawOp::GtkBox {
            state,
            rect: op_rect,
            ..
        } => {
            let r = eval_rect(op_rect, env);
            set_source_color(cr, gtk_color(info, ColorComponent::Bg, *state));
            rectangle(cr, r);
            cr.fill()?;
            set_source_color(cr, gtk_color(info, ColorComponent::Dark, *state));
            cr.rectangle(
                f64::from(r.x) + 0.5,
                f64::from(r.y) + 0.5,
                f64::from(r.width - 1),
                f64::from(r.height - 1),
            );
            cr.stroke()
        }
        DrawOp::GtkVline { state, x, y1, y2 } => {
            // GTK draws lines in the border colour, which is the text colour unless the
            // GTK theme sets one.
            let x = f64::from(x.x(env)) + 0.5;
            set_source_color(cr, gtk_color(info, ColorComponent::Fg, *state));
            cr.move_to(x, f64::from(y1.y(env)));
            cr.line_to(x, f64::from(y2.y(env)));
            cr.stroke()
        }
        DrawOp::Icon {
            alpha,
            rect: op_rect,
            fill,
        } => {
            let scale = info.scale;
            let s = f64::from(scale);
            cr.scale(1.0 / s, 1.0 / s);
            let width = f64::from(op_rect.width.size(env) * scale);
            let height = f64::from(op_rect.height.size(env) * scale);
            let fits = |icon: &&ImageSurface| {
                width <= f64::from(icon.width()) && height <= f64::from(icon.height())
            };
            let Some(icon) = info.mini_icon.filter(fits).or(info.icon) else {
                return Ok(());
            };
            let surface = surface_at_size(icon, *fill, width, height, false, false)?;
            let x = f64::from(op_rect.x.x(env) * scale);
            let y = f64::from(op_rect.y.y(env) * scale);
            paint_with_alpha(cr, &surface, x, y, width, height, alpha.as_ref())
        }
        DrawOp::Title {
            color,
            x,
            y,
            ellipsize_width,
        } => draw_title(cr, info, color, x.x(env), y.y(env), ellipsize_width, env),
        DrawOp::Include {
            list,
            rect: op_rect,
        } => draw_op_list(cr, info, list, eval_rect(op_rect, env)),
        DrawOp::Tile {
            list,
            rect: op_rect,
            tile_xoffset,
            tile_yoffset,
            tile_width,
            tile_height,
        } => {
            let r = eval_rect(op_rect, env);
            cr.save()?;
            rectangle(cr, r);
            cr.clip();
            // The offsets don't include the rectangle's position.
            let x_offset = tile_xoffset.x(env) - rect.x;
            let y_offset = tile_yoffset.y(env) - rect.y;
            let mut tile = Rect {
                x: r.x - x_offset,
                y: 0,
                width: tile_width.size(env),
                height: tile_height.size(env),
            };
            // Marco loops forever on a tile it can't size; this draws nothing.
            if tile.width > 0 && tile.height > 0 {
                while tile.x < r.x + r.width {
                    tile.y = r.y - y_offset;
                    while tile.y < r.y + r.height {
                        draw_op_list(cr, info, list, tile)?;
                        tile.y += tile.height;
                    }
                    tile.x += tile.width;
                }
            }
            cr.restore()
        }
    }
}

/// A `gtk:` colour for the approximated `gtk_*` ops.
fn gtk_color(info: &DrawInfo, component: ColorComponent, state: GtkState) -> Rgba {
    info.color(&ColorSpec::Gtk { component, state })
}

/// Paints an image or icon surface at `x`, `y` in device pixels, through its alpha if
/// it has one.
fn paint_with_alpha(
    cr: &Context,
    surface: &Surface,
    x: f64,
    y: f64,
    width: f64,
    height: f64,
    alpha: Option<&AlphaGradient>,
) -> Result<(), cairo::Error> {
    cr.set_source_surface(surface, x, y)?;
    match alpha {
        Some(alpha) => {
            cr.translate(x, y);
            cr.scale(width, height);
            cr.mask(alpha_mask(alpha))
        }
        None => cr.paint(),
    }
}

/// Marco's `meta_gradient_spec_render`: a linear gradient across the rectangle.
fn draw_gradient(
    cr: &Context,
    info: &DrawInfo,
    kind: GradientType,
    colors: &[ColorSpec],
    alpha: Option<&AlphaGradient>,
    r: Rect,
) -> Result<(), cairo::Error> {
    let n = colors.len();
    if n == 0 {
        return Ok(());
    }
    // Marco asserts that there's one alpha, or one for each colour.
    if alpha.is_some_and(|alpha| alpha.0.len() != 1 && alpha.0.len() != n) {
        return Ok(());
    }
    let (x1, y1) = match kind {
        GradientType::Horizontal => (1.0, 0.0),
        GradientType::Vertical => (0.0, 1.0),
        GradientType::Diagonal => (1.0, 1.0),
    };
    let pattern = LinearGradient::new(0.0, 0.0, x1, y1);
    for (i, spec) in colors.iter().enumerate() {
        let color = info.color(spec);
        let offset = stop(i, n);
        match alpha {
            Some(alpha) => {
                let a = if alpha.0.len() == 1 {
                    alpha.0[0]
                } else {
                    alpha.0[i]
                };
                pattern.add_color_stop_rgba(
                    offset,
                    color.red,
                    color.green,
                    color.blue,
                    f64::from(a) / 255.0,
                );
            }
            None => pattern.add_color_stop_rgb(offset, color.red, color.green, color.blue),
        }
    }
    cr.save()?;
    rectangle(cr, r);
    cr.translate(f64::from(r.x), f64::from(r.y));
    cr.scale(f64::from(r.width), f64::from(r.height));
    cr.set_source(&pattern)?;
    cr.fill()?;
    cr.restore()
}

/// The `title` op of Marco's `meta_draw_op_draw_with_env`: the title's layout at `x`,
/// `y`, ellipsized to `ellipsize_width` if
/// the theme gives one, and otherwise faded out over 30 pixels when it reaches the
/// rectangle's edge.
fn draw_title(
    cr: &Context,
    info: &DrawInfo,
    color: &ColorSpec,
    x: i32,
    y: i32,
    ellipsize_width: &Option<Expr>,
    env: &ExprEnv,
) -> Result<(), cairo::Error> {
    let layout = info.title;
    let color = info.color(color);
    set_source_color(cr, color);
    if let Some(ellipsize_width) = ellipsize_width {
        // The expression is an x position, so it has the rectangle's x to take off.
        let mut width = ellipsize_width.x(env) - env.rect.x;
        layout.set_width(-1);
        let (ink, logical) = layout.pixel_extents();
        // Pango ellipsizes to the logical rect; take off what the ink overhangs it by
        // on the right.
        let right_bearing = ((ink.x() + ink.width()) - (logical.x() + logical.width())).max(0);
        width = (width - right_bearing).max(0);
        if width < logical.width() {
            layout.set_width(pango::SCALE * width);
        }
    } else if x - env.rect.x + env.title_width >= env.rect.width {
        const ALPHA_MARGIN: f64 = 30.0;
        let text_space = env.rect.x + env.rect.width - (x - env.rect.x) - env.right_width;
        let start_alpha = 1.0 - ALPHA_MARGIN / f64::from(text_space);
        // Marco's end point is the text's space and height, not a position.
        let pattern = LinearGradient::new(
            f64::from(x),
            f64::from(y),
            f64::from(text_space),
            f64::from(env.title_height),
        );
        pattern.add_color_stop_rgba(0.0, color.red, color.green, color.blue, color.alpha);
        pattern.add_color_stop_rgba(start_alpha, color.red, color.green, color.blue, color.alpha);
        pattern.add_color_stop_rgba(1.0, color.red, color.green, color.blue, 0.0);
        cr.set_source(&pattern)?;
    }
    cr.move_to(f64::from(x), f64::from(y));
    pangocairo::functions::show_layout(cr, layout);
    layout.set_width(-1);
    Ok(())
}

/// Marco's `meta_frame_style_draw_with_style`: each piece in order, clipped to its
/// rectangle, and the buttons just before the overlay.
fn draw_frame_style(
    cr: &Context,
    style: &FrameStyle,
    info: &DrawInfo,
    frame: &Frame,
) -> Result<(), cairo::Error> {
    let geometry = info.geometry;
    let borders = geometry.borders;
    let visible = geometry.visible_rect();
    let titlebar = Rect {
        height: borders.visible.top,
        ..visible
    };
    let left_titlebar_edge = Rect {
        x: titlebar.x,
        y: titlebar.y + geometry.top_titlebar_edge,
        width: geometry.left_titlebar_edge,
        height: titlebar.height - geometry.top_titlebar_edge - geometry.bottom_titlebar_edge,
    };
    let right_titlebar_edge = Rect {
        x: titlebar.x + titlebar.width - geometry.right_titlebar_edge,
        width: geometry.right_titlebar_edge,
        ..left_titlebar_edge
    };
    let top_titlebar_edge = Rect {
        height: geometry.top_titlebar_edge,
        ..titlebar
    };
    let bottom_titlebar_edge = Rect {
        y: titlebar.y + titlebar.height - geometry.bottom_titlebar_edge,
        height: geometry.bottom_titlebar_edge,
        ..titlebar
    };

    for piece in FramePiece::ALL {
        let rect = match piece {
            FramePiece::EntireBackground | FramePiece::Overlay => visible,
            FramePiece::Titlebar => titlebar,
            FramePiece::TitlebarMiddle => Rect {
                x: left_titlebar_edge.x + left_titlebar_edge.width,
                y: top_titlebar_edge.y + top_titlebar_edge.height,
                width: titlebar.width - left_titlebar_edge.width - right_titlebar_edge.width,
                height: titlebar.height - top_titlebar_edge.height - bottom_titlebar_edge.height,
            },
            FramePiece::LeftTitlebarEdge => left_titlebar_edge,
            FramePiece::RightTitlebarEdge => right_titlebar_edge,
            FramePiece::TopTitlebarEdge => top_titlebar_edge,
            FramePiece::BottomTitlebarEdge => bottom_titlebar_edge,
            FramePiece::Title => geometry.title_rect,
            FramePiece::LeftEdge => geometry.strip_rect(Strip::Left),
            FramePiece::RightEdge => geometry.strip_rect(Strip::Right),
            FramePiece::BottomEdge => geometry.strip_rect(Strip::Bottom),
        };
        if let Some(list) = style.piece(piece) {
            draw_clipped(cr, info, list, rect)?;
        }
        if piece == FramePiece::BottomEdge {
            draw_buttons(cr, style, info, frame)?;
        }
    }
    Ok(())
}

/// Draws a list clipped to its rectangle, if any of that is left to draw on.
fn draw_clipped(
    cr: &Context,
    info: &DrawInfo,
    list: &DrawOpList,
    rect: Rect,
) -> Result<(), cairo::Error> {
    cr.save()?;
    rectangle(cr, rect);
    cr.clip();
    if has_clip(cr)? {
        draw_op_list(cr, info, list, rect)?;
    }
    cr.restore()
}

/// The button part of `meta_frame_style_draw_with_style`: every button type in order,
/// the middle backgrounds once for each slot.
///
/// Marco's loop visits one middle slot more than there are, reading the next field of
/// its geometry, which is the side's right background. A background with no button
/// gets `META_BUTTON_STATE_LAST`, which reads the next button type's normal ops. Both
/// are kept, since they draw: a side's right background is drawn a second time, in its
/// normal state, before its own turn.
fn draw_buttons(
    cr: &Context,
    style: &FrameStyle,
    info: &DrawInfo,
    frame: &Frame,
) -> Result<(), cairo::Error> {
    let geometry = info.geometry;
    let backgrounds = &geometry.button_backgrounds;
    let middle =
        |middles: &[Option<Rect>; MAX_MIDDLE_BACKGROUNDS], past: Option<Rect>, i: usize| {
            if i < MAX_MIDDLE_BACKGROUNDS {
                middles[i]
            } else {
                past
            }
        };
    for button in ButtonType::ALL {
        let slots = match button {
            ButtonType::LeftMiddleBackground | ButtonType::RightMiddleBackground => {
                MAX_MIDDLE_BACKGROUNDS + 1
            }
            _ => 1,
        };
        for offset in 0..slots {
            let rect = match button {
                ButtonType::LeftLeftBackground => backgrounds.left_left,
                ButtonType::LeftMiddleBackground => {
                    middle(&backgrounds.left_middle, backgrounds.left_right, offset)
                }
                ButtonType::LeftRightBackground => backgrounds.left_right,
                ButtonType::LeftSingleBackground => backgrounds.left_single,
                ButtonType::RightLeftBackground => backgrounds.right_left,
                ButtonType::RightMiddleBackground => {
                    middle(&backgrounds.right_middle, backgrounds.right_right, offset)
                }
                ButtonType::RightRightBackground => backgrounds.right_right,
                ButtonType::RightSingleBackground => backgrounds.right_single,
                _ => button_function(button)
                    .and_then(|function| geometry.button(function))
                    .map(|space| space.visible),
            }
            .unwrap_or_default();
            let ops = match button_state(button, geometry, frame, offset) {
                Some(state) => style.button(button, state),
                None => unmapped_button(style, button),
            };
            if let Some(list) = ops {
                draw_clipped(cr, info, list, rect)?;
            }
        }
    }
    Ok(())
}

/// The function a button type draws, for those that aren't backgrounds.
fn button_function(button: ButtonType) -> Option<ButtonFunction> {
    ButtonFunction::ALL
        .into_iter()
        .find(|function| function.button_type() == button)
}

/// Marco's `map_button_state`: a button's own state, or a background's from the button
/// on it. Marco takes that button from the layout by position among the buttons shown,
/// so it counts toggles' opposites and dropped buttons. `None` for a background with no
/// button, `META_BUTTON_STATE_LAST`.
fn button_state(
    button: ButtonType,
    geometry: &FrameGeometry,
    frame: &Frame,
    middle_offset: usize,
) -> Option<ButtonState> {
    if let Some(function) = button_function(button) {
        return Some(frame.button_states.get(function));
    }
    let layout = frame.button_layout;
    let (side, n) = match button {
        ButtonType::LeftLeftBackground
        | ButtonType::LeftMiddleBackground
        | ButtonType::LeftRightBackground
        | ButtonType::LeftSingleBackground => (&layout.left, geometry.n_left_buttons),
        _ => (&layout.right, geometry.n_right_buttons),
    };
    let index = match button {
        ButtonType::LeftLeftBackground
        | ButtonType::LeftSingleBackground
        | ButtonType::RightLeftBackground
        | ButtonType::RightSingleBackground => (n > 0).then_some(0),
        ButtonType::LeftRightBackground | ButtonType::RightRightBackground => n.checked_sub(1),
        _ => (middle_offset + 1 < n).then_some(middle_offset + 1),
    }?;
    side.get(index)
        .map(|button| frame.button_states.get(button.function))
}

/// Marco's `get_button` with `META_BUTTON_STATE_LAST`, one past the end of a button's
/// row of states: the next button type's normal ops, then the same fallbacks from one
/// background to another.
fn unmapped_button(style: &FrameStyle, button: ButtonType) -> Option<&Arc<DrawOpList>> {
    let next = ButtonType::ALL[button as usize + 1];
    style
        .own_button(next, ButtonState::Normal)
        .or_else(|| match button {
            ButtonType::LeftSingleBackground => {
                unmapped_button(style, ButtonType::LeftLeftBackground)
            }
            ButtonType::RightSingleBackground => {
                unmapped_button(style, ButtonType::RightRightBackground)
            }
            ButtonType::LeftLeftBackground | ButtonType::LeftRightBackground => {
                unmapped_button(style, ButtonType::LeftMiddleBackground)
            }
            ButtonType::RightLeftBackground | ButtonType::RightRightBackground => {
                unmapped_button(style, ButtonType::RightMiddleBackground)
            }
            _ => None,
        })
}

#[cfg(test)]
mod tests {
    use std::collections::BTreeMap;
    use std::path::Path;

    use super::*;
    use crate::palette::StateColors;

    const RED: &str =
        r##"<rectangle color="#ff0000" x="0" y="0" width="width" height="height" filled="true"/>"##;

    /// A theme whose frames are 3 wide at the sides, 4 at the bottom and 20 at the top,
    /// drawn with `background`'s ops, and rounded by 5 at every corner.
    fn theme(background: &str) -> Theme {
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
        let frames: String = ["yes", "no"]
            .iter()
            .map(|focus| {
                format!(
                    r#"<frame focus="{focus}" state="normal" resize="both" style="s"/>
<frame focus="{focus}" state="maximized" style="s"/>
<frame focus="{focus}" state="shaded" style="s"/>
<frame focus="{focus}" state="maximized_and_shaded" style="s"/>"#
                )
            })
            .collect();
        let text = format!(
            r##"<metacity_theme>
<info><name>T</name><author>A</author><copyright>C</copyright><date>D</date><description>X</description></info>
<frame_geometry name="g" rounded_top_left="5" rounded_top_right="5" rounded_bottom_left="5" rounded_bottom_right="5">
<distance name="left_width" value="3"/>
<distance name="right_width" value="3"/>
<distance name="bottom_height" value="4"/>
<distance name="title_vertical_pad" value="0"/>
<distance name="left_titlebar_edge" value="0"/>
<distance name="right_titlebar_edge" value="0"/>
<distance name="button_width" value="16"/>
<distance name="button_height" value="16"/>
<border name="title_border" left="0" right="0" top="2" bottom="2"/>
<border name="button_border" left="0" right="0" top="2" bottom="2"/>
</frame_geometry>
<draw_ops name="empty"/>
<draw_ops name="dot"><rectangle color="#ff0000" x="0" y="0" width="1" height="1" filled="true"/></draw_ops>
<draw_ops name="background">{background}</draw_ops>
<frame_style name="s" geometry="g">
<piece position="entire_background" draw_ops="background"/>
{buttons}
</frame_style>
<frame_style_set name="set">{frames}</frame_style_set>
{windows}
</metacity_theme>"##
        );
        Theme::parse(
            text.as_bytes(),
            "T",
            Path::new("/themes/T/metacity-1/metacity-theme-3.xml"),
            3,
        )
        .unwrap_or_else(|e| panic!("{e}"))
    }

    fn colors() -> PaletteColors {
        let grey = Rgba::from_rgb8(128, 128, 128);
        let states = GtkState::ALL.map(|_| StateColors {
            background: grey,
            text: grey,
        });
        PaletteColors::new(states, BTreeMap::new())
    }

    fn title() -> Title {
        let context = crate::title::title_context(
            &cairo::FontOptions::new().unwrap(),
            96.0,
            &pango::Language::from_string("en-us"),
        );
        let mut title = Title::new(&context, &pango::FontDescription::new(), 1.0, "");
        // The titlebar is then the buttons' 20.
        title.text_height = 0;
        title
    }

    fn flags() -> FrameFlags {
        FrameFlags {
            allows_vertical_resize: true,
            allows_horizontal_resize: true,
            has_focus: true,
            ..FrameFlags::default()
        }
    }

    /// Checks one frame of the test theme: a 100 by 50 client.
    fn with_frame(background: &str, check: impl FnOnce(&Theme, &Frame, &FrameGeometry)) {
        let theme = theme(background);
        let title = title();
        let layout = ButtonLayout::parse(":");
        let colors = colors();
        let frame = Frame {
            frame_type: FrameType::Normal,
            flags: flags(),
            client_width: 100,
            client_height: 50,
            title: &title,
            button_layout: &layout,
            button_states: ButtonStates::default(),
            mini_icon: None,
            icon: None,
            colors: &colors,
        };
        let geometry = theme.geometry_of(&frame);
        check(&theme, &frame, &geometry);
    }

    fn rect(x: i32, y: i32, width: i32, height: i32) -> Rect {
        Rect {
            x,
            y,
            width,
            height,
        }
    }

    #[test]
    fn the_strips_surround_the_client_inside_the_invisible_border() {
        with_frame(RED, |_, _, geometry| {
            // 10 of invisible border on each side.
            assert_eq!(geometry.visible_rect(), rect(10, 10, 106, 74));
            assert_eq!(geometry.strip_rect(Strip::Top), rect(10, 10, 106, 20));
            assert_eq!(geometry.strip_rect(Strip::Left), rect(10, 30, 3, 50));
            assert_eq!(geometry.strip_rect(Strip::Right), rect(113, 30, 3, 50));
            assert_eq!(geometry.strip_rect(Strip::Bottom), rect(10, 80, 106, 4));
        });
    }

    #[test]
    fn corners_are_cut_as_frames_c_cuts_them() {
        with_frame(RED, |_, _, geometry| {
            let cutouts = geometry.corner_cutouts(1);
            // floor(0.5 + r - sqrt(r² - (r - (i + 0.5))²)) with r = 5 + √5.
            let top_left: Vec<Rect> = cutouts[..5].to_vec();
            assert_eq!(
                top_left,
                [
                    rect(10, 10, 5, 1),
                    rect(10, 11, 3, 1),
                    rect(10, 12, 2, 1),
                    rect(10, 13, 1, 1),
                    rect(10, 14, 1, 1),
                ]
            );
            assert_eq!(cutouts[5], rect(111, 10, 5, 1));
            assert_eq!(cutouts[10], rect(10, 83, 5, 1));
            assert_eq!(cutouts[15], rect(111, 83, 5, 1));
            assert_eq!(cutouts.len(), 20);
            // At scale 2 the radius is 10 device pixels.
            assert_eq!(geometry.corner_cutouts(2).len(), 40);
            assert_eq!(geometry.corner_cutouts(2)[0].x, 20);
        });
    }

    /// A pixel of an ARGB32 surface.
    fn pixel(surface: &ImageSurface, x: usize, y: usize) -> u32 {
        let stride = surface.stride() as usize;
        let mut word = [0; 4];
        surface
            .with_data(|data| word.copy_from_slice(&data[y * stride + x * 4..][..4]))
            .unwrap();
        u32::from_ne_bytes(word)
    }

    #[test]
    fn tiles_repeat_from_the_offset_across_the_rectangle() {
        let tile = r#"<tile name="dot" x="2" y="0" width="10" height="3" tile_xoffset="1" tile_yoffset="0" tile_width="4" tile_height="2"/>"#;
        with_frame(tile, |theme, frame, _| {
            let surface = theme.render_strip(frame, Strip::Top, 1).unwrap();
            let red: Vec<usize> = (0..14).filter(|&x| pixel(&surface, x, 0) != 0).collect();
            // Tiles start 1 before the rectangle at 2, so their dots from 5 are in it.
            assert_eq!(red, [5, 9]);
            assert_ne!(pixel(&surface, 5, 2), 0, "the second row of tiles");
            assert_eq!(pixel(&surface, 5, 3), 0, "clipped to the rectangle");
        });
    }

    #[test]
    fn a_strip_is_the_frame_there_with_its_corners_cut() {
        with_frame(RED, |theme, frame, _| {
            let surface = theme.render_strip(frame, Strip::Top, 2).unwrap();
            assert_eq!((surface.width(), surface.height()), (212, 40));
            assert_eq!(pixel(&surface, 0, 0), 0, "cut out");
            assert_eq!(
                pixel(&surface, 10, 0),
                0xffff0000,
                "past the first row's cut"
            );
            assert_eq!(pixel(&surface, 211, 0), 0, "cut out");
            assert_eq!(pixel(&surface, 0, 39), 0xffff0000, "below the corner");
        });
    }
}
