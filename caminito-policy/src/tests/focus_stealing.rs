//! `focus-new-windows` on xdg-activation tokens, urgency, and `alt-tab-urgent-placement`
//! (165); and the rules XWayland's windows add: a transient for the focused window,
//! `_NET_ACTIVE_WINDOW`'s source indications, and the urgency a client asks for (166).

use super::*;
use crate::{
    ActivationSource, ActivationToken, FocusClaim, FocusNewWindows, MapStates, SwitchList,
    TOKEN_LIFETIME, UrgentPlacement,
};

const SMART: FocusNewWindows = FocusNewWindows::Smart;
const STRICT: FocusNewWindows = FocusNewWindows::Strict;

/// `policy()` with `focus-new-windows` as `mode`, and window 1 of the process 100 mapped and
/// focused at 1 ms.
fn focused_other_client(mode: FocusNewWindows) -> Policy<TestWindow> {
    let mut policy = policy();
    policy.settings_changed(Config {
        focus_new_windows: mode,
        ..Config::default()
    });
    policy.map_window(TestWindow::new(1, 400, 300).with_pid(100), ms(1));
    assert_eq!(policy.focus(SEAT), Some(id(1)));
    policy
}

/// Maps a window of the process `pid` with `claim`.
fn map_claiming(policy: &mut Policy<TestWindow>, w: u64, pid: u32, claim: FocusClaim, now: u64) {
    let states = MapStates {
        focus: claim,
        ..MapStates::default()
    };
    policy.map_window_with(TestWindow::new(w, 400, 300).with_pid(pid), states, ms(now));
}

fn token(issued: u64, input: bool) -> FocusClaim {
    FocusClaim::Token(ActivationToken {
        issued: ms(issued),
        input,
    })
}

/// Whether window 2 took the focus from window 1, checking that a refused window is urgent
/// and under the focused one, and that a focused one is neither.
fn second_took_focus(policy: &Policy<TestWindow>) -> bool {
    let took = policy.focus(SEAT) == Some(id(2));
    if took {
        assert!(!policy.is_urgent(id(2)));
        assert_eq!(policy.stacking(0), [id(1), id(2)]);
    } else {
        assert_eq!(policy.focus(SEAT), Some(id(1)));
        assert!(policy.is_urgent(id(2)));
        assert_eq!(policy.stacking(0), [id(2), id(1)]);
    }
    took
}

const LATER: u64 = 2_000;

#[test]
fn a_recent_token_gives_the_focus_under_both_modes() {
    for mode in [SMART, STRICT] {
        let mut policy = focused_other_client(mode);
        map_claiming(&mut policy, 2, 200, token(LATER - 100, true), LATER);
        assert!(second_took_focus(&policy), "{mode:?}");
    }
}

/// Maps the process 200's first window on another workspace, so that it isn't focused and
/// the next window of the process is its second.
fn map_first_elsewhere(policy: &mut Policy<TestWindow>) {
    policy.map_window(
        TestWindow::new(3, 400, 300)
            .with_pid(200)
            .with_requested_workspace(1),
        ms(10),
    );
}

#[test]
fn an_old_token_gives_no_focus_to_a_second_window() {
    let old = LATER + TOKEN_LIFETIME.as_millis() as u64 + 1;
    for mode in [SMART, STRICT] {
        let mut policy = focused_other_client(mode);
        map_first_elsewhere(&mut policy);
        map_claiming(&mut policy, 2, 200, token(LATER, true), old);
        assert!(!second_took_focus(&policy), "{mode:?}");
    }
}

#[test]
fn a_token_older_than_the_last_focus_change_gives_no_focus() {
    let mut policy = focused_other_client(SMART);
    policy.map_window(TestWindow::new(3, 400, 300).with_pid(200), ms(10));
    // The user went back to window 1 after the token was issued.
    policy.click(SEAT, id(1), ms(LATER));
    map_claiming(&mut policy, 2, 200, token(LATER - 100, true), LATER + 100);

    assert_eq!(policy.focus(SEAT), Some(id(1)));
    assert!(policy.is_urgent(id(2)));
}

#[test]
fn a_token_with_no_input_serial_gives_the_focus_under_smart_only() {
    let mut smart = focused_other_client(SMART);
    map_claiming(&mut smart, 2, 200, token(LATER - 100, false), LATER);
    assert!(second_took_focus(&smart));

    let mut strict = focused_other_client(STRICT);
    map_claiming(&mut strict, 2, 200, token(LATER - 100, false), LATER);
    assert!(!second_took_focus(&strict));
}

#[test]
fn a_first_window_with_no_token_takes_the_focus_under_smart_only() {
    let mut smart = focused_other_client(SMART);
    map_claiming(&mut smart, 2, 200, FocusClaim::NoToken, LATER);
    assert!(second_took_focus(&smart));

    let mut strict = focused_other_client(STRICT);
    map_claiming(&mut strict, 2, 200, FocusClaim::NoToken, LATER);
    assert!(!second_took_focus(&strict));
}

#[test]
fn a_second_window_with_no_token_is_refused_under_both_modes() {
    for mode in [SMART, STRICT] {
        let mut policy = focused_other_client(mode);
        map_first_elsewhere(&mut policy);
        map_claiming(&mut policy, 2, 200, FocusClaim::NoToken, LATER);
        assert!(!second_took_focus(&policy), "{mode:?}");
    }
}

#[test]
fn a_window_of_the_focused_client_takes_the_focus_with_no_token_under_both_modes() {
    for mode in [SMART, STRICT] {
        let mut policy = focused_other_client(mode);
        map_claiming(&mut policy, 2, 100, FocusClaim::NoToken, LATER);
        assert!(second_took_focus(&policy), "{mode:?}");
    }
}

#[test]
fn a_window_mapped_while_nothing_has_the_focus_takes_it_under_both_modes() {
    for mode in [SMART, STRICT] {
        let mut policy = policy();
        policy.settings_changed(Config {
            focus_new_windows: mode,
            ..Config::default()
        });
        policy.map_window(
            TestWindow::new(1, 400, 300)
                .with_pid(100)
                .with_requested_workspace(1),
            ms(1),
        );
        map_claiming(&mut policy, 2, 100, FocusClaim::NoToken, LATER);
        assert_eq!(policy.focus(SEAT), Some(id(2)), "{mode:?}");
    }
}

#[test]
fn an_unchecked_window_takes_the_focus_under_strict() {
    let mut policy = focused_other_client(STRICT);
    map_claiming(&mut policy, 2, 200, FocusClaim::Unchecked, LATER);
    assert!(second_took_focus(&policy));
}

#[test]
fn urgency_clears_when_the_window_is_focused() {
    let mut policy = focused_other_client(STRICT);
    map_claiming(&mut policy, 2, 200, FocusClaim::NoToken, LATER);
    assert!(policy.is_urgent(id(2)));

    policy.click(SEAT, id(2), ms(LATER + 10));

    assert!(!policy.is_urgent(id(2)));
}

#[test]
fn an_activation_with_a_token_that_counts_activates_the_window() {
    let mut policy = focused_other_client(STRICT);
    policy.map_window(TestWindow::new(2, 400, 300).with_pid(100), ms(10));
    policy.click(SEAT, id(1), ms(20));
    let token = ActivationToken {
        issued: ms(LATER - 100),
        input: true,
    };

    policy.request_activation(SEAT, id(2), Some(token), ms(LATER));

    assert_eq!(policy.focus(SEAT), Some(id(2)));
    assert_eq!(policy.stacking(0), [id(1), id(2)]);
    assert!(!policy.is_urgent(id(2)));
}

#[test]
fn an_activation_refused_makes_the_window_urgent_and_leaves_the_stacking() {
    let old = LATER + TOKEN_LIFETIME.as_millis() as u64 + 1;
    let no_input = Some((LATER - 100, false));
    for (mode, token, now) in [
        (STRICT, no_input, LATER),
        (SMART, Some((LATER, true)), old),
        (SMART, None, LATER),
    ] {
        let mut policy = focused_other_client(mode);
        policy.map_window(TestWindow::new(2, 400, 300).with_pid(100), ms(10));
        policy.click(SEAT, id(1), ms(20));
        let token = token.map(|(issued, input)| ActivationToken {
            issued: ms(issued),
            input,
        });

        policy.request_activation(SEAT, id(2), token, ms(now));

        assert_eq!(policy.focus(SEAT), Some(id(1)), "{mode:?} {token:?}");
        assert_eq!(policy.stacking(0), [id(2), id(1)]);
        assert!(policy.is_urgent(id(2)));
    }
}

#[test]
fn an_activation_of_the_focused_window_does_not_make_it_urgent() {
    let mut policy = focused_other_client(STRICT);

    policy.request_activation(SEAT, id(1), None, ms(LATER));

    assert!(!policy.is_urgent(id(1)));
}

/// Windows 1 to 3 focused in turn, then window 4 of another process refused the focus
/// under *strict*, and window 2 focused last.
fn urgent_fourth(placement: UrgentPlacement) -> Policy<TestWindow> {
    let mut policy = policy();
    policy.settings_changed(Config {
        focus_new_windows: STRICT,
        urgent_placement: placement,
        ..Config::default()
    });
    for w in 1..=3 {
        policy.map_window(TestWindow::new(w, 400, 300).with_pid(100), ms(w));
    }
    map_claiming(&mut policy, 4, 200, FocusClaim::NoToken, 10);
    policy.click(SEAT, id(2), ms(20));
    assert!(policy.is_urgent(id(4)));
    policy
}

#[test]
fn urgent_placement_start_lists_an_urgent_window_first() {
    let policy = urgent_fourth(UrgentPlacement::Start);
    assert_eq!(
        policy.switch_list(SEAT, SwitchList::Windows),
        [id(4), id(2), id(3), id(1)]
    );
}

/// `mru`, and Marco 1.29.0 with no key, which `Config::default` stands for: the urgent window,
/// never focused, comes after those that were, as 108 has it.
#[test]
fn urgent_placement_mru_and_no_key_keep_an_urgent_window_in_its_place() {
    for policy in [
        urgent_fourth(UrgentPlacement::Mru),
        urgent_fourth(Config::default().urgent_placement),
    ] {
        assert_eq!(
            policy.switch_list(SEAT, SwitchList::Windows),
            [id(2), id(3), id(1), id(4)]
        );
    }
}

#[test]
fn urgent_placement_start_still_lists_a_minimised_urgent_window_last() {
    let mut policy = urgent_fourth(UrgentPlacement::Start);
    policy.minimize(id(4));
    assert_eq!(
        policy.switch_list(SEAT, SwitchList::Windows),
        [id(2), id(3), id(1), id(4)]
    );
}

/// A window refused the focus while the Window switcher raises its selection stays under the
/// focused window as the switcher steps and closes.
#[test]
fn a_window_refused_while_the_switcher_raises_windows_stays_under_the_focused_one() {
    let mut policy = policy();
    policy.settings_changed(Config {
        alt_tab_raise_windows: true,
        ..Config::default()
    });
    for (w, pid) in [(1, 100), (2, 200), (3, 100)] {
        policy.map_window(TestWindow::new(w, 400, 300).with_pid(pid), ms(w));
    }
    let switch = Action::Switch {
        list: SwitchList::Windows,
        backward: false,
    };
    policy.action(SEAT, switch, ms(10));
    map_claiming(&mut policy, 4, 200, FocusClaim::NoToken, 11);
    assert!(policy.is_urgent(id(4)));
    assert_eq!(policy.stacking(0), [id(1), id(4), id(3), id(2)]);

    policy.switcher_step(false);
    assert_eq!(policy.stacking(0), [id(2), id(4), id(3), id(1)]);
    policy.switcher_cancel(ms(12));
    assert_eq!(policy.focus(SEAT), Some(id(3)));
    assert_eq!(policy.stacking(0), [id(1), id(2), id(4), id(3)]);
}

// XWayland's windows, which carry no token (166).

/// `focused_other_client` with window 3 of the process 200 mapped too, and window 1 focused
/// again, so that the next window of the process 200 is its second.
fn focused_beside_another_client(mode: FocusNewWindows) -> Policy<TestWindow> {
    let mut policy = focused_other_client(mode);
    policy.map_window(TestWindow::new(3, 400, 300).with_pid(200), ms(10));
    policy.click(SEAT, id(1), ms(20));
    assert_eq!(policy.focus(SEAT), Some(id(1)));
    policy
}

/// Maps window 2 of the process 200, transient for `parent`, with no token.
fn map_transient(policy: &mut Policy<TestWindow>, parent: u64) {
    let states = MapStates {
        focus: FocusClaim::NoToken,
        ..MapStates::default()
    };
    let window = TestWindow::new(2, 300, 200)
        .with_pid(200)
        .with_parent(parent);
    policy.map_window_with(window, states, ms(LATER));
}

#[test]
fn a_transient_for_the_focused_window_takes_the_focus_with_no_token_under_both_modes() {
    for mode in [SMART, STRICT] {
        let mut policy = focused_beside_another_client(mode);

        map_transient(&mut policy, 1);

        assert_eq!(policy.focus(SEAT), Some(id(2)), "{mode:?}");
        assert!(!policy.is_urgent(id(2)), "{mode:?}");
    }
}

#[test]
fn a_transient_for_a_window_that_is_not_focused_is_refused_under_both_modes() {
    for mode in [SMART, STRICT] {
        let mut policy = focused_beside_another_client(mode);

        map_transient(&mut policy, 3);

        assert_eq!(policy.focus(SEAT), Some(id(1)), "{mode:?}");
        assert!(policy.is_urgent(id(2)), "{mode:?}");
    }
}

#[test]
fn an_activation_by_a_pager_activates_the_window_under_both_modes() {
    for mode in [SMART, STRICT] {
        let mut policy = focused_beside_another_client(mode);

        policy.request_activation_from(SEAT, id(3), ActivationSource::Pager, ms(LATER));

        assert_eq!(policy.focus(SEAT), Some(id(3)), "{mode:?}");
        assert_eq!(policy.stacking(0), [id(1), id(3)], "{mode:?}");
        assert!(!policy.is_urgent(id(3)), "{mode:?}");
    }
}

#[test]
fn an_activation_by_the_focused_windows_client_activates_the_window() {
    for mode in [SMART, STRICT] {
        let mut policy = focused_other_client(mode);
        policy.map_window(TestWindow::new(2, 400, 300).with_pid(100), ms(10));
        policy.click(SEAT, id(1), ms(20));

        policy.request_activation_from(SEAT, id(2), ActivationSource::Application, ms(LATER));

        assert_eq!(policy.focus(SEAT), Some(id(2)), "{mode:?}");
        assert_eq!(policy.stacking(0), [id(1), id(2)], "{mode:?}");
        assert!(!policy.is_urgent(id(2)), "{mode:?}");
    }
}

#[test]
fn an_activation_by_another_client_makes_the_window_urgent_and_leaves_the_stacking() {
    for mode in [SMART, STRICT] {
        let mut policy = focused_beside_another_client(mode);
        let stacking = policy.stacking(0);

        policy.request_activation_from(SEAT, id(3), ActivationSource::Application, ms(LATER));

        assert_eq!(policy.focus(SEAT), Some(id(1)), "{mode:?}");
        assert_eq!(policy.stacking(0), stacking, "{mode:?}");
        assert!(policy.is_urgent(id(3)), "{mode:?}");
    }
}

#[test]
fn an_activation_by_a_client_with_no_pid_is_refused() {
    let mut policy = focused_other_client(SMART);
    policy.map_window(TestWindow::new(2, 400, 300), ms(10));
    policy.click(SEAT, id(1), ms(20));
    // Window 1 with no pid either isn't the same client's.
    policy.window_mut(id(1)).unwrap().pid = None;

    policy.request_activation_from(SEAT, id(2), ActivationSource::Application, ms(LATER));

    assert_eq!(policy.focus(SEAT), Some(id(1)));
    assert!(policy.is_urgent(id(2)));
}

#[test]
fn an_activation_by_an_application_while_nothing_has_the_focus_activates_the_window() {
    let mut policy = focused_other_client(STRICT);
    policy.map_window(TestWindow::new(2, 400, 300).with_pid(200), ms(10));
    policy.minimize(id(1));
    policy.minimize(id(2));
    assert_eq!(policy.focus(SEAT), None);

    policy.request_activation_from(SEAT, id(2), ActivationSource::Application, ms(LATER));

    assert_eq!(policy.focus(SEAT), Some(id(2)));
    assert!(!policy.is_urgent(id(2)));
}

#[test]
fn an_activation_by_an_application_of_the_focused_window_does_not_make_it_urgent() {
    let mut policy = focused_other_client(SMART);
    policy.window_mut(id(1)).unwrap().pid = None;

    policy.request_activation_from(SEAT, id(1), ActivationSource::Application, ms(LATER));

    assert!(!policy.is_urgent(id(1)));
}

#[test]
fn a_client_asks_for_attention_and_takes_the_ask_back() {
    let mut policy = focused_beside_another_client(SMART);
    assert!(!policy.demands_attention(id(3)));

    policy.set_demands_attention(id(3), true);
    assert!(policy.demands_attention(id(3)));
    assert!(policy.is_urgent(id(3)));

    policy.set_demands_attention(id(3), false);
    assert!(!policy.demands_attention(id(3)));
    assert!(!policy.is_urgent(id(3)));
}

#[test]
fn the_urgency_policy_sets_is_the_windows_demand_for_attention() {
    let mut refused = focused_other_client(STRICT);
    map_claiming(&mut refused, 2, 200, FocusClaim::NoToken, LATER);
    assert!(refused.demands_attention(id(2)));

    let mut asked = focused_beside_another_client(SMART);
    asked.request_activation_from(SEAT, id(3), ActivationSource::Application, ms(LATER));
    assert!(asked.demands_attention(id(3)));
}

#[test]
fn a_client_takes_back_the_urgency_policy_set() {
    let mut policy = focused_other_client(STRICT);
    map_claiming(&mut policy, 2, 200, FocusClaim::NoToken, LATER);

    policy.set_demands_attention(id(2), false);

    assert!(!policy.is_urgent(id(2)));
}

#[test]
fn a_demand_for_attention_clears_when_the_window_is_focused() {
    let mut policy = focused_beside_another_client(SMART);
    policy.set_demands_attention(id(3), true);

    policy.click(SEAT, id(3), ms(LATER));

    assert!(!policy.demands_attention(id(3)));
    assert!(!policy.is_urgent(id(3)));
}

#[test]
fn the_focused_window_does_not_demand_attention() {
    let mut policy = focused_other_client(SMART);

    policy.set_demands_attention(id(1), true);

    assert!(!policy.demands_attention(id(1)));
}
