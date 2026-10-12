//! A seat's Interaction (187): which of the four it's in the middle of, and that none opens
//! while it has another, as Marco's `meta_display_begin_grab_op` begins no grab over the
//! display's (display.c:3583-3590).

use super::*;
use crate::{Interaction, ResizeEdges, SwitchList};

const OTHER: SeatId = SeatId(1);

const SWITCH: Action = Action::Switch {
    list: SwitchList::Windows,
    backward: false,
};
const CYCLE: Action = Action::Cycle {
    list: SwitchList::Windows,
    backward: false,
};

/// What opens an Interaction.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum Opening {
    PointerMove,
    PointerResize,
    KeyboardMove,
    KeyboardResize,
    Switch,
    Cycle,
    WindowMenu,
    WorkspacePopup,
}

const OPENINGS: [Opening; 8] = [
    Opening::PointerMove,
    Opening::PointerResize,
    Opening::KeyboardMove,
    Opening::KeyboardResize,
    Opening::Switch,
    Opening::Cycle,
    Opening::WindowMenu,
    Opening::WorkspacePopup,
];

impl Opening {
    /// The Interaction it opens.
    fn interaction(self) -> Interaction {
        match self {
            Self::PointerMove | Self::PointerResize | Self::KeyboardMove | Self::KeyboardResize => {
                Interaction::MoveOrResize
            }
            Self::Switch | Self::Cycle => Interaction::WindowSwitcher,
            Self::WindowMenu => Interaction::WindowMenu,
            Self::WorkspacePopup => Interaction::WorkspacePopup,
        }
    }

    fn is_move_or_resize(self) -> bool {
        self.interaction() == Interaction::MoveOrResize
    }

    fn is_window_switcher(self) -> bool {
        self.interaction() == Interaction::WindowSwitcher
    }

    /// Has `seat` open it on `window`, if it has one. Returns whether it opened.
    fn open(self, policy: &mut Policy<TestWindow>, seat: SeatId, window: u64) -> bool {
        let pointer = Point::from((0., 0.));
        match self {
            Self::PointerMove => policy.begin_move(seat, id(window), pointer),
            Self::PointerResize => {
                let edges = ResizeEdges {
                    right: true,
                    ..ResizeEdges::default()
                };
                policy.begin_resize(seat, id(window), edges, pointer)
            }
            Self::KeyboardMove => policy.begin_keyboard_move(seat, id(window)),
            Self::KeyboardResize => policy.begin_keyboard_resize(seat, id(window)),
            Self::Switch | Self::Cycle => {
                let action = if self == Self::Switch { SWITCH } else { CYCLE };
                let before = policy.switcher().cloned();
                policy.action(seat, action, ms(10));
                policy.switcher().cloned() != before
            }
            Self::WindowMenu => policy.open_window_menu(seat, id(window)),
            Self::WorkspacePopup => policy.open_workspace_popup(false),
        }
    }
}

fn four_workspaces() -> Config {
    Config {
        workspace_count: 4,
        ..Config::default()
    }
}

/// Four workspaces and three windows on the first, 3 focused.
fn three() -> Policy<TestWindow> {
    let mut policy = Policy::new(four_workspaces());
    policy.add_output(output(1, 0, 0, 1920, 1080));
    for w in 1..=3 {
        policy.map_window(TestWindow::new(w, 400, 300), ms(w));
    }
    policy
}

#[test]
fn a_seat_with_nothing_open_has_no_interaction() {
    let policy = three();

    assert_eq!(policy.interaction(SEAT), None);
}

#[test]
fn each_interaction_is_the_seats_while_its_open() {
    for opening in OPENINGS {
        let mut policy = three();

        assert!(opening.open(&mut policy, SEAT, 1), "{opening:?}");

        assert_eq!(
            policy.interaction(SEAT),
            Some(opening.interaction()),
            "{opening:?}"
        );
    }
}

#[test]
fn with_each_interaction_open_no_other_opens() {
    for first in OPENINGS {
        for second in OPENINGS {
            if first.interaction() == second.interaction() {
                continue;
            }
            let mut policy = three();
            assert!(first.open(&mut policy, SEAT, 1), "{first:?}");

            assert!(
                !second.open(&mut policy, SEAT, 2),
                "{second:?} over {first:?}"
            );

            assert_eq!(
                policy.interaction(SEAT),
                Some(first.interaction()),
                "{second:?} over {first:?}"
            );
            assert_eq!(policy.grab(SEAT).is_some(), first.is_move_or_resize());
            assert_eq!(policy.switcher().is_some(), first.is_window_switcher());
            assert_eq!(policy.window_menu().is_some(), first == Opening::WindowMenu);
            let popup = policy.workspace_popup().is_some();
            assert_eq!(popup, first == Opening::WorkspacePopup);
        }
    }
}

// The Window switcher, the Window menu and the Workspace popup are one each, and every
// seat's.

#[test]
fn the_window_switcher_the_window_menu_and_the_workspace_popup_are_every_seats() {
    for opening in OPENINGS
        .into_iter()
        .filter(|opening| !opening.is_move_or_resize())
    {
        let mut policy = three();

        assert!(opening.open(&mut policy, SEAT, 1), "{opening:?}");

        assert_eq!(
            policy.interaction(OTHER),
            Some(opening.interaction()),
            "{opening:?}"
        );
    }
}

#[test]
fn a_move_or_resize_is_its_own_seats_alone() {
    let mut policy = three();

    assert!(Opening::PointerMove.open(&mut policy, SEAT, 1));

    assert_eq!(policy.interaction(OTHER), None);
    assert!(Opening::PointerMove.open(&mut policy, OTHER, 2));
    assert_eq!(policy.interaction(SEAT), Some(Interaction::MoveOrResize));
    assert_eq!(policy.interaction(OTHER), Some(Interaction::MoveOrResize));
}

#[test]
fn none_opens_for_a_seat_over_another_seats() {
    for first in OPENINGS {
        for second in OPENINGS {
            // Two seats each move or resize a window of their own.
            if first.is_move_or_resize() && second.is_move_or_resize() {
                continue;
            }
            // A `switch-*` or `cycle-*` action steps the one switcher, whoever's it is.
            if first.is_window_switcher() && second.is_window_switcher() {
                continue;
            }
            // The one menu opens in place of itself.
            if first == Opening::WindowMenu && second == Opening::WindowMenu {
                continue;
            }
            let mut policy = three();
            assert!(first.open(&mut policy, SEAT, 1), "{first:?}");

            assert!(
                !second.open(&mut policy, OTHER, 2),
                "{second:?} over {first:?}"
            );

            assert_eq!(
                policy.interaction(SEAT),
                Some(first.interaction()),
                "{second:?} over {first:?}"
            );
            let others = (!first.is_move_or_resize()).then_some(first.interaction());
            assert_eq!(
                policy.interaction(OTHER),
                others,
                "{second:?} over {first:?}"
            );
        }
    }
}

// What opens in place of itself.

#[test]
fn the_window_menu_opens_in_place_of_itself() {
    let mut policy = three();
    assert!(policy.open_window_menu(SEAT, id(1)));

    assert!(policy.open_window_menu(SEAT, id(2)));

    assert_eq!(policy.window_menu().unwrap().window, id(2));
}

#[test]
fn a_switch_action_steps_the_open_window_switcher() {
    let mut policy = three();
    policy.action(SEAT, SWITCH, ms(10));
    let selected = policy.switcher().unwrap().selected;

    policy.action(SEAT, SWITCH, ms(11));

    assert_ne!(policy.switcher().unwrap().selected, selected);
}

// The Workspace popup.

#[test]
fn the_workspace_popup_remembers_whether_a_move_opened_it() {
    for moves in [false, true] {
        let mut policy = three();

        assert!(policy.open_workspace_popup(moves));

        assert_eq!(policy.workspace_popup().unwrap().moves, moves);
    }
}

#[test]
fn an_open_workspace_popup_stays_as_it_is() {
    let mut policy = three();
    assert!(policy.open_workspace_popup(true));

    assert!(!policy.open_workspace_popup(false));

    assert!(policy.workspace_popup().unwrap().moves);
}

#[test]
fn the_workspace_popup_stays_open_as_the_workspace_changes() {
    let mut policy = three();
    assert!(policy.open_workspace_popup(false));

    policy.action(SEAT, Action::SwitchToWorkspaceRight, ms(10));

    assert_eq!(policy.active_workspace(), 1);
    assert_eq!(policy.interaction(SEAT), Some(Interaction::WorkspacePopup));
}

#[test]
fn closing_the_workspace_popup_leaves_the_seat_with_no_interaction() {
    let mut policy = three();
    assert!(policy.open_workspace_popup(false));

    policy.close_workspace_popup();

    assert_eq!(policy.interaction(SEAT), None);
    assert!(Opening::KeyboardMove.open(&mut policy, SEAT, 1));
}

// The pointer focus modes.

#[test]
fn every_interaction_holds_the_pointer_focus_modes_off() {
    use crate::{Crossing, FocusMode, PointerTarget};

    for opening in OPENINGS {
        let mut policy = three();
        policy.settings_changed(Config {
            focus_mode: FocusMode::Sloppy,
            ..four_workspaces()
        });
        assert!(opening.open(&mut policy, SEAT, 3), "{opening:?}");
        let focus = policy.focus(SEAT);

        let target = PointerTarget::Window(id(2));
        policy.pointer_entered(SEAT, target, Crossing::Motion, ms(20));

        assert_eq!(policy.focus(SEAT), focus, "{opening:?}");
    }
}
