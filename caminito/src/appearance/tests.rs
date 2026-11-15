use caminito_theme::Palette;

use super::*;

fn themes(gtk: &str, marco: &str, icons: &str) -> Themes {
    Themes {
        gtk: gtk.into(),
        marco: marco.into(),
        icons: icons.into(),
    }
}

/// Caminito's keys and MATE's, as dconf holds them, and Caminito reconciling them.
struct Desktop {
    keys: AppearanceKeys,
    mate: Themes,
    appearances: Appearances,
}

impl Desktop {
    /// Caminito set up before, light, and MATE showing the light Appearance.
    fn light(light: Themes, dark: Themes) -> Desktop {
        let mut desktop = Desktop {
            mate: light.clone(),
            keys: AppearanceKeys {
                colour_scheme: Some(ColourScheme::Light),
                light,
                dark,
            },
            appearances: Appearances::default(),
        };
        desktop.read();
        desktop
    }

    /// Caminito's first start, with MATE showing `mate`.
    fn first_start(mate: Themes) -> Desktop {
        Desktop {
            mate,
            keys: AppearanceKeys {
                colour_scheme: None,
                light: Themes::default(),
                dark: Themes::default(),
            },
            appearances: Appearances::default(),
        }
    }

    /// A reading, with the darkness of the table of known pairs alone. Returns the writes, and
    /// makes them, so the next reading shows them.
    fn read(&mut self) -> Outcome {
        self.read_with(known_scheme)
    }

    fn read_with(&mut self, scheme_of: impl FnOnce(&str) -> Option<ColourScheme>) -> Outcome {
        let outcome = self
            .appearances
            .reconcile(&self.keys, &self.mate, scheme_of);
        for write in &outcome.writes {
            self.make(write);
        }
        outcome
    }

    /// A reading that doesn't show `writes` yet.
    fn read_before_writes_land(&mut self) -> Outcome {
        self.appearances.reconcile(&self.keys, &self.mate, |_| None)
    }

    fn make(&mut self, write: &KeyWrite) {
        let Value::Str(value) = &write.value else {
            panic!("a theme or a nick: {write:?}");
        };
        let value = value.clone();
        match (write.schema, write.key.as_str()) {
            (INTERFACE, "gtk-theme") => self.mate.gtk = value,
            (MARCO_GENERAL, "theme") => self.mate.marco = value,
            (INTERFACE, "icon-theme") => self.mate.icons = value,
            (CAMINITO, "colour-scheme") => {
                self.keys.colour_scheme = match value.as_str() {
                    "light" => Some(ColourScheme::Light),
                    "dark" => Some(ColourScheme::Dark),
                    other => panic!("not a colour scheme: {other}"),
                }
            }
            (CAMINITO, key) => {
                let (scheme, kind) = key.split_once('-').unwrap();
                let themes = match scheme {
                    "light" => &mut self.keys.light,
                    "dark" => &mut self.keys.dark,
                    other => panic!("not a scheme: {other}"),
                };
                match kind {
                    "gtk-theme" => themes.gtk = value,
                    "marco-theme" => themes.marco = value,
                    "icon-theme" => themes.icons = value,
                    other => panic!("not a theme key: {other}"),
                }
            }
            other => panic!("an unexpected key: {other:?}"),
        }
    }

    fn set_scheme(&mut self, scheme: ColourScheme) {
        self.keys.colour_scheme = Some(scheme);
    }
}

/// The writes of `outcome` as `schema key=value` lines.
fn writes(outcome: &Outcome) -> Vec<String> {
    outcome
        .writes
        .iter()
        .map(|w| format!("{} {}={}", w.schema, w.key, w.value))
        .collect()
}

fn breeze() -> (Themes, Themes) {
    (
        themes("Breeze", "Breeze", "breeze"),
        themes("Breeze-Dark", "Breeze-Dark", "breeze-dark"),
    )
}

#[test]
fn switching_to_dark_writes_the_dark_appearance_into_mates_keys() {
    let (light, dark) = breeze();
    let mut desktop = Desktop::light(light, dark.clone());

    desktop.set_scheme(ColourScheme::Dark);
    let outcome = desktop.read();

    assert_eq!(
        writes(&outcome),
        [
            "org.mate.interface gtk-theme='Breeze-Dark'",
            "org.mate.Marco.general theme='Breeze-Dark'",
            "org.mate.interface icon-theme='breeze-dark'",
        ]
    );
    assert_eq!(desktop.mate, dark);
}

#[test]
fn switching_back_to_light_writes_the_light_appearance() {
    let (light, dark) = breeze();
    let mut desktop = Desktop::light(light.clone(), dark);
    desktop.set_scheme(ColourScheme::Dark);
    desktop.read();
    desktop.read();

    desktop.set_scheme(ColourScheme::Light);
    desktop.read();

    assert_eq!(desktop.mate, light);
}

#[test]
fn only_the_themes_that_differ_are_written() {
    let mut desktop = Desktop::light(
        themes("Menta", "Menta", "menta"),
        themes("BlackMATE", "BlackMATE", "menta"),
    );

    desktop.set_scheme(ColourScheme::Dark);
    let outcome = desktop.read();

    assert_eq!(
        writes(&outcome),
        [
            "org.mate.interface gtk-theme='BlackMATE'",
            "org.mate.Marco.general theme='BlackMATE'",
        ]
    );
}

#[test]
fn a_reading_with_no_change_writes_nothing() {
    let (light, dark) = breeze();
    let mut desktop = Desktop::light(light, dark);

    assert_eq!(desktop.read().writes, []);
}

#[test]
fn a_theme_picked_elsewhere_edits_the_appearance_in_use_and_not_the_other() {
    let (light, dark) = breeze();
    let mut desktop = Desktop::light(light, dark.clone());

    desktop.mate.gtk = "Adwaita".into();
    let outcome = desktop.read();

    assert_eq!(
        writes(&outcome),
        ["org.mate.Caminito light-gtk-theme='Adwaita'"]
    );
    assert_eq!(desktop.keys.light, themes("Adwaita", "Breeze", "breeze"));
    assert_eq!(desktop.keys.dark, dark);
}

#[test]
fn a_theme_picked_under_dark_edits_the_dark_appearance() {
    let (light, dark) = breeze();
    let mut desktop = Desktop::light(light.clone(), dark);
    desktop.set_scheme(ColourScheme::Dark);
    desktop.read();
    desktop.read();

    desktop.mate.icons = "Papirus-Dark".into();
    desktop.read();

    assert_eq!(
        desktop.keys.dark,
        themes("Breeze-Dark", "Breeze-Dark", "Papirus-Dark")
    );
    assert_eq!(desktop.keys.light, light);
}

#[test]
fn a_reading_from_before_the_switch_landed_is_not_taken_for_an_edit() {
    let (light, dark) = breeze();
    let mut desktop = Desktop::light(light.clone(), dark.clone());
    desktop.set_scheme(ColourScheme::Dark);
    desktop.read_before_writes_land();

    // The reading still shows the light themes, while the dark Appearance is in use.
    let outcome = desktop.read_before_writes_land();

    assert_eq!(outcome.writes, []);
    assert_eq!(desktop.keys.dark, dark);
}

#[test]
fn switching_back_before_the_first_switch_landed_ends_light() {
    let (light, dark) = breeze();
    let mut desktop = Desktop::light(light.clone(), dark.clone());
    desktop.set_scheme(ColourScheme::Dark);
    let to_dark = desktop.read_before_writes_land();
    desktop.set_scheme(ColourScheme::Light);
    let to_light = desktop.read_before_writes_land();

    // The dark writes land, then the light ones.
    for write in to_dark.writes.iter().chain(&to_light.writes) {
        desktop.make(write);
        desktop.read();
    }

    assert_eq!(desktop.mate, light);
    assert_eq!(desktop.keys.light, light);
    assert_eq!(desktop.keys.dark, dark);
}

#[test]
fn the_first_start_under_blackmate_stays_dark_with_a_light_appearance_from_the_table() {
    let mut desktop = Desktop::first_start(themes("BlackMATE", "BlackMATE", "menta"));

    let outcome = desktop.read();

    assert_eq!(
        writes(&outcome),
        [
            "org.mate.Caminito colour-scheme='dark'",
            "org.mate.Caminito light-gtk-theme='Menta'",
            "org.mate.Caminito light-marco-theme='Menta'",
            "org.mate.Caminito light-icon-theme='menta'",
            "org.mate.Caminito dark-gtk-theme='BlackMATE'",
            "org.mate.Caminito dark-marco-theme='BlackMATE'",
            "org.mate.Caminito dark-icon-theme='menta'",
        ]
    );
    assert_eq!(outcome.prefer_dark, Some(false));
}

#[test]
fn the_first_start_waits_until_the_themes_darkness_is_known() {
    let mut desktop = Desktop::first_start(themes("Nordic", "Nordic", "Papirus"));

    assert_eq!(desktop.read_with(|_| None), Outcome::default());

    let outcome = desktop.read_with(|theme| (theme == "Nordic").then_some(ColourScheme::Dark));
    assert_eq!(desktop.keys.colour_scheme, Some(ColourScheme::Dark));
    assert_eq!(desktop.keys.dark, themes("Nordic", "Nordic", "Papirus"));
    assert_eq!(desktop.keys.light, themes("Nordic", "Nordic", "Papirus"));
    assert_eq!(outcome.prefer_dark, Some(true));
}

#[test]
fn an_unknown_theme_gets_the_prefer_dark_fallback_as_its_dark_appearance() {
    let mut desktop = Desktop::first_start(themes("Pop", "Pop", "Pop"));
    let seeded = desktop.read_with(|_| Some(ColourScheme::Light));
    assert_eq!(seeded.prefer_dark, Some(false));

    desktop.set_scheme(ColourScheme::Dark);
    let outcome = desktop.read();

    assert_eq!(desktop.keys.dark, themes("Pop", "Pop", "Pop"));
    assert_eq!(outcome.writes, []);
    assert_eq!(outcome.prefer_dark, Some(true));
}

#[test]
fn a_dark_appearance_of_its_own_has_prefer_dark_written_off_again_on_the_switch() {
    let (light, dark) = breeze();
    let mut desktop = Desktop::light(light, dark);

    desktop.set_scheme(ColourScheme::Dark);
    let outcome = desktop.read();

    // Written again, as another tool may have changed the file since.
    assert_eq!(outcome.prefer_dark, Some(false));
}

#[test]
fn prefer_dark_isnt_written_again_without_a_change() {
    let (light, dark) = breeze();
    let mut desktop = Desktop::light(light, dark);

    desktop.mate.gtk = "Adwaita".into();

    assert_eq!(desktop.read().prefer_dark, None);
}

#[test]
fn an_unset_appearance_is_filled_in_from_the_table() {
    let desktop = Desktop::light(themes("Yaru", "Menta", "Yaru"), Themes::default());

    assert_eq!(
        desktop.keys.dark,
        themes("Yaru-dark", "BlackMATE", "Yaru-dark")
    );
}

#[test]
fn an_unset_light_appearance_is_filled_in_from_the_dark_one() {
    let dark = themes("Arc-Dark", "Arc-Dark", "Papirus-Dark");
    let mut desktop = Desktop {
        mate: dark.clone(),
        keys: AppearanceKeys {
            colour_scheme: Some(ColourScheme::Dark),
            light: Themes::default(),
            dark,
        },
        appearances: Appearances::default(),
    };
    desktop.read();

    assert_eq!(desktop.keys.light, themes("Arc", "Arc", "Papirus"));
}

#[test]
fn the_table_knows_which_side_of_a_pair_a_theme_is() {
    assert_eq!(known_scheme("BlackMATE"), Some(ColourScheme::Dark));
    assert_eq!(known_scheme("Menta"), Some(ColourScheme::Light));
    assert_eq!(known_scheme("Yaru-dark"), Some(ColourScheme::Dark));
    assert_eq!(known_scheme("Nordic"), None);
}

#[test]
fn a_theme_is_dark_when_its_background_is_darker_than_its_text() {
    let adwaita = Palette::neutral();

    assert!(!palette_is_dark(&adwaita.default));
    assert!(palette_is_dark(&adwaita.dark));
}

#[test]
fn prefer_dark_replaces_the_key_and_keeps_the_rest_of_the_file() {
    let text = "# mine\n[Settings]\ngtk-theme-name=Breeze\ngtk-application-prefer-dark-theme = false\n\n[Other]\nx=1\n";

    assert_eq!(
        with_prefer_dark(text, true),
        "# mine\n[Settings]\ngtk-theme-name=Breeze\ngtk-application-prefer-dark-theme=true\n\n[Other]\nx=1\n"
    );
}

#[test]
fn prefer_dark_is_added_to_the_settings_group() {
    let text = "[Other]\nx=1\n[Settings]\ngtk-theme-name=Breeze\n";

    assert_eq!(
        with_prefer_dark(text, false),
        "[Other]\nx=1\n[Settings]\ngtk-application-prefer-dark-theme=false\ngtk-theme-name=Breeze\n"
    );
}

#[test]
fn prefer_dark_adds_the_settings_group_to_a_file_without_it() {
    assert_eq!(
        with_prefer_dark("", true),
        "[Settings]\ngtk-application-prefer-dark-theme=true\n"
    );
    assert_eq!(
        with_prefer_dark("[Other]\nx=1\n", true),
        "[Other]\nx=1\n\n[Settings]\ngtk-application-prefer-dark-theme=true\n"
    );
}

#[test]
fn prefer_dark_is_written_into_a_new_file_and_its_directory() {
    let home = tempfile::tempdir().unwrap();
    let path = home.path().join("gtk-3.0/settings.ini");

    write_prefer_dark(&path, true).unwrap();
    write_prefer_dark(&path, false).unwrap();

    assert_eq!(
        std::fs::read_to_string(&path).unwrap(),
        "[Settings]\ngtk-application-prefer-dark-theme=false\n"
    );
}
