use smithay::input::keyboard::Keysym;

use super::*;

fn combination(text: &str) -> Combination {
    Combination::parse(text).unwrap()
}

/// A group of `key-remaps`, its rules as `("Alt+T", &["Ctrl+T"])`.
fn group(app_ids: &[&str], no_remaps: bool, rules: &[(&str, &[&str])]) -> RemapGroupValue {
    let rules = rules
        .iter()
        .map(|(trigger, steps)| {
            let steps = steps.iter().map(|step| (*step).to_owned()).collect();
            ((*trigger).to_owned(), steps)
        })
        .collect();
    let app_ids = app_ids.iter().map(|id| (*id).to_owned()).collect();
    (String::new(), app_ids, no_remaps, rules)
}

fn remaps(groups: &[RemapGroupValue]) -> KeyRemaps {
    let (remaps, unremappable) = KeyRemaps::from_groups(groups);
    assert_eq!(unremappable, []);
    remaps
}

fn steps<'a>(
    remaps: &'a KeyRemaps,
    app_id: Option<&str>,
    pressed: &str,
) -> Option<&'a [Combination]> {
    remaps.steps_for(app_id, &combination(pressed))
}

#[test]
fn a_combination_is_modifiers_in_any_order_and_case_and_a_key_by_its_first_level() {
    let held = Held {
        ctrl: true,
        shift: true,
        ..Held::NOTHING
    };
    assert_eq!(
        combination("shift+CTRL+T"),
        Combination {
            held,
            key: Keysym::t
        }
    );
    assert_eq!(combination("Ctrl+Shift+t"), combination("Shift+Ctrl+T"));
    assert_eq!(combination("BackSpace").key, Keysym::BackSpace);
    assert_eq!(combination("Alt+BracketLeft").key, Keysym::bracketleft);
    assert!(combination("Super+Left").held.logo);
}

#[test]
fn a_combination_that_cant_be_parsed_says_why() {
    assert_eq!(Combination::parse("Alt+"), Err(ParseError::NoKey));
    assert_eq!(
        Combination::parse("Hyper+T"),
        Err(ParseError::UnknownModifier("Hyper".into()))
    );
    assert_eq!(
        Combination::parse("Alt+NoSuchKey"),
        Err(ParseError::UnknownKey("NoSuchKey".into()))
    );
}

#[test]
fn all_applications_rules_apply_to_every_app_id_and_to_no_app_id() {
    let remaps = remaps(&[group(&[], false, &[("Alt+T", &["Ctrl+T"])])]);
    let target = [combination("Ctrl+T")];
    assert_eq!(steps(&remaps, Some("firefox"), "Alt+T"), Some(&target[..]));
    assert_eq!(steps(&remaps, None, "Alt+T"), Some(&target[..]));
    assert_eq!(steps(&remaps, None, "Ctrl+T"), None);
    assert_eq!(steps(&remaps, None, "Alt+Shift+T"), None);
}

#[test]
fn a_groups_rule_replaces_all_applications_for_its_app_ids_alone() {
    let remaps = remaps(&[
        group(
            &[],
            false,
            &[("Alt+C", &["Ctrl+C"]), ("Alt+T", &["Ctrl+T"])],
        ),
        group(
            &["kitty", "org.mate.Terminal"],
            false,
            &[("Alt+C", &["Ctrl+Shift+C"])],
        ),
    ]);
    let terminal = [combination("Ctrl+Shift+C")];
    let all = [combination("Ctrl+C")];
    assert_eq!(steps(&remaps, Some("kitty"), "Alt+C"), Some(&terminal[..]));
    assert_eq!(
        steps(&remaps, Some("org.mate.Terminal"), "Alt+C"),
        Some(&terminal[..])
    );
    assert_eq!(steps(&remaps, Some("firefox"), "Alt+C"), Some(&all[..]));
    // What the group has no rule for is All applications'.
    let tab = [combination("Ctrl+T")];
    assert_eq!(steps(&remaps, Some("kitty"), "Alt+T"), Some(&tab[..]));
}

#[test]
fn a_group_with_no_remaps_has_none_of_all_applications() {
    let remaps = remaps(&[
        group(&[], false, &[("Alt+T", &["Ctrl+T"])]),
        group(&["org.remmina.Remmina"], true, &[("Alt+W", &["Ctrl+W"])]),
    ]);
    assert_eq!(steps(&remaps, Some("org.remmina.Remmina"), "Alt+T"), None);
    assert_eq!(steps(&remaps, Some("org.remmina.Remmina"), "Alt+W"), None);
}

#[test]
fn a_rule_to_nothing_has_no_steps() {
    let remaps = remaps(&[group(&[], false, &[("Alt+H", &[])])]);
    assert_eq!(steps(&remaps, None, "Alt+H"), Some(&[][..]));
}

#[test]
fn a_rule_that_cant_be_parsed_is_left_out_and_the_others_kept() {
    let (remaps, unremappable) = KeyRemaps::from_groups(&[group(
        &[],
        false,
        &[
            ("Alt+NoSuchKey", &["Ctrl+T"]),
            ("Alt+W", &["Hyper+W"]),
            ("Alt+T", &["Ctrl+T"]),
        ],
    )]);
    assert_eq!(
        unremappable,
        [
            Unremappable {
                rule: "Alt+NoSuchKey = Ctrl+T".into(),
                reason: ParseError::UnknownKey("NoSuchKey".into()),
            },
            Unremappable {
                rule: "Alt+W = Hyper+W".into(),
                reason: ParseError::UnknownModifier("Hyper".into()),
            },
        ]
    );
    assert_eq!(steps(&remaps, None, "Alt+W"), None);
    assert!(steps(&remaps, None, "Alt+T").is_some());
}

#[test]
fn no_groups_or_no_rules_are_no_remaps() {
    assert!(remaps(&[]).is_empty());
    assert!(remaps(&[group(&[], false, &[]), group(&["kitty"], true, &[])]).is_empty());
    assert!(
        !remaps(&[
            group(&[], false, &[]),
            group(&["kitty"], false, &[("Alt+C", &["Ctrl+C"])])
        ])
        .is_empty()
    );
}
