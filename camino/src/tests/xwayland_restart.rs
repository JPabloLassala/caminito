//! Xwayland's restart (42): Camino outlives an Xwayland crash, forgets its windows, and
//! brings it back on the same display with backoff, or gives up and runs without X. What's
//! tested is what happens to XWayland windows, so the tests map those, and xdg ones only to
//! show Camino carries on and to own a selection. Between a crash and the restart there's no
//! X client left to ask, so what Camino holds then is read from its state.

use std::ffi::OsStr;
use std::time::Duration;

use camino_test_client::selection::Selection;
use camino_test_client::x11::XSetting;

use super::fixture::{ClientId, Fixture, Kind, WindowSpec};
use crate::helpers::RestartPolicy;
use crate::input::KeyboardFocus;
use crate::scene::SceneLayer;
use crate::settings::Value;
use crate::settings::translate::INTERFACE;

/// Restarts quick enough to crash through in a test.
fn quick_restarts() -> RestartPolicy {
    RestartPolicy {
        first_delay: Duration::from_millis(10),
        max_delay: Duration::from_millis(40),
        give_up_after: 3,
        window: Duration::from_secs(60),
    }
}

/// The root's `name`, as an X client reads it.
fn root_property(f: &mut Fixture, x: ClientId, name: &'static str) -> String {
    f.x11_call(x, move |x| {
        camino_test_client::x11::Names::new(x).property(x, x.root(), name)
    })
    .unwrap()
}

/// The XSETTINGS `name`, as an X client reads it.
fn xsetting(f: &mut Fixture, x: ClientId, name: &'static str) -> Option<XSetting> {
    let table = f.x11_call(x, |x| x.xsettings()).unwrap()?;
    table.into_iter().find(|(n, _)| n == name).map(|(_, s)| s)
}

#[test]
fn a_crashed_xwayland_leaves_no_xwayland_windows_and_comes_back_on_the_same_display() {
    let mut f = Fixture::with_settings(vec![(
        INTERFACE,
        "gtk-theme".into(),
        Value::Str("BlackMATE".into()),
    )]);
    let output = f.add_output((1920, 1080), 1.0);
    f.restarts_xwayland();
    let x = f.connect(Kind::X11);
    let first = f.map(x, WindowSpec::default());
    assert!(f.has_keyboard(first), "an XWayland window has the keyboard");
    f.map(
        x,
        WindowSpec {
            position: Some((1000, 600)),
            ..WindowSpec::default()
        },
    );
    f.map_override_redirect(x, (100, 100), (50, 50), 0xff0000);
    let display = f.state.camino.xwayland_display().unwrap();
    let names = root_property(&mut f, x, "_NET_DESKTOP_NAMES");

    f.crash_xwayland();

    let camino = &f.state.camino;
    assert_eq!(
        camino.policy.windows().count(),
        0,
        "policy has no XWayland windows"
    );
    assert_eq!(camino.override_redirect_windows().count(), 0);
    let keyboard = camino.seat.get_keyboard().unwrap();
    assert!(
        !matches!(keyboard.current_focus(), Some(KeyboardFocus::X11(_))),
        "no XWayland window has the keyboard"
    );
    let layers = f.state.scene_layers(&output);
    assert!(
        !layers
            .iter()
            .any(|layer| matches!(layer, SceneLayer::Windows | SceneLayer::OverrideRedirect)),
        "the scene shows no XWayland windows: {layers:?}"
    );

    // Well inside the first delay, 500 ms, so Xwayland would be back by now without it.
    f.dispatch_for(Duration::from_millis(200));
    assert!(f.state.camino.xwayland.is_none(), "the restart backs off");
    f.wait_for_xwayland();
    assert_eq!(f.state.camino.xwayland_display(), Some(display));
    assert_eq!(
        f.state.camino.environment.get(OsStr::new("DISPLAY")),
        Some(&format!(":{display}").into())
    );

    let x = f.connect_x11();
    assert_eq!(root_property(&mut f, x, "_NET_DESKTOP_NAMES"), names);
    let supported = root_property(&mut f, x, "_NET_SUPPORTED");
    assert!(supported.contains("_NET_WORKAREA"), "{supported}");
    assert_eq!(
        xsetting(&mut f, x, "Net/ThemeName"),
        Some(XSetting::Str("BlackMATE".into()))
    );
    let window = f.map(x, WindowSpec::default());
    assert!(
        f.policy_id(window).is_some(),
        "a new XWayland window is managed"
    );
}

#[test]
fn xwayland_crashing_again_and_again_as_it_starts_makes_camino_give_up_and_run_without_x() {
    let mut f = Fixture::new();
    f.restarts_xwayland();
    f.state.camino.xwayland_restart.policy = quick_restarts();
    f.add_output((1920, 1080), 1.0);
    f.start_xwayland();

    // Each crash after the first is before the restarted Xwayland is ready, well inside the
    // policy's window.
    f.crash_xwayland();
    for _ in 1..quick_restarts().give_up_after {
        f.crash_xwayland_starting();
    }

    // Longer than any restart the policy would wait for.
    f.dispatch_for(Duration::from_millis(500));
    let camino = &f.state.camino;
    assert!(camino.xwayland.is_none(), "Xwayland isn't started again");
    assert_eq!(camino.environment.get(OsStr::new("DISPLAY")), None);

    let wayland = f.connect(Kind::Xdg);
    let window = f.map(wayland, WindowSpec::default());
    assert!(
        f.policy_id(window).is_some(),
        "Camino still manages windows"
    );
}

#[test]
fn a_restarted_xwayland_is_owed_the_wayland_clipboard() {
    let (mut f, wayland) = Fixture::connected(Kind::Xdg);
    f.restarts_xwayland();
    f.client(wayland).bind_selections();
    let wayland_window = f.map(wayland, WindowSpec::default());
    f.start_xwayland();
    f.client(wayland)
        .set_selection(Selection::Clipboard, "from Wayland");
    f.roundtrip(wayland);

    f.crash_xwayland();
    f.wait_for_xwayland();

    let x = f.connect_x11();
    let x_window = f.map(
        x,
        WindowSpec {
            position: Some((1000, 600)),
            ..WindowSpec::default()
        },
    );
    assert!(!f.geometry(wayland_window).overlaps(f.geometry(x_window)));
    let geometry = f.geometry(x_window);
    let middle = geometry.loc + geometry.size.downscale(2).to_point();
    f.click(middle.x.into(), middle.y.into());
    assert!(f.has_keyboard(x_window), "the XWayland window is focused");
    let window = x_window.x11();
    let pasted = f
        .x11_call(x, move |x| {
            x.convert_selection(window, "CLIPBOARD", "UTF8_STRING")
        })
        .unwrap();
    assert_eq!(pasted.as_deref(), Some(&b"from Wayland"[..]));
}
