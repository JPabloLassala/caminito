//! The states a window maps with, kept while it's Pending (189): each is the window's once
//! it maps, all applied as one.

use super::*;
use crate::test_window::Call;
use crate::{ActivationSource, FocusClaim, StateRequest};

/// Maps window 2 with `states` over window 1, focused.
fn map_with(states: MapStates) -> Policy<TestWindow> {
    let mut policy = policy();
    policy.map_window(TestWindow::new(1, 400, 300), ms(1));
    policy.map_window_with(TestWindow::new(2, 400, 300), states, ms(2));
    policy
}

#[test]
fn a_window_that_maps_kept_above_is_above() {
    let policy = map_with(MapStates {
        above: true,
        ..MapStates::default()
    });
    assert!(policy.is_above(id(2)));
    assert_eq!(policy.layer(id(2)), Some(Layer::Top));
}

#[test]
fn a_window_that_maps_kept_below_is_below() {
    let policy = map_with(MapStates {
        below: true,
        ..MapStates::default()
    });
    assert!(policy.is_below(id(2)));
    assert_eq!(policy.stacking(0), [id(2), id(1)]);
}

#[test]
fn a_window_that_maps_sticky_is_on_every_workspace() {
    let policy = map_with(MapStates {
        sticky: true,
        ..MapStates::default()
    });
    assert!(policy.is_sticky(id(2)));
    assert!(policy.stacking(1).contains(&id(2)));
}

#[test]
fn a_window_that_maps_asking_to_be_active_is_activated() {
    let mut policy = policy();
    policy.map_window(TestWindow::new(1, 400, 300), ms(1));
    let active = MapStates {
        active: Some(ActivationSource::Pager),
        ..MapStates::default()
    };
    // On another workspace, where a window that maps isn't focused.
    policy.map_window_with(
        TestWindow::new(2, 400, 300).with_requested_workspace(1),
        active,
        ms(2),
    );
    assert_eq!(policy.active_workspace(), 1);
    assert_eq!(policy.focus(SEAT), Some(id(2)));
    assert!(window(&policy, 2).is_activated());
}

#[test]
fn a_window_that_maps_with_every_state_is_sent_its_geometry_once() {
    let mut policy = policy();
    let every = MapStates {
        maximized: true,
        fullscreen: Some(None),
        above: true,
        below: true,
        sticky: true,
        active: Some(ActivationSource::Pager),
        demands_attention: true,
        focus: FocusClaim::NoToken,
    };
    policy.map_window_with(TestWindow::new(1, 400, 300), every, ms(1));

    let geometries = window(&policy, 1)
        .calls
        .iter()
        .filter(|call| matches!(call, Call::SetGeometry(..)))
        .count();
    assert_eq!(geometries, 1);
    assert!(policy.is_fullscreen(id(1)));
    assert!(policy.is_above(id(1)) && policy.is_below(id(1)) && policy.is_sticky(id(1)));
    assert_eq!(policy.focus(SEAT), Some(id(1)));
}

#[test]
fn a_pending_window_keeps_each_request_and_is_configured_again_only_for_its_size() {
    let source = ActivationSource::Application { asked: None };
    let start = MapStates {
        maximized: true,
        fullscreen: Some(None),
        above: true,
        focus: FocusClaim::NoToken,
        ..MapStates::default()
    };
    let with = |change: fn(&mut MapStates)| {
        let mut states = start;
        change(&mut states);
        states
    };
    let output = Some(OutputId(1));
    let table = [
        (
            StateRequest::Maximized(false),
            with(|s| s.maximized = false),
            true,
        ),
        (StateRequest::Maximized(true), start, true),
        (
            StateRequest::Fullscreen(output),
            with(|s| s.fullscreen = Some(Some(OutputId(1)))),
            true,
        ),
        (
            StateRequest::Unfullscreen,
            with(|s| s.fullscreen = None),
            true,
        ),
        // A window that isn't managed has nothing to minimise.
        (StateRequest::Minimized, start, false),
        (StateRequest::Above(false), with(|s| s.above = false), false),
        (StateRequest::Below(true), with(|s| s.below = true), false),
        (StateRequest::Sticky(true), with(|s| s.sticky = true), false),
        (
            StateRequest::Active(source),
            with(|s| s.active = Some(ActivationSource::Application { asked: None })),
            false,
        ),
        (
            StateRequest::DemandsAttention(true),
            with(|s| s.demands_attention = true),
            false,
        ),
    ];
    for (request, left, configure) in table {
        let mut states = start;
        assert_eq!(states.request(request), configure, "{request:?}");
        assert_eq!(states, left, "{request:?}");
    }
}
