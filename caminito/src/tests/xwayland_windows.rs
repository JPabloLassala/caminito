//! XWayland windows managed like native ones (11, 35), through a real Xwayland and x11rb.
//! What they check is X's side of management, so they map X windows only (45).

use caminito_policy::{PolicyWindow, WindowId};
use caminito_test_client::x11::XWindowSpec;
use smithay::utils::{Logical, Point, Rectangle};

use super::fixture::{
    BTN_LEFT, ClientId, ClientWindow, Decoration, Fixture, KEY_LEFT, KEY_LEFTALT, KEY_LEFTCTRL,
    KEY_RIGHT, Kind, WindowSpec,
};

fn spec(w: i32, h: i32) -> WindowSpec {
    WindowSpec {
        size: (w, h),
        ..WindowSpec::default()
    }
}

fn center(rect: Rectangle<i32, Logical>) -> Point<f64, Logical> {
    rect.to_f64().loc + rect.to_f64().size.downscale(2.).to_point()
}

/// Where the X client sees its window.
fn x_position(f: &mut Fixture, window: ClientWindow) -> (i32, i32) {
    let id = window.x11();
    f.x11_call(window.client(), move |x| x.position(id))
        .unwrap()
}

/// The X window that has X's input focus, as client `x` sees it.
fn x_focus(f: &mut Fixture, x: ClientId) -> u32 {
    f.x11_call(x, |x| x.input_focus()).unwrap()
}

/// Sends a `_NET_WM_STATE` request, as a client does: `action` 0 removes, 1 adds.
fn net_wm_state(f: &mut Fixture, window: ClientWindow, action: u32, states: [&str; 2]) {
    let states = states.map(str::to_owned);
    let id = window.x11();
    f.x11_call(window.client(), move |x| {
        let first = x.atom(&states[0])?;
        let second = x.atom(&states[1])?;
        x.send_client_message(id, "_NET_WM_STATE", [action, first, second, 1, 0])
    })
    .unwrap();
}

/// The root's `_NET_CLIENT_LIST_STACKING`, bottom to top.
pub(super) fn client_list_stacking(f: &mut Fixture, x: ClientId) -> Vec<u32> {
    let property = f
        .x11_call(x, |x| x.property(x.root(), "_NET_CLIENT_LIST_STACKING"))
        .unwrap();
    match property.map(|p| p.value) {
        Some(caminito_test_client::x11::PropertyValue::Cardinals(ids)) => ids,
        other => panic!("_NET_CLIENT_LIST_STACKING is a WINDOW list: {other:?}"),
    }
}

#[test]
fn a_normal_window_maps_framed_takes_focus_on_click_and_moves() {
    let (mut f, x) = Fixture::connected(Kind::X11);
    let first = f.map(x, spec(400, 300));
    let second = f.map(x, spec(400, 300));
    assert!(f.mapped(first).shows_frame());
    assert_eq!(x_focus(&mut f, x), second.x11());

    let point = center(f.geometry(first));
    f.click(point.x, point.y);
    assert_eq!(x_focus(&mut f, x), first.x11());

    // Alt+drag moves it, and the X client is told where it went.
    let before = f.geometry(first).loc;
    f.key(KEY_LEFTALT, true);
    f.pointer_button(BTN_LEFT, true);
    f.pointer_move(point.x + 100., point.y + 50.);
    f.pointer_button(BTN_LEFT, false);
    f.key(KEY_LEFTALT, false);
    let after = f.geometry(first).loc;
    assert_eq!(after, before + Point::from((100, 50)));
    f.sync(first);
    assert_eq!(x_position(&mut f, first), (after.x, after.y));
}

#[test]
fn a_dialog_transient_for_a_window_stacks_above_it_on_its_workspace() {
    let (mut f, x) = Fixture::connected(Kind::X11);
    let parent = f.map(x, spec(600, 400));
    f.chord(&[KEY_LEFTCTRL, KEY_LEFTALT, KEY_RIGHT]);
    assert_eq!(f.state.caminito.policy.active_workspace(), 1);

    let dialog = f.map(
        x,
        WindowSpec {
            parent: Some(parent),
            ..spec(300, 200)
        },
    );

    let policy = &f.state.caminito.policy;
    let (parent_id, dialog_id) = (f.pid(parent), f.pid(dialog));
    assert_eq!(policy.workspace_of(dialog_id), Some(0));
    assert_eq!(policy.parent(dialog_id), Some(parent_id));
    assert_eq!(
        f.mapped(dialog).window_type(),
        caminito_policy::WindowType::Dialog
    );
    assert_eq!(policy.stacking(0), [parent_id, dialog_id]);

    // Raising the parent keeps its dialog above it.
    f.chord(&[KEY_LEFTCTRL, KEY_LEFTALT, KEY_LEFT]);
    let point = center(f.geometry(parent)) - Point::from((250., 150.));
    f.click(point.x, point.y);
    assert_eq!(f.state.caminito.policy.stacking(0), [parent_id, dialog_id]);
}

#[test]
fn a_window_with_us_position_is_placed_where_it_asked() {
    let (mut f, x) = Fixture::connected(Kind::X11);
    let window = f.map(
        x,
        WindowSpec {
            position: Some((300, 200)),
            ..spec(400, 300)
        },
    );

    // Its gravity is north-west, so its frame's corner goes where it asked (ICCCM 4.1.2.3).
    let extents = f.frame_extents(window);
    let at = Point::from((300 + extents.left, 200 + extents.top));
    assert_eq!(f.geometry(window).loc, at);
    f.sync(window);
    assert_eq!(x_position(&mut f, window), (at.x, at.y));
}

#[test]
fn a_configure_request_is_honoured_for_a_floating_window_and_refused_for_a_maximised_one() {
    let (mut f, x) = Fixture::connected(Kind::X11);
    let window = f.map(x, spec(400, 300));

    let id = window.x11();
    f.x11_call(x, move |x| x.configure(id, (100, 150), (500, 400)))
        .unwrap();
    f.dispatch_until(|f| f.geometry(window).size == (500, 400).into());
    let extents = f.frame_extents(window);
    assert_eq!(
        f.geometry(window).loc,
        Point::from((100 + extents.left, 150 + extents.top))
    );

    net_wm_state(
        &mut f,
        window,
        1,
        [
            "_NET_WM_STATE_MAXIMIZED_VERT",
            "_NET_WM_STATE_MAXIMIZED_HORZ",
        ],
    );
    f.dispatch_until(|f| f.state.caminito.policy.is_maximized(f.pid(window)));
    f.dispatch_until(|f| f.geometry(window).size.w > 1800);
    let maximized = f.geometry(window);

    f.x11_call(x, move |x| x.configure(id, (10, 10), (100, 100)))
        .unwrap();
    // Caminito has refused the request once the client is in sync, as Xwayland redirects it
    // to Caminito before it answers the client.
    f.sync(window);
    assert_eq!(f.geometry(window), maximized);
}

/// A launcher such as Albert, undecorated, resizes itself as its list of results grows and
/// shrinks: it's shown at the size it last drew at until it has drawn the new one, as a
/// window being resized is, rather than black where it hasn't drawn yet.
#[test]
fn a_window_that_takes_sync_requests_and_asks_for_a_new_size_keeps_its_size_until_it_has_drawn_it()
 {
    let (mut f, x) = Fixture::connected(Kind::X11);
    let window = f.map(
        x,
        WindowSpec {
            decoration: Decoration::Client,
            sync_request: true,
            ..spec(400, 300)
        },
    );
    let start = f.geometry(window);
    let at = x_position(&mut f, window);

    let id = window.x11();
    f.x11_call(x, move |x| x.configure(id, at, (400, 500)))
        .unwrap();
    f.sync(window);

    assert_eq!(f.x11_call(x, move |x| x.size(id)).unwrap(), (400, 500));
    assert_eq!(f.geometry(window), start);

    let acked = f.x11_call(x, move |x| x.ack_sync_requests(id));
    assert_eq!(acked.unwrap(), 1);
    f.dispatch_until(|f| f.geometry(window).size == (400, 500).into());
    assert_eq!(f.geometry(window).loc, start.loc);
}

#[test]
fn a_window_with_motif_decorations_off_gets_no_frame() {
    let (mut f, x) = Fixture::connected(Kind::X11);
    let window = f.map(
        x,
        WindowSpec {
            decoration: Decoration::Client,
            ..spec(400, 300)
        },
    );

    assert!(!f.mapped(window).is_ssd());
    assert!(!f.mapped(window).shows_frame());
}

#[test]
fn net_client_list_stacking_follows_caminitos_order_after_a_raise() {
    let (mut f, x) = Fixture::connected(Kind::X11);
    let first = f.map(x, spec(400, 300));
    let second = f.map(x, spec(400, 300));
    assert_eq!(client_list_stacking(&mut f, x), [first.x11(), second.x11()]);

    let point = center(f.geometry(first));
    f.click(point.x, point.y);

    assert_eq!(client_list_stacking(&mut f, x), [second.x11(), first.x11()]);
}

#[test]
fn a_window_that_asks_to_be_maximised_before_it_maps_maps_maximised() {
    let (mut f, x) = Fixture::connected(Kind::X11);
    let window = f.map(
        x,
        WindowSpec {
            states: vec![
                "_NET_WM_STATE_MAXIMIZED_VERT",
                "_NET_WM_STATE_MAXIMIZED_HORZ",
            ],
            ..spec(400, 300)
        },
    );

    assert!(f.state.caminito.policy.is_maximized(f.pid(window)));
}

#[test]
fn above_below_and_sticky_asked_for_before_map_are_recorded_at_map() {
    let (mut f, x) = Fixture::connected(Kind::X11);
    let above = f.map(
        x,
        WindowSpec {
            states: vec!["_NET_WM_STATE_ABOVE", "_NET_WM_STATE_STICKY"],
            ..spec(400, 300)
        },
    );
    let below = f.map(
        x,
        WindowSpec {
            states: vec!["_NET_WM_STATE_BELOW"],
            ..spec(400, 300)
        },
    );

    let policy = &f.state.caminito.policy;
    let (above, below) = (f.pid(above), f.pid(below));
    assert!(policy.is_above(above) && policy.is_sticky(above) && !policy.is_below(above));
    assert!(policy.is_below(below) && !policy.is_above(below) && !policy.is_sticky(below));
}

#[test]
fn an_unmap_and_remap_gives_a_new_policy_id() {
    let (mut f, x) = Fixture::connected(Kind::X11);
    let window = f.map(x, spec(400, 300));
    let before = f.pid(window);

    let id = window.x11();
    f.x11_call(x, move |x| x.unmap(id)).unwrap();
    f.dispatch_until(|f| f.policy_id(window).is_none());
    f.x11_call(x, move |x| x.map(id)).unwrap();
    f.dispatch_until(|f| f.policy_id(window).is_some());

    assert_ne!(f.pid(window), before);
}

#[test]
fn a_window_destroyed_while_pending_leaves_no_trace() {
    let (mut f, x) = Fixture::connected(Kind::X11);
    // Pending windows never map, so this one is made by hand.
    let window = f
        .x11_call(x, |x| {
            let window = x.create_window(&XWindowSpec {
                size: (400, 300),
                empty: true,
                ..XWindowSpec::default()
            })?;
            x.map(window)?;
            Ok::<_, x11rb::errors::ReplyOrIdError>(window)
        })
        .unwrap();
    let window = ClientWindow::X11(x, window);
    f.dispatch_until(|f| !f.state.caminito.pending.is_empty());

    let id = window.x11();
    f.x11_call(x, move |x| x.destroy(id)).unwrap();
    f.dispatch_until(|f| f.state.caminito.pending.is_empty());

    assert!(f.policy_id(window).is_none());
    assert_eq!(f.state.caminito.policy.windows().count(), 0);
}

#[test]
fn focusing_a_window_moves_x_input_focus_to_it() {
    let (mut f, x) = Fixture::connected(Kind::X11);
    let first = f.map(x, spec(400, 300));
    let second = f.map(x, spec(400, 300));
    assert_eq!(x_focus(&mut f, x), second.x11());

    let point = center(f.geometry(first));
    f.click(point.x, point.y);

    assert_eq!(x_focus(&mut f, x), first.x11());
}

/// The debug dump's entry for policy's window `id`.
fn dumped(f: &Fixture, id: WindowId) -> serde_json::Value {
    let dump: serde_json::Value = serde_json::from_str(&f.state.caminito.state_dump()).unwrap();
    dump["windows"]
        .as_array()
        .unwrap()
        .iter()
        .find(|window| window["id"] == id.0)
        .expect("the window is dumped")
        .clone()
}

#[test]
fn the_state_dump_shows_an_xwayland_windows_x_id_type_and_skipping() {
    let (mut f, x) = Fixture::connected(Kind::X11);
    let parent = f.map(x, spec(600, 400));
    let dialog = f.map(
        x,
        WindowSpec {
            types: vec!["_NET_WM_WINDOW_TYPE_DIALOG"],
            parent: Some(parent),
            ..spec(300, 200)
        },
    );

    let window = dumped(&f, f.pid(dialog));
    assert_eq!(window["xwayland"], true);
    assert_eq!(window["x11"]["window"], dialog.x11());
    assert_eq!(window["x11"]["window_type"], "Dialog");
    assert_eq!(window["skip_taskbar"], true);
    assert_eq!(window["skip_pager"], false);
    assert_eq!(window["urgent"], false);
}

#[test]
fn a_window_demanding_attention_is_urgent() {
    let (mut f, x) = Fixture::connected(Kind::X11);
    let window = f.map(x, spec(400, 300));
    let id = f.pid(window);
    // A window that has the focus demands none (166).
    f.map(x, spec(400, 300));

    net_wm_state(
        &mut f,
        window,
        1,
        [
            "_NET_WM_STATE_DEMANDS_ATTENTION",
            "_NET_WM_STATE_DEMANDS_ATTENTION",
        ],
    );

    f.dispatch_until(|f| dumped(f, id)["urgent"] == true);
}

#[test]
fn the_title_follows_net_wm_name_live() {
    let (mut f, x) = Fixture::connected(Kind::X11);
    let window = f.map(
        x,
        WindowSpec {
            title: Some("xterm"),
            ..spec(400, 300)
        },
    );
    assert_eq!(f.mapped(window).title(), "xterm");

    let id = window.x11();
    f.x11_call(x, move |x| x.set_string(id, "_NET_WM_NAME", "vim"))
        .unwrap();

    f.dispatch_until(|f| f.mapped(window).title() == "vim");
}

#[test]
fn a_dialog_that_asks_to_be_modal_becomes_a_modal_dialog_and_x_is_told() {
    let (mut f, x) = Fixture::connected(Kind::X11);
    let parent = f.map(x, spec(600, 400));
    let dialog = f.map(
        x,
        WindowSpec {
            parent: Some(parent),
            ..spec(300, 200)
        },
    );

    net_wm_state(
        &mut f,
        dialog,
        1,
        ["_NET_WM_STATE_MODAL", "_NET_WM_STATE_MODAL"],
    );
    f.dispatch_until(|f| {
        f.mapped(dialog).window_type() == caminito_policy::WindowType::ModalDialog
    });

    let id = dialog.x11();
    let states = f
        .x11_call(x, move |x| {
            let mut names = caminito_test_client::x11::Names::new(x);
            names.property(x, id, "_NET_WM_STATE")
        })
        .unwrap();
    assert!(states.contains("_NET_WM_STATE_MODAL"), "{states}");
}

/// `_NET_WM_PID`, as Qt and GTK set it: an application's request to be active is honoured
/// while its process has the focus (166).
fn of_process(pid: u32) -> Vec<(&'static str, Vec<u32>)> {
    vec![("_NET_WM_PID", vec![pid])]
}

/// A launcher such as Albert maps a utility window, which isn't focused as it maps, and
/// asks for the focus itself, as Qt's `requestActivate` does.
#[test]
fn a_utility_window_that_asks_to_be_active_takes_the_focus() {
    let (mut f, x) = Fixture::connected(Kind::X11);
    let editor = f.map(
        x,
        WindowSpec {
            cardinals: of_process(100),
            ..spec(400, 300)
        },
    );
    let launcher = f.map(
        x,
        WindowSpec {
            types: vec!["_NET_WM_WINDOW_TYPE_UTILITY"],
            states: vec!["_NET_WM_STATE_ABOVE"],
            cardinals: of_process(100),
            ..spec(600, 100)
        },
    );
    assert_eq!(x_focus(&mut f, x), editor.x11());

    // EWMH: the source is 1 for an application, then its timestamp and its active window.
    let (id, active) = (launcher.x11(), editor.x11());
    f.x11_call(x, move |x| {
        x.send_client_message(id, "_NET_ACTIVE_WINDOW", [1, 0, active, 0, 0])
    })
    .unwrap();

    f.dispatch_until_mut(|f| x_focus(f, x) == launcher.x11());
    let policy = &f.state.caminito.policy;
    assert_eq!(policy.stacking(0).last(), Some(&f.pid(launcher)));
}

/// Qt asks as soon as X has mapped the window, before its first buffer reaches Caminito.
#[test]
fn a_utility_window_that_asks_to_be_active_while_pending_maps_focused() {
    let (mut f, x) = Fixture::connected(Kind::X11);
    let editor = f.map(
        x,
        WindowSpec {
            cardinals: of_process(100),
            ..spec(400, 300)
        },
    );

    let active = editor.x11();
    let launcher = f
        .x11_call(x, move |x| {
            let window = x.create_window(&XWindowSpec {
                size: (600, 100),
                types: vec!["_NET_WM_WINDOW_TYPE_UTILITY"],
                cardinals: of_process(100),
                ..XWindowSpec::default()
            })?;
            x.map(window)?;
            x.send_client_message(window, "_NET_ACTIVE_WINDOW", [1, 0, active, 0, 0])?;
            Ok::<_, x11rb::errors::ReplyOrIdError>(window)
        })
        .unwrap();

    f.dispatch_until_mut(|f| x_focus(f, x) == launcher);
}

/// Albert, shown by a key binding's command while another client has the focus: Qt asks as
/// soon as X has mapped the window, with the time it was shown at, and sends the message
/// with `StructureNotify`, which Caminito's connection must select to read its source.
#[test]
fn a_launchers_utility_window_that_asks_to_be_active_while_pending_maps_focused() {
    let (mut f, x) = Fixture::connected(Kind::X11);
    let editor = f.map(
        x,
        WindowSpec {
            cardinals: of_process(100),
            ..spec(400, 300)
        },
    );

    let active = editor.x11();
    let now = f.state.caminito.now().as_millis() as u32;
    let launcher = f
        .x11_call(x, move |x| {
            let window = x.create_window(&XWindowSpec {
                size: (600, 100),
                types: vec!["_NET_WM_WINDOW_TYPE_UTILITY"],
                cardinals: of_process(200),
                ..XWindowSpec::default()
            })?;
            x.map(window)?;
            x.send_client_message_as_qt(window, "_NET_ACTIVE_WINDOW", [1, now, active, 0, 0])?;
            Ok::<_, x11rb::errors::ReplyOrIdError>(window)
        })
        .unwrap();

    f.dispatch_until_mut(|f| x_focus(f, x) == launcher);
}
