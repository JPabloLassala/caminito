//! The table against msd's code at the revision the module names: `translations[]`
//! (`msd-xsettings-manager.c:205-240`) and the Xft settings (`:395-485`, `:573-597`).

use std::time::Duration;

use insta::assert_snapshot;

use super::*;
use crate::settings::translate::{Antialiasing, Hinting, Reading, RgbaOrder, translate};

/// The settings at the schema defaults.
fn defaults() -> Settings {
    translate(&Reading::default()).settings
}

/// One line per setting, `name = value`, with strings quoted.
fn show(table: &Table) -> String {
    table
        .iter()
        .map(|(name, setting)| match setting {
            XSetting::Int(i) => format!("{name} = {i}"),
            XSetting::Str(s) => format!("{name} = {s:?}"),
        })
        .collect::<Vec<_>>()
        .join("\n")
}

fn int(i: i32) -> XSetting {
    XSetting::Int(i)
}

fn string(s: &str) -> XSetting {
    XSetting::Str(s.into())
}

/// The settings at their defaults with `change` made.
fn changed(change: impl FnOnce(&mut Settings)) -> Table {
    let mut settings = defaults();
    change(&mut settings);
    table(&settings, 1)
}

#[test]
fn the_defaults_give_msds_table() {
    assert_snapshot!(show(&table(&defaults(), 1)), @r#"
    Gdk/UnscaledDPI = 98304
    Gdk/WindowScalingFactor = 1
    Gtk/AutoMnemonics = 1
    Gtk/ButtonImages = 1
    Gtk/ColorScheme = ""
    Gtk/CursorThemeName = ""
    Gtk/CursorThemeSize = 24
    Gtk/DecorationLayout = "menu:minimize,maximize,close"
    Gtk/DialogsUseHeader = 0
    Gtk/EnableAnimations = 1
    Gtk/EnablePrimaryPaste = 1
    Gtk/FileChooserBackend = "gio"
    Gtk/FontName = "Sans 10"
    Gtk/IMModule = ""
    Gtk/IMPreeditStyle = "callback"
    Gtk/IMStatusStyle = "callback"
    Gtk/KeyThemeName = "Default"
    Gtk/MenuBarAccel = "F10"
    Gtk/MenuImages = 1
    Gtk/ShellShowsAppMenu = 0
    Gtk/ShellShowsMenubar = 0
    Gtk/ShowInputMethodMenu = 1
    Gtk/ShowUnicodeMenu = 1
    Gtk/ToolbarIconSize = "large-toolbar"
    Gtk/ToolbarStyle = "both-horiz"
    Net/CursorBlink = 1
    Net/CursorBlinkTime = 1200
    Net/DndDragThreshold = 8
    Net/DoubleClickTime = 400
    Net/EnableEventSounds = 0
    Net/EnableInputFeedbackSounds = 0
    Net/FallbackIconTheme = "mate"
    Net/IconThemeName = "menta"
    Net/SoundThemeName = "freedesktop"
    Net/ThemeName = "Menta"
    Xft/Antialias = 1
    Xft/DPI = 98304
    Xft/HintStyle = "hintslight"
    Xft/Hinting = 1
    Xft/RGBA = "rgb"
    Xft/lcdfilter = "lcddefault"
    "#);
}

#[test]
fn each_key_msd_passes_on_reaches_its_setting() {
    type Change = fn(&mut Settings);
    let cases: &[(Change, &str, XSetting)] = &[
        (
            |s| s.mouse.double_click_time = Duration::from_millis(250),
            "Net/DoubleClickTime",
            int(250),
        ),
        (
            |s| s.mouse.drag_threshold = 3,
            "Net/DndDragThreshold",
            int(3),
        ),
        (
            |s| s.mouse.cursor_theme = "mate-black".into(),
            "Gtk/CursorThemeName",
            string("mate-black"),
        ),
        (|s| s.mouse.cursor_size = 48, "Gtk/CursorThemeSize", int(48)),
        (
            |s| s.interface.font_name = "Cantarell 11".into(),
            "Gtk/FontName",
            string("Cantarell 11"),
        ),
        (
            |s| s.interface.gtk.key_theme = "Emacs".into(),
            "Gtk/KeyThemeName",
            string("Emacs"),
        ),
        (
            |s| s.interface.gtk.toolbar_style = "icons".into(),
            "Gtk/ToolbarStyle",
            string("icons"),
        ),
        (
            |s| s.interface.gtk.toolbar_icons_size = "small-toolbar".into(),
            "Gtk/ToolbarIconSize",
            string("small-toolbar"),
        ),
        (
            |s| s.interface.gtk.cursor_blink = false,
            "Net/CursorBlink",
            int(0),
        ),
        (
            |s| s.interface.gtk.cursor_blink_time = 800,
            "Net/CursorBlinkTime",
            int(800),
        ),
        (
            |s| s.interface.gtk_theme = "BlackMATE".into(),
            "Net/ThemeName",
            string("BlackMATE"),
        ),
        (
            |s| s.interface.gtk.color_scheme = "fg_color:#000".into(),
            "Gtk/ColorScheme",
            string("fg_color:#000"),
        ),
        (
            |s| s.interface.gtk.im_preedit_style = "nothing".into(),
            "Gtk/IMPreeditStyle",
            string("nothing"),
        ),
        (
            |s| s.interface.gtk.im_status_style = "none".into(),
            "Gtk/IMStatusStyle",
            string("none"),
        ),
        (
            |s| s.interface.gtk.im_module = "ibus".into(),
            "Gtk/IMModule",
            string("ibus"),
        ),
        (
            |s| s.interface.icon_theme = "mate".into(),
            "Net/IconThemeName",
            string("mate"),
        ),
        (
            |s| s.interface.gtk.file_chooser_backend = "gnome-vfs".into(),
            "Gtk/FileChooserBackend",
            string("gnome-vfs"),
        ),
        (
            |s| s.interface.gtk.decoration_layout = "close:".into(),
            "Gtk/DecorationLayout",
            string("close:"),
        ),
        (
            |s| s.interface.gtk.shell_shows_app_menu = true,
            "Gtk/ShellShowsAppMenu",
            int(1),
        ),
        (
            |s| s.interface.gtk.shell_shows_menubar = true,
            "Gtk/ShellShowsMenubar",
            int(1),
        ),
        (
            |s| s.interface.gtk.menus_have_icons = false,
            "Gtk/MenuImages",
            int(0),
        ),
        (
            |s| s.interface.gtk.buttons_have_icons = false,
            "Gtk/ButtonImages",
            int(0),
        ),
        (
            |s| s.interface.gtk.menubar_accel = "F9".into(),
            "Gtk/MenuBarAccel",
            string("F9"),
        ),
        (
            |s| s.interface.gtk.show_input_method_menu = false,
            "Gtk/ShowInputMethodMenu",
            int(0),
        ),
        (
            |s| s.interface.gtk.show_unicode_menu = false,
            "Gtk/ShowUnicodeMenu",
            int(0),
        ),
        (
            |s| s.interface.gtk.automatic_mnemonics = false,
            "Gtk/AutoMnemonics",
            int(0),
        ),
        (
            |s| s.interface.gtk.enable_primary_paste = false,
            "Gtk/EnablePrimaryPaste",
            int(0),
        ),
        (
            |s| s.interface.gtk.enable_animations = false,
            "Gtk/EnableAnimations",
            int(0),
        ),
        (
            |s| s.interface.gtk.dialogs_use_header = true,
            "Gtk/DialogsUseHeader",
            int(1),
        ),
        (
            |s| s.sound.theme_name = "mate".into(),
            "Net/SoundThemeName",
            string("mate"),
        ),
        (
            |s| s.sound.event_sounds = true,
            "Net/EnableEventSounds",
            int(1),
        ),
        (
            |s| s.sound.input_feedback_sounds = true,
            "Net/EnableInputFeedbackSounds",
            int(1),
        ),
    ];
    for (change, name, expected) in cases {
        let table = changed(change);
        assert_eq!(table.get(name), Some(expected), "{name}");
        // Nothing else changes.
        let mut others = table.clone();
        let mut default_others = self::table(&defaults(), 1);
        others.remove(name);
        default_others.remove(name);
        assert_eq!(others, default_others, "only {name} changes");
    }
}

#[test]
fn a_both_horiz_toolbar_style_is_spelled_as_gtk_wants() {
    let table = changed(|s| s.interface.gtk.toolbar_style = "both_horiz".into());
    assert_eq!(table["Gtk/ToolbarStyle"], string("both-horiz"));
}

#[test]
fn antialiasing_sets_antialias_and_subpixel_order() {
    let xft = |antialiasing, rgba_order| {
        let table = changed(|s| {
            s.font_rendering.antialiasing = antialiasing;
            s.font_rendering.rgba_order = rgba_order;
        });
        (
            table["Xft/Antialias"].clone(),
            table["Xft/RGBA"].clone(),
            table["Xft/lcdfilter"].clone(),
        )
    };
    // Only rgba antialiasing uses the subpixel order, and only rgb gets the LCD filter.
    assert_eq!(
        xft(Antialiasing::None, RgbaOrder::Rgb),
        (int(0), string("none"), string("none"))
    );
    assert_eq!(
        xft(Antialiasing::Grayscale, RgbaOrder::Bgr),
        (int(1), string("none"), string("none"))
    );
    assert_eq!(
        xft(Antialiasing::Rgba, RgbaOrder::Rgb),
        (int(1), string("rgb"), string("lcddefault"))
    );
    for (order, name) in [
        (RgbaOrder::Bgr, "bgr"),
        (RgbaOrder::Vrgb, "vrgb"),
        (RgbaOrder::Vbgr, "vbgr"),
    ] {
        assert_eq!(
            xft(Antialiasing::Rgba, order),
            (int(1), string(name), string("none"))
        );
    }
}

#[test]
fn hinting_sets_hinting_and_its_style() {
    for (hinting, on, style) in [
        (Hinting::None, 0, "hintnone"),
        (Hinting::Slight, 1, "hintslight"),
        (Hinting::Medium, 1, "hintmedium"),
        (Hinting::Full, 1, "hintfull"),
    ] {
        let table = changed(|s| s.font_rendering.hinting = hinting);
        assert_eq!(table["Xft/Hinting"], int(on), "{hinting:?}");
        assert_eq!(table["Xft/HintStyle"], string(style), "{hinting:?}");
    }
}

/// `Xft/DPI` and `Gdk/UnscaledDPI` for a `dpi` key.
fn dpi(dpi: f64) -> (XSetting, XSetting) {
    let table = changed(|s| s.font_rendering.dpi = dpi);
    (table["Xft/DPI"].clone(), table["Gdk/UnscaledDPI"].clone())
}

#[test]
fn a_dpi_of_0_or_less_is_96() {
    // msd measures the screen instead, which Xwayland makes up (13).
    assert_eq!(dpi(0.0), (int(96 * 1024), int(96 * 1024)));
    assert_eq!(dpi(-10.0), (int(96 * 1024), int(96 * 1024)));
}

#[test]
fn a_set_dpi_is_given_in_1024ths_of_an_inch() {
    assert_eq!(dpi(120.0), (int(120 * 1024), int(120 * 1024)));
    // Truncated, as C's conversion to int does.
    assert_eq!(dpi(96.3), (int(98611), int(98611)));
}

#[test]
fn dpi_is_clamped_to_what_msd_finds_reasonable() {
    assert_eq!(dpi(20.0), (int(50 * 1024), int(50 * 1024)));
    assert_eq!(dpi(600.0), (int(500 * 1024), int(500 * 1024)));
}

/// The X11 scale settings for a `dpi` key at the X11 scale `scale`, one per line.
fn scaled(dpi: f64, scale: u32) -> String {
    let mut settings = defaults();
    settings.font_rendering.dpi = dpi;
    let table = table(&settings, scale);
    let names = [
        "Xft/DPI",
        "Gdk/UnscaledDPI",
        "Gdk/WindowScalingFactor",
        "Gtk/CursorThemeSize",
    ];
    names
        .iter()
        .map(|name| format!("{name} = {:?}", table[name]))
        .collect::<Vec<_>>()
        .join("\n")
}

#[test]
fn the_x11_scale_multiplies_dpi_and_cursor_size_as_msd_does() {
    // `msd-xsettings-manager.c:356-374`, `:411-421`: Xft/DPI = dpi × s × 1024,
    // Gdk/UnscaledDPI = dpi × 1024, and the cursor size × s. The default cursor size is 24.
    assert_snapshot!(scaled(0.0, 1), @r"
    Xft/DPI = Int(98304)
    Gdk/UnscaledDPI = Int(98304)
    Gdk/WindowScalingFactor = Int(1)
    Gtk/CursorThemeSize = Int(24)
    ");
    assert_snapshot!(scaled(0.0, 2), @r"
    Xft/DPI = Int(196608)
    Gdk/UnscaledDPI = Int(98304)
    Gdk/WindowScalingFactor = Int(2)
    Gtk/CursorThemeSize = Int(48)
    ");
    assert_snapshot!(scaled(96.0, 1), @r"
    Xft/DPI = Int(98304)
    Gdk/UnscaledDPI = Int(98304)
    Gdk/WindowScalingFactor = Int(1)
    Gtk/CursorThemeSize = Int(24)
    ");
    assert_snapshot!(scaled(96.0, 2), @r"
    Xft/DPI = Int(196608)
    Gdk/UnscaledDPI = Int(98304)
    Gdk/WindowScalingFactor = Int(2)
    Gtk/CursorThemeSize = Int(48)
    ");
    assert_snapshot!(scaled(120.0, 1), @r"
    Xft/DPI = Int(122880)
    Gdk/UnscaledDPI = Int(122880)
    Gdk/WindowScalingFactor = Int(1)
    Gtk/CursorThemeSize = Int(24)
    ");
    assert_snapshot!(scaled(120.0, 2), @r"
    Xft/DPI = Int(245760)
    Gdk/UnscaledDPI = Int(122880)
    Gdk/WindowScalingFactor = Int(2)
    Gtk/CursorThemeSize = Int(48)
    ");
}
