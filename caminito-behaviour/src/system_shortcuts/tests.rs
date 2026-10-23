use super::*;
use crate::testing::settings;

fn combination(text: &str) -> Combination {
    Combination::parse(text).unwrap()
}

#[test]
fn marcos_accelerators_are_read_as_combinations() {
    let read = |text| accelerator(text).map(|combination| combination.to_string());
    assert_eq!(read("<Alt>F4").as_deref(), Some("Alt+F4"));
    assert_eq!(read("<Mod1>Tab").as_deref(), Some("Alt+Tab"));
    assert_eq!(read("<Control><Alt>Left").as_deref(), Some("Ctrl+Alt+Left"));
    assert_eq!(read("<Primary><Shift>d").as_deref(), Some("Ctrl+Shift+D"));
    assert_eq!(read("<Super>w").as_deref(), Some("Super+W"));
    assert_eq!(read("<Mod4>Up").as_deref(), Some("Super+Up"));
    assert_eq!(read("disabled"), None);
    assert_eq!(read(""), None);
    assert_eq!(read("<Hyper>a"), None);
}

#[test]
fn a_combination_that_is_a_system_shortcut_is_named() {
    let shortcuts = SystemShortcuts::from_accelerators([
        ("Close window".to_owned(), "<Alt>F4".to_owned()),
        ("Nothing".to_owned(), "disabled".to_owned()),
    ]);
    assert_eq!(
        shortcuts.named(&combination("Alt+F4")),
        Some("Close window")
    );
    assert_eq!(shortcuts.named(&combination("Alt+T")), None);
}

#[test]
fn caminitos_overview_shortcut_is_read_with_its_summary() {
    let dir = tempfile::tempdir().unwrap();
    let settings = settings(dir.path());
    settings
        .set_string("application-overview", "<Super>a")
        .unwrap();
    let mut shortcuts = SystemShortcuts::default();
    shortcuts.add(&settings, CAMINITO_KEYS);
    assert_eq!(
        shortcuts.named(&combination("Super+W")),
        Some("Show the Overview")
    );
    assert_eq!(
        shortcuts.named(&combination("Super+A")),
        Some("Show the Application overview")
    );
}
