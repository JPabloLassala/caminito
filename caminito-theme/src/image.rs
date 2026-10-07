//! Theme images and window icons, as Marco turns them into cairo sources
//! (`theme.c:98-283`, `3354-3866`).
//!
//! Images load through gdk-pixbuf, as Marco loads them, which reads SVG through librsvg.
//! Their pixels are then worked on as bytes, as Marco works on its pixbufs, and turned
//! into cairo surfaces as `gdk_cairo_surface_create_from_pixbuf` turns them.

use std::collections::HashMap;
use std::path::{Path, PathBuf};
use std::sync::{Arc, Mutex};

use gdk_pixbuf::Pixbuf;
use pangocairo::cairo::{self, Content, Context, Extend, Format, ImageSurface, Surface};

use crate::color::Rgba;
use crate::model::ImageFill;

/// A pixbuf's pixels: 8-bit RGB or RGBA rows.
#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) struct Pixels {
    pub width: i32,
    pub height: i32,
    /// 3 or 4.
    pub n_channels: usize,
    pub rowstride: usize,
    pub data: Vec<u8>,
}

impl Pixels {
    fn from_pixbuf(pixbuf: &Pixbuf) -> Pixels {
        Pixels {
            width: pixbuf.width(),
            height: pixbuf.height(),
            n_channels: pixbuf.n_channels() as usize,
            rowstride: pixbuf.rowstride() as usize,
            data: pixbuf.read_pixel_bytes().to_vec(),
        }
    }

    /// A pixbuf of one colour, as `gdk_pixbuf_fill` fills it with Marco's
    /// `GDK_COLOR_RGBA`, which truncates the channels to bytes.
    pub fn filled(width: i32, height: i32, color: Rgba, alpha: u8) -> Pixels {
        let byte = |channel: f64| (channel * 255.0) as i32 as u8;
        let pixel = [byte(color.red), byte(color.green), byte(color.blue), alpha];
        let row: Vec<u8> = pixel.repeat(width as usize);
        Pixels {
            width,
            height,
            n_channels: 4,
            rowstride: row.len(),
            data: row.repeat(height as usize),
        }
    }

    fn row(&self, y: usize) -> &[u8] {
        let start = y * self.rowstride;
        &self.data[start..start + self.width as usize * self.n_channels]
    }

    /// Marco's `meta_gradient_add_alpha_horizontal`: the alpha channel multiplied by a
    /// gradient across the width, worked out in 8.8 fixed point.
    pub fn add_alpha(&mut self, alphas: &[u8]) {
        assert_eq!(
            self.n_channels, 4,
            "only a pixbuf with alpha takes an alpha gradient"
        );
        let width = self.width as usize;
        let n_alphas = alphas.len().min(width);
        if n_alphas == 1 || width == 0 {
            // Marco's simple_multiply_alpha.
            for y in 0..self.height as usize {
                let start = y * self.rowstride;
                for x in 0..width {
                    let alpha = &mut self.data[start + x * 4 + 3];
                    *alpha = (i32::from(*alpha) * i32::from(alphas[0]) / 255) as u8;
                }
            }
            return;
        }
        let width2 = width / (n_alphas - 1);
        let mut gradient = Vec::with_capacity(width);
        let mut a = i64::from(alphas[0]) << 8;
        for i in 1..n_alphas {
            let da = ((i64::from(alphas[i]) - i64::from(alphas[i - 1])) << 8) / width2 as i64;
            for _ in 0..width2 {
                gradient.push((a >> 8) as u8);
                a += da;
            }
            a = i64::from(alphas[i]) << 8;
        }
        gradient.resize(width, (a >> 8) as u8);
        for y in 0..self.height as usize {
            let start = y * self.rowstride;
            for (x, &factor) in gradient.iter().enumerate() {
                let alpha = &mut self.data[start + x * 4 + 3];
                *alpha = (i32::from(*alpha) * i32::from(factor) / 255) as u8;
            }
        }
    }

    /// Marco's `colorize_pixbuf`: black to the colour to white, by each pixel's
    /// intensity, with alpha kept.
    pub fn colorize(&self, color: Rgba) -> Pixels {
        let clamp = |v: f64| (v as i32).clamp(0, 255) as u8;
        let mut data = self.data.clone();
        for y in 0..self.height as usize {
            let start = y * self.rowstride;
            for x in 0..self.width as usize {
                let pixel = &mut data[start + x * self.n_channels..][..3];
                let intensity = (f64::from(pixel[0]) * 0.30
                    + f64::from(pixel[1]) * 0.59
                    + f64::from(pixel[2]) * 0.11)
                    / 255.0;
                let channel = |c: f64| {
                    if intensity <= 0.5 {
                        c * intensity * 2.0
                    } else {
                        c + (1.0 - c) * (intensity - 0.5) * 2.0
                    }
                };
                pixel[0] = clamp(255.0 * channel(color.red));
                pixel[1] = clamp(255.0 * channel(color.green));
                pixel[2] = clamp(255.0 * channel(color.blue));
            }
        }
        Pixels {
            data,
            ..self.clone()
        }
    }

    /// GTK3's `gdk_cairo_surface_create_from_pixbuf`: RGB24 for RGB, and premultiplied
    /// ARGB32, rounded as GDK rounds it, for RGBA.
    pub fn to_surface(&self) -> Result<ImageSurface, cairo::Error> {
        let format = if self.n_channels == 3 {
            Format::Rgb24
        } else {
            Format::ARgb32
        };
        let mut surface = ImageSurface::create(format, self.width, self.height)?;
        let stride = surface.stride() as usize;
        {
            let mut data = surface.data().map_err(|_| cairo::Error::SurfaceFinished)?;
            for y in 0..self.height as usize {
                let dest = &mut data[y * stride..][..self.width as usize * 4];
                for (q, p) in dest
                    .as_chunks_mut::<4>()
                    .0
                    .iter_mut()
                    .zip(self.row(y).chunks_exact(self.n_channels))
                {
                    // Cairo's pixels are native-endian 0xAARRGGBB words.
                    let [red, green, blue] = [p[0], p[1], p[2]];
                    let (alpha, red, green, blue) = if self.n_channels == 3 {
                        (0xff, red, green, blue)
                    } else {
                        let alpha = p[3];
                        (
                            alpha,
                            mult(red, alpha),
                            mult(green, alpha),
                            mult(blue, alpha),
                        )
                    };
                    let word = u32::from_be_bytes([alpha, red, green, blue]);
                    q.copy_from_slice(&word.to_ne_bytes());
                }
            }
        }
        Ok(surface)
    }
}

/// GDK's `MULT`: a channel premultiplied by alpha, rounded.
fn mult(channel: u8, alpha: u8) -> u8 {
    let t = u32::from(channel) * u32::from(alpha) + 0x80;
    (((t >> 8) + t) >> 8) as u8
}

/// A window icon as Marco draws it: the pixbuf converted as
/// `gdk_cairo_surface_create_from_pixbuf` converts it.
pub fn surface_from_pixbuf(pixbuf: &Pixbuf) -> Result<ImageSurface, cairo::Error> {
    Pixels::from_pixbuf(pixbuf).to_surface()
}

/// A theme image loaded at one scale.
#[derive(Debug)]
pub(crate) struct Image {
    pub pixels: Pixels,
    /// Every row is one colour, so it's stretched by repeating columns.
    pub horizontal_stripes: bool,
    /// Every column is one colour, so it's stretched by repeating rows.
    pub vertical_stripes: bool,
}

impl Image {
    /// `meta_theme_load_image`: the file at its own size times the scale, through
    /// gdk-pixbuf. `None` if it doesn't load.
    fn load(path: &Path, scale: i32) -> Option<Image> {
        let (_, width, height) = Pixbuf::file_info(path)?;
        let pixbuf = Pixbuf::from_file_at_size(path, width * scale, height * scale).ok()?;
        let pixels = Pixels::from_pixbuf(&pixbuf);
        let (horizontal_stripes, vertical_stripes) = stripes(&pixels);
        Some(Image {
            pixels,
            horizontal_stripes,
            vertical_stripes,
        })
    }
}

/// The parser's stripe checks (`theme-parser.c:2265`), which step through the first
/// row's bytes by pixel count rather than pixel size, so they look at fewer pixels
/// than they mean to.
fn stripes(pixels: &Pixels) -> (bool, bool) {
    let (width, height) = (pixels.width as usize, pixels.height as usize);
    let n = pixels.n_channels;
    let rs = pixels.rowstride;
    let px = &pixels.data;
    let horizontal = (0..height)
        .all(|h| (1..width).all(|w| (0..n).all(|c| px[h * rs + c] == px[h * rs + w + c])));
    let vertical =
        (0..width).all(|w| (1..height).all(|h| (0..n).all(|c| px[w + c] == px[h * rs + w + c])));
    (horizontal, vertical)
}

/// Images by file and scale, `None` for one that didn't load.
type Images = HashMap<(PathBuf, i32), Option<Arc<Image>>>;

/// A theme's images, loaded once per file and scale.
#[derive(Debug, Default)]
pub(crate) struct ImageCache {
    images: Mutex<Images>,
}

impl ImageCache {
    pub fn get(&self, path: &Path, scale: i32) -> Option<Arc<Image>> {
        let mut images = self.images.lock().unwrap_or_else(|e| e.into_inner());
        images
            .entry((path.to_owned(), scale))
            .or_insert_with(|| Image::load(path, scale).map(Arc::new))
            .clone()
    }
}

/// Marco's `scale_surface`: a stripe image is only stretched across its stripes.
fn scale_surface(
    surface: &Surface,
    old_width: f64,
    old_height: f64,
    mut new_width: f64,
    mut new_height: f64,
    vertical_stripes: bool,
    horizontal_stripes: bool,
) -> Result<Surface, cairo::Error> {
    let mut scale_x = new_width / old_width;
    let mut scale_y = new_height / old_height;
    if horizontal_stripes && !vertical_stripes {
        new_width = old_width;
        scale_x = 1.0;
    } else if vertical_stripes && !horizontal_stripes {
        new_height = old_height;
        scale_y = 1.0;
    }
    let scaled = surface.create_similar(
        Content::ColorAlpha,
        new_width.ceil() as i32,
        new_height.ceil() as i32,
    )?;
    let cr = Context::new(&scaled)?;
    cr.scale(scale_x, scale_y);
    cr.set_source_surface(surface, 0.0, 0.0)?;
    cr.source().set_extend(Extend::Pad);
    // A size Marco can't evaluate is 0, which puts this context in error and leaves
    // the surface empty, as it does in Marco.
    let _ = cr.paint();
    Ok(scaled)
}

/// Marco's `get_surface_from_pixbuf`: an image or icon at `width` by `height` device
/// pixels, scaled or tiled.
pub(crate) fn surface_at_size(
    source: &ImageSurface,
    fill: ImageFill,
    width: f64,
    height: f64,
    vertical_stripes: bool,
    horizontal_stripes: bool,
) -> Result<Surface, cairo::Error> {
    let pixbuf_width = f64::from(source.width());
    let pixbuf_height = f64::from(source.height());
    let mut surface: Surface = (**source).clone();
    if pixbuf_width == width && pixbuf_height == height {
        return Ok(surface);
    }
    if fill != ImageFill::Tile {
        surface = scale_surface(
            &surface,
            pixbuf_width,
            pixbuf_height,
            width,
            height,
            vertical_stripes,
            horizontal_stripes,
        )?;
    }
    let copy = surface.create_similar(
        Content::ColorAlpha,
        width.ceil() as i32,
        height.ceil() as i32,
    )?;
    let cr = Context::new(&copy)?;
    cr.set_source_surface(&surface, 0.0, 0.0)?;
    if fill == ImageFill::Tile || vertical_stripes || horizontal_stripes {
        cr.source().set_extend(Extend::Repeat);
    }
    let _ = cr.paint();
    Ok(copy)
}

#[cfg(test)]
mod tests {
    use super::*;

    fn rgba(width: i32, height: i32, data: &[u8]) -> Pixels {
        Pixels {
            width,
            height,
            n_channels: 4,
            rowstride: width as usize * 4,
            data: data.to_vec(),
        }
    }

    #[test]
    fn filling_truncates_the_channels() {
        let color = Rgba {
            red: 1.0,
            green: 0.5,
            blue: 0.999,
            alpha: 1.0,
        };
        assert_eq!(Pixels::filled(1, 1, color, 51).data, [255, 127, 254, 51]);
    }

    #[test]
    fn an_alpha_gradient_steps_across_the_width() {
        let mut pixels = Pixels::filled(4, 1, Rgba::from_rgb8(0, 0, 0), 255);
        pixels.add_alpha(&[0, 255]);
        let alphas: Vec<u8> = pixels.data.chunks(4).map(|p| p[3]).collect();
        // 255 * 256 / 4 per pixel, in 8.8 fixed point.
        assert_eq!(alphas, [0, 63, 127, 191]);
    }

    #[test]
    fn colorizing_goes_from_black_through_the_colour_to_white() {
        let color = Rgba {
            red: 0.25,
            green: 0.25,
            blue: 0.25,
            alpha: 1.0,
        };
        let pixels = rgba(
            4,
            1,
            &[
                0, 0, 0, 10, 101, 101, 101, 20, 200, 200, 200, 30, 255, 255, 255, 40,
            ],
        );
        let channels: Vec<[u8; 2]> = pixels
            .colorize(color)
            .data
            .chunks(4)
            .map(|p| [p[0], p[3]])
            .collect();
        // 2 * 0.25 * 101 below halfway; 0.25 * 110 + 145 above it, both truncated.
        assert_eq!(channels, [[0, 10], [50, 20], [172, 30], [255, 40]]);
    }

    #[test]
    fn surfaces_are_premultiplied_as_gdk_rounds() {
        let pixels = rgba(1, 1, &[255, 128, 1, 128]);
        let mut surface = pixels.to_surface().unwrap();
        let data = surface.data().unwrap();
        let word = u32::from_ne_bytes(data[..4].try_into().unwrap());
        assert_eq!(word.to_be_bytes(), [128, 128, 64, 1]);
    }

    #[test]
    fn stripes_are_checked_as_marco_checks_them() {
        // Every row one colour.
        let rows = rgba(2, 2, &[1, 1, 1, 1, 1, 1, 1, 1, 2, 2, 2, 2, 2, 2, 2, 2]);
        assert_eq!(stripes(&rows), (true, false));
        // Every column one colour.
        let columns = rgba(2, 2, &[1, 1, 1, 1, 2, 2, 2, 2, 1, 1, 1, 1, 2, 2, 2, 2]);
        assert_eq!(stripes(&columns), (false, true));
        // Marco compares bytes 0..n with bytes w..w+n, so a row whose second pixel
        // differs only past its first byte still counts as one colour.
        let quirk = rgba(2, 1, &[1, 1, 1, 1, 1, 9, 9, 9]);
        assert_eq!(stripes(&quirk), (true, true));
    }
}
