//! XWayland windows managed like native ones (11, 35), through a real Xwayland and x11rb.
//! What they check is X's side of management, so they map X windows only (45).

use camino_policy::{PolicyWindow, WindowId};
use camino_test_client::x11::XWindowSpec;
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
fn client_list_stacking(f: &mut Fixture, x: ClientId) -> Vec<u32> {
    let property = f
        .x11_call(x, |x| x.property(x.root(), "_NET_CLIENT_LIST_STACKING"))
        .unwrap();
    match property.map(|p| p.value) {
        Some(camino_test_client::x11::PropertyValue::Cardinals(ids)) => ids,
        other => panic!("_NET_CLIENT_LIST_STACKING is a WINDOW list: {other:?}"),
    }
}

#[test]
fn a_normal_window_maps_framed_takes_focus_on_click_and_moves() {
    let (mut f, x) = Fixture::connected(Kind::X11);
    let first = f.map(x, spec(400, 300));
    let second = f.map(x, spec(400, 300));
    assert!(f.mapped(first).shows_frame());
    assert_eq!(
        f.state.camino.policy.focus(crate::state::SEAT),
        Some(f.pid(second))
    );

    let point = center(f.geometry(first));
    f.click(point.x, point.y);
    assert_eq!(
        f.state.camino.policy.focus(crate::state::SEAT),
        Some(f.pid(first))
    );

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
    assert_eq!(f.state.camino.policy.active_workspace(), 1);

    let dialog = f.map(
        x,
        WindowSpec {
            parent: Some(parent),
            ..spec(300, 200)
        },
    );

    let policy = &f.state.camino.policy;
    let (parent_id, dialog_id) = (f.pid(parent), f.pid(dialog));
    assert_eq!(policy.workspace_of(dialog_id), Some(0));
    assert_eq!(policy.parent(dialog_id), Some(parent_id));
    assert_eq!(
        f.mapped(dialog).window_type(),
        camino_policy::WindowType::Dialog
    );
    assert_eq!(policy.stacking(0), [parent_id, dialog_id]);

    // Raising the parent keeps its dialog above it.
    f.chord(&[KEY_LEFTCTRL, KEY_LEFTALT, KEY_LEFT]);
    let point = center(f.geometry(parent)) - Point::from((250., 150.));
    f.click(point.x, point.y);
    assert_eq!(f.state.camino.policy.stacking(0), [parent_id, dialog_id]);
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
    f.dispatch_until(|f| f.state.camino.policy.is_maximized(f.pid(window)));
    f.dispatch_until(|f| f.geometry(window).size.w > 1800);
    let maximized = f.geometry(window);

    f.x11_call(x, move |x| x.configure(id, (10, 10), (100, 100)))
        .unwrap();
    f.dispatch_for(std::time::Duration::from_millis(200));
    assert_eq!(f.geometry(window), maximized);
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
fn net_client_list_stacking_follows_caminos_order_after_a_raise() {
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

    assert!(f.state.camino.policy.is_maximized(f.pid(window)));
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
    f.dispatch_until(|f| !f.state.camino.pending.is_empty());

    let id = window.x11();
    f.x11_call(x, move |x| x.destroy(id)).unwrap();
    f.dispatch_until(|f| f.state.camino.pending.is_empty());

    assert!(f.policy_id(window).is_none());
    assert_eq!(f.state.camino.policy.windows().count(), 0);
}

#[test]
fn focusing_a_window_moves_x_input_focus_to_it() {
    let (mut f, x) = Fixture::connected(Kind::X11);
    let first = f.map(x, spec(400, 300));
    let second = f.map(x, spec(400, 300));
    let focus = |f: &mut Fixture| f.x11_call(x, |x| x.input_focus()).unwrap();
    assert_eq!(focus(&mut f), second.x11());

    let point = center(f.geometry(first));
    f.click(point.x, point.y);

    assert_eq!(focus(&mut f), first.x11());
}

/// The debug dump's entry for policy's window `id`.
fn dumped(f: &Fixture, id: WindowId) -> serde_json::Value {
    let dump: serde_json::Value = serde_json::from_str(&f.state.camino.state_dump()).unwrap();
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
    f.dispatch_until(|f| f.mapped(dialog).window_type() == camino_policy::WindowType::ModalDialog);

    let id = dialog.x11();
    let states = f
        .x11_call(x, move |x| {
            let mut names = camino_test_client::x11::Names::new(x);
            names.property(x, id, "_NET_WM_STATE")
        })
        .unwrap();
    assert!(states.contains("_NET_WM_STATE_MODAL"), "{states}");
}
