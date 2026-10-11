//! The window menu's drawing (161, ADR 0026): one buffer for a menu, drawn with cairo in
//! the GTK theme's colours and font (ADR 0003), as the Window switcher's chrome is (108).

use caminito_policy::MenuMark;
use caminito_theme::cairo::{self, Context};
use caminito_theme::{GtkState, PaletteColors, Rgba, Title, pango, pangocairo};
use smithay::backend::renderer::element::memory::MemoryRenderBuffer;
use smithay::utils::{Logical, Rectangle};

use super::labels::Label;
use super::layout::{ARROW_COLUMN, Layout, MARK_COLUMN};
use crate::decoration::memory_buffer;
use crate::switcher::draw::{
    context, draw_background, fill_selected, selection_shows, set_color, surface,
};

/// How much of the text colour a separator's line is.
const SEPARATOR_ALPHA: f64 = 0.3;
/// The side of a check or radio mark's box, and the width of its lines.
const MARK_SIZE: f64 = 12.0;
const MARK_LINE: f64 = 1.0;
/// How much of the text colour a greyed item is drawn in, where the theme's insensitive
/// text is its normal text, as in the neutral palette, and would hide that it's greyed.
const GREYED_ALPHA: f64 = 0.5;
/// The submenu arrow's width and height.
const ARROW_SIZE: (f64, f64) = (5.0, 8.0);

/// What a row shows.
pub(crate) enum Row {
    Separator,
    Item(Item),
}

/// An item, laid out.
pub(crate) struct Item {
    pub(crate) label: Title,
    /// Its keybinding's text.
    pub(crate) accelerator: Option<Title>,
    pub(crate) sensitive: bool,
    pub(crate) mark: MenuMark,
    /// Whether it opens the submenu, and so has an arrow.
    pub(crate) submenu: bool,
}

/// Lays `label` out, with its mnemonic's letter underlined.
pub(crate) fn label(
    context: &pango::Context,
    font: &pango::FontDescription,
    label: &Label,
) -> Title {
    let title = Title::new(context, font, 1.0, &label.text);
    if let Some((start, letter)) = label.mnemonic {
        let attributes = pango::AttrList::new();
        let mut underline = pango::AttrInt::new_underline(pango::Underline::Low);
        let end = label.text[start..]
            .chars()
            .next()
            .map_or(letter.len_utf8(), char::len_utf8);
        underline.set_start_index(start as u32);
        underline.set_end_index((start + end) as u32);
        attributes.insert(underline);
        title.layout.set_attributes(Some(&attributes));
    }
    title
}

/// Draws the menu of `layout` with row `selected` marked, at `scale`. `rows` are in the
/// layout's order.
pub(crate) fn draw(
    layout: &Layout,
    rows: &[Row],
    selected: Option<usize>,
    colors: &PaletteColors,
    scale: i32,
) -> Result<MemoryRenderBuffer, cairo::Error> {
    let surface = surface(layout.rect, scale)?;
    {
        let cr = context(&surface, layout.rect, scale)?;
        draw_background(&cr, layout.rect, colors)?;
        for (index, (rect, row)) in layout.rows.iter().zip(rows).enumerate() {
            let item = match row {
                Row::Separator => {
                    draw_separator(&cr, *rect, colors)?;
                    continue;
                }
                Row::Item(item) => item,
            };
            let is_selected = selected == Some(index);
            if is_selected {
                fill_selected(&cr, *rect, colors)?;
            }
            // On the theme's selection the text is in the theme's selected text.
            let normal = colors.text(GtkState::Normal);
            let insensitive = colors.text(GtkState::Insensitive);
            let color = if !item.sensitive && insensitive == normal {
                Rgba {
                    alpha: normal.alpha * GREYED_ALPHA,
                    ..normal
                }
            } else if !item.sensitive {
                insensitive
            } else if is_selected && selection_shows(colors) {
                colors.text(GtkState::Selected)
            } else {
                normal
            };
            draw_mark(&cr, *rect, item.mark, color)?;
            let left = f64::from(rect.loc.x + MARK_COLUMN);
            draw_text(&cr, *rect, left, &item.label, color);
            if let Some(accelerator) = &item.accelerator {
                let (width, _) = accelerator.layout.pixel_size();
                let right = rect.loc.x + rect.size.w - ARROW_COLUMN;
                draw_text(&cr, *rect, f64::from(right - width), accelerator, color);
            }
            if item.submenu {
                draw_arrow(&cr, *rect, color)?;
            }
        }
    }
    memory_buffer(surface, scale).ok_or(cairo::Error::NoMemory)
}

/// `text` from `left`, in the middle of `row`'s height.
fn draw_text(cr: &Context, row: Rectangle<i32, Logical>, left: f64, text: &Title, color: Rgba) {
    let layout = &text.layout;
    // The whole text: the menu is as wide as its widest.
    layout.set_width(-1);
    let (_, height) = layout.pixel_size();
    let row = row.to_f64();
    set_color(cr, color, 1.0);
    cr.move_to(
        left,
        row.loc.y + ((row.size.h - f64::from(height)) / 2.0).max(0.0),
    );
    pangocairo::functions::show_layout(cr, layout);
}

/// A line across the middle of `row`.
fn draw_separator(
    cr: &Context,
    row: Rectangle<i32, Logical>,
    colors: &PaletteColors,
) -> Result<(), cairo::Error> {
    let row = row.to_f64();
    set_color(cr, colors.text(GtkState::Normal), SEPARATOR_ALPHA);
    cr.rectangle(
        row.loc.x + 1.0,
        row.loc.y + (row.size.h / 2.0).floor(),
        row.size.w - 2.0,
        1.0,
    );
    cr.fill()
}

/// Where an item's mark is drawn: a square in the middle of the mark column.
pub(crate) fn mark_rect(row: Rectangle<i32, Logical>) -> Rectangle<f64, Logical> {
    let row = row.to_f64();
    Rectangle::new(
        (
            row.loc.x + (f64::from(MARK_COLUMN) - MARK_SIZE) / 2.0,
            row.loc.y + (row.size.h - MARK_SIZE) / 2.0,
        )
            .into(),
        (MARK_SIZE, MARK_SIZE).into(),
    )
}

/// A check box with its tick, or a radio button with its dot, as GTK draws a menu's.
fn draw_mark(
    cr: &Context,
    row: Rectangle<i32, Logical>,
    mark: MenuMark,
    color: Rgba,
) -> Result<(), cairo::Error> {
    let rect = mark_rect(row);
    let (x, y, size) = (rect.loc.x, rect.loc.y, rect.size.w);
    set_color(cr, color, 1.0);
    cr.set_line_width(MARK_LINE);
    match mark {
        MenuMark::None => {}
        MenuMark::Check(checked) => {
            let half = MARK_LINE / 2.0;
            cr.rectangle(x + half, y + half, size - MARK_LINE, size - MARK_LINE);
            cr.stroke()?;
            if checked {
                cr.set_line_width(2.0 * MARK_LINE);
                cr.move_to(x + size * 0.22, y + size * 0.52);
                cr.line_to(x + size * 0.43, y + size * 0.74);
                cr.line_to(x + size * 0.78, y + size * 0.26);
                cr.stroke()?;
            }
        }
        MenuMark::Radio(set) => {
            let (cx, cy) = (x + size / 2.0, y + size / 2.0);
            cr.arc(cx, cy, (size - MARK_LINE) / 2.0, 0.0, std::f64::consts::TAU);
            cr.stroke()?;
            if set {
                cr.arc(cx, cy, size / 4.0, 0.0, std::f64::consts::TAU);
                cr.fill()?;
            }
        }
    }
    Ok(())
}

/// The submenu's arrow, pointing right, in the middle of the arrow column.
fn draw_arrow(cr: &Context, row: Rectangle<i32, Logical>, color: Rgba) -> Result<(), cairo::Error> {
    let row = row.to_f64();
    let (width, height) = ARROW_SIZE;
    let x = row.loc.x + row.size.w - (f64::from(ARROW_COLUMN) + width) / 2.0;
    let y = row.loc.y + (row.size.h - height) / 2.0;
    set_color(cr, color, 1.0);
    cr.move_to(x, y);
    cr.line_to(x + width, y + height / 2.0);
    cr.line_to(x, y + height);
    cr.close_path();
    cr.fill()
}
