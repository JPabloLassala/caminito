use std::fs;

use super::*;
use crate::key_remaps::{Combination, Group, Rule, parse_target};

/// The spec's example.
const EXAMPLE: &str = "\
# name: macOS (PC keyboard)
# Comments start with #.
[All applications]
Alt+T = Ctrl+T
Alt+BackSpace = Shift+Home, BackSpace
[Terminals] org.mate.Terminal kitty Alacritty
Alt+C = Ctrl+Shift+C
Alt+H = nothing
[Remote desktops] org.remmina.Remmina virt-manager
no remaps
";

fn rule(combination: &str, target: &str) -> Rule {
    Rule {
        combination: Combination::parse(combination).unwrap(),
        steps: parse_target(target).unwrap(),
    }
}

fn example() -> Remaps {
    Remaps {
        all: vec![
            rule("Alt+T", "Ctrl+T"),
            rule("Alt+BackSpace", "Shift+Home, BackSpace"),
        ],
        groups: vec![
            Group {
                name: "Terminals".to_owned(),
                app_ids: vec![
                    "org.mate.Terminal".into(),
                    "kitty".into(),
                    "Alacritty".into(),
                ],
                no_remaps: false,
                rules: vec![rule("Alt+C", "Ctrl+Shift+C"), rule("Alt+H", "nothing")],
            },
            Group {
                name: "Remote desktops".to_owned(),
                app_ids: vec!["org.remmina.Remmina".into(), "virt-manager".into()],
                no_remaps: true,
                rules: Vec::new(),
            },
        ],
    }
}

/// A directory of shipped templates and one of the user's, the user's not made yet.
struct Dirs {
    _root: tempfile::TempDir,
    shipped: PathBuf,
    user: PathBuf,
}

fn dirs() -> Dirs {
    let root = tempfile::tempdir().unwrap();
    let shipped = root.path().join("usr/share/caminito/remap-templates");
    let user = root
        .path()
        .join("home/.local/share/caminito/remap-templates");
    fs::create_dir_all(&shipped).unwrap();
    fs::write(shipped.join("macos-pc.remaps"), EXAMPLE).unwrap();
    fs::write(
        shipped.join("no-remaps.remaps"),
        include_str!("../../../data/remap-templates/no-remaps.remaps"),
    )
    .unwrap();
    Dirs {
        _root: root,
        shipped,
        user,
    }
}

#[test]
fn the_specs_example_parses() {
    let (name, remaps) = parse(EXAMPLE).unwrap();
    assert_eq!(name.as_deref(), Some("macOS (PC keyboard)"));
    assert_eq!(remaps, example());
}

#[test]
fn a_template_written_and_read_back_gives_the_same_remaps() {
    let mut remaps = example();
    // A group with no remaps keeps its rules, as the key does.
    remaps.groups[1].rules.push(rule("Super+Left", "Ctrl+Left"));
    let text = write("Mine", &remaps);
    assert_eq!(parse(&text).unwrap(), (Some("Mine".to_owned()), remaps));
}

#[test]
fn a_line_that_is_no_rule_is_refused_with_its_number() {
    let err = parse("# name: Bad\n[All applications]\nAlt+T Ctrl+T\n").unwrap_err();
    assert_eq!(err.line, 3);
    let err = parse("[Terminals] kitty\nAlt+T = Hyper+T\n").unwrap_err();
    assert_eq!(err.line, 2);
}

#[test]
fn a_name_is_slugged_for_its_file() {
    assert_eq!(slug("macOS (PC keyboard)"), "macos-pc-keyboard");
    assert_eq!(slug("  Line keys, only!"), "line-keys-only");
}

#[test]
fn the_templates_are_caminitos_then_the_users() {
    let dirs = dirs();
    let templates = Templates::new(dirs.shipped.clone(), dirs.user.clone());
    templates.save_as("Line keys", &Remaps::default()).unwrap();
    let listed: Vec<(String, bool)> = templates
        .list()
        .0
        .into_iter()
        .map(|template| (template.name, template.shipped))
        .collect();
    assert_eq!(
        listed,
        [
            ("macOS (PC keyboard)".to_owned(), true),
            ("No remaps".to_owned(), true),
            ("Line keys".to_owned(), false),
        ]
    );
}

#[test]
fn save_as_writes_the_users_file_of_its_slug() {
    let dirs = dirs();
    let templates = Templates::new(dirs.shipped.clone(), dirs.user.clone());
    let saved = templates.save_as("Line keys", &example()).unwrap();
    assert_eq!(saved.path, dirs.user.join("line-keys.remaps"));
    assert!(!saved.shipped);
    let text = fs::read_to_string(&saved.path).unwrap();
    assert_eq!(
        parse(&text).unwrap(),
        (Some("Line keys".to_owned()), example())
    );
}

#[test]
fn a_template_that_comes_with_caminito_cant_be_overwritten_or_deleted() {
    let dirs = dirs();
    let templates = Templates::new(dirs.shipped.clone(), dirs.user.clone());
    assert!(matches!(
        templates.save_as("no remaps", &example()),
        Err(Error::Shipped(_))
    ));
    assert!(!dirs.user.exists());
    let shipped = templates.list().0.remove(0);
    assert!(matches!(templates.delete(&shipped), Err(Error::Shipped(_))));
    assert!(shipped.path.exists());
}

#[test]
fn the_users_template_is_deleted() {
    let dirs = dirs();
    let templates = Templates::new(dirs.shipped.clone(), dirs.user.clone());
    let saved = templates.save_as("Line keys", &example()).unwrap();
    templates.delete(&saved).unwrap();
    assert!(!saved.path.exists());
    assert_eq!(templates.list().0.len(), 2);
}

#[test]
fn a_file_that_cant_be_read_is_reported_and_the_others_listed() {
    let dirs = dirs();
    fs::write(dirs.shipped.join("broken.remaps"), "Alt+T\n").unwrap();
    fs::write(dirs.shipped.join("notes.txt"), "not a template").unwrap();
    let templates = Templates::new(dirs.shipped.clone(), dirs.user.clone());
    let (listed, problems) = templates.list();
    assert_eq!(listed.len(), 2);
    assert_eq!(problems.len(), 1);
}

#[test]
fn the_remaps_in_use_are_found_in_the_template_that_holds_them() {
    let dirs = dirs();
    let templates = Templates::new(dirs.shipped.clone(), dirs.user.clone());
    let listed = templates.list().0;
    assert_eq!(
        holding(&listed, &Remaps::default()).map(|template| template.name.as_str()),
        Some("No remaps")
    );
    let mut changed = example();
    changed.all.pop();
    assert!(holding(&listed, &changed).is_none());
}

/// The macOS template, as tools/toshy-to-remaps writes it from Toshy's config (CMN-190).
const MACOS: &str = include_str!("../../../data/remap-templates/macos-pc.remaps");

#[test]
fn the_macos_template_has_toshys_shortcuts_on_a_pc_keyboard() {
    let (name, remaps) = parse(MACOS).unwrap();
    assert_eq!(name.as_deref(), Some("macOS (PC keyboard)"));
    let target = |scope, combination| {
        let combination = Combination::parse(combination).unwrap();
        let rule = remaps
            .rules(scope)
            .iter()
            .find(|rule| rule.combination == combination);
        rule.map(|rule| target_text(&rule.steps))
    };
    let all = Scope::AllApplications;
    assert_eq!(target(all, "Alt+T").as_deref(), Some("Ctrl+T"));
    assert_eq!(target(all, "Alt+Right").as_deref(), Some("End"));
    assert_eq!(
        target(all, "Alt+BackSpace").as_deref(),
        Some("Shift+Home, BackSpace")
    );
    let terminals = remaps.group_of("mate-terminal").unwrap();
    assert_eq!(remaps.groups[terminals].name, "Terminals");
    assert_eq!(
        target(Scope::Group(terminals), "Alt+C").as_deref(),
        Some("Ctrl+Shift+C")
    );
    let remote = remaps.group_of("org.remmina.Remmina").unwrap();
    assert!(remaps.groups[remote].no_remaps);
}

#[test]
fn the_macos_template_is_listed_with_caminitos_with_no_problem() {
    let dirs = dirs();
    fs::write(dirs.shipped.join("macos-pc.remaps"), MACOS).unwrap();
    let templates = Templates::new(dirs.shipped.clone(), dirs.user.clone());
    let (listed, problems) = templates.list();
    assert!(problems.is_empty(), "{problems:?}");
    assert!(
        listed
            .iter()
            .any(|t| t.name == "macOS (PC keyboard)" && t.shipped)
    );
}
