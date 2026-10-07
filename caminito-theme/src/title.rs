//! The title's text, laid out as Marco's `meta_frames_ensure_layout` lays it out. Its
//! text height sizes the titlebar, so geometry and drawing share this.

use pango::prelude::*;
use pangocairo::cairo;

/// A pango context for titles, set up as GTK sets up a widget's: the default cairo
/// font map, with the given font options, resolution and language.
pub fn title_context(
    font_options: &cairo::FontOptions,
    dpi: f64,
    language: &pango::Language,
) -> pango::Context {
    let context = pangocairo::FontMap::default().create_context();
    pangocairo::functions::context_set_font_options(&context, Some(font_options));
    pangocairo::functions::context_set_resolution(&context, dpi);
    context.set_language(Some(language));
    context
}

/// A window's title, ready to draw, and the text height the frame's geometry takes.
#[derive(Debug, Clone)]
pub struct Title {
    pub layout: pango::Layout,
    pub text_height: i32,
}

impl Title {
    /// Lays out `text` in the title font scaled by the frame style's `title_scale`
    /// ([`Theme::title_scale`](crate::Theme::title_scale)): ellipsized at the end, in one
    /// paragraph, left to right.
    pub fn new(
        context: &pango::Context,
        font: &pango::FontDescription,
        title_scale: f64,
        text: &str,
    ) -> Title {
        let font = scaled_font(font, title_scale);
        let layout = pango::Layout::new(context);
        layout.set_text(text);
        layout.set_ellipsize(pango::EllipsizeMode::End);
        layout.set_auto_dir(false);
        layout.set_single_paragraph_mode(true);
        layout.set_font_description(Some(&font));
        Title {
            layout,
            text_height: text_height(context, &font),
        }
    }
}

/// Marco's `meta_gtk_widget_get_font_desc`, after the font is chosen: the size scaled
/// and truncated, at least one pango unit.
fn scaled_font(font: &pango::FontDescription, scale: f64) -> pango::FontDescription {
    let mut font = font.clone();
    font.set_size(((scale * f64::from(font.size())) as i32).max(1));
    font
}

/// Marco's `meta_pango_font_desc_get_text_height`: the font's ascent plus descent,
/// rounded to pixels.
fn text_height(context: &pango::Context, font: &pango::FontDescription) -> i32 {
    let metrics = context.metrics(Some(font), context.language().as_ref());
    pixels(metrics.ascent() + metrics.descent())
}

/// `PANGO_PIXELS`: pango units to pixels, rounding halves up.
fn pixels(units: i32) -> i32 {
    (units + pango::SCALE / 2) >> 10
}
