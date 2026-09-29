//! Camino's own cursors (13): the xcursor theme `cursor-theme` names, at `cursor-size`
//! times each output's rounded-up scale, and which cursor shows where.
//!
//! A cursor is loaded once per icon and scale, and loaded again when the theme or size
//! changes. On a fractional output it's loaded at the scale rounded up and the renderer
//! downsamples it, as niri and cosmic-comp do. An icon the theme lacks, under any of its
//! names, is Camino's built-in arrow.

use std::cell::RefCell;
use std::collections::HashMap;

use camino_theme::cairo::{self, Context, Format, ImageSurface};
use smithay::backend::allocator::Fourcc;
use smithay::backend::renderer::element::memory::MemoryRenderBuffer;
use smithay::input::pointer::{CursorIcon, CursorImageStatus};
use smithay::utils::{Logical, Physical, Point, Size, Transform};

use crate::decoration::control::resize_cursor;
use crate::input::GrabKind;
use crate::settings::translate::Mouse;
use crate::state::Camino;

/// Where cursors come from.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum CursorSource {
    /// The xcursor theme, found through `XCURSOR_PATH` and the icon directories.
    Xcursor,
    /// The built-in arrow only, so tests don't depend on the themes installed.
    #[cfg(test)]
    BuiltIn,
}

/// A cursor ready to draw at one scale.
#[derive(Debug, Clone)]
pub struct CursorImage {
    pub buffer: MemoryRenderBuffer,
    /// The pixel the pointer's position is, from the image's top left corner.
    pub hotspot: Point<i32, Logical>,
    pub size: Size<i32, Physical>,
    /// The integer scale it was loaded at.
    pub scale: i32,
}

/// A cursor for Xwayland's root window (41): premultiplied ARGB32 in the machine's byte
/// order, as X's depth-32 pictures take it, which is also xcursor's file order and cairo's.
pub(crate) struct XCursorImage {
    /// Four bytes a pixel, row by row.
    pub(crate) pixels: Vec<u8>,
    /// In pixels.
    pub(crate) size: Size<u16, Logical>,
    /// The pixel the pointer's position is, from the image's top left corner.
    pub(crate) hotspot: Point<u16, Logical>,
}

/// The cursor theme, and the cursors loaded from it.
pub struct Cursors {
    source: CursorSource,
    theme_name: String,
    size: u32,
    theme: Option<xcursor::CursorTheme>,
    loaded: RefCell<HashMap<(CursorIcon, i32), CursorImage>>,
}

impl Cursors {
    pub fn new(mouse: &Mouse, source: CursorSource) -> Self {
        let mut cursors = Self {
            source,
            theme_name: String::new(),
            size: 0,
            theme: None,
            loaded: RefCell::default(),
        };
        cursors.settings_changed(mouse);
        cursors
    }

    /// Takes up a new `cursor-theme` or `cursor-size`. Returns whether either changed.
    pub fn settings_changed(&mut self, mouse: &Mouse) -> bool {
        if (mouse.cursor_theme.as_str(), mouse.cursor_size) == (&self.theme_name, self.size) {
            return false;
        }
        self.theme_name = mouse.cursor_theme.clone();
        self.size = mouse.cursor_size;
        self.theme = match self.source {
            // An empty name is the default theme, which xcursor calls "default".
            CursorSource::Xcursor => Some(xcursor::CursorTheme::load(
                Some(self.theme_name.as_str())
                    .filter(|name| !name.is_empty())
                    .unwrap_or("default"),
            )),
            #[cfg(test)]
            CursorSource::BuiltIn => None,
        };
        self.loaded.borrow_mut().clear();
        true
    }

    /// `icon` for an output at `output_scale`, loaded at its scale rounded up.
    pub fn image(&self, icon: CursorIcon, output_scale: f64) -> CursorImage {
        let scale = (output_scale.ceil() as i32).max(1);
        self.loaded
            .borrow_mut()
            .entry((icon, scale))
            .or_insert_with(|| self.load(icon, scale))
            .clone()
    }

    /// The default cursor for Xwayland's root window at the X11 scale `scale`: X clients
    /// are in client space, so it's `cursor-size` × `scale` pixels, drawn unscaled.
    pub(crate) fn x_root_cursor(&self, scale: u32) -> XCursorImage {
        let pixels = self.size.saturating_mul(scale) as i32;
        let from_theme = self.theme.as_ref().and_then(|theme| {
            names(CursorIcon::Default)
                .into_iter()
                .find_map(|name| xcursor_image(theme, name, pixels))
        });
        match from_theme {
            Some(image) => XCursorImage {
                pixels: image.pixels_rgba,
                size: Size::from((image.width as u16, image.height as u16)),
                hotspot: Point::from((image.xhot as u16, image.yhot as u16)),
            },
            None => {
                let surface = draw_arrow(pixels).expect("cairo draws the built-in cursor");
                XCursorImage {
                    pixels: argb_rows(surface),
                    size: Size::from((pixels as u16, pixels as u16)),
                    hotspot: Point::from((0, 0)),
                }
            }
        }
    }

    fn load(&self, icon: CursorIcon, scale: i32) -> CursorImage {
        let pixels = self.size as i32 * scale;
        let from_theme = self.theme.as_ref().and_then(|theme| {
            names(icon)
                .into_iter()
                .find_map(|name| load_xcursor(theme, name, pixels, scale))
        });
        from_theme.unwrap_or_else(|| {
            tracing::debug!(
                "no {} cursor in the theme; drawing the built-in one",
                icon.name()
            );
            builtin_arrow(pixels, scale)
        })
    }
}

/// Every name an icon goes by in xcursor themes, the cursor-shape name first. Marco's move
/// cursor is X's `fleur`, which themes have far more often than `move`.
fn names(icon: CursorIcon) -> Vec<&'static str> {
    let extra: &[&str] = match icon {
        CursorIcon::Move => &["fleur"],
        _ => &[],
    };
    let mut names = vec![icon.name()];
    names.extend_from_slice(icon.alt_names());
    names.extend_from_slice(extra);
    names
}

/// The first frame of the theme's `name` cursor at the nominal size nearest `pixels`.
fn xcursor_image(
    theme: &xcursor::CursorTheme,
    name: &str,
    pixels: i32,
) -> Option<xcursor::parser::Image> {
    let path = theme.load_icon(name)?;
    let content = std::fs::read(&path)
        .inspect_err(|err| tracing::warn!("can't read the cursor {}: {err}", path.display()))
        .ok()?;
    let images = xcursor::parser::parse_xcursor(&content)?;
    let nearest = images
        .iter()
        .min_by_key(|image| (image.size as i32 - pixels).abs())?
        .size;
    images.into_iter().find(|image| image.size == nearest)
}

/// The first frame of the theme's `name` cursor at the nominal size nearest `pixels`, for
/// an output at `scale`.
fn load_xcursor(
    theme: &xcursor::CursorTheme,
    name: &str,
    pixels: i32,
    scale: i32,
) -> Option<CursorImage> {
    let image = xcursor_image(theme, name, pixels)?;
    let size = Size::from((image.width as i32, image.height as i32));
    Some(CursorImage {
        // xcursor's premultiplied RGBA bytes are DRM's little-endian ABGR8888.
        buffer: MemoryRenderBuffer::from_slice(
            &image.pixels_rgba,
            Fourcc::Abgr8888,
            (size.w, size.h),
            scale,
            Transform::Normal,
            None,
        ),
        hotspot: Point::from((image.xhot as i32 / scale, image.yhot as i32 / scale)),
        size,
        scale,
    })
}

/// Camino's own arrow, `pixels` square: white, outlined in black, with its hotspot at the
/// tip.
fn builtin_arrow(pixels: i32, scale: i32) -> CursorImage {
    let surface = draw_arrow(pixels).expect("cairo draws the built-in cursor");
    let size = Size::from((pixels, pixels));
    CursorImage {
        buffer: memory_buffer(surface, scale),
        hotspot: Point::from((0, 0)),
        size,
        scale,
    }
}

fn draw_arrow(pixels: i32) -> Result<ImageSurface, cairo::Error> {
    let surface = ImageSurface::create(Format::ARgb32, pixels, pixels)?;
    let cr = Context::new(&surface)?;
    let unit = f64::from(pixels) / 24.;
    cr.scale(unit, unit);
    // A classic arrow in a 24-unit box.
    for (x, y) in [
        (1., 1.),
        (1., 18.),
        (5.5, 14.),
        (8.5, 21.),
        (11.5, 19.8),
        (8.5, 13.),
        (14., 13.),
    ] {
        cr.line_to(x, y);
    }
    cr.close_path();
    cr.set_source_rgb(1., 1., 1.);
    cr.fill_preserve()?;
    cr.set_source_rgb(0., 0., 0.);
    cr.set_line_width(1.2);
    cr.stroke()?;
    drop(cr);
    Ok(surface)
}

/// A cairo ARGB32 surface's pixels, without the padding at the end of each row.
fn argb_rows(mut surface: ImageSurface) -> Vec<u8> {
    let stride = surface.stride() as usize;
    let row = surface.width() as usize * 4;
    let data = surface.data().expect("the cursor surface isn't shared");
    data.chunks(stride)
        .flat_map(|line| &line[..row])
        .copied()
        .collect()
}

/// A cairo ARGB32 surface as a buffer, which is `Argb8888` on little-endian machines.
fn memory_buffer(surface: ImageSurface, scale: i32) -> MemoryRenderBuffer {
    let (width, height) = (surface.width(), surface.height());
    let pixels = argb_rows(surface);
    MemoryRenderBuffer::from_slice(
        &pixels,
        Fourcc::Argb8888,
        (width, height),
        scale,
        Transform::Normal,
        None,
    )
}

impl Camino {
    /// The cursor to show: a move or resize cursor during a grab, Marco's cursor for the
    /// frame control under the pointer, or else the client's.
    pub fn cursor(&self) -> CursorImageStatus {
        let icon = match (self.window_grab, &self.frame_pointer) {
            (Some(GrabKind::Move), _) => CursorIcon::Move,
            (Some(GrabKind::Resize(edges)), _) => resize_cursor(edges),
            (None, pointer) if pointer.pressed.is_some() => CursorIcon::Default,
            (None, pointer) => match pointer.hover {
                Some((_, control)) => control.cursor(),
                None => return self.client_cursor.clone(),
            },
        };
        CursorImageStatus::Named(icon)
    }
}
