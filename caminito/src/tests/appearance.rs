//! The Colour scheme and the Appearances (ADR 0059, CMN-327), through GSettings as
//! `gsettings`, Appearance and the applets would set them: Caminito writes MATE's theme keys
//! and its own, and each comes back as a reading.

use super::fixture::Fixture;
use crate::settings::Value;
use crate::settings::translate::{CAMINITO, ColourScheme, INTERFACE, MARCO_GENERAL, Themes};

fn s(value: &str) -> Value {
    Value::Str(value.into())
}

/// Caminito set up before, light with Breeze, with Breeze-Dark as its dark Appearance.
fn breeze() -> Fixture {
    Fixture::with_settings(vec![
        (INTERFACE, "gtk-theme".into(), s("Breeze")),
        (MARCO_GENERAL, "theme".into(), s("Breeze")),
        (INTERFACE, "icon-theme".into(), s("breeze")),
        (CAMINITO, "colour-scheme".into(), s("light")),
        (CAMINITO, "light-gtk-theme".into(), s("Breeze")),
        (CAMINITO, "light-marco-theme".into(), s("Breeze")),
        (CAMINITO, "light-icon-theme".into(), s("breeze")),
        (CAMINITO, "dark-gtk-theme".into(), s("Breeze-Dark")),
        (CAMINITO, "dark-marco-theme".into(), s("Breeze-Dark")),
        (CAMINITO, "dark-icon-theme".into(), s("breeze-dark")),
    ])
}

/// The themes MATE's keys have, as Caminito last read them.
fn shown(f: &Fixture) -> Themes {
    let settings = &f.state.caminito.settings;
    Themes {
        gtk: settings.interface.gtk_theme.clone(),
        marco: settings.frames.theme.clone(),
        icons: settings.interface.icon_theme.clone(),
    }
}

fn themes(gtk: &str, marco: &str, icons: &str) -> Themes {
    Themes {
        gtk: gtk.into(),
        marco: marco.into(),
        icons: icons.into(),
    }
}

fn appearance(f: &Fixture) -> &crate::settings::translate::AppearanceKeys {
    f.state.caminito.settings.appearance.as_ref().unwrap()
}

#[test]
fn switching_the_colour_scheme_switches_the_gtk_marco_and_icon_themes() {
    let mut f = breeze();

    f.set_setting(CAMINITO, "colour-scheme", s("dark"));
    f.dispatch_until(|f| shown(f) == themes("Breeze-Dark", "Breeze-Dark", "breeze-dark"));

    f.set_setting(CAMINITO, "colour-scheme", s("light"));
    f.dispatch_until(|f| shown(f) == themes("Breeze", "Breeze", "breeze"));
}

#[test]
fn a_theme_set_with_gsettings_changes_the_appearance_in_use_and_not_the_other() {
    let mut f = breeze();

    f.set_setting(INTERFACE, "gtk-theme", s("Adwaita"));
    f.dispatch_until(|f| appearance(f).light.gtk == "Adwaita");

    assert_eq!(
        appearance(&f).dark,
        themes("Breeze-Dark", "Breeze-Dark", "breeze-dark")
    );
    assert_eq!(shown(&f).gtk, "Adwaita");
}

#[test]
fn the_first_start_under_blackmate_stays_dark_with_a_light_appearance_from_the_table() {
    let mut f = Fixture::with_settings(vec![
        (INTERFACE, "gtk-theme".into(), s("BlackMATE")),
        (MARCO_GENERAL, "theme".into(), s("BlackMATE")),
        (INTERFACE, "icon-theme".into(), s("menta")),
    ]);

    f.dispatch_until(|f| appearance(f).colour_scheme == Some(ColourScheme::Dark));
    f.dispatch_until(|f| appearance(f).light == themes("Menta", "Menta", "menta"));

    assert_eq!(
        appearance(&f).dark,
        themes("BlackMATE", "BlackMATE", "menta")
    );
    assert_eq!(shown(&f), themes("BlackMATE", "BlackMATE", "menta"));
}
