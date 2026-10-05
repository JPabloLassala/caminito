//! Xfce's foreign toplevel protocol (75): the workspace a panel's window list is told each
//! window is on, as the windows and the workspaces change.

use camino_policy::Config;
use insta::{allow_duplicates, assert_snapshot};

use super::fixture::{
    ClientId, ClientWindow, Fixture, KEY_LEFTALT, KEY_LEFTCTRL, KEY_RIGHT, Kind, WindowSpec,
    on_both_kinds,
};
use crate::settings::Value;
use crate::settings::translate::MARCO_GENERAL;

/// A window titled `title`.
fn titled(title: &'static str) -> WindowSpec {
    WindowSpec {
        title: Some(title),
        app_id: Some("caja"),
        ..WindowSpec::default()
    }
}

/// A client that has bound what libxfce4windowing binds, in its order: the outputs,
/// ext-workspace, foreign toplevel management, and Xfce's manager with it. It has seen the
/// toplevels there are, and asked for their Xfce handles.
fn panel(f: &mut Fixture) -> ClientId {
    let client = f.connect(Kind::Xdg);
    f.client(client).bind_outputs();
    f.client(client).bind_workspaces();
    f.client(client).bind_toplevel_workspaces();
    f.client(client).bind_foreign_toplevels();
    f.roundtrip(client);
    client
}

/// One output, a client of `kind` with a window, Caja, on the first workspace, and a panel
/// that knows of it and was told nothing since.
fn a_window_and_a_panel(kind: Kind) -> (Fixture, ClientWindow, ClientId) {
    let (mut f, client) = Fixture::connected(kind);
    let caja = f.map(client, titled("Caja"));
    let panel = panel(&mut f);
    // The answer to the Xfce handle's request.
    f.roundtrip(panel);
    f.client(panel).take_toplevel_events();
    (f, caja, panel)
}

/// What the panel was told of its toplevels since this was last asked.
fn events(f: &mut Fixture, panel: ClientId) -> String {
    f.roundtrip(panel);
    f.client(panel).take_toplevel_events()
}

/// The index of the workspace the panel has each open toplevel on, by title.
fn placements(f: &mut Fixture, panel: ClientId) -> String {
    f.roundtrip(panel);
    let toplevels = &f.client(panel).foreign_toplevels().toplevels;
    toplevels
        .iter()
        .filter(|toplevel| !toplevel.closed)
        .map(|toplevel| {
            let title = toplevel.title.as_deref().unwrap_or("(none)");
            match toplevel.workspace {
                Some(workspace) => format!("{title} on workspace {workspace}"),
                None => format!("{title} on no workspace"),
            }
        })
        .collect::<Vec<_>>()
        .join(", ")
}

/// Moves a window to the workspace at `workspace`, as the window menu does.
fn move_to_workspace(f: &mut Fixture, window: ClientWindow, workspace: usize) {
    let id = f.pid(window);
    f.state.camino.policy.move_to_workspace(id, workspace);
    f.dispatch();
}

on_both_kinds! {
    fn a_panel_is_told_the_workspace_of_each_window_after_its_details(kind: Kind) {
        let (mut f, client) = Fixture::connected(kind);
        f.map(client, titled("Caja"));

        let panel = panel(&mut f);

        // libxfce4windowing shows a window once it has two `done`s: one for the toplevel's
        // details and one for what its Xfce handle adds.
        allow_duplicates! {
            assert_snapshot!(events(&mut f, panel), @"
            toplevel 0: new
            toplevel 0: title Caja
            toplevel 0: app_id caja
            toplevel 0: output_enter headless-1
            toplevel 0: state [Activated]
            toplevel 0: done
            toplevel 0: workspace_enter workspace 0
            toplevel 0: done
            ");
        }
    }
}

on_both_kinds! {
    fn a_window_that_maps_on_another_workspace_is_on_that_one(kind: Kind) {
        let (mut f, _, panel) = a_window_and_a_panel(kind);
        f.chord(&[KEY_LEFTCTRL, KEY_LEFTALT, KEY_RIGHT]);
        let client = f.connect(kind);

        f.map(client, titled("Pluma"));

        allow_duplicates! {
            assert_snapshot!(placements(&mut f, panel), @"Caja on workspace 0, Pluma on workspace 1");
        }
    }
}

on_both_kinds! {
    fn a_window_moved_to_another_workspace_leaves_one_and_enters_the_other(kind: Kind) {
        let (mut f, caja, panel) = a_window_and_a_panel(kind);

        move_to_workspace(&mut f, caja, 2);

        allow_duplicates! {
            assert_snapshot!(events(&mut f, panel), @"
            toplevel 0: state []
            toplevel 0: done
            toplevel 0: workspace_leave workspace 0
            toplevel 0: workspace_enter workspace 2
            toplevel 0: done
            ");
        }
    }
}

#[test]
fn switching_workspace_moves_no_window() {
    let (mut f, _, panel) = a_window_and_a_panel(Kind::Xdg);

    f.chord(&[KEY_LEFTCTRL, KEY_LEFTALT, KEY_RIGHT]);

    // The window loses the focus, and stays where it is.
    assert_snapshot!(events(&mut f, panel), @"
    toplevel 0: state []
    toplevel 0: done
    ");
    assert_snapshot!(placements(&mut f, panel), @"Caja on workspace 0");
}

#[test]
fn a_window_leaves_a_workspace_that_goes_before_the_workspace_is_removed() {
    let (mut f, caja, panel) = a_window_and_a_panel(Kind::Xdg);
    move_to_workspace(&mut f, caja, 3);
    f.roundtrip(panel);
    f.client(panel).take_toplevel_events();
    f.client(panel).take_workspace_events();

    f.set_setting(MARCO_GENERAL, "num-workspaces", Value::Int(2));
    f.dispatch_until(|f| f.state.camino.policy.workspace_count() == 2);

    // The panel destroys a workspace's handle when it's removed, so a `workspace_leave`
    // that came after `removed` would name an object the panel no longer has.
    assert_snapshot!(events(&mut f, panel), @"
    toplevel 0: workspace_leave workspace 3
    toplevel 0: workspace_enter workspace 1
    toplevel 0: done
    ");
    assert_snapshot!(f.client(panel).take_workspace_events(), @"
    group 0: workspace_leave workspace 2
    workspace 2: removed
    group 0: workspace_leave workspace 3
    workspace 3: removed
    done
    ");
}

#[test]
fn a_window_enters_a_workspace_that_is_new_once_the_panel_has_it() {
    let (mut f, caja, panel) = a_window_and_a_panel(Kind::Xdg);
    f.client(panel).take_workspace_events();
    let id = f.pid(caja);

    // Both in one turn of the loop, so the workspace is new to the panel in the refresh
    // pass that moves the window to it.
    let policy = &mut f.state.camino.policy;
    policy.settings_changed(Config {
        workspace_count: 6,
        ..Config::default()
    });
    policy.move_to_workspace(id, 5);
    f.dispatch();

    // The window loses the focus as it goes. It leaves before ext-workspace's refresh and
    // enters after it, when the panel has the workspace's handle. The panel only reads:
    // a request of its own would give Camino another refresh pass to enter in.
    f.client(panel).dispatch();
    assert_snapshot!(f.client(panel).take_toplevel_events(), @"
    toplevel 0: state []
    toplevel 0: done
    toplevel 0: workspace_leave workspace 0
    toplevel 0: done
    toplevel 0: workspace_enter workspace 5
    toplevel 0: done
    ");
}

#[test]
fn a_panel_that_binds_the_workspaces_late_is_told_then() {
    let (mut f, client) = Fixture::connected(Kind::Xdg);
    f.map(client, titled("Caja"));
    let panel = f.connect(Kind::Xdg);
    f.client(panel).bind_foreign_toplevels();
    f.roundtrip(panel);
    f.client(panel).bind_toplevel_workspaces();
    // With no workspace to name, the Xfce handle still ends its first details.
    assert_snapshot!(events(&mut f, panel), @"
    toplevel 0: new
    toplevel 0: title Caja
    toplevel 0: app_id caja
    toplevel 0: state [Activated]
    toplevel 0: done
    toplevel 0: done
    ");

    f.client(panel).bind_workspaces();

    assert_snapshot!(events(&mut f, panel), @"
    toplevel 0: workspace_enter workspace 0
    toplevel 0: done
    ");
}

#[test]
fn a_closed_window_s_xfce_handle_is_told_nothing_more() {
    let (mut f, caja, panel) = a_window_and_a_panel(Kind::Xdg);
    let ClientWindow::Xdg(client, xdg) = caja else {
        unreachable!("the window is an xdg toplevel");
    };
    f.client(panel).keep_closed_toplevel_handles();

    f.client(client).destroy_window(xdg);
    f.roundtrip(client);

    assert_snapshot!(events(&mut f, panel), @"toplevel 0: closed");
}

#[test]
fn the_requests_camino_ignores_change_nothing_and_an_icon_s_pixels_fail() {
    let (mut f, caja, panel) = a_window_and_a_panel(Kind::Xdg);
    let before = f.geometry(caja);

    f.client(panel).send_other_xfce_requests(0);

    // A protocol error would end the panel here.
    assert_snapshot!(events(&mut f, panel), @"toplevel 0: icon failed Value(NoPixelData)");
    let policy = &f.state.camino.policy;
    let id = f.pid(caja);
    assert_eq!(policy.workspace_of(id), Some(0));
    assert!(!policy.is_shaded(id) && !policy.is_sticky(id) && !policy.is_above(id));
    assert_eq!(f.geometry(caja), before);
}

#[test]
#[should_panic(expected = "Protocol error 1 on object xfce_foreign_toplevel_manager_private_v1")]
fn a_second_xfce_handle_for_a_toplevel_is_a_protocol_error() {
    let (mut f, _, panel) = a_window_and_a_panel(Kind::Xdg);

    f.client(panel).get_another_xfce_handle(0);

    f.roundtrip(panel);
}

#[test]
fn a_second_panel_has_its_own_handles() {
    let (mut f, caja, first) = a_window_and_a_panel(Kind::Xdg);
    let second = panel(&mut f);
    assert_snapshot!(placements(&mut f, second), @"Caja on workspace 0");

    f.disconnect(second);
    move_to_workspace(&mut f, caja, 1);

    assert_snapshot!(placements(&mut f, first), @"Caja on workspace 1");
}
