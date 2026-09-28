//! Colour specs, parsed as Marco's `meta_color_spec_new_from_string` does (`theme.c:1389`).
//!
//! A spec is kept as a tree. Resolving `gtk:` colours against a [`Palette`] is the drawing
//! ticket's job (27).
//!
//! [`Palette`]: crate::Palette

use crate::color_names::COLOR_NAMES;
use crate::number::strtod_prefix;

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
