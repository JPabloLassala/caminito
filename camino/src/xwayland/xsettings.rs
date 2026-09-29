//! XSETTINGS (40, ADR 0006): Camino is the manager, through Smithay's `X11Wm`, so X11 GTK
//! apps get MATE's theme, fonts and DPI. The table is a port of mate-settings-daemon's
//! xsettings plugin at `927e9dc`: `translations[]` (`msd-xsettings-manager.c:205-240`) and
//! the Xft settings (`:395-485`, `:573-597`). The two must be kept in step (ADR 0006), so
//! a change to msd's table is a change here.
//!
//! Values come from Camino's settings, so a few differ from msd's where Camino's rules for
//! a key (09) replace a value msd would pass on: an empty theme, icon theme or font takes
//! its default, as do a double-click time or cursor size below 1 and a negative drag
//! threshold. A `dpi` of 0 or less is 96 (13), where msd measures the screen for 0 and
//! clamps a negative one to 50.
//!
//! Left out of msd's plugin: its `overrides` key, the `Fontconfig/Timestamp` its fontconfig
//! monitor bumps, the Xft resources it writes to `RESOURCE_MANAGER`, and the Qt variables
//! it exports, which would scale Wayland Qt apps twice (13).

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
    let fonts = &settings.font_rendering;

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

    // `xft_settings_get` and `xft_settings_set_xsettings`.
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
    set("Xft/Antialias", antialias.into());
    set("Xft/Hinting", hinting.into());
    set("Xft/HintStyle", hint_style.into());
    set("Gdk/WindowScalingFactor", saturate(scale));
    // In 1024ths of an inch, truncated as C's conversion to int is.
    set(
        "Gdk/UnscaledDPI",
        ((dpi / f64::from(scale) * 1024.0) as i32).into(),
    );
    set("Xft/DPI", ((dpi * 1024.0) as i32).into());
    set("Xft/RGBA", rgba.into());
    let lcd_filter = if rgba == "rgb" { "lcddefault" } else { "none" };
    set("Xft/lcdfilter", lcd_filter.into());
    set(
        "Gtk/CursorThemeSize",
        saturate(mouse.cursor_size.saturating_mul(scale)),
    );

    table
}

/// A size the settings keep unsigned, which came from a GSettings `i32`.
fn saturate(value: u32) -> XSetting {
    i32::try_from(value).unwrap_or(i32::MAX).into()
}

#[cfg(test)]
mod tests;
