//! The arrangement: the displays that are on, drawn to scale in the top of the window, to
//! be dragged about.

use caminito_displays::page::Page;
use caminito_layout::Rect;
use gtk::prelude::*;
use gtk::{cairo, gdk, pango};

/// How near, in the arrangement's own pixels, a dragged display's edge comes to the
/// matching edge of a display it lies against before it takes it, whatever the zoom
/// (the prototype's verdict).
const SNAP: f64 = 12.0;

/// How far the pointer moves, in the arrangement's pixels, before a press is a drag.
pub(crate) const DRAG_THRESHOLD: f64 = 5.0;

/// The arrangement's colour for the selected display and the primary display's star, where
/// the theme names none.
const ACCENT: gdk::RGBA = gdk::RGBA::new(0.165, 0.498, 0.831, 1.0);

/// How the layout's logical pixels map to the arrangement's.
#[derive(Debug, Clone, Copy, PartialEq)]
pub(crate) struct View {
    /// The arrangement's pixels per logical pixel.
    zoom: f64,
    /// Where the layout's 0,0 is in the arrangement.
    origin: (f64, f64),
}

impl View {
    /// The view that shows all of `rects` in the middle of an arrangement of `width` by
    /// `height`, with room around them to drag a display into.
    pub(crate) fn fit(rects: &[Rect], width: f64, height: f64) -> View {
        let left = rects.iter().map(|r| r.x).min().unwrap_or(0);
        let top = rects.iter().map(|r| r.y).min().unwrap_or(0);
        let right = rects.iter().map(|r| r.x + r.w).max().unwrap_or(1);
        let bottom = rects.iter().map(|r| r.y + r.h).max().unwrap_or(1);
        let across = f64::from((right - left).max(1));
        let down = f64::from((bottom - top).max(1));
        // The prototype's shares, and its largest zoom, so one small display isn't huge.
        let zoom = (width * 0.72 / across).min(height * 0.66 / down).min(0.13);
        View {
            zoom,
            origin: (
                (width - across * zoom) / 2.0 - f64::from(left) * zoom,
                (height - down * zoom) / 2.0 - f64::from(top) * zoom,
            ),
        }
    }

    /// `rect` in the arrangement: its left, top, width and height.
    fn place(&self, rect: Rect) -> (f64, f64, f64, f64) {
        (
            f64::from(rect.x) * self.zoom + self.origin.0,
            f64::from(rect.y) * self.zoom + self.origin.1,
            f64::from(rect.w) * self.zoom,
            f64::from(rect.h) * self.zoom,
        )
    }

    /// Whether the point `x`, `y` of the arrangement is on `rect`.
    pub(crate) fn hits(&self, rect: Rect, x: f64, y: f64) -> bool {
        let (left, top, width, height) = self.place(rect);
        (left..left + width).contains(&x) && (top..top + height).contains(&y)
    }

    /// A distance in the arrangement's pixels, in logical pixels.
    pub(crate) fn logical(&self, pixels: f64) -> i32 {
        (pixels / self.zoom).round() as i32
    }

    /// The snap distance in logical pixels.
    pub(crate) fn snap(&self) -> i32 {
        self.logical(SNAP)
    }
}

/// The theme's accent colour, for the selected display.
fn accent(widget: &gtk::DrawingArea) -> gdk::RGBA {
    #[expect(
        deprecated,
        reason = "GTK 4 has no other way to read a theme's named colour, and a widget's \
                  own colour isn't the accent"
    )]
    let named = |name| widget.style_context().lookup_color(name);
    named("accent_bg_color")
        .or_else(|| named("theme_selected_bg_color"))
        .unwrap_or(ACCENT)
}

fn set_color(cr: &cairo::Context, color: gdk::RGBA, alpha: f32) {
    cr.set_source_rgba(
        f64::from(color.red()),
        f64::from(color.green()),
        f64::from(color.blue()),
        f64::from(color.alpha() * alpha),
    );
}

fn rounded(cr: &cairo::Context, (x, y, width, height): (f64, f64, f64, f64)) {
    use std::f64::consts::{FRAC_PI_2, PI};
    let radius = 6.0f64.min(width / 2.0).min(height / 2.0);
    cr.new_sub_path();
    cr.arc(x + width - radius, y + radius, radius, -FRAC_PI_2, 0.0);
    cr.arc(
        x + width - radius,
        y + height - radius,
        radius,
        0.0,
        FRAC_PI_2,
    );
    cr.arc(x + radius, y + height - radius, radius, FRAC_PI_2, PI);
    cr.arc(x + radius, y + radius, radius, PI, 3.0 * FRAC_PI_2);
    cr.close_path();
}

/// Draws the arrangement of `page` as `view` has it. Cairo keeps a drawing error in the
/// context, and a frame that failed is drawn again at the next change, so the errors
/// aren't looked at.
pub(crate) fn draw(area: &gtk::DrawingArea, cr: &cairo::Context, page: &Page, view: &View) {
    let text = area.color();
    let accent = accent(area);
    let (width, height) = (f64::from(area.width()), f64::from(area.height()));

    set_color(cr, text, 0.06);
    cr.rectangle(0.0, 0.0, width, height);
    let _ = cr.fill();

    let dragged = page.dragged_from();
    if let Some((_, origin)) = dragged {
        // Where the display was picked up from.
        rounded(cr, inset(view.place(origin)));
        set_color(cr, text, 0.35);
        cr.set_line_width(2.0);
        cr.set_dash(&[6.0, 4.0], 0.0);
        let _ = cr.stroke();
        cr.set_dash(&[], 0.0);
    }

    let arrangement = page.arrangement();
    let selected = page.selected_index();
    // The selected display last, on top of the others' edges, and the dragged one is it.
    let below = arrangement.iter().filter(|(index, _)| *index != selected);
    let above = arrangement.iter().filter(|(index, _)| *index == selected);
    for &(index, rect) in below.chain(above) {
        let tile = inset(view.place(rect));
        let is_selected = index == selected;
        rounded(cr, tile);
        if is_selected {
            set_color(cr, accent, 0.22);
        } else {
            set_color(cr, text, 0.10);
        }
        let _ = cr.fill_preserve();
        if is_selected {
            set_color(cr, accent, 1.0);
        } else {
            set_color(cr, text, 0.45);
        }
        cr.set_line_width(2.0);
        let _ = cr.stroke();

        let display = &page.draft().displays[index];
        let star = if page.draft().is_primary(index) {
            "★ "
        } else {
            ""
        };
        let mode = display.mode().map_or(String::new(), |mode| {
            format!("{} × {}", mode.width, mode.height)
        });
        let lines = [
            format!(
                "<b>{star}{}</b>",
                gtk::glib::markup_escape_text(&display.title)
            ),
            format!("<small>{mode}</small>"),
            format!("<small>{}%</small>", display.scale.percent()),
        ];
        label(area, cr, tile, &lines, text);
    }

    if arrangement.len() > 1 {
        let hint = area.create_pango_layout(None);
        hint.set_markup("<small>Drag a display to rearrange</small>");
        let (hint_width, hint_height) = hint.pixel_size();
        set_color(cr, text, 0.6);
        cr.move_to(
            (width - f64::from(hint_width)) / 2.0,
            height - f64::from(hint_height) - 8.0,
        );
        pangocairo::functions::show_layout(cr, &hint);
    }
}

/// A tile a little inside its display's rectangle, so that two displays' borders that
/// touch are two lines.
fn inset((x, y, width, height): (f64, f64, f64, f64)) -> (f64, f64, f64, f64) {
    (
        x + 1.0,
        y + 1.0,
        (width - 2.0).max(1.0),
        (height - 2.0).max(1.0),
    )
}

/// Writes as many of `lines` as fit in the middle of `tile`, the first ones first.
fn label(
    area: &gtk::DrawingArea,
    cr: &cairo::Context,
    (x, y, width, height): (f64, f64, f64, f64),
    lines: &[String],
    color: gdk::RGBA,
) {
    let layout = area.create_pango_layout(None);
    layout.set_alignment(pango::Alignment::Center);
    layout.set_ellipsize(pango::EllipsizeMode::End);
    layout.set_width(((width - 8.0).max(1.0) * f64::from(pango::SCALE)) as i32);
    for count in (1..=lines.len()).rev() {
        layout.set_markup(&lines[..count].join("\n"));
        let (_, text_height) = layout.pixel_size();
        if f64::from(text_height) <= height - 6.0 {
            set_color(cr, color, 1.0);
            cr.move_to(x + 4.0, y + (height - f64::from(text_height)) / 2.0);
            pangocairo::functions::show_layout(cr, &layout);
            return;
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    const LAPTOP: Rect = Rect {
        x: 0,
        y: 0,
        w: 1422,
        h: 889,
    };
    const MONITOR: Rect = Rect {
        x: 1422,
        y: 0,
        w: 2560,
        h: 1440,
    };

    #[test]
    fn the_layout_is_in_the_middle_of_the_arrangement_with_room_around_it() {
        let view = View::fit(&[LAPTOP, MONITOR], 780.0, 300.0);

        let (left, top, _, _) = view.place(LAPTOP);
        let (monitor_left, monitor_top, width, height) = view.place(MONITOR);
        assert!((left - (780.0 - (monitor_left + width))).abs() < 1e-9);
        assert!((monitor_top - (300.0 - (monitor_top + height))).abs() < 1e-9);
        assert!(left > 0.0 && top > 0.0);
        assert!(view.hits(LAPTOP, left + 1.0, top + 1.0));
        assert!(!view.hits(LAPTOP, monitor_left + 1.0, top + 1.0));
    }

    #[test]
    fn one_small_display_isnt_drawn_huge() {
        let view = View::fit(&[LAPTOP], 780.0, 300.0);

        assert_eq!(view.place(LAPTOP).2, 1422.0 * 0.13);
    }

    #[test]
    fn the_snap_distance_is_twelve_of_the_arrangements_pixels_whatever_the_zoom() {
        let near = View::fit(&[LAPTOP], 780.0, 300.0);
        let far = View::fit(&[LAPTOP, MONITOR, MONITOR], 780.0, 300.0);

        assert_eq!(near.snap(), 92);
        assert!((f64::from(near.snap()) * near.zoom - 12.0).abs() < 0.1);
        assert!((f64::from(far.snap()) * far.zoom - 12.0).abs() < 0.1);
    }
}
