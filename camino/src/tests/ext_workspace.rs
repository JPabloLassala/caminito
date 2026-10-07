//! ext-workspace (66): the workspaces a panel's workspace switcher sees, what it's told as
//! they change, and what its requests do. No test maps a window: workspaces are the same
//! whichever kind of window is on them.

use camino_test_client::output_management::HeadChange;
use insta::assert_snapshot;

use super::fixture::{ClientId, Fixture, KEY_LEFTALT, KEY_LEFTCTRL, KEY_RIGHT, Kind};
use crate::settings::Value;
use crate::settings::translate::{CAMINO, MARCO_GENERAL, MARCO_WORKSPACE_NAMES};

/// One output, and a client that has bound the outputs and ext-workspace, as a pager does,
/// has seen the workspaces there are, and was told nothing since.
fn pager() -> (Fixture, ClientId) {
    pager_with(Vec::new())
}

/// A pager, as `pager`, where the workspaces are in two rows (96).
fn pager_with_two_rows() -> (Fixture, ClientId) {
    pager_with(vec![(
        CAMINO,
        "workspace-layout-rows".into(),
        Value::Int(2),
    )])
}

/// `pager`, with these settings from the start.
fn pager_with(settings: Vec<(&'static str, String, Value)>) -> (Fixture, ClientId) {
    let mut f = Fixture::with_settings(settings);
    f.add_output((1920, 1080), 1.0);
    let client = f.connect(Kind::Xdg);
    f.client(client).bind_outputs();
    f.client(client).bind_workspaces();
    f.roundtrip(client);
    f.client(client).take_workspace_events();
    (f, client)
}

/// The coordinates the pager was last sent for each workspace it has, from the first.
fn coordinates(f: &mut Fixture, pager: ClientId) -> String {
    f.roundtrip(pager);
    let workspaces = &f.client(pager).workspaces().workspaces;
    workspaces
        .iter()
        .filter(|workspace| !workspace.removed)
        .map(|workspace| format!("{:?}", workspace.coordinates))
        .collect::<Vec<_>>()
        .join(", ")
}

/// What the pager was told since this was last asked.
fn events(f: &mut Fixture, pager: ClientId) -> String {
    f.roundtrip(pager);
    f.client(pager).take_workspace_events()
}

/// The names of the workspaces the pager has, the active one starred.
fn workspaces(f: &mut Fixture, pager: ClientId) -> String {
    f.roundtrip(pager);
    let workspaces = &f.client(pager).workspaces().workspaces;
    workspaces
        .iter()
        .filter(|workspace| !workspace.removed)
        .map(|workspace| {
            let star = if workspace.active { "*" } else { "" };
            format!("{}{star}", workspace.name.as_deref().unwrap_or("(none)"))
        })
        .collect::<Vec<_>>()
        .join(", ")
}

#[test]
fn a_client_gets_one_group_with_the_outputs_and_the_four_workspaces() {
    let (mut f, client) = Fixture::connected(Kind::Xdg);
    f.client(client).bind_outputs();
    f.client(client).bind_workspaces();

    // Each workspace is in the one row of the default layout, as column and row.
    assert_snapshot!(events(&mut f, client), @"
    group 0: new
    group 0: capabilities GroupCapabilities(0x0)
    group 0: output_enter headless-1
    workspace 0: new
    workspace 0: id 1
    workspace 0: name Workspace 1
    workspace 0: coordinates [0, 0]
    workspace 0: state State(Active)
    workspace 0: capabilities WorkspaceCapabilities(Activate)
    group 0: workspace_enter workspace 0
    workspace 1: new
    workspace 1: id 2
    workspace 1: name Workspace 2
    workspace 1: coordinates [1, 0]
    workspace 1: state State(0x0)
    workspace 1: capabilities WorkspaceCapabilities(Activate)
    group 0: workspace_enter workspace 1
    workspace 2: new
    workspace 2: id 3
    workspace 2: name Workspace 3
    workspace 2: coordinates [2, 0]
    workspace 2: state State(0x0)
    workspace 2: capabilities WorkspaceCapabilities(Activate)
    group 0: workspace_enter workspace 2
    workspace 3: new
    workspace 3: id 4
    workspace 3: name Workspace 4
    workspace 3: coordinates [3, 0]
    workspace 3: state State(0x0)
    workspace 3: capabilities WorkspaceCapabilities(Activate)
    group 0: workspace_enter workspace 3
    done
    ");
}

#[test]
fn switching_workspace_by_binding_moves_active_with_one_done() {
    let (mut f, pager) = pager();

    f.chord(&[KEY_LEFTCTRL, KEY_LEFTALT, KEY_RIGHT]);

    assert_snapshot!(events(&mut f, pager), @"
    workspace 0: state State(0x0)
    workspace 1: state State(Active)
    done
    ");
}

#[test]
fn activate_switches_workspace_at_the_commit() {
    let (mut f, pager) = pager();

    f.client(pager).activate_workspace(2);
    assert_snapshot!(events(&mut f, pager), @"");
    assert_snapshot!(workspaces(&mut f, pager), @"Workspace 1*, Workspace 2, Workspace 3, Workspace 4");

    f.client(pager).commit_workspaces();
    assert_snapshot!(events(&mut f, pager), @"
    workspace 0: state State(0x0)
    workspace 2: state State(Active)
    done
    ");
}

#[test]
fn the_last_activate_before_a_commit_is_the_one_that_counts() {
    let (mut f, pager) = pager();

    f.client(pager).activate_workspace(2);
    f.client(pager).activate_workspace(3);
    f.client(pager).commit_workspaces();

    assert_snapshot!(workspaces(&mut f, pager), @"Workspace 1, Workspace 2, Workspace 3, Workspace 4*");
}

#[test]
fn the_number_of_workspaces_and_their_names_follow_the_settings() {
    let (mut f, pager) = pager();

    f.set_setting(MARCO_GENERAL, "num-workspaces", Value::Int(6));
    f.dispatch_until(|f| f.state.camino.policy.workspace_count() == 6);
    assert_snapshot!(events(&mut f, pager), @"
    workspace 4: new
    workspace 4: id 5
    workspace 4: name Workspace 5
    workspace 4: coordinates [4, 0]
    workspace 4: state State(0x0)
    workspace 4: capabilities WorkspaceCapabilities(Activate)
    group 0: workspace_enter workspace 4
    workspace 5: new
    workspace 5: id 6
    workspace 5: name Workspace 6
    workspace 5: coordinates [5, 0]
    workspace 5: state State(0x0)
    workspace 5: capabilities WorkspaceCapabilities(Activate)
    group 0: workspace_enter workspace 5
    done
    ");

    f.set_setting(MARCO_GENERAL, "num-workspaces", Value::Int(2));
    f.dispatch_until(|f| f.state.camino.policy.workspace_count() == 2);
    assert_snapshot!(events(&mut f, pager), @"
    group 0: workspace_leave workspace 2
    workspace 2: removed
    group 0: workspace_leave workspace 3
    workspace 3: removed
    group 0: workspace_leave workspace 4
    workspace 4: removed
    group 0: workspace_leave workspace 5
    workspace 5: removed
    done
    ");

    f.set_setting(MARCO_WORKSPACE_NAMES, "name-2", Value::Str("Mail".into()));
    f.dispatch_until(|f| f.state.camino.policy.workspace_names()[1] == "Mail");
    assert_snapshot!(events(&mut f, pager), @"
    workspace 1: name Mail
    done
    ");
}

#[test]
fn a_workspace_that_comes_back_is_a_new_one_with_the_same_id() {
    let (mut f, pager) = pager();
    f.set_setting(MARCO_GENERAL, "num-workspaces", Value::Int(3));
    f.dispatch_until(|f| f.state.camino.policy.workspace_count() == 3);
    f.set_setting(MARCO_GENERAL, "num-workspaces", Value::Int(4));
    f.dispatch_until(|f| f.state.camino.policy.workspace_count() == 4);

    assert_snapshot!(events(&mut f, pager), @"
    group 0: workspace_leave workspace 3
    workspace 3: removed
    done
    workspace 4: new
    workspace 4: id 4
    workspace 4: name Workspace 4
    workspace 4: coordinates [3, 0]
    workspace 4: state State(0x0)
    workspace 4: capabilities WorkspaceCapabilities(Activate)
    group 0: workspace_enter workspace 4
    done
    ");
}

#[test]
fn remove_deactivate_and_create_workspace_change_nothing() {
    let (mut f, pager) = pager();

    f.client(pager).remove_workspace(1);
    f.client(pager).deactivate_workspace(0);
    f.client(pager).create_workspace("Extra");
    f.client(pager).commit_workspaces();

    assert_snapshot!(events(&mut f, pager), @"");
    assert_snapshot!(workspaces(&mut f, pager), @"Workspace 1*, Workspace 2, Workspace 3, Workspace 4");
}

#[test]
fn a_new_output_enters_the_group_and_one_that_s_unplugged_leaves_it() {
    let (mut f, pager) = pager();

    let second = f.add_output((1280, 720), 1.0);
    f.roundtrip(pager);
    f.client(pager).bind_outputs();
    assert_snapshot!(events(&mut f, pager), @"
    group 0: output_enter headless-2
    done
    ");

    f.unplug(&second);
    assert_snapshot!(events(&mut f, pager), @"
    group 0: output_leave headless-2
    done
    ");
}

#[test]
fn an_output_that_s_disabled_leaves_the_group_before_its_global_is_removed() {
    let (mut f, pager) = pager();
    f.add_output((1280, 720), 1.0);
    f.roundtrip(pager);
    f.client(pager).bind_outputs();
    f.client(pager).bind_output_management();
    f.roundtrip(pager);
    events(&mut f, pager);

    // As `wlr-randr --off` does. libxfce4windowing keeps a monitor that's gone in the
    // group if the leave comes after, and mate-panel dies of it at the next enter (91).
    let off = [HeadChange::of("headless-2").disable()];
    f.client(pager).apply_outputs(&off);
    assert_snapshot!(events(&mut f, pager), @"
    group 0: output_leave headless-2
    done
    ");
    let outputs = f.client(pager).state.globals.iter();
    assert_eq!(outputs.filter(|g| g.interface == "wl_output").count(), 1);
}

#[test]
fn a_second_client_gets_its_own_workspaces_and_outlives_the_first() {
    let (mut f, first) = pager();
    let second = f.connect(Kind::Xdg);
    f.client(second).bind_workspaces();
    f.roundtrip(second);

    f.client(second).activate_workspace(1);
    f.client(second).commit_workspaces();
    let expected = "Workspace 1, Workspace 2*, Workspace 3, Workspace 4";
    assert_eq!(workspaces(&mut f, second), expected);
    assert_eq!(workspaces(&mut f, first), expected);

    // An activate the first never committed goes with it.
    f.client(first).activate_workspace(3);
    f.roundtrip(first);
    f.disconnect(first);
    f.client(second).commit_workspaces();
    assert_eq!(workspaces(&mut f, second), expected);
}

#[test]
fn a_removed_workspace_s_handle_is_inert_though_its_id_comes_back() {
    let (mut f, pager) = pager();
    f.client(pager).keep_removed_workspace_handles();
    f.set_setting(MARCO_GENERAL, "num-workspaces", Value::Int(3));
    f.dispatch_until(|f| f.state.camino.policy.workspace_count() == 3);
    f.set_setting(MARCO_GENERAL, "num-workspaces", Value::Int(4));
    f.dispatch_until(|f| f.state.camino.policy.workspace_count() == 4);
    f.roundtrip(pager);

    // The handle of the fourth workspace that was removed, not the new one's.
    f.client(pager).activate_workspace(3);
    f.client(pager).commit_workspaces();

    assert_snapshot!(workspaces(&mut f, pager), @"Workspace 1*, Workspace 2, Workspace 3, Workspace 4");
}

#[test]
fn a_client_that_stops_is_told_finished_and_nothing_more() {
    let (mut f, pager) = pager();

    f.client(pager).stop_workspaces();
    f.roundtrip(pager);
    assert!(f.client(pager).workspaces_finished());

    f.chord(&[KEY_LEFTCTRL, KEY_LEFTALT, KEY_RIGHT]);
    assert_snapshot!(events(&mut f, pager), @"");
}

#[test]
fn four_workspaces_in_two_rows_have_the_grid_s_coordinates() {
    let (mut f, pager) = pager_with_two_rows();

    assert_snapshot!(
        coordinates(&mut f, pager),
        @"[0, 0], [1, 0], [0, 1], [1, 1]"
    );
}

#[test]
fn changing_a_layout_key_sends_the_new_coordinates_with_one_done() {
    let (mut f, pager) = pager();

    f.set_setting(CAMINO, "workspace-layout-rows", Value::Int(2));
    f.dispatch_until(|f| {
        let grid = f.state.camino.policy.workspace_grid();
        (grid.columns(), grid.rows()) == (2, 2)
    });
    // The first two stay where they were.
    assert_snapshot!(events(&mut f, pager), @"
    workspace 2: coordinates [0, 1]
    workspace 3: coordinates [1, 1]
    done
    ");

    f.set_setting(
        CAMINO,
        "workspace-layout-orientation",
        Value::Str("vertical".into()),
    );
    f.dispatch_until(|f| f.state.camino.policy.workspace_grid().cell(1) == (0, 1));
    // Down each column now, so the second and third change places.
    assert_snapshot!(events(&mut f, pager), @"
    workspace 1: coordinates [0, 1]
    workspace 2: coordinates [1, 0]
    done
    ");
}

#[test]
fn the_coordinates_follow_the_number_of_workspaces() {
    let (mut f, pager) = pager_with_two_rows();

    f.set_setting(MARCO_GENERAL, "num-workspaces", Value::Int(6));
    f.dispatch_until(|f| f.state.camino.policy.workspace_count() == 6);

    // Two rows of three now, so the third and fourth move, in the one `done` the new
    // workspaces come in.
    assert_snapshot!(events(&mut f, pager), @"
    workspace 2: coordinates [2, 0]
    workspace 3: coordinates [0, 1]
    workspace 4: new
    workspace 4: id 5
    workspace 4: name Workspace 5
    workspace 4: coordinates [1, 1]
    workspace 4: state State(0x0)
    workspace 4: capabilities WorkspaceCapabilities(Activate)
    group 0: workspace_enter workspace 4
    workspace 5: new
    workspace 5: id 6
    workspace 5: name Workspace 6
    workspace 5: coordinates [2, 1]
    workspace 5: state State(0x0)
    workspace 5: capabilities WorkspaceCapabilities(Activate)
    group 0: workspace_enter workspace 5
    done
    ");
}

#[test]
fn the_binding_for_the_workspace_to_the_right_stops_at_the_end_of_its_row() {
    let (mut f, pager) = pager_with_two_rows();

    f.chord(&[KEY_LEFTCTRL, KEY_LEFTALT, KEY_RIGHT]);
    f.chord(&[KEY_LEFTCTRL, KEY_LEFTALT, KEY_RIGHT]);

    assert_snapshot!(workspaces(&mut f, pager), @"Workspace 1, Workspace 2*, Workspace 3, Workspace 4");
}
