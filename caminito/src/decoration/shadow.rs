//! The shadow around a frame (07, 171): Marco's compositor shadow, a Gaussian of the frame's
//! visible rectangle with the frame's own shape cut out of it, so it shows outside the frame
//! and in the pixels its rounded corners leave.
//!
//! The parameters are `compositor-xrender.c`'s. Marco chooses them by the focus alone: a
//! framed window of any type has the large shadow while it has the focus and the medium one
//! otherwise (:2208-2211, :3357, :3403), and none while it's maximised or tiled to a side
//! (:1025-1043). Its small shadow is for nothing.
//!
//! The shadow is drawn at the integer scale the frame's strips are (13), so that the shape
//! cut out of it is the strips' own, and in four bands around the frame rather than one
//! image with the window's whole area in it.

use caminito_theme::FrameFlags;
use smithay::utils::{Buffer, Logical, Point, Rectangle, Size};

/// `SHADOW_OPACITY`, 0.66, as `make_shadow` applies it: in 25ths, rounded down
/// (compositor-xrender.c:238, :484).
const OPACITY_25THS: u32 = 16;

/// Which of Marco's shadows a frame has (`MetaShadowType`).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ShadowType {
    /// A frame without the focus.
    Medium,
    /// The focused frame.
    Large,
}

impl ShadowType {
    /// The shadow of a frame drawn with `flags`, or `None` where Marco draws none.
    pub fn of(flags: &FrameFlags) -> Option<Self> {
        if flags.maximized || flags.tiled_left || flags.tiled_right {
            return None;
        }
        Some(if flags.has_focus {
            ShadowType::Large
        } else {
            ShadowType::Medium
        })
    }

    /// The Gaussian's radius, in logical pixels (`SHADOW_MEDIUM_RADIUS`,
    /// `SHADOW_LARGE_RADIUS`).
    fn radius(self) -> i32 {
        match self {
            ShadowType::Medium => 6,
            ShadowType::Large => 12,
        }
    }

    /// Where the shadow's top left corner is from the visible rectangle's
    /// (`shadow_offsets_x`, `shadow_offsets_y`, truncated as `win_extents` does).
    fn offset(self) -> Point<i32, Logical> {
        match self {
            ShadowType::Medium => (-9, -7).into(),
            ShadowType::Large => (-15, -15).into(),
        }
    }

    /// How much larger the shadow is than the visible rectangle, each way: the size of
    /// the Gaussian's map (`make_gaussian_map`).
    fn spread(self) -> i32 {
        (self.radius() * 3 + 1) & !1
    }

    /// The shadow of a frame whose visible rectangle is `visible`, in the same space.
    pub fn rect(self, visible: Rectangle<i32, Logical>) -> Rectangle<i32, Logical> {
        let spread = self.spread();
        Rectangle::new(
            visible.loc + self.offset(),
            visible.size + Size::from((spread, spread)),
        )
    }
}

/// What a shadow is drawn from. Two frames with the same have the same shadow.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ShadowInputs {
    pub shadow_type: ShadowType,
    /// The size of the frame's visible rectangle.
    pub size: Size<i32, Logical>,
    /// The integer scale the shadow is drawn at: the strips'.
    pub scale: i32,
    /// The pixels the rounded corners cut out of the visible rectangle, from its top left
    /// corner at `scale`.
    pub cutouts: Vec<Rectangle<i32, Buffer>>,
}

/// One band of a shadow: black, with premultiplied alpha, as `Argb8888`.
#[derive(Debug, PartialEq, Eq)]
pub struct Band {
    /// Where it goes, from the visible rectangle's top left corner.
    pub rect: Rectangle<i32, Logical>,
    pub size: Size<i32, Buffer>,
    pub pixels: Vec<u8>,
}

/// How much of the Gaussian falls on each pixel along one axis of a shadow of a frame
/// `length` pixels long, from the shadow's start (`sum_gaussian`, one axis of it: the map
/// is the product of its axes). The map's centre is past its middle, and `make_shadow`
/// works out the near half alone and mirrors it (:537-588), as this does.
fn profile(radius: f64, spread: usize, length: usize) -> Vec<f64> {
    let centre = (spread / 2) as f64;
    let map: Vec<f64> = (0..spread)
        .map(|at| {
            let distance = at as f64 - centre;
            (-(distance * distance) / (2.0 * radius * radius)).exp()
        })
        .collect();
    let total: f64 = map.iter().sum();
    let whole = length + spread;
    (0..whole)
        .map(|at| {
            let at = at.min(whole - 1 - at);
            let start = spread.saturating_sub(at);
            let end = (length + spread - at).min(spread);
            map[start..end].iter().sum::<f64>() / total
        })
        .collect()
}

/// The shadow's bands, which together are all of it but the part under the frame: one
/// above and one below, each reaching as far into the frame as its rounded corners do, and
/// one at each side between them.
pub fn bands(inputs: &ShadowInputs) -> Vec<Band> {
    let ShadowInputs {
        shadow_type,
        size,
        scale,
        ref cutouts,
    } = *inputs;
    if size.w <= 0 || size.h <= 0 || scale <= 0 {
        return Vec::new();
    }
    let spread = shadow_type.spread();
    let radius = f64::from(shadow_type.radius() * scale);
    let along = |length: i32| profile(radius, (spread * scale) as usize, (length * scale) as usize);
    let (across, down) = (along(size.w), along(size.h));
    // Marco's tables hold the Gaussian's sum as a byte, and scale that by the opacity
    // (`presum_gaussian`).
    let alpha = |x: i32, y: i32| {
        let sum = (across[x as usize] * down[y as usize]).min(1.0);
        ((sum * 255.0) as u32 * OPACITY_25THS / 25) as u8
    };

    // From here on the space is the shadow's own, from its top left corner.
    let inside = Rectangle::new(Point::default() - shadow_type.offset(), size);
    let whole: Size<i32, Logical> = (size.w + spread, size.h + spread).into();
    let frame: Rectangle<i32, Buffer> = Rectangle::new(
        (inside.loc.x * scale, inside.loc.y * scale).into(),
        (size.w * scale, size.h * scale).into(),
    );
    let cutouts: Vec<Rectangle<i32, Buffer>> = cutouts
        .iter()
        .map(|cutout| Rectangle::new(cutout.loc + frame.loc, cutout.size))
        .collect();

    // How many rows of the frame the corners cut into, from its top and from its bottom.
    let rows = |pixels: i32| (pixels + scale - 1) / scale;
    let middle = frame.loc.y + frame.size.h / 2;
    let mut top = 0;
    let mut bottom = 0;
    for cutout in cutouts.iter().filter(|cutout| !cutout.is_empty()) {
        if cutout.loc.y < middle {
            top = top.max(rows(cutout.loc.y + cutout.size.h - frame.loc.y));
        } else {
            bottom = bottom.max(rows(frame.loc.y + frame.size.h - cutout.loc.y));
        }
    }
    let top = top.min(size.h);
    let bottom = bottom.min(size.h - top);
    let (above, below) = (inside.loc.y + top, inside.loc.y + size.h - bottom);
    let right = inside.loc.x + size.w;

    [
        Rectangle::new((0, 0).into(), (whole.w, above).into()),
        Rectangle::new((0, below).into(), (whole.w, whole.h - below).into()),
        Rectangle::new((0, above).into(), (inside.loc.x, below - above).into()),
        Rectangle::new(
            (right, above).into(),
            (whole.w - right, below - above).into(),
        ),
    ]
    .into_iter()
    .filter(|rect: &Rectangle<i32, Logical>| !rect.is_empty())
    .map(|rect| {
        let area: Rectangle<i32, Buffer> = Rectangle::new(
            (rect.loc.x * scale, rect.loc.y * scale).into(),
            (rect.size.w * scale, rect.size.h * scale).into(),
        );
        let mut pixels = vec![0; (area.size.w * area.size.h * 4) as usize];
        let mut fill = |part: Rectangle<i32, Buffer>, shown: &dyn Fn(i32, i32) -> bool| {
            for y in part.loc.y..part.loc.y + part.size.h {
                for x in part.loc.x..part.loc.x + part.size.w {
                    if shown(x, y) {
                        let at = ((y - area.loc.y) * area.size.w + x - area.loc.x) as usize * 4;
                        // Black needs its alpha alone.
                        pixels[at + 3] = alpha(x, y);
                    }
                }
            }
        };
        fill(area, &|x, y| !frame.contains((x, y)));
        for cutout in &cutouts {
            if let Some(part) = cutout.intersection(area) {
                fill(part, &|_, _| true);
            }
        }
        Band {
            rect: Rectangle::new(rect.loc + shadow_type.offset(), rect.size),
            size: area.size,
            pixels,
        }
    })
    .collect()
}

#[cfg(test)]
mod tests {
    use caminito_theme::FrameFlags;
    use smithay::utils::{Buffer, Logical, Physical, Point, Rectangle, Size};

    use super::{Band, ShadowInputs, ShadowType, bands};

    fn inputs(shadow_type: ShadowType, size: (i32, i32), scale: i32) -> ShadowInputs {
        ShadowInputs {
            shadow_type,
            size: size.into(),
            scale,
            cutouts: Vec::new(),
        }
    }

    /// The alpha at a logical point from the visible rectangle's top left corner: that of
    /// the first pixel there, or `None` where no band is.
    fn alpha_at(bands: &[Band], scale: i32, x: i32, y: i32) -> Option<u8> {
        let point = Point::<i32, Logical>::from((x, y));
        let band = bands.iter().find(|band| band.rect.contains(point))?;
        let (x, y) = ((x - band.rect.loc.x) * scale, (y - band.rect.loc.y) * scale);
        Some(band.pixels[((y * band.size.w + x) * 4 + 3) as usize])
    }

    #[test]
    fn the_focused_frame_has_the_large_shadow_and_any_other_the_medium_one() {
        let flags = |has_focus| FrameFlags {
            has_focus,
            ..FrameFlags::default()
        };
        assert_eq!(ShadowType::of(&flags(true)), Some(ShadowType::Large));
        assert_eq!(ShadowType::of(&flags(false)), Some(ShadowType::Medium));
    }

    #[test]
    fn a_maximised_or_side_tiled_frame_has_no_shadow() {
        let focused = FrameFlags {
            has_focus: true,
            ..FrameFlags::default()
        };
        for flags in [
            FrameFlags {
                maximized: true,
                ..focused
            },
            FrameFlags {
                tiled_left: true,
                ..focused
            },
            FrameFlags {
                tiled_right: true,
                ..FrameFlags::default()
            },
        ] {
            assert_eq!(ShadowType::of(&flags), None, "{flags:?}");
        }
        let shaded = FrameFlags {
            shaded: true,
            ..focused
        };
        assert_eq!(ShadowType::of(&shaded), Some(ShadowType::Large));
    }

    #[test]
    fn the_shadow_reaches_past_the_visible_rectangle_by_marcos_offsets() {
        let visible = Rectangle::new((100, 50).into(), (400, 300).into());
        assert_eq!(
            ShadowType::Large.rect(visible),
            Rectangle::new((85, 35).into(), (436, 336).into()),
            "15 left and above, 21 right and below"
        );
        assert_eq!(
            ShadowType::Medium.rect(visible),
            Rectangle::new((91, 43).into(), (418, 318).into()),
            "9 left and right, 7 above, 11 below"
        );
    }

    #[test]
    fn the_shadow_lands_on_whole_pixels_at_scales_1_and_1_5_and_2() {
        let visible = Rectangle::<i32, Logical>::new((101, 51).into(), (401, 301).into());
        for (shadow_type, scale, expected) in [
            (ShadowType::Large, 1.0, ((86, 36), (437, 337))),
            (ShadowType::Large, 1.5, ((129, 54), (656, 506))),
            (ShadowType::Large, 2.0, ((172, 72), (874, 674))),
            (ShadowType::Medium, 1.0, ((92, 44), (419, 319))),
            (ShadowType::Medium, 1.5, ((138, 66), (629, 479))),
            (ShadowType::Medium, 2.0, ((184, 88), (838, 638))),
        ] {
            let rect = shadow_type.rect(visible);
            let physical: Rectangle<i32, Physical> = rect.to_physical_precise_round(scale);
            let (loc, size) = expected;
            assert_eq!(
                physical,
                Rectangle::new(loc.into(), size.into()),
                "{shadow_type:?} at {scale}"
            );
        }
    }

    #[test]
    fn the_bands_are_the_shadows_rectangle_without_the_frame() {
        for shadow_type in [ShadowType::Medium, ShadowType::Large] {
            for scale in [1, 2] {
                let bands = bands(&inputs(shadow_type, (200, 100), scale));
                let whole = shadow_type.rect(Rectangle::from_size((200, 100).into()));

                let area: i32 = bands.iter().map(|b| b.rect.size.w * b.rect.size.h).sum();
                assert_eq!(area, whole.size.w * whole.size.h - 200 * 100);
                for (index, band) in bands.iter().enumerate() {
                    assert!(whole.contains_rect(band.rect), "{band:?}");
                    assert_eq!(
                        band.size,
                        Size::<i32, Buffer>::from((
                            band.rect.size.w * scale,
                            band.rect.size.h * scale
                        ))
                    );
                    assert_eq!(band.pixels.len() as i32, band.size.w * band.size.h * 4);
                    for other in &bands[index + 1..] {
                        assert!(!band.rect.overlaps(other.rect));
                    }
                }
                assert_eq!(alpha_at(&bands, scale, 100, 50), None, "under the frame");
            }
        }
    }

    #[test]
    fn the_shadow_is_black_and_darkest_beside_the_frame_and_fades_away_from_it() {
        let bands = bands(&inputs(ShadowType::Large, (200, 100), 1));
        let alpha = |x, y| alpha_at(&bands, 1, x, y).unwrap();

        for band in &bands {
            for pixel in band.pixels.chunks(4) {
                assert_eq!(pixel[..3], [0, 0, 0]);
            }
        }
        // Marco's opacity is 16/25 of a byte's 255.
        assert!(alpha(100, 100) > 64 && alpha(100, 100) <= 163);
        assert!(alpha(100, 100) > alpha(100, 110));
        assert!(alpha(100, 110) > alpha(100, 120));
        assert_eq!(alpha(-15, -15), 0, "the far corner");
        // Each side is the one across from it, mirrored, as Marco builds them.
        assert_eq!(alpha(220, 120), 0);
        assert_eq!(alpha(-10, 53), alpha(215, 53));
        assert_eq!(alpha(103, -10), alpha(103, 115));
        // The large shadow lies 3 pixels right of and below the frame's middle.
        assert!(alpha(205, 50) > alpha(-6, 50));
        assert!(alpha(100, 105) > alpha(100, -6));
    }

    #[test]
    fn the_shadow_shows_in_the_pixels_the_rounded_corners_cut_out() {
        let mut inputs = inputs(ShadowType::Large, (200, 100), 2);
        inputs.cutouts = vec![
            Rectangle::new((0, 0).into(), (3, 1).into()),
            Rectangle::new((0, 1).into(), (1, 1).into()),
            Rectangle::new((398, 199).into(), (2, 1).into()),
        ];
        let bands = bands(&inputs);
        let pixel = |x: i32, y: i32| {
            let at = Point::<i32, Logical>::from((x.div_euclid(2), y.div_euclid(2)));
            let band = bands.iter().find(|band| band.rect.contains(at))?;
            let (x, y) = (x - band.rect.loc.x * 2, y - band.rect.loc.y * 2);
            Some(band.pixels[((y * band.size.w + x) * 4 + 3) as usize])
        };

        assert!(pixel(2, 0).unwrap() > 0, "cut out of the top left");
        assert_eq!(pixel(3, 0), Some(0), "the frame's own pixel beside it");
        assert!(pixel(0, 1).unwrap() > 0);
        assert_eq!(pixel(1, 1), Some(0));
        assert!(pixel(399, 199).unwrap() > 0, "cut out of the bottom right");
        assert_eq!(pixel(397, 199), Some(0));
        assert_eq!(pixel(200, 100), None, "no band holds the frame's middle");
    }

    #[test]
    fn a_frame_shorter_than_its_corners_still_has_a_whole_shadow() {
        let mut inputs = inputs(ShadowType::Medium, (200, 3), 1);
        inputs.cutouts = (0..5)
            .map(|row| Rectangle::new((0, row).into(), (5 - row, 1).into()))
            .collect();
        let bands = bands(&inputs);
        let whole = ShadowType::Medium.rect(Rectangle::from_size((200, 3).into()));

        let area: i32 = bands.iter().map(|b| b.rect.size.w * b.rect.size.h).sum();
        assert_eq!(area, whole.size.w * whole.size.h);
    }

    #[test]
    fn a_frame_with_no_size_has_no_shadow() {
        assert_eq!(bands(&inputs(ShadowType::Large, (0, 100), 1)), Vec::new());
    }
}
