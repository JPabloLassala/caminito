//! The Workspace popup's chrome (152), which is all of it: drawn with cairo in the GTK
//! theme's colours and font (ADR 0003), as the Window switcher's is (108), in one buffer.

use caminito_theme::cairo::{self, Context};
use caminito_theme::{GtkState, PaletteColors, Rgba, Title, pango, pangocairo};
use smithay::backend::renderer::element::memory::MemoryRenderBuffer;
use smithay::utils::{Logical, Rectangle, Size};

use super::layout::Layout;
use crate::decoration::memory_buffer;
use crate::switcher::draw::{
    context, draw_background, fill_selected, selection_shows, set_color, surface,
};

/// How much of the text colour tints a preview, to set the workspace off from the popup.
const PREVIEW_TINT: f64 = 0.12;
/// The width of a window's outline, and how much of the text colour it is.
const OUTLINE: f64 = 1.0;
const OUTLINE_ALPHA: f64 = 0.5;

/// What the popup shows of one workspace.
pub(crate) struct Workspace {
    /// Its name, laid out.
    pub(crate) name: Title,
    /// Its windows' frames on the popup's output, from the output's corner, bottom to top.
    pub(crate) windows: Vec<Rectangle<i32, Logical>>,
}

/// Draws the popup of `layout` with workspace `selected` marked, at `scale`. `workspaces`
/// are in the layout's order, and their windows are on an output of `output` logical
/// pixels.
pub(crate) fn draw(
    layout: &Layout,
    selected: usize,
    workspaces: &[Workspace],
    output: Size<i32, Logical>,
    colors: &PaletteColors,
    scale: i32,
) -> Result<MemoryRenderBuffer, cairo::Error> {
    let surface = surface(layout.popup, scale)?;
    {
        let cr = context(&surface, layout.popup, scale)?;
        draw_background(&cr, layout.popup, colors)?;
        let text = colors.text(GtkState::Normal);
        for (index, (cell, workspace)) in layout.cells.iter().zip(workspaces).enumerate() {
            let is_selected = index == selected;
            if is_selected {
                fill_selected(&cr, cell.rect, colors)?;
            }
            draw_preview(&cr, cell.preview, &workspace.windows, output, colors)?;
            // On the theme's selection the name is in the theme's selected text.
            let color = if is_selected && selection_shows(colors) {
                colors.text(GtkState::Selected)
            } else {
                text
            };
            draw_name(&cr, cell.name, &workspace.name, color);
        }
    }
    memory_buffer(surface, scale).ok_or(cairo::Error::NoMemory)
}

/// A workspace in small: the output, tinted, and each window's frame where it is on the
/// output, in the popup's background inside a line, as Marco draws a workspace's windows
/// in its popup (tabpopup.c:1140-1196). Nothing is drawn past the preview's edge.
fn draw_preview(
    cr: &Context,
    preview: Rectangle<i32, Logical>,
    windows: &[Rectangle<i32, Logical>],
    output: Size<i32, Logical>,
    colors: &PaletteColors,
) -> Result<(), cairo::Error> {
    let preview = preview.to_f64();
    let text = colors.text(GtkState::Normal);
    cr.save()?;
    cr.rectangle(preview.loc.x, preview.loc.y, preview.size.w, preview.size.h);
    cr.clip();
    set_color(cr, text, PREVIEW_TINT);
    cr.paint()?;
    let factor = (
        preview.size.w / f64::from(output.w.max(1)),
        preview.size.h / f64::from(output.h.max(1)),
    );
    for window in windows {
        let window = window.to_f64();
        let (x, y) = (
            preview.loc.x + window.loc.x * factor.0,
            preview.loc.y + window.loc.y * factor.1,
        );
        let (width, height) = (window.size.w * factor.0, window.size.h * factor.1);
        set_color(cr, colors.background(GtkState::Normal), 1.0);
        cr.rectangle(x, y, width, height);
        cr.fill()?;
        set_color(cr, text, OUTLINE_ALPHA);
        cr.set_line_width(OUTLINE);
        cr.rectangle(
            x + OUTLINE / 2.0,
            y + OUTLINE / 2.0,
            (width - OUTLINE).max(0.0),
            (height - OUTLINE).max(0.0),
        );
        cr.stroke()?;
    }
    cr.restore()
}

/// The name on one line in the middle of its band, ellipsised at its end if it's wider,
/// and clipped to the band whatever pango makes of it.
fn draw_name(cr: &Context, band: Rectangle<i32, Logical>, name: &Title, color: Rgba) {
    let layout = &name.layout;
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
