//! The palette helper's stdout (28).
//!
//! It's line-based text. The first line is the version header, `camino-gtk-palette 1`.
//! Each palette the helper reads follows as a block:
//!
//! ```text
//! palette <GTK theme>
//! variant default
//! state <state> <background r g b a> <text r g b a>   (all eight states, once each)
//! named <name> <r g b a>                              (each name GTK defines)
//! variant dark
//! ...
//! end
//! ```
//!
//! Camino writes the helper the `gtk:custom` names to look up, since GTK can't list a
//! theme's `@define-color` table: a line `names <name> <name> ...`, see
//! [`names_request`]. The helper reports a palette for each such line, and again whenever
//! the GTK theme changes.
//!
//! A state is a [`GtkState`](crate::GtkState) name in lower case, and a channel is a
//! decimal from 0 to 1 that reads back to the same `f64`. A name the helper was asked for
//! that the GTK theme doesn't define is left out.

use super::{ColorsBuilder, Palette, write_colors};

/// The version header this reader understands.
pub const PALETTE_STREAM_HEADER: &str = "camino-gtk-palette 1";

/// Reads the helper's stdout as it arrives.
#[derive(Debug, Default)]
pub struct PaletteReader {
    /// Bytes after the last newline.
    partial: Vec<u8>,
    header: Header,
    block: Option<Block>,
}

#[derive(Debug, Default, PartialEq)]
enum Header {
    #[default]
    Expected,
    Read,
    /// The stream isn't one this reader understands, so the rest of it is ignored.
    Refused,
}

/// A palette block being read.
#[derive(Debug)]
struct Block {
    gtk_theme: String,
    default: Option<ColorsBuilder>,
    dark: Option<ColorsBuilder>,
    /// The variant `state` and `named` lines go to.
    current: Option<crate::Variant>,
    /// A block with an error is skipped up to its `end`.
    failed: bool,
}

/// A palette the helper reported, for the GTK theme it read it from.
#[derive(Debug, Clone, PartialEq)]
pub struct ReportedPalette {
    pub gtk_theme: String,
    pub palette: Palette,
}

impl PaletteReader {
    pub fn new() -> Self {
        PaletteReader::default()
    }

    /// Takes the next bytes of the stream, and returns what the lines they complete
    /// report. A palette with an error is dropped whole, and the reader carries on at the
    /// next one.
    pub fn feed(&mut self, bytes: &[u8]) -> Vec<Result<ReportedPalette, PaletteStreamError>> {
        let mut results = Vec::new();
        let mut rest = bytes;
        while let Some(newline) = rest.iter().position(|&b| b == b'\n') {
            self.partial.extend_from_slice(&rest[..newline]);
            rest = &rest[newline + 1..];
            let line = std::mem::take(&mut self.partial);
            if let Some(result) = self.line(&line) {
                results.push(result);
            }
        }
        self.partial.extend_from_slice(rest);
        results
    }

    /// The stream ended. A line without its newline, or a palette without its `end`, was
    /// cut short.
    pub fn finish(&mut self) -> Result<(), PaletteStreamError> {
        let partial = std::mem::take(&mut self.partial);
        let block = self.block.take();
        if self.header == Header::Refused {
            return Ok(());
        }
        if !partial.is_empty() {
            return Err(PaletteStreamError::Truncated {
                line: String::from_utf8_lossy(&partial).into_owned(),
            });
        }
        match block {
            Some(block) if !block.failed => Err(PaletteStreamError::Unfinished {
                gtk_theme: block.gtk_theme,
            }),
            _ => Ok(()),
        }
    }

    fn line(&mut self, line: &[u8]) -> Option<Result<ReportedPalette, PaletteStreamError>> {
        let Ok(line) = std::str::from_utf8(line) else {
            return self.fail(PaletteStreamError::Malformed {
                line: String::from_utf8_lossy(line).into_owned(),
            });
        };
        match self.header {
            Header::Refused => return None,
            Header::Expected => {
                if line == PALETTE_STREAM_HEADER {
                    self.header = Header::Read;
                    return None;
                }
                self.header = Header::Refused;
                return Some(Err(PaletteStreamError::UnknownVersion {
                    header: line.to_owned(),
                }));
            }
            Header::Read => {}
        }

        let (keyword, rest) = line.split_once(' ').unwrap_or((line, ""));
        let malformed = || PaletteStreamError::Malformed {
            line: line.to_owned(),
        };
        match (keyword, &mut self.block) {
            ("palette", block) => {
                let unfinished = block.take().filter(|block| !block.failed);
                *block = Some(Block {
                    gtk_theme: rest.to_owned(),
                    default: None,
                    dark: None,
                    current: None,
                    failed: rest.is_empty(),
                });
                match unfinished {
                    Some(unfinished) => Some(Err(PaletteStreamError::Unfinished {
                        gtk_theme: unfinished.gtk_theme,
                    })),
                    None if rest.is_empty() => Some(Err(malformed())),
                    None => None,
                }
            }
            (_, None) => Some(Err(malformed())),
            (_, Some(block)) if block.failed => {
                if keyword == "end" {
                    self.block = None;
                }
                None
            }
            ("variant", Some(block)) => {
                let (variant, slot) = match rest {
                    "default" => (crate::Variant::Default, &mut block.default),
                    "dark" => (crate::Variant::Dark, &mut block.dark),
                    _ => return self.fail(malformed()),
                };
                if slot.is_some() {
                    return self.fail(malformed());
                }
                *slot = Some(ColorsBuilder::default());
                block.current = Some(variant);
                None
            }
            ("end", Some(_)) => {
                let block = self.block.take()?;
                let (Some(default), Some(dark)) = (block.default, block.dark) else {
                    return Some(Err(malformed()));
                };
                let (Some(default), Some(dark)) = (default.build(), dark.build()) else {
                    return Some(Err(PaletteStreamError::MissingStates {
                        gtk_theme: block.gtk_theme,
                    }));
                };
                Some(Ok(ReportedPalette {
                    gtk_theme: block.gtk_theme,
                    palette: Palette { default, dark },
                }))
            }
            (_, Some(block)) => {
                let colors = match block.current {
                    Some(crate::Variant::Default) => block.default.as_mut(),
                    Some(crate::Variant::Dark) => block.dark.as_mut(),
                    None => None,
                };
                let read = colors.is_some_and(|colors| colors.line(keyword, rest));
                if read { None } else { self.fail(malformed()) }
            }
        }
    }

    /// Drops the palette being read, up to its `end`.
    fn fail(
        &mut self,
        error: PaletteStreamError,
    ) -> Option<Result<ReportedPalette, PaletteStreamError>> {
        if let Some(block) = &mut self.block {
            block.failed = true;
        }
        Some(Err(error))
    }
}

/// One palette as the helper writes it, from `palette` to `end`. The stream starts with
/// [`PALETTE_STREAM_HEADER`] before the first.
pub fn format_report(gtk_theme: &str, palette: &Palette) -> String {
    let mut out = format!("palette {gtk_theme}\n");
    for (name, colors) in [("default", &palette.default), ("dark", &palette.dark)] {
        out.push_str("variant ");
        out.push_str(name);
        out.push('\n');
        write_colors(&mut out, colors);
    }
    out.push_str("end\n");
    out
}

/// The line that asks the helper for these `gtk:custom` names, which replaces the names it
/// was asked for before.
pub fn names_request<'a>(names: impl IntoIterator<Item = &'a str>) -> String {
    let mut line = String::from("names");
    for name in names {
        line.push(' ');
        line.push_str(name);
    }
    line.push('\n');
    line
}

/// Why the helper's output was refused.
#[derive(Debug, Clone, PartialEq)]
pub enum PaletteStreamError {
    /// The first line isn't [`PALETTE_STREAM_HEADER`]. Nothing more of the stream is
    /// read.
    UnknownVersion { header: String },
    /// A line this format doesn't have, or one out of place.
    Malformed { line: String },
    /// A palette ended without every state of both variants.
    MissingStates { gtk_theme: String },
    /// A palette was cut off before its `end`.
    Unfinished { gtk_theme: String },
    /// The stream ended part way through a line.
    Truncated { line: String },
}

impl std::fmt::Display for PaletteStreamError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            PaletteStreamError::UnknownVersion { header } => {
                write!(f, "unknown palette stream header {header:?}")
            }
            PaletteStreamError::Malformed { line } => write!(f, "malformed palette line {line:?}"),
            PaletteStreamError::MissingStates { gtk_theme } => {
                write!(f, "the palette for {gtk_theme} is missing states")
            }
            PaletteStreamError::Unfinished { gtk_theme } => {
                write!(f, "the palette for {gtk_theme} was cut off")
            }
            PaletteStreamError::Truncated { line } => {
                write!(f, "the palette stream ended in the middle of {line:?}")
            }
        }
    }
}

impl std::error::Error for PaletteStreamError {}

#[cfg(test)]
mod tests {
    use std::collections::BTreeMap;

    use super::*;
    use crate::color::{GtkState, Rgba};
    use crate::palette::{PaletteColors, StateColors};

    const STATES: &str = "\
state normal 0.25 0.5 0.75 1 0 0 0 1
state prelight 0.25 0.5 0.75 1 0 0 0 1
state active 0.25 0.5 0.75 1 0 0 0 1
state selected 0.25 0.5 0.75 1 0 0 0 1
state insensitive 0.25 0.5 0.75 1 0 0 0 1
state inconsistent 0.25 0.5 0.75 1 0 0 0 1
state focused 0.25 0.5 0.75 1 0 0 0 1
state backdrop 0.25 0.5 0.75 1 0.5 0.5 0.5 1
";

    fn report() -> String {
        format!(
            "palette Adwaita\nvariant default\n{STATES}named wm_bg 1 1 1 0.5\n\
             variant dark\n{STATES}end\n"
        )
    }

    fn expected_colors(named: &[(&str, Rgba)]) -> PaletteColors {
        let states = GtkState::ALL.map(|state| StateColors {
            background: Rgba {
                red: 0.25,
                green: 0.5,
                blue: 0.75,
                alpha: 1.0,
            },
            text: if state == GtkState::Backdrop {
                Rgba {
                    red: 0.5,
                    green: 0.5,
                    blue: 0.5,
                    alpha: 1.0,
                }
            } else {
                Rgba {
                    red: 0.0,
                    green: 0.0,
                    blue: 0.0,
                    alpha: 1.0,
                }
            },
        });
        let named: BTreeMap<String, Rgba> = named
            .iter()
            .map(|(name, color)| (name.to_string(), *color))
            .collect();
        PaletteColors::new(states, named)
    }

    fn expected() -> ReportedPalette {
        let white = Rgba {
            red: 1.0,
            green: 1.0,
            blue: 1.0,
            alpha: 0.5,
        };
        ReportedPalette {
            gtk_theme: "Adwaita".to_owned(),
            palette: Palette {
                default: expected_colors(&[("wm_bg", white)]),
                dark: expected_colors(&[]),
            },
        }
    }

    #[test]
    fn a_report_is_read_into_its_palette() {
        let mut reader = PaletteReader::new();
        let stream = format!("camino-gtk-palette 1\n{}", report());
        assert_eq!(reader.feed(stream.as_bytes()), vec![Ok(expected())]);
    }

    #[test]
    fn a_stream_read_in_pieces_gives_the_same_palettes() {
        let stream = format!("camino-gtk-palette 1\n{}{}", report(), report());
        let mut reader = PaletteReader::new();
        let mut results = Vec::new();
        for byte in stream.as_bytes().chunks(7) {
            results.extend(reader.feed(byte));
        }
        assert_eq!(results, vec![Ok(expected()), Ok(expected())]);
        assert_eq!(reader.finish(), Ok(()));
    }

    #[test]
    fn a_line_cut_off_at_the_end_of_the_stream_is_truncated() {
        let mut reader = PaletteReader::new();
        let stream = format!("camino-gtk-palette 1\n{}palette Adw", report());
        assert_eq!(reader.feed(stream.as_bytes()), vec![Ok(expected())]);
        assert_eq!(
            reader.finish(),
            Err(PaletteStreamError::Truncated {
                line: "palette Adw".to_owned()
            })
        );
    }

    #[test]
    fn a_palette_cut_off_before_its_end_is_unfinished() {
        let mut reader = PaletteReader::new();
        let stream = "camino-gtk-palette 1\npalette Adwaita\nvariant default\n";
        assert_eq!(reader.feed(stream.as_bytes()), vec![]);
        assert_eq!(
            reader.finish(),
            Err(PaletteStreamError::Unfinished {
                gtk_theme: "Adwaita".to_owned()
            })
        );
    }

    #[test]
    fn a_truncated_line_drops_its_palette_and_the_next_one_is_read() {
        let broken = report().replace(
            "state active 0.25 0.5 0.75 1 0 0 0 1",
            "state active 0.25 0.5",
        );
        let stream = format!("camino-gtk-palette 1\n{broken}{}", report());
        let mut reader = PaletteReader::new();
        assert_eq!(
            reader.feed(stream.as_bytes()),
            vec![
                Err(PaletteStreamError::Malformed {
                    line: "state active 0.25 0.5".to_owned()
                }),
                Ok(expected()),
            ]
        );
    }

    #[test]
    fn a_palette_missing_a_state_is_refused() {
        let broken = report().replacen("state focused 0.25 0.5 0.75 1 0 0 0 1\n", "", 1);
        let mut reader = PaletteReader::new();
        let stream = format!("camino-gtk-palette 1\n{broken}");
        assert_eq!(
            reader.feed(stream.as_bytes()),
            vec![Err(PaletteStreamError::MissingStates {
                gtk_theme: "Adwaita".to_owned()
            })]
        );
    }

    #[test]
    fn channels_outside_zero_to_one_are_refused() {
        let broken = report().replace("named wm_bg 1 1 1 0.5", "named wm_bg 1 1 1 1.5");
        let mut reader = PaletteReader::new();
        let stream = format!("camino-gtk-palette 1\n{broken}");
        assert_eq!(
            reader.feed(stream.as_bytes()),
            vec![Err(PaletteStreamError::Malformed {
                line: "named wm_bg 1 1 1 1.5".to_owned()
            })]
        );
    }

    #[test]
    fn an_unknown_version_refuses_the_whole_stream() {
        let mut reader = PaletteReader::new();
        let stream = format!("camino-gtk-palette 2\n{}", report());
        assert_eq!(
            reader.feed(stream.as_bytes()),
            vec![Err(PaletteStreamError::UnknownVersion {
                header: "camino-gtk-palette 2".to_owned()
            })]
        );
        assert_eq!(reader.feed(report().as_bytes()), vec![]);
        assert_eq!(reader.finish(), Ok(()));
    }

    #[test]
    fn a_written_report_reads_back_exactly() {
        let mut palette = expected();
        // Channels a short decimal wouldn't give back.
        palette.palette.dark = expected_colors(&[(
            "wm_title",
            Rgba {
                red: 0.3021176470588235,
                green: 1.0 / 3.0,
                blue: 0.0,
                alpha: 0.18,
            },
        )]);
        let stream = format!(
            "{PALETTE_STREAM_HEADER}\n{}",
            format_report(&palette.gtk_theme, &palette.palette)
        );
        assert_eq!(
            PaletteReader::new().feed(stream.as_bytes()),
            vec![Ok(palette)]
        );
    }

    #[test]
    fn names_are_asked_for_on_one_line() {
        assert_eq!(
            names_request(["wm_bg", "wm_title"]),
            "names wm_bg wm_title\n"
        );
        assert_eq!(names_request([]), "names\n");
    }
}
