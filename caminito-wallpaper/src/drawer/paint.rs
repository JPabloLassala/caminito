//! What an output's buffer shows: the colour, the picture placed over it, and a frame of a
//! cross-fade between two of them. Pictures are decoded by gdk-pixbuf and drawn by cairo,
//! neither of them GTK (ADR 0031).

use std::collections::HashMap;

use gio::glib;

use crate::positioning::{Positioning, place};
use crate::settings::{Colour, Drawn};

/// The decoded pictures, by file, so that a picture shown on several outputs, or drawn
/// again at another size, is decoded once. A file that can't be decoded is noted, so it's
/// logged once.
#[derive(Default)]
pub struct Pictures {
    decoded: HashMap<String, Option<cairo::ImageSurface>>,
}

impl Pictures {
    /// The picture in `file`, decoded the first time it's asked for.
    fn get(&mut self, file: &str) -> Option<&cairo::ImageSurface> {
        self.decoded
            .entry(file.to_owned())
            .or_insert_with(|| decode(file))
            .as_ref()
    }

    /// Forgets every picture but those `kept` names, as when the settings change.
    pub fn keep_only<'a>(&mut self, kept: impl IntoIterator<Item = &'a str>) {
        let kept: Vec<&str> = kept.into_iter().collect();
        self.decoded.retain(|file, _| kept.contains(&file.as_str()));
    }
}

/// The picture in `file` as a cairo surface, or `None`, logged, if it can't be decoded.
fn decode(file: &str) -> Option<cairo::ImageSurface> {
    let pixbuf = match gdk_pixbuf::Pixbuf::from_file(file) {
        Ok(pixbuf) => pixbuf,
        Err(err) => {
            glib::g_warning!(
                "caminito-wallpaperd",
                "can't load the picture {file}: {err}"
            );
            return None;
        }
    };
    // A JPEG's EXIF orientation, which a camera's picture has.
    let pixbuf = pixbuf.apply_embedded_orientation().unwrap_or(pixbuf);
    match surface_of(&pixbuf) {
        Ok(surface) => Some(surface),
        Err(err) => {
            glib::g_warning!(
                "caminito-wallpaperd",
                "can't draw the picture {file}: {err}"
            );
            None
        }
    }
}

/// `pixbuf`'s pixels as cairo's: premultiplied, in native-endian ARGB words.
fn surface_of(pixbuf: &gdk_pixbuf::Pixbuf) -> Result<cairo::ImageSurface, cairo::Error> {
    let (width, height) = (pixbuf.width(), pixbuf.height());
    let channels = pixbuf.n_channels() as usize;
    let has_alpha = pixbuf.has_alpha();
    let rowstride = pixbuf.rowstride() as usize;
    let bytes = pixbuf.read_pixel_bytes();
    let mut surface = cairo::ImageSurface::create(cairo::Format::ARgb32, width, height)?;
    let stride = surface.stride() as usize;
    {
        let mut data = surface
            .data()
            .expect("a surface just made has no other reference to its data");
        for y in 0..height as usize {
            let row = &bytes[y * rowstride..];
            let out = &mut data[y * stride..];
            for x in 0..width as usize {
                let pixel = &row[x * channels..x * channels + channels];
                let alpha = if has_alpha { pixel[3] } else { 0xff };
                let premultiply =
                    |channel: u8| ((u16::from(channel) * u16::from(alpha) + 127) / 255) as u8;
                let word = u32::from(alpha) << 24
                    | u32::from(premultiply(pixel[0])) << 16
                    | u32::from(premultiply(pixel[1])) << 8
                    | u32::from(premultiply(pixel[2]));
                out[x * 4..x * 4 + 4].copy_from_slice(&word.to_ne_bytes());
            }
        }
    }
    Ok(surface)
}

/// Sets `colour` as the source.
fn set_colour(cr: &cairo::Context, colour: Colour) {
    let channel = |value: u8| f64::from(value) / 255.0;
    cr.set_source_rgb(
        channel(colour.red),
        channel(colour.green),
        channel(colour.blue),
    );
}

/// `drawn` on an output of `size` pixels. A picture that can't be decoded leaves its colour.
pub fn render(
    drawn: &Drawn,
    size: (i32, i32),
    pictures: &mut Pictures,
) -> Result<cairo::ImageSurface, cairo::Error> {
    let surface = cairo::ImageSurface::create(cairo::Format::Rgb24, size.0, size.1)?;
    let cr = cairo::Context::new(&surface)?;
    let colour = match drawn {
        Drawn::Picture { colour, .. } | Drawn::Colour(colour) => *colour,
    };
    set_colour(&cr, colour);
    cr.paint()?;
    if let Drawn::Picture {
        file, positioning, ..
    } = drawn
        && let Some(picture) = pictures.get(file)
    {
        draw_picture(&cr, picture, size, *positioning)?;
    }
    drop(cr);
    Ok(surface)
}

/// Draws `picture` on an output of `size` pixels, placed by `positioning`.
fn draw_picture(
    cr: &cairo::Context,
    picture: &cairo::ImageSurface,
    size: (i32, i32),
    positioning: Positioning,
) -> Result<(), cairo::Error> {
    let natural = (picture.width(), picture.height());
    let rect = place(natural, size, positioning);
    let pattern = cairo::SurfacePattern::create(picture);
    pattern.set_filter(cairo::Filter::Good);
    let mut matrix = cairo::Matrix::identity();
    matrix.translate(f64::from(rect.x), f64::from(rect.y));
    matrix.scale(
        f64::from(rect.width) / f64::from(natural.0),
        f64::from(rect.height) / f64::from(natural.1),
    );
    matrix.invert();
    pattern.set_matrix(matrix);
    if positioning == Positioning::Tile {
        pattern.set_extend(cairo::Extend::Repeat);
        cr.set_source(&pattern)?;
        return cr.paint();
    }
    // Pad keeps a scaled picture's edges from fading into the colour beside them.
    pattern.set_extend(cairo::Extend::Pad);
    cr.set_source(&pattern)?;
    let (x, y) = (f64::from(rect.x), f64::from(rect.y));
    cr.rectangle(x, y, f64::from(rect.width), f64::from(rect.height));
    cr.fill()
}

/// A frame `progress` of the way from `from` to `to`, of the same size.
pub fn blend(
    from: &cairo::ImageSurface,
    to: &cairo::ImageSurface,
    progress: f64,
) -> Result<cairo::ImageSurface, cairo::Error> {
    let surface = cairo::ImageSurface::create(cairo::Format::Rgb24, to.width(), to.height())?;
    let cr = cairo::Context::new(&surface)?;
    cr.set_operator(cairo::Operator::Source);
    cr.set_source_surface(from, 0.0, 0.0)?;
    cr.paint()?;
    cr.set_operator(cairo::Operator::Over);
    cr.set_source_surface(to, 0.0, 0.0)?;
    cr.paint_with_alpha(progress.clamp(0.0, 1.0))?;
    drop(cr);
    Ok(surface)
}

#[cfg(test)]
mod tests {
    use super::*;

    /// The pixel at `x`, `y` of `surface`, as `0xrrggbb`.
    fn pixel(surface: &mut cairo::ImageSurface, x: i32, y: i32) -> u32 {
        let stride = surface.stride() as usize;
        let data = surface.data().unwrap();
        let at = y as usize * stride + x as usize * 4;
        u32::from_ne_bytes(data[at..at + 4].try_into().unwrap()) & 0xff_ffff
    }

    /// A PNG of `size` in one colour, `0xrrggbb`, in `dir`.
    fn png(dir: &std::path::Path, size: (i32, i32), rgb: u32) -> String {
        let surface = cairo::ImageSurface::create(cairo::Format::Rgb24, size.0, size.1).unwrap();
        let cr = cairo::Context::new(&surface).unwrap();
        let channel = |shift: u32| f64::from((rgb >> shift) & 0xff) / 255.0;
        cr.set_source_rgb(channel(16), channel(8), channel(0));
        cr.paint().unwrap();
        drop(cr);
        let path = dir.join(format!("{rgb:06x}.png"));
        surface
            .write_to_png(&mut std::fs::File::create(&path).unwrap())
            .unwrap();
        path.to_str().unwrap().to_owned()
    }

    fn picture(file: &str, positioning: Positioning) -> Drawn {
        Drawn::Picture {
            file: file.into(),
            positioning,
            colour: Colour::parse("#608040").unwrap(),
        }
    }

    #[test]
    fn fit_leaves_bars_of_the_outputs_colour() {
        let dir = tempfile::tempdir().unwrap();
        let file = png(dir.path(), (100, 100), 0x406080);
        let mut pictures = Pictures::default();

        let drawn = picture(&file, Positioning::Fit);
        let mut frame = render(&drawn, (400, 200), &mut pictures).unwrap();

        assert_eq!(pixel(&mut frame, 10, 100), 0x608040);
        assert_eq!(pixel(&mut frame, 200, 100), 0x406080);
        assert_eq!(pixel(&mut frame, 390, 100), 0x608040);
    }

    #[test]
    fn a_picture_that_cant_be_decoded_leaves_the_colour() {
        let mut pictures = Pictures::default();

        let drawn = picture("/nonexistent.png", Positioning::Fill);
        let mut frame = render(&drawn, (40, 20), &mut pictures).unwrap();

        assert_eq!(pixel(&mut frame, 20, 10), 0x608040);
    }

    #[test]
    fn a_blend_halfway_is_between_the_two() {
        let mut pictures = Pictures::default();
        let black = Drawn::Colour(Colour::default());
        let white = Drawn::Colour(Colour::parse("#ffffff").unwrap());
        let from = render(&black, (4, 4), &mut pictures).unwrap();
        let to = render(&white, (4, 4), &mut pictures).unwrap();

        let mut half = blend(&from, &to, 0.5).unwrap();
        let mut whole = blend(&from, &to, 1.0).unwrap();

        let grey = pixel(&mut half, 1, 1) & 0xff;
        assert!((0x7e..=0x81).contains(&grey), "{grey:x}");
        assert_eq!(pixel(&mut whole, 1, 1), 0xffffff);
    }
}
