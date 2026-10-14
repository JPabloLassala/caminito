//! The Overview's chrome (241): each window's label, its icon and its title, drawn with
//! cairo in the GTK theme's colours and font (ADR 0003), as the Window switcher's popup
//! is, in a buffer of its own that the scene puts beneath the window.

use caminito_theme::cairo::{self, ImageSurface};
use caminito_theme::{GtkState, PaletteColors, Title};
use smithay::backend::renderer::element::memory::MemoryRenderBuffer;
use smithay::utils::{Logical, Point, Rectangle, Size};

use crate::decoration::memory_buffer;
use crate::switcher::draw::{context, draw_background, draw_text, surface};

/// The size a label's icon is drawn at, in logical pixels.
pub(crate) const ICON_SIZE: i32 = 16;
/// The room between a label's edge and its icon, and between the icon and the title.
const PADDING: i32 = 6;

/// What a label shows of its window.
pub(crate) struct Label {
    /// The window's icon, [`ICON_SIZE`] logical pixels square at the label's scale.
    pub(crate) icon: Option<ImageSurface>,
    /// The window's title, laid out.
    pub(crate) title: Title,
}

/// Draws `label` to fill `rect`, at `scale`: the icon at the left where the label has room
/// for it, and the title in the rest, ellipsised.
pub(crate) fn draw(
    rect: Rectangle<i32, Logical>,
    label: &Label,
    colors: &PaletteColors,
    scale: i32,
) -> Result<MemoryRenderBuffer, cairo::Error> {
    let surface = surface(rect, scale)?;
    {
        let cr = context(&surface, rect, scale)?;
        draw_background(&cr, rect, colors)?;
        let icon_room = 2 * PADDING + ICON_SIZE;
        let icon = label
            .icon
            .as_ref()
            .filter(|_| rect.size.w >= icon_room && rect.size.h >= ICON_SIZE);
        let mut text = Rectangle::new(
            rect.loc + Point::from((PADDING, 0)),
            Size::from((rect.size.w - 2 * PADDING, rect.size.h)),
        );
        if let Some(icon) = icon {
            let (x, y) = (
                f64::from(rect.loc.x + PADDING),
                f64::from(rect.loc.y + (rect.size.h - ICON_SIZE) / 2),
            );
            cr.save()?;
            cr.translate(x, y);
            // The icon was loaded for this size at this scale, or the nearest there was.
            let pixels = f64::from(icon.width().max(1));
            let factor = f64::from(ICON_SIZE) / pixels;
            cr.scale(factor, factor);
            cr.set_source_surface(icon, 0.0, 0.0)?;
            cr.paint()?;
            cr.restore()?;
            text.loc.x += ICON_SIZE + PADDING;
            text.size.w -= ICON_SIZE + PADDING;
        }
        if text.size.w > 0 {
            draw_text(&cr, text, &label.title, colors.text(GtkState::Normal));
        }
    }
    memory_buffer(surface, scale).ok_or(cairo::Error::NoMemory)
}
