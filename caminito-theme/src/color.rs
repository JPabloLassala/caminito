//! Colour specs, parsed as Marco's `meta_color_spec_new_from_string` does (`theme.c:1389`).
//!
//! A spec is kept as a tree, and [`ColorSpec::render`] evaluates it against a variant of
//! the [`Palette`] as `meta_color_spec_render` does against GTK (`theme.c:1823`).
//!
//! [`Palette`]: crate::Palette

use crate::color_names::COLOR_NAMES;
use crate::number::strtod_prefix;
use crate::palette::PaletteColors;

/// A colour with channels from 0 to 1, as GDK's `GdkRGBA`.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct Rgba {
    pub red: f64,
    pub green: f64,
    pub blue: f64,
    pub alpha: f64,
}

impl Rgba {
    /// An opaque colour from 8-bit channels.
    pub fn from_rgb8(red: u8, green: u8, blue: u8) -> Rgba {
        Rgba {
            red: f64::from(red) / 255.0,
            green: f64::from(green) / 255.0,
            blue: f64::from(blue) / 255.0,
            alpha: 1.0,
        }
    }

    /// GTK3's `gdk_rgba_parse`: `rgb()` and `rgba()`, or Pango's `#rgb` to
    /// `#rrrrggggbbbb` and X11 colour names.
    pub fn parse(text: &str) -> Option<Rgba> {
        if let Some(rest) = text.strip_prefix("rgba") {
            parse_css(rest, true)
        } else if let Some(rest) = text.strip_prefix("rgb") {
            parse_css(rest, false)
        } else if let Some(hex) = text.strip_prefix('#') {
            parse_hex(hex)
        } else {
            parse_name(text)
        }
    }
}

fn parse_hex(hex: &str) -> Option<Rgba> {
    if !matches!(hex.len(), 3 | 6 | 9 | 12) || !hex.bytes().all(|c| c.is_ascii_hexdigit()) {
        return None;
    }
    let len = hex.len() / 3;
    let channel = |i: usize| {
        let value = u32::from_str_radix(&hex[i * len..(i + 1) * len], 16).unwrap_or(0);
        // Pango repeats the digits to fill 16 bits.
        let mut bits = len * 4;
        let mut value = value << (16 - bits);
        while bits < 16 {
            value |= value >> bits;
            bits *= 2;
        }
        f64::from(value) / 65535.0
    };
    Some(Rgba {
        red: channel(0),
        green: channel(1),
        blue: channel(2),
        alpha: 1.0,
    })
}

/// Pango's `find_color`, which ignores case and any spaces but trailing ones.
fn parse_name(name: &str) -> Option<Rgba> {
    if name.ends_with(' ') {
        return None;
    }
    let key: String = name
        .chars()
        .filter(|&c| c != ' ')
        .map(|c| c.to_ascii_lowercase())
        .collect();
    let index = COLOR_NAMES
        .binary_search_by(|(name, ..)| (*name).cmp(key.as_str()))
        .ok()?;
    let (_, red, green, blue) = COLOR_NAMES[index];
    Some(Rgba::from_rgb8(red, green, blue))
}

/// The rest of `rgb(r, g, b)` or `rgba(r, g, b, a)`, as GTK3's `gdk_rgba_parse`.
fn parse_css(rest: &str, has_alpha: bool) -> Option<Rgba> {
    fn skip_spaces(text: &str) -> &str {
        text.trim_start_matches(|c: char| c.is_ascii_whitespace())
    }
    fn number(text: &str) -> Option<(f64, &str)> {
        let (value, len) = strtod_prefix(text)?;
        value.is_finite().then_some((value, &text[len..]))
    }
    fn channel(text: &str) -> Option<(f64, &str)> {
        let (value, rest) = number(text)?;
        let after = skip_spaces(rest);
        Some(match after.strip_prefix('%') {
            Some(rest) => ((value / 100.0).clamp(0.0, 1.0), rest),
            None => ((value / 255.0).clamp(0.0, 1.0), rest),
        })
    }
    fn expect(text: &str, c: char) -> Option<&str> {
        skip_spaces(text).strip_prefix(c)
    }

    let text = expect(rest, '(')?;
    let (red, text) = channel(skip_spaces(text))?;
    let text = expect(text, ',')?;
    let (green, text) = channel(skip_spaces(text))?;
    let text = expect(text, ',')?;
    let (blue, mut text) = channel(skip_spaces(text))?;
    let mut alpha = 1.0;
    if has_alpha {
        let rest = expect(text, ',')?;
        let (value, rest) = number(skip_spaces(rest))?;
        alpha = value;
        text = rest;
    }
    let text = expect(text, ')')?;
    if !skip_spaces(text).is_empty() {
        return None;
    }
    Some(Rgba {
        red,
        green,
        blue,
        alpha: alpha.clamp(0.0, 1.0),
    })
}

/// The GTK state flags Marco maps colours and the `gtk_*` draw ops to (`theme.c:6743`).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum GtkState {
    Normal,
    Prelight,
    Active,
    Selected,
    Insensitive,
    Inconsistent,
    Focused,
    Backdrop,
}

impl GtkState {
    pub const ALL: [GtkState; 8] = [
        GtkState::Normal,
        GtkState::Prelight,
        GtkState::Active,
        GtkState::Selected,
        GtkState::Insensitive,
        GtkState::Inconsistent,
        GtkState::Focused,
        GtkState::Backdrop,
    ];

    /// The state's name, which Marco matches ignoring case.
    pub fn name(self) -> &'static str {
        match self {
            GtkState::Normal => "NORMAL",
            GtkState::Prelight => "PRELIGHT",
            GtkState::Active => "ACTIVE",
            GtkState::Selected => "SELECTED",
            GtkState::Insensitive => "INSENSITIVE",
            GtkState::Inconsistent => "INCONSISTENT",
            GtkState::Focused => "FOCUSED",
            GtkState::Backdrop => "BACKDROP",
        }
    }

    pub fn from_name(name: &str) -> Option<GtkState> {
        GtkState::ALL
            .into_iter()
            .find(|state| state.name().eq_ignore_ascii_case(name))
    }
}

/// The part of the GTK style a `gtk:` colour reads.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum ColorComponent {
    Fg,
    Bg,
    Light,
    Dark,
    Mid,
    Text,
    Base,
    TextAa,
}

impl ColorComponent {
    fn from_name(name: &str) -> Option<ColorComponent> {
        Some(match name {
            "fg" => ColorComponent::Fg,
            "bg" => ColorComponent::Bg,
            "light" => ColorComponent::Light,
            "dark" => ColorComponent::Dark,
            "mid" => ColorComponent::Mid,
            "text" => ColorComponent::Text,
            "base" => ColorComponent::Base,
            "text_aa" => ColorComponent::TextAa,
            _ => return None,
        })
    }
}

/// A colour as a theme writes it.
#[derive(Debug, Clone, PartialEq)]
pub enum ColorSpec {
    Basic(Rgba),
    /// `gtk:<component>[<state>]`, from the palette.
    Gtk {
        component: ColorComponent,
        state: GtkState,
    },
    /// `gtk:custom(name,fallback)`, the GTK theme's `@define-color name`.
    GtkCustom {
        name: String,
        fallback: Box<ColorSpec>,
    },
    /// `blend/background/foreground/alpha`.
    Blend {
        background: Box<ColorSpec>,
        foreground: Box<ColorSpec>,
        alpha: f64,
    },
    /// `shade/base/factor`.
    Shade {
        base: Box<ColorSpec>,
        factor: f64,
    },
}

impl ColorSpec {
    /// Adds the names of the `gtk:custom` colours in the spec, fallbacks included.
    pub(crate) fn custom_names(&self, names: &mut std::collections::BTreeSet<String>) {
        match self {
            ColorSpec::Basic(_) | ColorSpec::Gtk { .. } => {}
            ColorSpec::GtkCustom { name, fallback } => {
                names.insert(name.clone());
                fallback.custom_names(names);
            }
            ColorSpec::Blend {
                background,
                foreground,
                ..
            } => {
                background.custom_names(names);
                foreground.custom_names(names);
            }
            ColorSpec::Shade { base, .. } => base.custom_names(names),
        }
    }

    /// Parses a spec, with Marco's error message if it's malformed. Colour constants
    /// aren't looked up here: Marco resolves them only for a whole attribute value.
    pub fn parse(text: &str) -> Result<ColorSpec, String> {
        if let Some(rest) = text.strip_prefix("gtk:custom") {
            parse_gtk_custom(text, rest)
        } else if let Some(rest) = text.strip_prefix("gtk:") {
            parse_gtk(text, rest)
        } else if text.starts_with("blend/") {
            let parts: Vec<&str> = text.splitn(4, '/').collect();
            let [_, background, foreground, alpha] = parts[..] else {
                return Err(format!(
                    "Blend format is \"blend/bg_color/fg_color/alpha\", \"{text}\" does not fit \
                     the format"
                ));
            };
            let Some((alpha_value, _)) = strtod_prefix(alpha) else {
                return Err(format!(
                    "Could not parse alpha value \"{alpha}\" in blended color"
                ));
            };
            // Marco's bounds are written 1e6 where 1e-6 was meant (`theme.c:1596`).
            if alpha_value < -1e6 || alpha_value > 1.0 + 1e6 {
                return Err(format!(
                    "Alpha value \"{alpha}\" in blended color is not between 0.0 and 1.0"
                ));
            }
            Ok(ColorSpec::Blend {
                background: Box::new(ColorSpec::parse(background)?),
                foreground: Box::new(ColorSpec::parse(foreground)?),
                alpha: alpha_value,
            })
        } else if text.starts_with("shade/") {
            let parts: Vec<&str> = text.splitn(3, '/').collect();
            let [_, base, factor] = parts[..] else {
                return Err(format!(
                    "Shade format is \"shade/base_color/factor\", \"{text}\" does not fit the \
                     format"
                ));
            };
            let Some((factor_value, _)) = strtod_prefix(factor) else {
                return Err(format!(
                    "Could not parse shade factor \"{factor}\" in shaded color"
                ));
            };
            if factor_value < -1e6 {
                return Err(format!(
                    "Shade factor \"{factor}\" in shaded color is negative"
                ));
            }
            Ok(ColorSpec::Shade {
                base: Box::new(ColorSpec::parse(base)?),
                factor: factor_value,
            })
        } else {
            Rgba::parse(text)
                .map(ColorSpec::Basic)
                .ok_or_else(|| format!("Could not parse color \"{text}\""))
        }
    }
}

impl ColorSpec {
    /// The colour the spec stands for (`meta_color_spec_render`). The palette holds
    /// what Marco's style context gives for `bg` and `fg`, a transparent background
    /// already replaced by Marco's fallback.
    pub fn render(&self, colors: &PaletteColors) -> Rgba {
        match self {
            ColorSpec::Basic(color) => *color,
            ColorSpec::Gtk { component, state } => {
                let background = colors.background(*state);
                let text = colors.text(*state);
                match component {
                    ColorComponent::Bg | ColorComponent::Base => background,
                    ColorComponent::Fg | ColorComponent::Text => text,
                    ColorComponent::TextAa => halfway(text, background),
                    ColorComponent::Light => shade(background, LIGHTNESS_MULT),
                    ColorComponent::Dark => shade(background, DARKNESS_MULT),
                    ColorComponent::Mid => halfway(
                        shade(background, LIGHTNESS_MULT),
                        shade(background, DARKNESS_MULT),
                    ),
                }
            }
            ColorSpec::GtkCustom { name, fallback } => colors
                .named(name)
                .unwrap_or_else(|| fallback.render(colors)),
            ColorSpec::Blend {
                background,
                foreground,
                alpha,
            } => {
                // color_composite: the background's alpha is kept.
                let bg = background.render(colors);
                let fg = foreground.render(colors);
                Rgba {
                    red: bg.red + (fg.red - bg.red) * alpha,
                    green: bg.green + (fg.green - bg.green) * alpha,
                    blue: bg.blue + (fg.blue - bg.blue) * alpha,
                    alpha: bg.alpha,
                }
            }
            ColorSpec::Shade { base, factor } => shade(base.render(colors), *factor),
        }
    }
}

/// GTK2's `set_color`, which Marco's `light` and `dark` copy.
const LIGHTNESS_MULT: f64 = 1.3;
const DARKNESS_MULT: f64 = 0.7;

/// The average of two colours' channels, with the first's alpha.
fn halfway(a: Rgba, b: Rgba) -> Rgba {
    Rgba {
        red: (a.red + b.red) / 2.0,
        green: (a.green + b.green) / 2.0,
        blue: (a.blue + b.blue) / 2.0,
        alpha: a.alpha,
    }
}

/// GTK2's `gtk_style_shade`, as Marco copies it: lightness and saturation both scaled
/// by `k` and clamped, alpha kept.
fn shade(color: Rgba, k: f64) -> Rgba {
    let (hue, lightness, saturation) = rgb_to_hls(color.red, color.green, color.blue);
    let (red, green, blue) = hls_to_rgb(
        hue,
        (lightness * k).clamp(0.0, 1.0),
        (saturation * k).clamp(0.0, 1.0),
    );
    Rgba {
        red,
        green,
        blue,
        alpha: color.alpha,
    }
}

/// Marco's `rgb_to_hls`: hue in degrees, lightness and saturation from 0 to 1.
fn rgb_to_hls(red: f64, green: f64, blue: f64) -> (f64, f64, f64) {
    let (max, min) = if red > green {
        (
            if red > blue { red } else { blue },
            if green < blue { green } else { blue },
        )
    } else {
        (
            if green > blue { green } else { blue },
            if red < blue { red } else { blue },
        )
    };
    let lightness = (max + min) / 2.0;
    if max == min {
        return (0.0, lightness, 0.0);
    }
    let saturation = if lightness <= 0.5 {
        (max - min) / (max + min)
    } else {
        (max - min) / (2.0 - max - min)
    };
    let delta = max - min;
    let mut hue = if red == max {
        (green - blue) / delta
    } else if green == max {
        2.0 + (blue - red) / delta
    } else {
        4.0 + (red - green) / delta
    };
    hue *= 60.0;
    if hue < 0.0 {
        hue += 360.0;
    }
    (hue, lightness, saturation)
}

/// Marco's `hls_to_rgb`.
fn hls_to_rgb(hue: f64, lightness: f64, saturation: f64) -> (f64, f64, f64) {
    let m2 = if lightness <= 0.5 {
        lightness * (1.0 + saturation)
    } else {
        lightness + saturation - lightness * saturation
    };
    let m1 = 2.0 * lightness - m2;
    if saturation == 0.0 {
        return (lightness, lightness, lightness);
    }
    let channel = |mut hue: f64| {
        while hue > 360.0 {
            hue -= 360.0;
        }
        while hue < 0.0 {
            hue += 360.0;
        }
        if hue < 60.0 {
            m1 + (m2 - m1) * hue / 60.0
        } else if hue < 180.0 {
            m2
        } else if hue < 240.0 {
            m1 + (m2 - m1) * (240.0 - hue) / 60.0
        } else {
            m1
        }
    };
    (channel(hue + 120.0), channel(hue), channel(hue - 120.0))
}

fn parse_gtk_custom(text: &str, rest: &str) -> Result<ColorSpec, String> {
    let Some(args) = rest.strip_prefix('(') else {
        return Err(format!(
            "GTK custom color specification must have color name and fallback in \
             parentheses, e.g. gtk:custom(foo,bar); could not parse \"{text}\""
        ));
    };
    for c in args.chars().take_while(|&c| c != ',') {
        if !(c.is_ascii_alphanumeric() || c == '-' || c == '_') {
            return Err(format!(
                "Invalid character '{c}' in color_name parameter of gtk:custom, only \
                 A-Za-z0-9-_ are valid"
            ));
        }
    }
    let not_the_format = || {
        format!(
            "Gtk:custom format is \"gtk:custom(color_name,fallback)\", \"{text}\" does not fit \
             the format"
        )
    };
    // Marco reads past the end of the string when there's no comma.
    let comma = args.find(',').ok_or_else(not_the_format)?;
    let end = args
        .rfind(')')
        .filter(|&end| end > comma)
        .ok_or_else(not_the_format)?;
    Ok(ColorSpec::GtkCustom {
        name: args[..comma].to_owned(),
        fallback: Box::new(ColorSpec::parse(&args[comma + 1..end])?),
    })
}

fn parse_gtk(text: &str, rest: &str) -> Result<ColorSpec, String> {
    let Some(open) = rest.find('[') else {
        return Err(format!(
            "GTK color specification must have the state in brackets, e.g. gtk:fg[NORMAL] \
             where NORMAL is the state; could not parse \"{text}\""
        ));
    };
    let Some(close) = rest[open..].find(']').map(|close| open + close) else {
        return Err(format!(
            "GTK color specification must have a close bracket after the state, e.g. \
             gtk:fg[NORMAL] where NORMAL is the state; could not parse \"{text}\""
        ));
    };
    let state_name = &rest[open + 1..close];
    let state = GtkState::from_name(state_name).ok_or_else(|| {
        format!("Did not understand state \"{state_name}\" in color specification")
    })?;
    let component_name = &rest[..open];
    let component = ColorComponent::from_name(component_name).ok_or_else(|| {
        format!("Did not understand color component \"{component_name}\" in color specification")
    })?;
    Ok(ColorSpec::Gtk { component, state })
}

#[cfg(test)]
mod tests {
    use super::*;

    fn rgb(red: u8, green: u8, blue: u8) -> ColorSpec {
        ColorSpec::Basic(Rgba::from_rgb8(red, green, blue))
    }

    fn parse(text: &str) -> ColorSpec {
        ColorSpec::parse(text).unwrap_or_else(|e| panic!("{text:?}: {e}"))
    }

    fn error(text: &str) -> String {
        ColorSpec::parse(text).expect_err(text)
    }

    #[test]
    fn hex_colours_take_one_to_four_digits_per_channel() {
        assert_eq!(parse("#ff8000"), rgb(255, 128, 0));
        assert_eq!(parse("#F80"), rgb(255, 136, 0));
        assert_eq!(parse("#fff000000"), rgb(255, 0, 0));
        assert_eq!(parse("#ffff80800000"), rgb(255, 128, 0));
    }

    #[test]
    fn x11_colour_names_ignore_case_and_spaces() {
        assert_eq!(parse("white"), rgb(255, 255, 255));
        assert_eq!(parse("LightSteelBlue"), rgb(176, 196, 222));
        assert_eq!(parse("light steel blue"), rgb(176, 196, 222));
        assert_eq!(parse("gray50"), rgb(127, 127, 127));
    }

    #[test]
    fn css_rgb_and_rgba_are_accepted() {
        assert_eq!(parse("rgb(255, 128, 0)"), rgb(255, 128, 0));
        assert_eq!(parse("rgb(100%,0%, 0)"), rgb(255, 0, 0));
        assert_eq!(
            parse("rgba(0,0,0,0.5)"),
            ColorSpec::Basic(Rgba {
                red: 0.0,
                green: 0.0,
                blue: 0.0,
                alpha: 0.5
            })
        );
    }

    #[test]
    fn gtk_colours_name_a_component_and_a_state_in_any_case() {
        assert_eq!(
            parse("gtk:bg[NORMAL]"),
            ColorSpec::Gtk {
                component: ColorComponent::Bg,
                state: GtkState::Normal
            }
        );
        assert_eq!(
            parse("gtk:text_aa[selected]"),
            ColorSpec::Gtk {
                component: ColorComponent::TextAa,
                state: GtkState::Selected
            }
        );
    }

    #[test]
    fn gtk_custom_names_a_define_color_with_a_fallback() {
        assert_eq!(
            parse("gtk:custom(wm_bg,#d9d9d9)"),
            ColorSpec::GtkCustom {
                name: "wm_bg".to_owned(),
                fallback: Box::new(rgb(0xd9, 0xd9, 0xd9)),
            }
        );
        assert_eq!(
            parse("gtk:custom(wm-title_2,gtk:fg[NORMAL])"),
            ColorSpec::GtkCustom {
                name: "wm-title_2".to_owned(),
                fallback: Box::new(parse("gtk:fg[NORMAL]")),
            }
        );
    }

    #[test]
    fn blend_and_shade_nest_other_specs() {
        assert_eq!(
            parse("blend/#000000/gtk:bg[NORMAL]/0.5"),
            ColorSpec::Blend {
                background: Box::new(rgb(0, 0, 0)),
                foreground: Box::new(parse("gtk:bg[NORMAL]")),
                alpha: 0.5,
            }
        );
        assert_eq!(
            parse("shade/gtk:custom(selected_bg_color,#35a854)/0.8"),
            ColorSpec::Shade {
                base: Box::new(parse("gtk:custom(selected_bg_color,#35a854)")),
                factor: 0.8,
            }
        );
    }

    #[test]
    fn numbers_in_blend_and_shade_are_read_as_far_as_they_go() {
        // g_ascii_strtod stops at the first character it can't use, and Marco's range
        // checks allow anything within a million.
        let ColorSpec::Blend { alpha, .. } = parse("blend/white/black/0.25/extra") else {
            panic!()
        };
        assert_eq!(alpha, 0.25);
        let ColorSpec::Shade { factor, .. } = parse("shade/white/-3") else {
            panic!()
        };
        assert_eq!(factor, -3.0);
    }

    #[test]
    fn numbers_are_read_as_g_ascii_strtod_reads_them() {
        let alpha = |text: &str| match parse(&format!("blend/white/black/{text}")) {
            ColorSpec::Blend { alpha, .. } => alpha,
            _ => unreachable!(),
        };
        assert_eq!(alpha("0x1p-1"), 0.5);
        assert_eq!(alpha("5e-1"), 0.5);
        assert!(alpha("nan").is_nan());
        assert!(error("blend/white/black/inf").contains("is not between"));
    }

    mod rendering {
        use std::collections::BTreeMap;

        use super::*;
        use crate::palette::{PaletteColors, StateColors};

        fn rgba(red: f64, green: f64, blue: f64, alpha: f64) -> Rgba {
            Rgba {
                red,
                green,
                blue,
                alpha,
            }
        }

        fn grey(level: f64) -> Rgba {
            rgba(level, level, level, 1.0)
        }

        /// Backgrounds a grey of the state's index over 10, text the inverse, and one
        /// named colour.
        fn colors() -> PaletteColors {
            let states = GtkState::ALL.map(|state| StateColors {
                background: grey(state as usize as f64 / 10.0),
                text: rgba(1.0, 0.8, 0.6, 0.5),
            });
            let named = BTreeMap::from([("wm_bg".to_owned(), rgba(0.1, 0.2, 0.3, 1.0))]);
            PaletteColors::new(states, named)
        }

        fn render(text: &str) -> Rgba {
            parse(text).render(&colors())
        }

        fn assert_close(actual: Rgba, expected: Rgba) {
            let channels = |c: Rgba| [c.red, c.green, c.blue, c.alpha];
            for (a, e) in channels(actual).into_iter().zip(channels(expected)) {
                assert!((a - e).abs() < 1e-12, "{actual:?} is not {expected:?}");
            }
        }

        #[test]
        fn basic_colours_are_themselves() {
            assert_eq!(render("#ff8000"), Rgba::from_rgb8(255, 128, 0));
        }

        #[test]
        fn bg_and_base_read_the_background_and_fg_and_text_the_text() {
            assert_eq!(render("gtk:bg[SELECTED]"), grey(0.3));
            assert_eq!(render("gtk:base[PRELIGHT]"), grey(0.1));
            assert_eq!(render("gtk:fg[NORMAL]"), rgba(1.0, 0.8, 0.6, 0.5));
            assert_eq!(render("gtk:text[BACKDROP]"), rgba(1.0, 0.8, 0.6, 0.5));
        }

        #[test]
        fn light_dark_and_mid_shade_the_background() {
            // GTK2's set_color: light is the background shaded by 1.3, dark by 0.7, and
            // mid halfway between them.
            assert_close(render("gtk:light[INSENSITIVE]"), grey(0.4 * 1.3));
            assert_close(render("gtk:dark[INSENSITIVE]"), grey(0.4 * 0.7));
            assert_close(render("gtk:mid[INSENSITIVE]"), grey(0.4));
        }

        #[test]
        fn text_aa_is_halfway_between_text_and_base_with_the_texts_alpha() {
            assert_close(
                render("gtk:text_aa[ACTIVE]"),
                rgba((1.0 + 0.2) / 2.0, (0.8 + 0.2) / 2.0, (0.6 + 0.2) / 2.0, 0.5),
            );
        }

        #[test]
        fn custom_colours_come_from_the_named_table_or_the_fallback() {
            assert_eq!(
                render("gtk:custom(wm_bg,#ffffff)"),
                rgba(0.1, 0.2, 0.3, 1.0)
            );
            assert_eq!(render("gtk:custom(wm_title,gtk:bg[SELECTED])"), grey(0.3));
        }

        #[test]
        fn blend_mixes_the_colours_and_keeps_the_backgrounds_alpha() {
            assert_close(
                render("blend/gtk:fg[NORMAL]/#000000/0.25"),
                rgba(0.75, 0.6, 0.45, 0.5),
            );
        }

        #[test]
        fn shade_scales_lightness_and_saturation_and_keeps_alpha() {
            // Red is lightness 0.5 and saturation 1; both halve.
            assert_close(render("shade/#ff0000/0.5"), rgba(0.375, 0.125, 0.125, 1.0));
            // Both are clamped to 1.
            assert_close(render("shade/#ff0000/3"), grey(1.0));
            assert_close(render("shade/gtk:fg[NORMAL]/1"), rgba(1.0, 0.8, 0.6, 0.5));
        }
    }

    #[test]
    fn malformed_specs_are_rejected() {
        assert!(error("nonsense").contains("Could not parse color"));
        assert!(error("#12345").contains("Could not parse color"));
        assert!(error("gtk:bg").contains("state in brackets"));
        assert!(error("gtk:bg[NORMAL").contains("close bracket"));
        assert!(error("gtk:bg[SHINY]").contains("state \"SHINY\""));
        assert!(error("gtk:purple[NORMAL]").contains("component \"purple\""));
        assert!(error("gtk:custom").contains("parentheses"));
        assert!(error("gtk:custom(a b,#fff)").contains("Invalid character ' '"));
        assert!(error("gtk:custom(wm_bg)").contains("Invalid character ')'"));
        assert!(error("gtk:custom(wm_bg").contains("does not fit the format"));
        assert!(error("gtk:custom(wm_bg,#fff").contains("does not fit the format"));
        assert!(error("gtk:custom(wm_bg,nonsense)").contains("Could not parse color"));
        assert!(error("blend/white/black").contains("does not fit the format"));
        assert!(error("blend/white/black/x").contains("alpha value \"x\""));
        assert!(error("shade/white").contains("does not fit the format"));
        assert!(error("shade/white/x").contains("shade factor \"x\""));
        assert!(error("shade/nonsense/1").contains("Could not parse color"));
        // Marco's bounds are a million out, but they do bound.
        assert!(error("blend/white/black/2000000").contains("is not between 0.0 and 1.0"));
        assert!(error("shade/white/-2000000").contains("is negative"));
    }
}
