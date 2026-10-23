use std::cell::RefCell;
use std::rc::Rc;

use super::*;
use crate::testing::{outside, settings};

#[test]
fn every_corner_is_off_until_it_is_set() {
    let dir = tempfile::tempdir().unwrap();
    let corners = HotCorners::new(settings(dir.path()));
    for corner in Corner::ALL {
        assert_eq!(corners.action(corner), Action::None);
    }
}

#[test]
fn a_corner_set_is_written_to_its_key_at_once() {
    let dir = tempfile::tempdir().unwrap();
    let settings = settings(dir.path());
    let corners = HotCorners::new(settings.clone());
    corners.set(Corner::TopRight, Action::Lock).unwrap();
    let outside = outside(&settings);
    assert_eq!(outside.string("hot-corner-top-right"), "lock");
    assert_eq!(outside.string("hot-corner-top-left"), "none");
    assert_eq!(corners.action(Corner::TopRight), Action::Lock);
}

#[test]
fn a_corner_set_from_outside_is_heard_and_other_keys_are_not() {
    // GSettings signals a change in the context its Settings were made in.
    let context = gio::glib::MainContext::new();
    context
        .with_thread_default(|| {
            let dir = tempfile::tempdir().unwrap();
            let settings = settings(dir.path());
            let corners = HotCorners::new(settings.clone());
            let heard = Rc::new(RefCell::new(Vec::new()));
            corners.connect_changed({
                let heard = heard.clone();
                move |corner, action| heard.borrow_mut().push((corner, action))
            });

            let outside = outside(&settings);
            outside.set_int("workspace-layout-rows", 2).unwrap();
            outside
                .set_string("hot-corner-bottom-left", "show-desktop")
                .unwrap();
            while context.iteration(false) {}

            assert_eq!(*heard.borrow(), [(Corner::BottomLeft, Action::ShowDesktop)]);
        })
        .unwrap();
}

#[test]
fn the_activation_delay_is_150_ms_until_it_is_set() {
    let dir = tempfile::tempdir().unwrap();
    let corners = HotCorners::new(settings(dir.path()));
    assert_eq!(corners.activation_delay(), 150);
}

#[test]
fn an_activation_delay_set_is_written_to_its_key_at_once() {
    let dir = tempfile::tempdir().unwrap();
    let settings = settings(dir.path());
    let corners = HotCorners::new(settings.clone());
    corners.set_activation_delay(400).unwrap();
    assert_eq!(outside(&settings).uint("hot-corner-activation-delay"), 400);
    assert_eq!(corners.activation_delay(), 400);
}

#[test]
fn an_activation_delay_past_a_second_is_refused() {
    let dir = tempfile::tempdir().unwrap();
    let corners = HotCorners::new(settings(dir.path()));
    assert!(corners.set_activation_delay(1001).is_err());
    assert_eq!(corners.activation_delay(), 150);
}

#[test]
fn an_activation_delay_set_from_outside_is_heard_and_a_corner_is_not() {
    let context = gio::glib::MainContext::new();
    context
        .with_thread_default(|| {
            let dir = tempfile::tempdir().unwrap();
            let settings = settings(dir.path());
            let corners = HotCorners::new(settings.clone());
            let heard = Rc::new(RefCell::new(Vec::new()));
            corners.connect_activation_delay_changed({
                let heard = heard.clone();
                move |delay| heard.borrow_mut().push(delay)
            });

            let outside = outside(&settings);
            outside.set_string("hot-corner-top-left", "lock").unwrap();
            outside
                .set_uint("hot-corner-activation-delay", 250)
                .unwrap();
            while context.iteration(false) {}

            assert_eq!(*heard.borrow(), [250]);
        })
        .unwrap();
}

#[test]
fn a_corner_is_on_once_one_has_an_action() {
    let dir = tempfile::tempdir().unwrap();
    let corners = HotCorners::new(settings(dir.path()));
    assert!(!corners.any_on());
    corners.set(Corner::BottomRight, Action::Overview).unwrap();
    assert!(corners.any_on());
}
