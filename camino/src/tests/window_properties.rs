//! The per-window half of Camino's X connection (11, 39): the properties it writes and
//! reads on X windows, and the client messages Smithay drops. They're X properties and
//! requests, so these tests map XWayland windows only.

use camino_test_client::x11::{Names, XWindowSpec};
use insta::assert_snapshot;
use smithay::utils::Point;

use super::fixture::{
    ClientId, ClientWindow, Decoration, Fixture, KEY_LEFTALT, KEY_TAB, Kind, WindowSpec,
};
use crate::settings::Value;
use crate::settings::translate::MARCO_GENERAL;

/// A property of an X window, as `Names` writes it.
fn property(f: &mut Fixture, window: ClientWindow, name: &'static str) -> String {
    let id = window.x11();
    f.x11_call(window.client(), move |x| {
        Names::new(x).property(x, id, name)
    })
    .unwrap()
}

/// Sends a client message about the window to the root, as EWMH's requests go.
fn send(f: &mut Fixture, window: ClientWindow, type_: &'static str, data: [u32; 5]) {
    let id = window.x11();
    f.x11_call(window.client(), move |x| {
        x.send_client_message(id, type_, data)
    })
    .unwrap();
}

#[test]
fn a_window_that_asks_for_desktop_2_before_it_maps_maps_on_the_third_workspace() {
    let (mut f, x) = Fixture::connected(Kind::X11);
    let first = f.map(x, WindowSpec::default());

    let window = f.map(
        x,
        WindowSpec {
            cardinals: vec![("_NET_WM_DESKTOP", vec![2])],
            ..WindowSpec::default()
        },
    );

    let policy = &f.state.camino.policy;
    assert_eq!(policy.workspace_of(f.pid(window)), Some(2));
    assert_eq!(policy.active_workspace(), 0);
    assert_eq!(policy.focus(crate::state::SEAT), Some(f.pid(first)));
}

#[test]
fn a_desktop_message_moves_a_window_and_net_wm_desktop_follows() {
    let (mut f, x) = Fixture::connected(Kind::X11);
    let window = f.map(x, WindowSpec::default());
    assert_snapshot!(property(&mut f, window, "_NET_WM_DESKTOP"), @"_NET_WM_DESKTOP(CARDINAL) = 0");

    // Source indication 2: a pager.
    send(&mut f, window, "_NET_WM_DESKTOP", [1, 2, 0, 0, 0]);
    f.dispatch_until(|f| f.state.camino.policy.workspace_of(f.pid(window)) == Some(1));

    f.sync(window);
    assert_snapshot!(property(&mut f, window, "_NET_WM_DESKTOP"), @"_NET_WM_DESKTOP(CARDINAL) = 1");
}

#[test]
fn every_desktop_before_map_or_by_message_makes_a_window_sticky() {
    let (mut f, x) = Fixture::connected(Kind::X11);
    let sticky = f.map(
        x,
        WindowSpec {
            cardinals: vec![("_NET_WM_DESKTOP", vec![0xFFFF_FFFF])],
            ..WindowSpec::default()
        },
    );
    let other = f.map(x, WindowSpec::default());
    assert!(f.state.camino.policy.is_sticky(f.pid(sticky)));
    assert_eq!(f.state.camino.policy.workspace_of(f.pid(sticky)), Some(0));

    send(&mut f, other, "_NET_WM_DESKTOP", [0xFFFF_FFFF, 2, 0, 0, 0]);
    f.dispatch_until(|f| f.state.camino.policy.is_sticky(f.pid(other)));

    f.sync(other);
    assert_snapshot!(property(&mut f, sticky, "_NET_WM_DESKTOP"), @"_NET_WM_DESKTOP(CARDINAL) = 4294967295");
    assert_snapshot!(property(&mut f, other, "_NET_WM_DESKTOP"), @"_NET_WM_DESKTOP(CARDINAL) = 4294967295");
}

/// `_NET_FRAME_EXTENTS` as policy lays the window out: left, right, top, bottom.
fn policys_extents(f: &Fixture, window: ClientWindow) -> String {
    let e = f.frame_extents(window);
    format!(
        "_NET_FRAME_EXTENTS(CARDINAL) = {}, {}, {}, {}",
        e.left, e.right, e.top, e.bottom
    )
}

#[test]
fn net_frame_extents_follow_the_frame_through_maximise_fullscreen_and_a_theme_switch() {
    let (mut f, x) = Fixture::connected(Kind::X11);
    let window = f.map(x, WindowSpec::default());
    let floating = property(&mut f, window, "_NET_FRAME_EXTENTS");
    assert_eq!(floating, policys_extents(&f, window));
    assert_ne!(f.frame_extents(window), Default::default());

    let id = window.x11();
    f.x11_call(x, move |x| {
        let vert = x.atom("_NET_WM_STATE_MAXIMIZED_VERT")?;
        let horz = x.atom("_NET_WM_STATE_MAXIMIZED_HORZ")?;
        x.send_client_message(id, "_NET_WM_STATE", [1, vert, horz, 1, 0])
    })
    .unwrap();
    f.dispatch_until(|f| f.state.camino.policy.is_maximized(f.pid(window)));
    f.sync(window);
    let maximized = property(&mut f, window, "_NET_FRAME_EXTENTS");
    assert_eq!(maximized, policys_extents(&f, window));
    assert_ne!(maximized, floating);

    f.x11_call(x, move |x| {
        let fullscreen = x.atom("_NET_WM_STATE_FULLSCREEN")?;
        x.send_client_message(id, "_NET_WM_STATE", [1, fullscreen, 0, 1, 0])
    })
    .unwrap();
    f.dispatch_until(|f| f.state.camino.policy.is_fullscreen(f.pid(window)));
    f.sync(window);
    assert_snapshot!(
        property(&mut f, window, "_NET_FRAME_EXTENTS"),
        @"_NET_FRAME_EXTENTS(CARDINAL) = 0, 0, 0, 0"
    );
    f.x11_call(x, move |x| {
        let fullscreen = x.atom("_NET_WM_STATE_FULLSCREEN")?;
        x.send_client_message(id, "_NET_WM_STATE", [0, fullscreen, 0, 1, 0])
    })
    .unwrap();
    f.dispatch_until(|f| !f.state.camino.policy.is_fullscreen(f.pid(window)));

    f.set_setting(MARCO_GENERAL, "theme", Value::Str("Atlanta".into()));
    f.dispatch_until(|f| policys_extents(f, window) != maximized);
    f.sync(window);
    assert_eq!(
        property(&mut f, window, "_NET_FRAME_EXTENTS"),
        policys_extents(&f, window)
    );
}

#[test]
fn a_window_with_motif_decorations_off_has_no_frame_extents() {
    let (mut f, x) = Fixture::connected(Kind::X11);
    let window = f.map(
        x,
        WindowSpec {
            decoration: Decoration::Client,
            ..WindowSpec::default()
        },
    );

    assert_snapshot!(
        property(&mut f, window, "_NET_FRAME_EXTENTS"),
        @"_NET_FRAME_EXTENTS(CARDINAL) = 0, 0, 0, 0"
    );
}

/// Creates an X window without mapping it, asks for its frame extents, and waits for the
/// answer.
fn requested_frame_extents(f: &mut Fixture, x: ClientId, spec: XWindowSpec) -> (u32, String) {
    let window = f
        .x11_call(x, move |x| {
            let window = x.create_window(&spec)?;
            x.send_client_message(window, "_NET_REQUEST_FRAME_EXTENTS", [0; 5])?;
            Ok::<_, x11rb::errors::ReplyOrIdError>(window)
        })
        .unwrap();
    let window = ClientWindow::X11(x, window);
    let mut extents = String::new();
    f.dispatch_until_mut(|f| {
        extents = property(f, window, "_NET_FRAME_EXTENTS");
        !extents.ends_with("unset")
    });
    (window.x11(), extents)
}

#[test]
fn net_request_frame_extents_is_answered_before_map_with_the_frame_the_window_maps_with() {
    let (mut f, x) = Fixture::connected(Kind::X11);
    let spec = XWindowSpec {
        size: (400, 300),
        delete_window: true,
        ..XWindowSpec::default()
    };

    let (window, answered) = requested_frame_extents(&mut f, x, spec);

    f.x11_call(x, move |x| x.map(window)).unwrap();
    let window = ClientWindow::X11(x, window);
    f.dispatch_until(|f| f.policy_id(window).is_some());
    assert_eq!(answered, policys_extents(&f, window));
    assert_ne!(f.frame_extents(window), Default::default());
}

#[test]
fn net_request_frame_extents_is_zero_for_a_window_with_motif_decorations_off() {
    let (mut f, x) = Fixture::connected(Kind::X11);
    let spec = XWindowSpec {
        size: (400, 300),
        decorations: Some(false),
        ..XWindowSpec::default()
    };

    let (_, answered) = requested_frame_extents(&mut f, x, spec);

    assert_snapshot!(answered, @"_NET_FRAME_EXTENTS(CARDINAL) = 0, 0, 0, 0");
}

#[test]
fn net_request_frame_extents_is_zero_for_a_window_that_asks_to_map_fullscreen() {
    let (mut f, x) = Fixture::connected(Kind::X11);
    let spec = XWindowSpec {
        size: (400, 300),
        states: vec!["_NET_WM_STATE_FULLSCREEN"],
        ..XWindowSpec::default()
    };

    let (_, answered) = requested_frame_extents(&mut f, x, spec);

    assert_snapshot!(answered, @"_NET_FRAME_EXTENTS(CARDINAL) = 0, 0, 0, 0");
}

#[test]
fn a_close_window_message_asks_the_window_to_close() {
    let (mut f, x) = Fixture::connected(Kind::X11);
    let window = f.map(x, WindowSpec::default());

    send(&mut f, window, "_NET_CLOSE_WINDOW", [0, 2, 0, 0, 0]);

    f.dispatch_until_mut(|f| f.close_requested(window));
}

#[test]
fn a_moveresize_window_message_moves_and_resizes_the_window_as_a_configure_request_does() {
    let (mut f, x) = Fixture::connected(Kind::X11);
    let window = f.map(x, WindowSpec::default());

    // North-west gravity (1), with x, y, width and height (bits 8 to 11), from a pager (2).
    let flags = 1 | 0xf << 8 | 2 << 12;
    send(
        &mut f,
        window,
        "_NET_MOVERESIZE_WINDOW",
        [flags, 100, 150, 500, 400],
    );
    f.dispatch_until(|f| f.geometry(window).size == (500, 400).into());

    // The frame's corner goes where it asked, as for a `ConfigureRequest`.
    let extents = f.frame_extents(window);
    assert_eq!(
        f.geometry(window).loc,
        (100 + extents.left, 150 + extents.top).into()
    );
}

/// `_NET_WM_MOVERESIZE` with `direction`, from an application (1).
fn moveresize(f: &mut Fixture, window: ClientWindow, direction: u32) {
    send(f, window, "_NET_WM_MOVERESIZE", [0, 0, direction, 0, 1]);
}

/// The window policy is moving or resizing.
fn grabbed(f: &Fixture) -> Option<camino_policy::WindowId> {
    f.state.camino.policy.grab(crate::state::SEAT)
}

#[test]
fn a_keyboard_move_request_moves_the_window_with_the_pointer_until_it_is_cancelled() {
    let (mut f, x) = Fixture::connected(Kind::X11);
    let window = f.map(x, WindowSpec::default());
    let start = f.geometry(window);
    f.pointer_move(f64::from(start.loc.x) + 50., f64::from(start.loc.y) + 50.);

    moveresize(&mut f, window, 10);
    f.dispatch_until(|f| grabbed(f) == Some(f.pid(window)));
    f.pointer_move_by(40., 30.);
    assert_eq!(f.geometry(window).loc, start.loc + Point::from((40, 30)));

    moveresize(&mut f, window, 11);
    f.dispatch_until(|f| grabbed(f).is_none());
    f.pointer_move_by(40., 30.);
    assert_eq!(f.geometry(window).loc, start.loc + Point::from((40, 30)));
}

#[test]
fn a_keyboard_resize_request_resizes_the_window_with_the_pointer_until_a_click() {
    let (mut f, x) = Fixture::connected(Kind::X11);
    let window = f.map(x, WindowSpec::default());
    let start = f.geometry(window);
    f.pointer_move(f64::from(start.loc.x) + 50., f64::from(start.loc.y) + 50.);

    moveresize(&mut f, window, 9);
    f.dispatch_until(|f| grabbed(f) == Some(f.pid(window)));
    f.pointer_move_by(40., 30.);
    f.draw_at_asked_size(window, (440, 330));
    assert_eq!(f.geometry(window).loc, start.loc);

    f.pointer_button(super::fixture::BTN_LEFT, true);
    f.pointer_button(super::fixture::BTN_LEFT, false);
    assert_eq!(grabbed(&f), None);
}

/// A 16 × 16 `_NET_WM_ICON` of one ARGB colour.
fn icon(argb: u32) -> Vec<u32> {
    let mut icon = vec![16, 16];
    icon.extend(std::iter::repeat_n(argb, 16 * 16));
    icon
}

#[test]
fn net_wm_icon_becomes_the_frame_icon_at_map_and_when_it_changes() {
    let (mut f, x) = Fixture::connected(Kind::X11);
    let window = f.map(
        x,
        WindowSpec {
            app_id: Some("pluma"),
            cardinals: vec![("_NET_WM_ICON", icon(0xffff_0000))],
            ..WindowSpec::default()
        },
    );
    assert_eq!(f.frame_icon_color(window), Some(0xffff_0000));

    let id = window.x11();
    f.x11_call(x, move |x| {
        x.set_cardinals(id, "_NET_WM_ICON", &icon(0xff00_00ff))
    })
    .unwrap();

    f.dispatch_until(|f| f.frame_icon_color(window) == Some(0xff00_00ff));
}

/// The debug dump's `x11` entry for a window.
fn dumped_x11(f: &Fixture, window: ClientWindow) -> String {
    let dump: serde_json::Value = serde_json::from_str(&f.state.camino.state_dump()).unwrap();
    let id = f.pid(window).0;
    let entry = dump["windows"]
        .as_array()
        .unwrap()
        .iter()
        .find(|entry| entry["id"] == id)
        .expect("the window is dumped");
    let mut x11 = entry["x11"].clone();
    x11["window"] = "[X id]".into();
    serde_json::to_string_pretty(&x11).unwrap()
}

#[test]
fn struts_are_read_at_map_and_when_they_change_and_the_dump_shows_them() {
    let (mut f, x) = Fixture::connected(Kind::X11);
    // A panel along the top of the output's left half.
    let partial = vec![0, 0, 30, 0, 0, 0, 0, 0, 0, 959, 0, 0];
    let window = f.map(
        x,
        WindowSpec {
            types: vec!["_NET_WM_WINDOW_TYPE_DOCK"],
            cardinals: vec![("_NET_WM_STRUT_PARTIAL", partial)],
            ..WindowSpec::default()
        },
    );
    assert_snapshot!(dumped_x11(&f, window), @r#"
    {
      "frame_extents": {
        "bottom": 0,
        "left": 0,
        "right": 0,
        "top": 0
      },
      "struts": [
        {
          "side": "top",
          "span": [
            0,
            959
          ],
          "width": 30
        }
      ],
      "window": "[X id]",
      "window_type": "Dock"
    }
    "#);

    // `_NET_WM_STRUT` counts only without `_NET_WM_STRUT_PARTIAL`.
    let id = window.x11();
    f.x11_call(x, move |x| {
        x.delete_property(id, "_NET_WM_STRUT_PARTIAL")?;
        x.set_cardinals(id, "_NET_WM_STRUT", &[0, 0, 0, 40])
    })
    .unwrap();
    f.dispatch_until(|f| dumped_x11(f, window).contains(r#""side": "bottom""#));
    assert_snapshot!(dumped_x11(&f, window), @r#"
    {
      "frame_extents": {
        "bottom": 0,
        "left": 0,
        "right": 0,
        "top": 0
      },
      "struts": [
        {
          "side": "bottom",
          "span": null,
          "width": 40
        }
      ],
      "window": "[X id]",
      "window_type": "Dock"
    }
    "#);
}

#[test]
fn the_dump_shows_an_x_windows_frame_extents_as_published() {
    let (mut f, x) = Fixture::connected(Kind::X11);
    let window = f.map(x, WindowSpec::default());

    assert_snapshot!(dumped_x11(&f, window), @r#"
    {
      "frame_extents": {
        "bottom": 1,
        "left": 1,
        "right": 1,
        "top": 31
      },
      "struts": [],
      "window": "[X id]",
      "window_type": "Normal"
    }
    "#);
    assert!(dumped_x11(&f, window).contains(&format!("\"top\": {}", f.frame_extents(window).top)));
}

#[test]
fn a_click_beneath_another_windows_input_shaped_shadow_reaches_the_window_beneath() {
    let (mut f, x) = Fixture::connected(Kind::X11);
    // Client-side decorated, so each window's geometry is its whole X window.
    let csd_at = |position| WindowSpec {
        decoration: Decoration::Client,
        position: Some(position),
        ..WindowSpec::default()
    };
    let beneath = f.map(x, csd_at((100, 100)));
    // Its input shape leaves out a 50 pixel shadow on the left, over the window beneath.
    let id = f
        .x11_call(x, |x| {
            let window = x.create_window(&XWindowSpec {
                size: (400, 300),
                position: Some((300, 100)),
                decorations: Some(false),
                ..XWindowSpec::default()
            })?;
            x.set_input_shape(window, &[(50, 0, 350, 300)])?;
            x.map(window)?;
            Ok::<_, x11rb::errors::ReplyOrIdError>(window)
        })
        .unwrap();
    let shadowed = ClientWindow::X11(x, id);
    f.dispatch_until(|f| f.policy_id(shadowed).is_some());
    f.sync(shadowed);
    assert_eq!(f.geometry(shadowed).loc, Point::from((300, 100)));

    f.click(320., 200.);
    assert_eq!(focused(&f), Some(f.pid(beneath)));

    // Raised again, and with the shadow in its input shape, the same click is its own.
    f.click(600., 200.);
    f.x11_call(x, move |x| x.set_input_shape(id, &[(0, 0, 400, 300)]))
        .unwrap();
    f.sync(shadowed);
    f.click(320., 200.);
    assert_eq!(focused(&f), Some(f.pid(shadowed)));
}

fn focused(f: &Fixture) -> Option<camino_policy::WindowId> {
    f.state.camino.policy.focus(crate::state::SEAT)
}

/// Sends a `_NET_WM_STATE` request: `action` 0 removes, 1 adds.
fn net_wm_state(f: &mut Fixture, window: ClientWindow, action: u32, state: &'static str) {
    let id = window.x11();
    f.x11_call(window.client(), move |x| {
        let state = x.atom(state)?;
        x.send_client_message(id, "_NET_WM_STATE", [action, state, 0, 1, 0])
    })
    .unwrap();
}

#[test]
fn a_minimised_window_is_iconic_and_hidden_until_it_is_restored() {
    let (mut f, x) = Fixture::connected(Kind::X11);
    let window = f.map(x, WindowSpec::default());

    // ICCCM's request to iconify.
    send(&mut f, window, "WM_CHANGE_STATE", [3, 0, 0, 0, 0]);
    f.dispatch_until(|f| f.state.camino.policy.is_minimized(f.pid(window)));
    f.sync(window);
    assert_snapshot!(property(&mut f, window, "WM_STATE"), @"WM_STATE(WM_STATE) = 3, 0");
    assert!(property(&mut f, window, "_NET_WM_STATE").contains("_NET_WM_STATE_HIDDEN"));

    // Alt+Tab brings it back.
    f.chord(&[KEY_LEFTALT, KEY_TAB]);
    assert!(!f.state.camino.policy.is_minimized(f.pid(window)));
    f.sync(window);
    assert_snapshot!(property(&mut f, window, "WM_STATE"), @"WM_STATE(WM_STATE) = 1, 0");
    assert!(!property(&mut f, window, "_NET_WM_STATE").contains("_NET_WM_STATE_HIDDEN"));
}

#[test]
fn above_below_and_sticky_go_back_into_net_wm_state() {
    let (mut f, x) = Fixture::connected(Kind::X11);
    let above = f.map(x, WindowSpec::default());
    let below = f.map(x, WindowSpec::default());

    net_wm_state(&mut f, above, 1, "_NET_WM_STATE_ABOVE");
    net_wm_state(&mut f, below, 1, "_NET_WM_STATE_BELOW");
    send(&mut f, below, "_NET_WM_DESKTOP", [0xFFFF_FFFF, 2, 0, 0, 0]);
    f.dispatch_until(|f| f.state.camino.policy.is_sticky(f.pid(below)));
    f.sync(below);

    let states = |f: &mut Fixture, window| {
        let states = property(f, window, "_NET_WM_STATE");
        ["ABOVE", "BELOW", "STICKY"]
            .into_iter()
            .filter(|state| {
                states.contains(&format!("_NET_WM_STATE_{state},"))
                    || states.ends_with(&format!("_NET_WM_STATE_{state}"))
            })
            .collect::<Vec<_>>()
    };
    assert_eq!(states(&mut f, above), ["ABOVE"]);
    assert_eq!(states(&mut f, below), ["BELOW", "STICKY"]);

    net_wm_state(&mut f, above, 0, "_NET_WM_STATE_ABOVE");
    f.dispatch_until(|f| !f.state.camino.policy.is_above(f.pid(above)));
    f.sync(above);
    assert!(states(&mut f, above).is_empty());
}

/// The debug dump's skip-taskbar and skip-pager for a window.
fn dumped_skips(f: &Fixture, window: ClientWindow) -> (bool, bool) {
    let dump: serde_json::Value = serde_json::from_str(&f.state.camino.state_dump()).unwrap();
    let id = f.pid(window).0;
    let entry = dump["windows"]
        .as_array()
        .unwrap()
        .iter()
        .find(|entry| entry["id"] == id)
        .expect("the window is dumped")
        .clone();
    (
        entry["skip_taskbar"].as_bool().unwrap(),
        entry["skip_pager"].as_bool().unwrap(),
    )
}

#[test]
fn skip_taskbar_and_skip_pager_change_after_map_without_being_written_back() {
    let (mut f, x) = Fixture::connected(Kind::X11);
    let window = f.map(x, WindowSpec::default());
    assert_eq!(dumped_skips(&f, window), (false, false));

    let id = window.x11();
    f.x11_call(x, move |x| {
        let taskbar = x.atom("_NET_WM_STATE_SKIP_TASKBAR")?;
        let pager = x.atom("_NET_WM_STATE_SKIP_PAGER")?;
        x.send_client_message(id, "_NET_WM_STATE", [1, taskbar, pager, 1, 0])
    })
    .unwrap();
    f.dispatch_until(|f| dumped_skips(f, window) == (true, true));

    // Toggled, one at a time.
    net_wm_state(&mut f, window, 2, "_NET_WM_STATE_SKIP_PAGER");
    f.dispatch_until(|f| dumped_skips(f, window) == (true, false));

    f.sync(window);
    assert!(!property(&mut f, window, "_NET_WM_STATE").contains("SKIP"));
}

#[test]
fn a_withdrawn_window_loses_its_net_wm_desktop() {
    let (mut f, x) = Fixture::connected(Kind::X11);
    let window = f.map(x, WindowSpec::default());
    assert_snapshot!(property(&mut f, window, "_NET_WM_DESKTOP"), @"_NET_WM_DESKTOP(CARDINAL) = 0");

    let id = window.x11();
    f.x11_call(x, move |x| x.unmap(id)).unwrap();
    f.dispatch_until(|f| f.policy_id(window).is_none());

    f.x11_call(x, |x| x.sync()).unwrap();
    assert_snapshot!(property(&mut f, window, "_NET_WM_DESKTOP"), @"_NET_WM_DESKTOP unset");
}

/// Xwayland gives an override-redirect window's input shape to its own `wl_surface`, so
/// Camino reads none for it. A managed window's surface is Smithay's frame window's, which
/// has no shape, hence Camino's reading.
#[test]
fn the_pointer_goes_through_an_override_redirect_windows_input_shaped_shadow() {
    let (mut f, x) = Fixture::connected(Kind::X11);
    let window = f.map(
        x,
        WindowSpec {
            decoration: Decoration::Client,
            position: Some((100, 100)),
            ..WindowSpec::default()
        },
    );
    // A menu whose input shape leaves out a 100 pixel shadow on its left.
    let menu = f.map_override_redirect(x, (50, 150), (200, 100), 0x0000ff);
    f.x11_call(x, move |x| x.set_input_shape(menu, &[(100, 0, 100, 100)]))
        .unwrap();
    f.dispatch();

    f.pointer_move(120., 200.);
    assert_eq!(f.override_redirect_under_pointer(), None);
    assert_eq!(
        f.state.camino.window_under((120., 200.).into()),
        Some(f.pid(window))
    );

    f.pointer_move(200., 200.);
    assert_eq!(f.override_redirect_under_pointer(), Some(menu));
}
