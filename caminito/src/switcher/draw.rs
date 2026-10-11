//! The Window switcher's chrome (108): everything of the popup but the thumbnails, drawn
//! with cairo in the GTK theme's colours and font (ADR 0003), as two buffers the scene puts
//! behind and in front of the thumbnails.

use caminito_theme::cairo::{self, Context, Format, ImageSurface};
use caminito_theme::{GtkState, PaletteColors, Rgba, Title, pango, pangocairo};
use smithay::backend::renderer::element::memory::MemoryRenderBuffer;
use smithay::utils::{Logical, Rectangle};

use super::layout::{CellStyle, Grid};
use crate::decoration::memory_buffer;

/// The width of the popup's border.
const BORDER: f64 = 1.0;
/// How much of the text colour the border is.
const BORDER_ALPHA: f64 = 0.3;
/// How much of the text colour tints the selected cell where the theme's selection has
/// the background of everything else.
const SELECTION_TINT: f64 = 0.25;
/// The corner icon's share of its content box's height, and the sizes it's kept within.
const CORNER_ICON_FRACTION: i32 = 4;
const CORNER_ICON_RANGE: (i32, i32) = (16, 48);
/// The urgent mark's diameter, and its distance from the cell's top right corner.
const URGENT_MARK: f64 = 8.0;
const URGENT_MARK_INSET: f64 = 4.0;
/// The GTK colour an urgent window is marked in, where the theme has it.
const URGENT_COLOR: &str = "warning_color";

/// What the chrome shows of one listed window.
pub(crate) struct Entry {
    /// Whether the window asks for the user's attention.
    pub(crate) urgent: bool,
    /// The window's icon, [`icon_size`] logical pixels square at the chrome's scale.
    pub(crate) icon: Option<ImageSurface>,
}

/// The popup's chrome for one output, as buffers at the output's scale that cover the
/// popup: `under` is behind the thumbnails (the background, the selected cell, the urgent
/// marks, the title, and the icons of an icons-only grid), and `over` is in front of them
/// (the thumbnails' corner icons).
pub(crate) struct Chrome {
    pub(crate) under: MemoryRenderBuffer,
    pub(crate) over: Option<MemoryRenderBuffer>,
}

/// The size a cell's icon is drawn at, in logical pixels: the cell's whole content box in
/// an icons-only grid, which is the `icon-size` key's unless the grid shrank it, and a
/// corner of it in a thumbnails grid.
pub(crate) fn icon_size(grid: &Grid, index: usize) -> i32 {
    let content = grid.content(index).size;
    match grid.style {
        CellStyle::Icons { icon_size } => icon_size.min(content.w).min(content.h),
        CellStyle::Thumbnails => (content.h / CORNER_ICON_FRACTION)
            .clamp(CORNER_ICON_RANGE.0, CORNER_ICON_RANGE.1)
            .min(content.w)
            .min(content.h),
    }
    .max(1)
}

/// Where a cell's icon is drawn, from the output's corner: in the middle of the content
/// box of an icons-only grid, and in the bottom right corner of a thumbnail's.
pub(crate) fn icon_rect(grid: &Grid, index: usize) -> Rectangle<i32, Logical> {
    let content = grid.content(index);
    let size = icon_size(grid, index);
    let loc = match grid.style {
        CellStyle::Icons { .. } => (
            content.loc.x + (content.size.w - size) / 2,
            content.loc.y + (content.size.h - size) / 2,
        ),
        CellStyle::Thumbnails => (
            content.loc.x + content.size.w - size,
            content.loc.y + content.size.h - size,
        ),
    };
    Rectangle::new(loc.into(), (size, size).into())
}

/// Where an urgent window's mark is drawn, from the output's corner.
pub(crate) fn urgent_mark_rect(grid: &Grid, index: usize) -> Rectangle<f64, Logical> {
    let cell = grid.cells[index].to_f64();
    Rectangle::new(
        (
            cell.loc.x + cell.size.w - URGENT_MARK_INSET - URGENT_MARK,
            cell.loc.y + URGENT_MARK_INSET,
        )
            .into(),
        (URGENT_MARK, URGENT_MARK).into(),
    )
}

/// Draws the chrome of `grid` with entry `selected` selected and `title` under the cells,
/// at `scale`. `entries` are in the grid's order.
pub(crate) fn draw(
    grid: &Grid,
    selected: usize,
    entries: &[Entry],
    title: &Title,
    colors: &PaletteColors,
    scale: i32,
) -> Result<Chrome, cairo::Error> {
    let thumbnails = grid.style == CellStyle::Thumbnails;
    let under = surface(grid.popup, scale)?;
    let over = thumbnails.then(|| surface(grid.popup, scale)).transpose()?;
    {
        let cr = context(&under, grid.popup, scale)?;
        draw_background(&cr, grid.popup, colors)?;
        if let Some(cell) = grid.cells.get(selected) {
            fill_selected(&cr, *cell, colors)?;
        }
        let urgent = colors
            .named(URGENT_COLOR)
            .unwrap_or(colors.text(GtkState::Normal));
        for (index, _) in entries.iter().enumerate().filter(|(_, e)| e.urgent) {
            let mark = urgent_mark_rect(grid, index);
            set_color(&cr, urgent, 1.0);
            cr.arc(
                mark.loc.x + mark.size.w / 2.0,
                mark.loc.y + mark.size.h / 2.0,
                mark.size.w / 2.0,
                0.0,
                std::f64::consts::TAU,
            );
            cr.fill()?;
        }
        draw_text(&cr, grid.title, title, colors.text(GtkState::Normal));
        if !thumbnails {
            draw_icons(&cr, grid, entries)?;
        }
    }
    if let Some(over) = &over {
        let cr = context(over, grid.popup, scale)?;
        draw_icons(&cr, grid, entries)?;
    }
    let buffer =
        |surface: ImageSurface| memory_buffer(surface, scale).ok_or(cairo::Error::NoMemory);
    Ok(Chrome {
        under: buffer(under)?,
        over: over.map(buffer).transpose()?,
    })
}

/// A transparent surface that covers `popup` at `scale`.
pub(crate) fn surface(
    popup: Rectangle<i32, Logical>,
    scale: i32,
) -> Result<ImageSurface, cairo::Error> {
    let size = popup.size;
    ImageSurface::create(Format::ARgb32, size.w * scale, size.h * scale)
}

/// A context that draws on `surface`, which covers `popup`, in logical pixels from the
/// output's corner, as a popup's rectangles are.
pub(crate) fn context(
    surface: &ImageSurface,
    popup: Rectangle<i32, Logical>,
    scale: i32,
) -> Result<Context, cairo::Error> {
    let cr = Context::new(surface)?;
    cr.scale(f64::from(scale), f64::from(scale));
    cr.translate(-f64::from(popup.loc.x), -f64::from(popup.loc.y));
    Ok(cr)
}

/// Sets the colour cairo draws in to `alpha` of `color`.
pub(crate) fn set_color(cr: &Context, color: Rgba, alpha: f64) {
    cr.set_source_rgba(color.red, color.green, color.blue, color.alpha * alpha);
}

/// A popup's background, in the theme's, inside a line of its text colour. The Workspace
/// popup's is the same (152).
pub(crate) fn draw_background(
    cr: &Context,
    popup: Rectangle<i32, Logical>,
    colors: &PaletteColors,
) -> Result<(), cairo::Error> {
    let popup = popup.to_f64();
    set_color(cr, colors.background(GtkState::Normal), 1.0);
    cr.rectangle(popup.loc.x, popup.loc.y, popup.size.w, popup.size.h);
    cr.fill()?;
    set_color(cr, colors.text(GtkState::Normal), BORDER_ALPHA);
    cr.set_line_width(BORDER);
    cr.rectangle(
        popup.loc.x + BORDER / 2.0,
        popup.loc.y + BORDER / 2.0,
        popup.size.w - BORDER,
        popup.size.h - BORDER,
    );
    cr.stroke()
}

/// Whether the theme's selection shows on the popup's background. A theme, or the neutral
/// palette, that gives a selected cell the background of any other would hide the
/// selection: the text colour tints the cell then.
pub(crate) fn selection_shows(colors: &PaletteColors) -> bool {
    colors.background(GtkState::Selected) != colors.background(GtkState::Normal)
}

/// The selected cell's fill, over `cell`.
pub(crate) fn fill_selected(
    cr: &Context,
    cell: Rectangle<i32, Logical>,
    colors: &PaletteColors,
) -> Result<(), cairo::Error> {
    let cell = cell.to_f64();
    if selection_shows(colors) {
        set_color(cr, colors.background(GtkState::Selected), 1.0);
    } else {
        set_color(cr, colors.text(GtkState::Normal), SELECTION_TINT);
    }
    cr.rectangle(cell.loc.x, cell.loc.y, cell.size.w, cell.size.h);
    cr.fill()
}

/// `text` on one line in the middle of `band`, ellipsised at its end if it's wider than the
/// band, and clipped to the band whatever pango makes of it: the selected window's title
/// here, and a workspace's name in the Workspace popup (152).
pub(crate) fn draw_text(cr: &Context, band: Rectangle<i32, Logical>, text: &Title, color: Rgba) {
    let layout = &text.layout;
    layout.set_width(band.size.w.max(0) * pango::SCALE);
    layout.set_alignment(pango::Alignment::Center);
    let (_, height) = layout.pixel_size();
    let band = band.to_f64();
    cr.save().ok();
    cr.rectangle(band.loc.x, band.loc.y, band.size.w, band.size.h);
    cr.clip();
    set_color(cr, color, 1.0);
    cr.move_to(
        band.loc.x,
        band.loc.y + ((band.size.h - f64::from(height)) / 2.0).max(0.0),
    );
    pangocairo::functions::show_layout(cr, layout);
    cr.restore().ok();
}

/// Each entry's icon in its place, scaled from the pixels it was loaded at.
fn draw_icons(cr: &Context, grid: &Grid, entries: &[Entry]) -> Result<(), cairo::Error> {
    for (index, entry) in entries.iter().enumerate() {
        let Some(icon) = &entry.icon else {
            continue;
        };
        let rect = icon_rect(grid, index).to_f64();
        cr.save()?;
        cr.translate(rect.loc.x, rect.loc.y);
        // The icon was loaded for this size at this scale, or the nearest there was.
        let pixels = f64::from(icon.width().max(1));
        cr.scale(rect.size.w / pixels, rect.size.h / pixels);
        cr.set_source_surface(icon, 0.0, 0.0)?;
        cr.paint()?;
        cr.restore()?;
    }
    Ok(())
}
