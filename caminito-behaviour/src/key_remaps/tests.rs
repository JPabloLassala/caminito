use std::cell::Cell;
use std::rc::Rc;

use super::*;
use crate::testing::{outside, settings};

fn rule(combination: &str, target: &str) -> Rule {
    Rule {
        combination: Combination::parse(combination).unwrap(),
        steps: parse_target(target).unwrap(),
    }
}

fn group(name: &str, app_ids: &[&str], rules: Vec<Rule>) -> Group {
    Group {
        name: name.to_owned(),
        app_ids: app_ids.iter().map(|id| id.to_string()).collect(),
        no_remaps: false,
        rules,
    }
}

/// All applications' Alt+T and Alt+C, and Terminals' own Alt+C and Alt+H.
fn terminals() -> Remaps {
    Remaps {
        all: vec![rule("Alt+T", "Ctrl+T"), rule("Alt+C", "Ctrl+C")],
        groups: vec![group(
            "Terminals",
            &["org.mate.Terminal", "kitty"],
            vec![rule("Alt+C", "Ctrl+Shift+C"), rule("Alt+H", "nothing")],
        )],
    }
}

#[test]
fn the_remaps_are_all_applications_with_none_until_they_are_set() {
    let dir = tempfile::tempdir().unwrap();
    let keys = KeyRemapKeys::new(settings(dir.path()));
    assert_eq!(keys.remaps(), Remaps::default());
    assert_eq!(keys.template(), "");
}

#[test]
fn remaps_set_are_written_to_the_key_at_once() {
    let dir = tempfile::tempdir().unwrap();
    let settings = settings(dir.path());
    let keys = KeyRemapKeys::new(settings.clone());
    let remaps = Remaps {
        all: vec![rule("Alt+T", "Ctrl+T")],
        groups: vec![Group {
            name: "Terminals".to_owned(),
            app_ids: vec!["kitty".to_owned()],
            no_remaps: false,
            rules: vec![
                rule("Alt+BackSpace", "Shift+Home, BackSpace"),
                rule("Alt+H", "nothing"),
            ],
        }],
    };
    keys.set_remaps(&remaps).unwrap();
    assert_eq!(
        outside(&settings).value("key-remaps").to_string(),
        "[('', @as [], false, [('Alt+T', ['Ctrl+T'])]), \
         ('Terminals', ['kitty'], false, [('Alt+BackSpace', ['Shift+Home', 'BackSpace']), ('Alt+H', [])])]"
    );
    assert_eq!(keys.remaps(), remaps);
}

#[test]
fn a_combination_is_read_in_any_order_and_case_and_written_as_ctrl_alt_shift_super() {
    let combination = Combination::parse("shift + SUPER+ctrl+alt+t").unwrap();
    assert_eq!(combination.to_string(), "Ctrl+Alt+Shift+Super+T");
    assert_eq!(
        combination,
        Combination::parse("Ctrl+Alt+Shift+Super+t").unwrap()
    );
    assert_eq!(
        Combination::parse("Hyper+T"),
        Err(ParseError::UnknownModifier("Hyper".to_owned()))
    );
    assert!(Combination::parse("Alt+").is_err());
}

#[test]
fn a_target_is_a_sequence_or_nothing() {
    let steps = parse_target(" Shift+Home ,BackSpace").unwrap();
    assert_eq!(target_text(&steps), "Shift+Home, BackSpace");
    assert_eq!(parse_target("Nothing").unwrap(), []);
    assert_eq!(target_text(&[]), "nothing");
    assert!(parse_target("Ctrl+T,").is_err());
}

#[test]
fn an_app_id_in_a_group_gets_its_groups_rules_over_all_applications() {
    let remaps = terminals();
    let in_effect = remaps.in_effect(Some("kitty"));
    let rows: Vec<(String, String, Source, Option<String>)> = in_effect
        .iter()
        .map(|row| {
            (
                row.rule.combination.to_string(),
                target_text(&row.rule.steps),
                row.source,
                row.replaces.as_deref().map(target_text),
            )
        })
        .collect();
    assert_eq!(
        rows,
        [
            (
                "Alt+C".into(),
                "Ctrl+Shift+C".into(),
                Source::Group(0),
                Some("Ctrl+C".into())
            ),
            ("Alt+H".into(), "nothing".into(), Source::Group(0), None),
            (
                "Alt+T".into(),
                "Ctrl+T".into(),
                Source::AllApplications,
                None
            ),
        ]
    );
}

#[test]
fn an_app_id_in_no_group_and_no_app_id_get_all_applications() {
    let remaps = terminals();
    for app_id in [Some("firefox"), None] {
        let combinations: Vec<String> = remaps
            .in_effect(app_id)
            .iter()
            .inspect(|row| assert_eq!(row.source, Source::AllApplications))
            .map(|row| row.rule.combination.to_string())
            .collect();
        assert_eq!(combinations, ["Alt+C", "Alt+T"]);
    }
}

#[test]
fn a_group_with_no_remaps_has_none_in_effect() {
    let mut remaps = terminals();
    remaps.set_no_remaps(0, true);
    assert!(remaps.in_effect(Some("kitty")).is_empty());
    assert_eq!(remaps.in_effect(Some("firefox")).len(), 2);
}

#[test]
fn change_for_a_group_adds_all_applications_rule_to_it() {
    let mut remaps = terminals();
    remaps.change_for(0, &Combination::parse("Alt+T").unwrap());
    assert_eq!(
        remaps.groups[0].rules.last(),
        Some(&rule("Alt+T", "Ctrl+T"))
    );
    let alt_t = remaps.in_effect(Some("kitty"));
    let alt_t = alt_t
        .iter()
        .find(|row| row.rule.combination.key == "T")
        .unwrap();
    assert_eq!(alt_t.source, Source::Group(0));
    assert_eq!(remaps.all.len(), 2);
}

#[test]
fn a_rule_set_replaces_the_one_of_its_combination_and_is_removed_by_it() {
    let mut remaps = terminals();
    remaps.set_rule(Scope::Group(0), rule("alt+h", "Ctrl+H"));
    remaps.set_rule(Scope::AllApplications, rule("Alt+W", "Ctrl+W"));
    assert_eq!(remaps.groups[0].rules[1], rule("Alt+H", "Ctrl+H"));
    assert_eq!(remaps.groups[0].rules.len(), 2);
    assert_eq!(remaps.all.last(), Some(&rule("Alt+W", "Ctrl+W")));
    remaps.remove_rule(
        Scope::AllApplications,
        &Combination::parse("Alt+T").unwrap(),
    );
    assert_eq!(
        remaps.all,
        [rule("Alt+C", "Ctrl+C"), rule("Alt+W", "Ctrl+W")]
    );
}

#[test]
fn moving_an_app_id_to_another_group_removes_it_from_its_old_one() {
    let mut remaps = terminals();
    let browsers = remaps.add_group("Browsers");
    remaps.move_app_id("kitty", Some(browsers));
    assert_eq!(remaps.groups[0].app_ids, ["org.mate.Terminal"]);
    assert_eq!(remaps.groups[browsers].app_ids, ["kitty"]);
    assert_eq!(remaps.group_of("kitty"), Some(browsers));
    remaps.move_app_id("kitty", None);
    assert_eq!(remaps.group_of("kitty"), None);
    assert!(remaps.groups[browsers].app_ids.is_empty());
}

#[test]
fn remaps_set_from_outside_are_heard_and_other_keys_are_not() {
    // GSettings signals a change in the context its Settings were made in.
    let context = gio::glib::MainContext::new();
    context
        .with_thread_default(|| {
            let dir = tempfile::tempdir().unwrap();
            let settings = settings(dir.path());
            let keys = KeyRemapKeys::new(settings.clone());
            let heard = Rc::new(Cell::new(0));
            keys.connect_changed({
                let heard = heard.clone();
                move || heard.set(heard.get() + 1)
            });

            let outside = outside(&settings);
            outside.set_int("workspace-layout-rows", 2).unwrap();
            let value =
                gio::glib::Variant::parse(None, "[('', @as [], false, [('Alt+Q', ['Alt+F4'])])]")
                    .unwrap();
            outside.set_value("key-remaps", &value).unwrap();
            while context.iteration(false) {}

            assert_eq!(heard.get(), 1);
            assert_eq!(keys.remaps().all, [rule("Alt+Q", "Alt+F4")]);
        })
        .unwrap();
}
