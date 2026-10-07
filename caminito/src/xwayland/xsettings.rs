//! XSETTINGS (40, ADR 0006): Caminito is the manager, through Smithay's `X11Wm`, so X11 GTK
//! apps get MATE's theme, fonts and DPI. The table is a port of mate-settings-daemon's
//! xsettings plugin at `927e9dc`: `translations[]` (`msd-xsettings-manager.c:205-240`) and
//! the Xft settings (`:395-485`, `:573-597`). The two must be kept in step (ADR 0006), so
//! a change to msd's table is a change here.
//!
//! Values come from Caminito's settings, so a few differ from msd's where Caminito's rules for
//! a key (09) replace a value msd would pass on: an empty theme, icon theme or font takes
//! its default, as do a double-click time or cursor size below 1 and a negative drag
//! threshold. A `dpi` of 0 or less is 96 (13), where msd measures the screen for 0 and
//! clamps a negative one to 50.
//!
//! The same Xft settings go to the root window's `RESOURCE_MANAGER` too (47), as msd's
//! `xft_settings_set_xresources` (`:599-677`) writes them, for Xft apps that don't read
//! XSETTINGS. Caminito's X connection writes them, since Smithay's has no call for it.
//!
//! Left out of msd's plugin: its `overrides` key, the `Fontconfig/Timestamp` its fontconfig
//! monitor bumps, and the Qt variables it exports, which would scale Wayland Qt apps twice
//! (13).

use std::collections::BTreeMap;

use crate::settings::Settings;
use crate::settings::translate::{Antialiasing, Hinting, RgbaOrder};

/// An XSETTINGS value. msd sets no colours.
#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) enum XSetting {
    Int(i32),
    Str(String),
}

impl From<i32> for XSetting {
    fn from(value: i32) -> Self {
        XSetting::Int(value)
    }
}

impl XSetting {
    /// The value as Smithay's `X11Wm` takes it.
    pub(crate) fn to_smithay(&self) -> smithay::xwayland::xwm::settings::Value {
        use smithay::xwayland::xwm::settings::Value;
        match self {
            XSetting::Int(i) => Value::Integer(*i),
            XSetting::Str(s) => Value::String(s.clone()),
        }
    }
}

/// XSETTINGS has no booleans: msd's `translate_bool_int` sets 1 or 0.
impl From<bool> for XSetting {
    fn from(value: bool) -> Self {
        XSetting::Int(value.into())
    }
}

impl From<&str> for XSetting {
    fn from(value: &str) -> Self {
        XSetting::Str(value.into())
    }
}

/// The XSETTINGS table, by setting name.
pub(crate) type Table = BTreeMap<&'static str, XSetting>;

/// msd's fallback when it can't measure the screen, which Xwayland's made-up size rules out
/// (13): so it's used whenever `dpi` is 0 or less, where msd uses 0 alone.
const DPI_FALLBACK: f64 = 96.0;
/// The DPIs msd believes, beyond which it clamps.
const DPI_REASONABLE: std::ops::RangeInclusive<f64> = 50.0..=500.0;

/// The table msd would publish for `settings` at the X11 scale `scale` (41).
pub(crate) fn table(settings: &Settings, scale: u32) -> Table {
    let mouse = &settings.mouse;
    let interface = &settings.interface;
    let gtk = &interface.gtk;
    let sound = &settings.sound;

    let mut table = Table::new();
    let mut set = |name, value: XSetting| {
        table.insert(name, value);
    };

    // `translations[]`, in its order. Its cursor size is replaced by the Xft settings'.
    let double_click = i32::try_from(mouse.double_click_time.as_millis()).unwrap_or(i32::MAX);
    set("Net/DoubleClickTime", double_click.into());
    set("Net/DndDragThreshold", saturate(mouse.drag_threshold));
    set("Gtk/CursorThemeName", mouse.cursor_theme.as_str().into());
    set("Gtk/FontName", interface.font_name.as_str().into());
    set("Gtk/KeyThemeName", gtk.key_theme.as_str().into());
    // `translate_string_string_toolbar`: GNOME's nick has an underscore where GTK's has a
    // dash.
    let toolbar_style = match gtk.toolbar_style.as_str() {
        "both_horiz" => "both-horiz",
        style => style,
    };
    set("Gtk/ToolbarStyle", toolbar_style.into());
    set(
        "Gtk/ToolbarIconSize",
        gtk.toolbar_icons_size.as_str().into(),
    );
    set("Net/CursorBlink", gtk.cursor_blink.into());
    set("Net/CursorBlinkTime", gtk.cursor_blink_time.into());
    set("Net/ThemeName", interface.gtk_theme.as_str().into());
    set("Gtk/ColorScheme", gtk.color_scheme.as_str().into());
    set("Gtk/IMPreeditStyle", gtk.im_preedit_style.as_str().into());
    set("Gtk/IMStatusStyle", gtk.im_status_style.as_str().into());
    set("Gtk/IMModule", gtk.im_module.as_str().into());
    set("Net/IconThemeName", interface.icon_theme.as_str().into());
    set(
        "Gtk/FileChooserBackend",
        gtk.file_chooser_backend.as_str().into(),
    );
    set(
        "Gtk/DecorationLayout",
        gtk.decoration_layout.as_str().into(),
    );
    set("Gtk/ShellShowsAppMenu", gtk.shell_shows_app_menu.into());
    set("Gtk/ShellShowsMenubar", gtk.shell_shows_menubar.into());
    set("Gtk/MenuImages", gtk.menus_have_icons.into());
    set("Gtk/ButtonImages", gtk.buttons_have_icons.into());
    set("Gtk/MenuBarAccel", gtk.menubar_accel.as_str().into());
    set("Gtk/ShowInputMethodMenu", gtk.show_input_method_menu.into());
    set("Gtk/ShowUnicodeMenu", gtk.show_unicode_menu.into());
    set("Gtk/AutoMnemonics", gtk.automatic_mnemonics.into());
    set("Gtk/EnablePrimaryPaste", gtk.enable_primary_paste.into());
    set("Gtk/EnableAnimations", gtk.enable_animations.into());
    set("Gtk/DialogsUseHeader", gtk.dialogs_use_header.into());
    set("Net/SoundThemeName", sound.theme_name.as_str().into());
    set("Net/EnableEventSounds", sound.event_sounds.into());
    set(
        "Net/EnableInputFeedbackSounds",
        sound.input_feedback_sounds.into(),
    );
    // Set once at start-up (`:1014-1017`) and again on every change (`xsettings_callback`).
    set("Net/FallbackIconTheme", "mate".into());

    // `xft_settings_set_xsettings`.
    let xft = Xft::new(settings, scale);
    set("Xft/Antialias", xft.antialias.into());
    set("Xft/Hinting", xft.hinting.into());
    set("Xft/HintStyle", xft.hint_style.into());
    set("Gdk/WindowScalingFactor", saturate(scale));
    // In 1024ths of an inch, truncated as C's conversion to int is.
    set(
        "Gdk/UnscaledDPI",
        ((xft.dpi / f64::from(scale) * 1024.0) as i32).into(),
    );
    set("Xft/DPI", xft.dpi_1024ths().into());
    set("Xft/RGBA", xft.rgba.into());
    set("Xft/lcdfilter", xft.lcd_filter().into());
    set("Gtk/CursorThemeSize", saturate(xft.cursor_size));

    table
}

/// msd's `MateXftSettings`, from `xft_settings_get` (`:395-483`), which both XSETTINGS and
/// the X resources are written from.
struct Xft {
    antialias: bool,
    hinting: bool,
    hint_style: &'static str,
    rgba: &'static str,
    /// Dots per inch at the X11 scale, clamped.
    dpi: f64,
    /// At the X11 scale.
    cursor_size: u32,
}

impl Xft {
    fn new(settings: &Settings, scale: u32) -> Self {
        let fonts = &settings.font_rendering;
        let (antialias, subpixel) = match fonts.antialiasing {
            Antialiasing::None => (false, false),
            Antialiasing::Grayscale => (true, false),
            Antialiasing::Rgba => (true, true),
        };
        let rgba = if subpixel {
            match fonts.rgba_order {
                RgbaOrder::Rgb => "rgb",
                RgbaOrder::Bgr => "bgr",
                RgbaOrder::Vrgb => "vrgb",
                RgbaOrder::Vbgr => "vbgr",
            }
        } else {
            "none"
        };
        let (hinting, hint_style) = match fonts.hinting {
            Hinting::None => (false, "hintnone"),
            Hinting::Slight => (true, "hintslight"),
            Hinting::Medium => (true, "hintmedium"),
            Hinting::Full => (true, "hintfull"),
        };
        let dpi = if fonts.dpi > 0.0 {
            fonts.dpi
        } else {
            DPI_FALLBACK
        };
        let dpi = (dpi * f64::from(scale)).clamp(*DPI_REASONABLE.start(), *DPI_REASONABLE.end());
        Xft {
            antialias,
            hinting,
            hint_style,
            rgba,
            dpi,
            cursor_size: settings.mouse.cursor_size.saturating_mul(scale),
        }
    }

    /// `Xft/DPI`: in 1024ths of an inch, truncated as C's conversion to int is.
    fn dpi_1024ths(&self) -> i32 {
        (self.dpi * 1024.0) as i32
    }

    fn lcd_filter(&self) -> &'static str {
        if self.rgba == "rgb" {
            "lcddefault"
        } else {
            "none"
        }
    }
}

/// X resources by name, in the order msd writes them.
pub(super) type Resources = Vec<(&'static str, String)>;

/// The X resources msd would write to `RESOURCE_MANAGER` for `settings` at the X11 scale
/// `scale` (47): `xft_settings_set_xresources` (`:630-677`). Xft apps that don't read
/// XSETTINGS, such as `xterm -fa`, take their DPI from `Xft.dpi`.
pub(super) fn resources(settings: &Settings, scale: u32) -> Resources {
    let xft = Xft::new(settings, scale);
    let flag = |on: bool| if on { "1" } else { "0" }.to_string();
    vec![
        // Whole dots, rounded from `Xft/DPI` as msd's `+ 0.5` does.
        (
            "Xft.dpi",
            ((f64::from(xft.dpi_1024ths()) / 1024.0 + 0.5) as i32).to_string(),
        ),
        ("Xft.antialias", flag(xft.antialias)),
        ("Xft.hinting", flag(xft.hinting)),
        ("Xft.hintstyle", xft.hint_style.to_string()),
        ("Xft.rgba", xft.rgba.to_string()),
        ("Xft.lcdfilter", xft.lcd_filter().to_string()),
        ("Xcursor.theme", settings.mouse.cursor_theme.clone()),
        ("Xcursor.size", xft.cursor_size.to_string()),
    ]
}

/// `RESOURCE_MANAGER` with `resources` merged into `existing`, as msd's `update_property`
/// (`:599-628`) does: every other line is kept, a line that sets one of `resources` is
/// replaced where it is, and the rest are appended as `name:\tvalue`. Unlike msd, a line
/// matches only if its resource name is the same, not if it contains it, so `XTerm*Xft.dpi`
/// is left alone. Whitespace around a name is ignored, as Xrm ignores it, so
/// `Xft.rgba : bgr` is replaced. Every line that sets a name is replaced, not only the
/// first, and blank lines, which Xrm skips, are dropped.
pub(super) fn merge_resources(existing: &[u8], resources: &Resources) -> Vec<u8> {
    let line_for = |(name, value): &(&str, String)| format!("{name}:\t{value}").into_bytes();
    let mut written = vec![false; resources.len()];
    let mut merged: Vec<u8> = Vec::with_capacity(existing.len());
    let lines = existing
        .split(|&b| b == b'\n')
        .filter(|line| !line.is_empty());
    for line in lines {
        let name = line
            .iter()
            .position(|&b| b == b':')
            .map(|colon| line[..colon].trim_ascii());
        let found = name.and_then(|name| {
            resources
                .iter()
                .position(|(resource, _)| resource.as_bytes() == name)
        });
        match found {
            Some(i) => {
                merged.extend(line_for(&resources[i]));
                written[i] = true;
            }
            None => merged.extend_from_slice(line),
        }
        merged.push(b'\n');
    }
    for (resource, written) in resources.iter().zip(written) {
        if !written {
            merged.extend(line_for(resource));
            merged.push(b'\n');
        }
    }
    merged
}

/// A size the settings keep unsigned, which came from a GSettings `i32`.
fn saturate(value: u32) -> XSetting {
    i32::try_from(value).unwrap_or(i32::MAX).into()
}

#[cfg(test)]
mod tests;
