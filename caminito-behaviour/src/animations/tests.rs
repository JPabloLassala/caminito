use std::cell::RefCell;
use std::rc::Rc;

use super::*;
use crate::testing::{outside, settings};

/// MATE's two schemas, with only the keys the page reads, compiled into `dir` and over one
/// backend in memory each.
fn mate(dir: &std::path::Path) -> (gio::Settings, gio::Settings) {
    let xml = r#"<schemalist>
      <schema id="org.mate.Marco.general" path="/org/mate/marco/general/">
        <key name="compositing-manager" type="b"><default>true</default></key>
        <key name="reduced-resources" type="b"><default>false</default></key>
      </schema>
      <schema id="org.mate.interface" path="/org/mate/desktop/interface/">
        <key name="enable-animations" type="b"><default>true</default></key>
      </schema>
    </schemalist>"#;
    std::fs::write(dir.join("mate.gschema.xml"), xml).unwrap();
    let status = std::process::Command::new("glib-compile-schemas")
        .arg(dir)
        .status()
        .unwrap();
    assert!(status.success());
    let source = gio::SettingsSchemaSource::from_directory(dir, None, false).unwrap();
    let open = |id| {
        let schema = source.lookup(id, false).unwrap();
        gio::Settings::new_full(&schema, Some(&gio::memory_settings_backend_new()), None)
    };
    (open("org.mate.Marco.general"), open("org.mate.interface"))
}

#[test]
fn the_popup_animation_is_the_slide_until_it_is_set() {
    let dir = tempfile::tempdir().unwrap();
    let animations = Animations::new(settings(dir.path()), None, None);
    assert_eq!(animations.popup_animation(), PopupAnimation::Slide);
}

#[test]
fn every_popup_animation_is_written_as_the_schemas_nick() {
    let dir = tempfile::tempdir().unwrap();
    let settings = settings(dir.path());
    let animations = Animations::new(settings.clone(), None, None);
    for animation in PopupAnimation::ALL {
        animations.set_popup_animation(animation).unwrap();
        assert_eq!(outside(&settings).string(POPUP_ANIMATION), animation.nick());
        assert_eq!(animations.popup_animation(), animation);
    }
}

#[test]
fn every_switch_is_on_until_it_is_set_and_is_written_to_its_own_key() {
    let dir = tempfile::tempdir().unwrap();
    let settings = settings(dir.path());
    let animations = Animations::new(settings.clone(), None, None);
    for switch in Switch::ALL {
        assert!(animations.is_on(switch), "{switch:?}");
        animations.set(switch, false).unwrap();
        assert!(!outside(&settings).boolean(switch.key()), "{switch:?}");
        animations.set(switch, true).unwrap();
    }
}

#[test]
fn keys_set_from_outside_are_heard_and_other_keys_are_not() {
    // GSettings signals a change in the context its Settings were made in.
    let context = gio::glib::MainContext::new();
    context
        .with_thread_default(|| {
            let dir = tempfile::tempdir().unwrap();
            let settings = settings(dir.path());
            let animations = Animations::new(settings.clone(), None, None);
            let switches = Rc::new(RefCell::new(Vec::new()));
            let popups = Rc::new(RefCell::new(Vec::new()));
            animations.connect_changed({
                let switches = switches.clone();
                move |switch, on| switches.borrow_mut().push((switch, on))
            });
            animations.connect_popup_animation_changed({
                let popups = popups.clone();
                move |animation| popups.borrow_mut().push(animation)
            });

            let outside = outside(&settings);
            outside.set_int("workspace-layout-rows", 2).unwrap();
            outside.set_boolean("maximize-animation", false).unwrap();
            outside.set_string(POPUP_ANIMATION, "fade").unwrap();
            while context.iteration(false) {}

            assert_eq!(*switches.borrow(), [(Switch::Maximise, false)]);
            assert_eq!(*popups.borrow(), [PopupAnimation::Fade]);
        })
        .unwrap();
}

#[test]
fn without_mates_schemas_nothing_blocks_the_animations() {
    let dir = tempfile::tempdir().unwrap();
    let animations = Animations::new(settings(dir.path()), None, None);
    assert_eq!(animations.blocker(), None);
}

#[test]
fn each_of_mates_three_keys_blocks_the_animations() {
    let dir = tempfile::tempdir().unwrap();
    let (marco, interface) = mate(dir.path());
    let dir = tempfile::tempdir().unwrap();
    let animations = Animations::new(
        settings(dir.path()),
        Some(marco.clone()),
        Some(interface.clone()),
    );
    assert_eq!(animations.blocker(), None);

    marco.set_boolean("compositing-manager", false).unwrap();
    assert_eq!(animations.blocker(), Some(Blocker::Compositing));
    marco.reset("compositing-manager");

    marco.set_boolean("reduced-resources", true).unwrap();
    assert_eq!(animations.blocker(), Some(Blocker::ReducedResources));
    marco.reset("reduced-resources");

    interface.set_boolean("enable-animations", false).unwrap();
    assert_eq!(animations.blocker(), Some(Blocker::Interface));
}

#[test]
fn a_change_of_mates_keys_is_heard() {
    let context = gio::glib::MainContext::new();
    context
        .with_thread_default(|| {
            let dir = tempfile::tempdir().unwrap();
            let (marco, interface) = mate(dir.path());
            let dir = tempfile::tempdir().unwrap();
            let animations =
                Animations::new(settings(dir.path()), Some(marco.clone()), Some(interface));
            let heard = Rc::new(RefCell::new(Vec::new()));
            animations.connect_blocker_changed({
                let heard = heard.clone();
                move |blocker| heard.borrow_mut().push(blocker)
            });

            marco.set_boolean("reduced-resources", true).unwrap();
            while context.iteration(false) {}
            marco.set_boolean("reduced-resources", false).unwrap();
            while context.iteration(false) {}

            assert_eq!(*heard.borrow(), [Some(Blocker::ReducedResources), None]);
        })
        .unwrap();
}
