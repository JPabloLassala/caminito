//! Where a picture goes on an output, for each positioning (210): Fill, Fit, Stretch,
//! Centre and Tile. Everything is in the output's pixels, its mode at its scale, so a
//! picture at its own size (Centre and Tile) is pixel for pixel.

/// How a picture is fitted to an output. The output's colour shows wherever the picture
/// doesn't: the bars Fit and Centre leave.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum Positioning {
    /// Scaled to cover the output, keeping its shape, and cropped to it, centred.
    #[default]
    Fill,
    /// Scaled to fit inside the output, keeping its shape, centred.
    Fit,
    /// Scaled to the output's size, whatever its shape.
    Stretch,
    /// At its own size, centred, and cropped if larger.
    Centre,
    /// At its own size, repeated from the top left corner.
    Tile,
}

impl Positioning {
    /// Each positioning, in the schema's order.
    pub const ALL: [Self; 5] = [
        Self::Fill,
        Self::Fit,
        Self::Stretch,
        Self::Centre,
        Self::Tile,
    ];

    /// Its nick in the schema's `org.mate.Caminito.WallpaperPositioning`.
    pub fn nick(self) -> &'static str {
        match self {
            Self::Fill => "fill",
            Self::Fit => "fit",
            Self::Stretch => "stretch",
            Self::Centre => "centre",
            Self::Tile => "tile",
        }
    }

    /// The positioning of a nick, or Fill for one the schema doesn't have.
    pub fn from_nick(nick: &str) -> Self {
        Self::ALL
            .into_iter()
            .find(|positioning| positioning.nick() == nick)
            .unwrap_or_default()
    }

    /// The positioning closest to `org.mate.background`'s `picture-options`. Spanned, one
    /// picture across every output, isn't a positioning (spec), so it's Fill on each.
    pub fn from_mate(options: &str) -> Self {
        match options {
            "wallpaper" => Self::Tile,
            "centered" => Self::Centre,
            "scaled" => Self::Fit,
            "stretched" => Self::Stretch,
            _ => Self::Fill,
        }
    }

    /// The `picture-options` of `org.mate.background` that says this positioning (211).
    pub fn mate(self) -> &'static str {
        match self {
            Self::Fill => "zoom",
            Self::Fit => "scaled",
            Self::Stretch => "stretched",
            Self::Centre => "centered",
            Self::Tile => "wallpaper",
        }
    }
}

/// A rectangle in an output's pixels.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Rect {
    pub x: i32,
    pub y: i32,
    pub width: i32,
    pub height: i32,
}

/// Where a picture of `picture` pixels goes on an output of `output` pixels: its one copy,
/// or for Tile the first, at the top left, which the rest repeat.
pub fn place(picture: (i32, i32), output: (i32, i32), positioning: Positioning) -> Rect {
    let (pw, ph) = (f64::from(picture.0.max(1)), f64::from(picture.1.max(1)));
    let (ow, oh) = (f64::from(output.0), f64::from(output.1));
    let (width, height) = match positioning {
        Positioning::Fill => scaled(pw, ph, (ow / pw).max(oh / ph)),
        Positioning::Fit => scaled(pw, ph, (ow / pw).min(oh / ph)),
        Positioning::Stretch => (output.0, output.1),
        Positioning::Centre | Positioning::Tile => (picture.0, picture.1),
    };
    if positioning == Positioning::Tile {
        return Rect {
            x: 0,
            y: 0,
            width,
            height,
        };
    }
    Rect {
        x: centred(output.0, width),
        y: centred(output.1, height),
        width,
        height,
    }
}

/// A picture's size scaled by `factor`, in whole pixels.
fn scaled(width: f64, height: f64, factor: f64) -> (i32, i32) {
    (
        (width * factor).round() as i32,
        (height * factor).round() as i32,
    )
}

/// Where something `size` long starts to be centred in `space`, in whole pixels.
fn centred(space: i32, size: i32) -> i32 {
    (space - size).div_euclid(2)
}

#[cfg(test)]
mod tests {
    use super::*;

    /// An output of 1920 by 1080 pixels.
    const OUTPUT: (i32, i32) = (1920, 1080);

    fn rect(x: i32, y: i32, width: i32, height: i32) -> Rect {
        Rect {
            x,
            y,
            width,
            height,
        }
    }

    #[test]
    fn fill_covers_the_output_and_crops_the_rest_evenly() {
        // A square picture is cropped top and bottom, a tall one too, a wide one at the sides.
        assert_eq!(
            place((1000, 1000), OUTPUT, Positioning::Fill),
            rect(0, -420, 1920, 1920)
        );
        assert_eq!(
            place((3000, 1000), OUTPUT, Positioning::Fill),
            rect(-660, 0, 3240, 1080)
        );
        // A picture of the output's shape is the output, at any size.
        assert_eq!(
            place((3840, 2160), OUTPUT, Positioning::Fill),
            rect(0, 0, 1920, 1080)
        );
    }

    #[test]
    fn fit_shows_the_whole_picture_with_bars_on_the_short_sides() {
        assert_eq!(
            place((1000, 1000), OUTPUT, Positioning::Fit),
            rect(420, 0, 1080, 1080)
        );
        assert_eq!(
            place((3840, 1080), OUTPUT, Positioning::Fit),
            rect(0, 270, 1920, 540)
        );
    }

    #[test]
    fn stretch_is_the_output_whatever_the_pictures_shape() {
        assert_eq!(
            place((640, 480), OUTPUT, Positioning::Stretch),
            rect(0, 0, 1920, 1080)
        );
    }

    #[test]
    fn centre_keeps_the_pictures_own_size_in_the_middle() {
        assert_eq!(
            place((640, 480), OUTPUT, Positioning::Centre),
            rect(640, 300, 640, 480)
        );
        // Larger than the output, it's cropped evenly, and an odd pixel more at the top
        // left.
        assert_eq!(
            place((2001, 1081), OUTPUT, Positioning::Centre),
            rect(-41, -1, 2001, 1081)
        );
    }

    #[test]
    fn tile_starts_at_the_top_left_at_the_pictures_own_size() {
        assert_eq!(
            place((300, 200), OUTPUT, Positioning::Tile),
            rect(0, 0, 300, 200)
        );
    }

    #[test]
    fn an_output_at_another_scale_has_its_own_pixels() {
        // The LG at 1.7 is 3840 by 2160 pixels, whatever its logical size.
        assert_eq!(
            place((1920, 1080), (3840, 2160), Positioning::Fill),
            rect(0, 0, 3840, 2160)
        );
        assert_eq!(
            place((1920, 1080), (3840, 2160), Positioning::Centre),
            rect(960, 540, 1920, 1080)
        );
    }

    #[test]
    fn mates_picture_options_map_to_the_nearest_positioning() {
        let options = [
            "wallpaper",
            "zoom",
            "centered",
            "scaled",
            "stretched",
            "spanned",
        ];
        let positionings = options.map(Positioning::from_mate);

        assert_eq!(
            positionings,
            [
                Positioning::Tile,
                Positioning::Fill,
                Positioning::Centre,
                Positioning::Fit,
                Positioning::Stretch,
                Positioning::Fill,
            ]
        );
    }

    #[test]
    fn each_positioning_is_written_to_mate_as_the_options_it_is_read_from() {
        for positioning in Positioning::ALL {
            assert_eq!(Positioning::from_mate(positioning.mate()), positioning);
        }
    }

    #[test]
    fn each_positioning_is_its_nick() {
        for positioning in Positioning::ALL {
            assert_eq!(Positioning::from_nick(positioning.nick()), positioning);
        }
        assert_eq!(Positioning::from_nick("spanned"), Positioning::Fill);
    }
}
