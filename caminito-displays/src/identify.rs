//! The labels Identify shows on the monitors themselves: each display's name, drawn here
//! and shown by the connection's layer surfaces.
//!
//! Decided in 84: a layer surface of caminito-displays, not a request to Caminito. Caminito
//! stays out of it, and the connection caminito-displays has for output management shows
//! them, with no library for GTK's own layer surfaces.

use caminito_displays::outputs::Label;
use caminito_displays::page::Layout;
use gtk::{cairo, pango};

/// Why a label couldn't be drawn.
#[derive(Debug)]
pub(crate) enum Error {
    /// Cairo couldn't make the picture or draw on it.
    Cairo(cairo::Error),
    /// The picture's pixels couldn't be read back.
    Pixels(cairo::BorrowError),
}

impl std::fmt::Display for Error {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Error::Cairo(err) => write!(f, "can't draw a label: {err}"),
            Error::Pixels(err) => write!(f, "can't read a label's pixels: {err}"),
        }
    }
}

impl std::error::Error for Error {}

impl From<cairo::Error> for Error {
    fn from(err: cairo::Error) -> Self {
        Error::Cairo(err)
    }
}

/// How long the labels stay, as in the prototype.
pub(crate) const SHOWN_FOR: std::time::Duration = std::time::Duration::from_secs(2);

/// A label for each display of `layout` that's on, which is what the monitors show.
pub(crate) fn labels(layout: &Layout) -> Vec<Label> {
    let label = |display: &caminito_displays::page::Display| {
        // A whole number of the picture's pixels for each logical one, enough for the
        // output's scale: the compositor scales the picture down, never up.
        let scale = display.scale.factor().ceil().clamp(1.0, 4.0) as i32;
        draw(&display.title, &display.name, scale)
            .map_err(|err| gtk::glib::g_warning!("caminito-displays", "{err}"))
            .ok()
    };
    let enabled = layout.displays.iter().filter(|display| display.enabled);
    enabled.filter_map(label).collect()
}

/// The picture of a label that says `title`, and under it the output's `name` where that
/// isn't the title already.
fn draw(title: &str, name: &str, scale: i32) -> Result<Label, Error> {
    const PADDING: i32 = 28;
    let markup = if title == name {
        format!("<b>{}</b>", gtk::glib::markup_escape_text(title))
    } else {
        format!(
            "<b>{}</b>\n<span size=\"50%\">{}</span>",
            gtk::glib::markup_escape_text(title),
            gtk::glib::markup_escape_text(name)
        )
    };
    let lay_out = |cr: &cairo::Context| {
        let layout = pangocairo::functions::create_layout(cr);
        layout.set_font_description(Some(&pango::FontDescription::from_string("Sans 30")));
        layout.set_alignment(pango::Alignment::Center);
        layout.set_markup(&markup);
        layout
    };

    // Measured on a surface of no size, then drawn on one that fits.
    let measure = cairo::ImageSurface::create(cairo::Format::ARgb32, 1, 1)?;
    let (text_width, text_height) = lay_out(&cairo::Context::new(&measure)?).pixel_size();
    let (width, height) = (text_width + 2 * PADDING, text_height + 2 * PADDING);

    let mut surface =
        cairo::ImageSurface::create(cairo::Format::ARgb32, width * scale, height * scale)?;
    {
        let cr = cairo::Context::new(&surface)?;
        cr.scale(f64::from(scale), f64::from(scale));
        let (w, h, radius) = (f64::from(width), f64::from(height), 14.0);
        use std::f64::consts::{FRAC_PI_2, PI};
        cr.arc(w - radius, radius, radius, -FRAC_PI_2, 0.0);
        cr.arc(w - radius, h - radius, radius, 0.0, FRAC_PI_2);
        cr.arc(radius, h - radius, radius, FRAC_PI_2, PI);
        cr.arc(radius, radius, radius, PI, 3.0 * FRAC_PI_2);
        cr.close_path();
        cr.set_source_rgba(0.1, 0.1, 0.1, 0.88);
        cr.fill()?;
        cr.set_source_rgb(1.0, 1.0, 1.0);
        cr.move_to(f64::from(PADDING), f64::from(PADDING));
        pangocairo::functions::show_layout(&cr, &lay_out(&cr));
    }
    surface.flush();
    // 32 bits a pixel, so cairo pads no row.
    let pixels = surface.data().map_err(Error::Pixels)?.to_vec();
    Ok(Label {
        output: name.to_owned(),
        width: width * scale,
        height: height * scale,
        scale,
        pixels,
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_label_is_a_picture_of_the_size_it_says_at_the_outputs_scale() {
        let once = draw("Dell U2723QE", "DP-1", 1).unwrap();
        let twice = draw("Dell U2723QE", "DP-1", 2).unwrap();

        assert_eq!(once.pixels.len(), (once.width * once.height * 4) as usize);
        assert_eq!(
            (twice.width, twice.height),
            (once.width * 2, once.height * 2)
        );
        assert_eq!(twice.output, "DP-1");
        // The corner is outside the rounded box, and the middle of the left edge is in it.
        assert_eq!(once.pixels[..4], [0, 0, 0, 0]);
        let middle = (once.height / 2 * once.width * 4) as usize;
        assert_ne!(once.pixels[middle + 4 * 4 + 3], 0);
    }
}
