use std::cell::RefCell;
use std::rc::Rc;

use super::*;
use crate::testing::{outside, settings};

#[test]
fn fullscreen_workspaces_are_off_until_they_are_turned_on() {
    let dir = tempfile::tempdir().unwrap();
    let settings = settings(dir.path());
    let windows = Windows::new(settings.clone());
    assert!(!windows.fullscreen_workspaces());

    windows.set_fullscreen_workspaces(true).unwrap();

    assert!(outside(&settings).boolean("fullscreen-workspaces"));
    assert!(windows.fullscreen_workspaces());
}

#[test]
fn the_key_set_from_outside_is_heard_and_other_keys_are_not() {
    // GSettings signals a change in the context its Settings were made in.
    let context = gio::glib::MainContext::new();
    context
        .with_thread_default(|| {
            let dir = tempfile::tempdir().unwrap();
            let settings = settings(dir.path());
            let windows = Windows::new(settings.clone());
            let heard = Rc::new(RefCell::new(Vec::new()));
            windows.connect_changed({
                let heard = heard.clone();
                move |on| heard.borrow_mut().push(on)
            });

            let outside = outside(&settings);
            outside.set_boolean("esmeril", false).unwrap();
            outside.set_boolean("fullscreen-workspaces", true).unwrap();
            outside.set_boolean("fullscreen-workspaces", false).unwrap();
            while context.iteration(false) {}

            assert_eq!(*heard.borrow(), [true, false]);
        })
        .unwrap();
}
