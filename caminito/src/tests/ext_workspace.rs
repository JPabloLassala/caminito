//! ext-workspace (66): the workspaces a panel's workspace switcher sees, what it's told as
//! they change, and what its requests do. One test alone maps a window: workspaces are the
//! same whichever kind of window is on them.
//!
//! The tests from `pager_on_two_outputs` on are of the group each output has with
//! `workspaces-per-output` on (ADR 0032, 226), and of the key's change.

use caminito_test_client::output_management::HeadChange;
use insta::assert_snapshot;
use smithay::output::Output;
use smithay::utils::{Logical, Point};

use super::fixture::{
    ClientId, Fixture, KEY_LEFTALT, KEY_LEFTCTRL, KEY_RIGHT, Kind, WINDOW_COLOR, WindowSpec,
};
use super::per_output_workspaces::two_outputs_with;
use crate::output::output_id;
use crate::settings::Value;
use crate::settings::translate::{CAMINITO, MARCO_GENERAL, MARCO_WORKSPACE_NAMES};
use crate::state::SEAT;

/// One output, and a client that has bound the outputs and ext-workspace, as a pager does,
/// has seen the workspaces there are, and was told nothing since.
fn pager() -> (Fixture, ClientId) {
    pager_with(Vec::new())
}

/// A pager, as `pager`, where the workspaces are in two rows (96).
fn pager_with_two_rows() -> (Fixture, ClientId) {
    pager_with(vec![(
        CAMINITO,
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

/// The handler asks policy alone: the refresh pass draws what policy changed (179). The
/// window has no frame, which would be drawn again as it lost the focus.
#[test]
fn a_switch_of_workspace_a_pager_asks_for_is_drawn() {
    let (mut f, pager) = pager();
    let app = f.connect(Kind::Xdg);
    let window = f.map(app, WindowSpec::plain((400, 300)));
    let inside = f.geometry(window).loc + Point::<i32, Logical>::from((200, 150));
    let body = WINDOW_COLOR & 0xff_ffff;
    f.wait_for_pixel(inside.x, inside.y, body);

    f.client(pager).activate_workspace(1);
    f.client(pager).commit_workspaces();
    f.roundtrip(pager);

    let output = f.state.caminito.outputs[0].clone();
    let headless = f.state.backend.headless_mut();
    let pixel = headless.pixel(&output, inside.to_physical(1));
    assert_ne!(pixel.expect("the output is drawn") & 0xff_ffff, body);
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
    f.dispatch_until(|f| f.state.caminito.policy.workspace_count() == 6);
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
    f.dispatch_until(|f| f.state.caminito.policy.workspace_count() == 2);
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
    f.dispatch_until(|f| f.state.caminito.policy.workspace_names()[1] == "Mail");
    assert_snapshot!(events(&mut f, pager), @"
    workspace 1: name Mail
    done
    ");
}

#[test]
fn a_workspace_that_comes_back_is_a_new_one_with_the_same_id() {
    let (mut f, pager) = pager();
    f.set_setting(MARCO_GENERAL, "num-workspaces", Value::Int(3));
    f.dispatch_until(|f| f.state.caminito.policy.workspace_count() == 3);
    f.set_setting(MARCO_GENERAL, "num-workspaces", Value::Int(4));
    f.dispatch_until(|f| f.state.caminito.policy.workspace_count() == 4);

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
    f.dispatch_until(|f| f.state.caminito.policy.workspace_count() == 3);
    f.set_setting(MARCO_GENERAL, "num-workspaces", Value::Int(4));
    f.dispatch_until(|f| f.state.caminito.policy.workspace_count() == 4);
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

    f.set_setting(CAMINITO, "workspace-layout-rows", Value::Int(2));
    f.dispatch_until(|f| {
        let grid = f.state.caminito.policy.workspace_grid();
        (grid.columns(), grid.rows()) == (2, 2)
    });
    // The first two stay where they were.
    assert_snapshot!(events(&mut f, pager), @"
    workspace 2: coordinates [0, 1]
    workspace 3: coordinates [1, 1]
    done
    ");

    f.set_setting(
        CAMINITO,
        "workspace-layout-orientation",
        Value::Str("vertical".into()),
    );
    f.dispatch_until(|f| f.state.caminito.policy.workspace_grid().cell(1) == (0, 1));
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
    f.dispatch_until(|f| f.state.caminito.policy.workspace_count() == 6);

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

/// Two outputs side by side, the left one the primary, with `workspaces-per-output` as
/// `per_output` says, and a pager, as `pager`.
fn pager_on_two_outputs(per_output: bool) -> (Fixture, ClientId, Output, Output) {
    let (mut f, pager, left, right) = two_outputs_with(Kind::Xdg, per_output, vec![]);
    f.client(pager).bind_outputs();
    f.client(pager).bind_workspaces();
    f.roundtrip(pager);
    f.client(pager).take_workspace_events();
    (f, pager, left, right)
}

/// Each group the pager has, with its outputs and the ids of its workspaces, the active
/// one starred.
fn groups(f: &mut Fixture, pager: ClientId) -> String {
    f.roundtrip(pager);
    let state = f.client(pager).workspaces();
    let groups = state.groups.iter().enumerate();
    groups
        .filter(|(_, group)| !group.removed)
        .map(|(index, group)| {
            let workspaces = state.workspaces.iter();
            let ids = workspaces
                .filter(|workspace| !workspace.removed && workspace.group == Some(index))
                .map(|workspace| {
                    let star = if workspace.active { "*" } else { "" };
                    format!("{}{star}", workspace.id.as_deref().unwrap_or("(none)"))
                })
                .collect::<Vec<_>>()
                .join(", ");
            format!("group {index} of {}: {ids}", group.outputs.join(" and "))
        })
        .collect::<Vec<_>>()
        .join("\n")
}

#[test]
fn with_the_key_on_a_client_gets_a_group_for_each_output_with_its_workspaces() {
    let (mut f, client, _, _) = two_outputs_with(Kind::Xdg, true, vec![]);
    f.client(client).bind_outputs();
    f.client(client).bind_workspaces();

    // The same names and coordinates in each, and an id that names the output too.
    assert_snapshot!(events(&mut f, client), @"
    group 0: new
    group 0: capabilities GroupCapabilities(0x0)
    group 0: output_enter headless-1
    workspace 0: new
    workspace 0: id headless-1:1
    workspace 0: name Workspace 1
    workspace 0: coordinates [0, 0]
    workspace 0: state State(Active)
    workspace 0: capabilities WorkspaceCapabilities(Activate)
    group 0: workspace_enter workspace 0
    workspace 1: new
    workspace 1: id headless-1:2
    workspace 1: name Workspace 2
    workspace 1: coordinates [1, 0]
    workspace 1: state State(0x0)
    workspace 1: capabilities WorkspaceCapabilities(Activate)
    group 0: workspace_enter workspace 1
    workspace 2: new
    workspace 2: id headless-1:3
    workspace 2: name Workspace 3
    workspace 2: coordinates [2, 0]
    workspace 2: state State(0x0)
    workspace 2: capabilities WorkspaceCapabilities(Activate)
    group 0: workspace_enter workspace 2
    workspace 3: new
    workspace 3: id headless-1:4
    workspace 3: name Workspace 4
    workspace 3: coordinates [3, 0]
    workspace 3: state State(0x0)
    workspace 3: capabilities WorkspaceCapabilities(Activate)
    group 0: workspace_enter workspace 3
    group 1: new
    group 1: capabilities GroupCapabilities(0x0)
    group 1: output_enter headless-2
    workspace 4: new
    workspace 4: id headless-2:1
    workspace 4: name Workspace 1
    workspace 4: coordinates [0, 0]
    workspace 4: state State(Active)
    workspace 4: capabilities WorkspaceCapabilities(Activate)
    group 1: workspace_enter workspace 4
    workspace 5: new
    workspace 5: id headless-2:2
    workspace 5: name Workspace 2
    workspace 5: coordinates [1, 0]
    workspace 5: state State(0x0)
    workspace 5: capabilities WorkspaceCapabilities(Activate)
    group 1: workspace_enter workspace 5
    workspace 6: new
    workspace 6: id headless-2:3
    workspace 6: name Workspace 3
    workspace 6: coordinates [2, 0]
    workspace 6: state State(0x0)
    workspace 6: capabilities WorkspaceCapabilities(Activate)
    group 1: workspace_enter workspace 6
    workspace 7: new
    workspace 7: id headless-2:4
    workspace 7: name Workspace 4
    workspace 7: coordinates [3, 0]
    workspace 7: state State(0x0)
    workspace 7: capabilities WorkspaceCapabilities(Activate)
    group 1: workspace_enter workspace 7
    done
    ");
    assert_snapshot!(groups(&mut f, client), @"
    group 0 of headless-1: headless-1:1*, headless-1:2, headless-1:3, headless-1:4
    group 1 of headless-2: headless-2:1*, headless-2:2, headless-2:3, headless-2:4
    ");
}

#[test]
fn a_switch_on_one_output_changes_active_in_its_group_only() {
    let (mut f, pager, _, right) = pager_on_two_outputs(true);

    let policy = &mut f.state.caminito.policy;
    policy.switch_output_workspace(output_id(&right), 1);
    f.dispatch();

    assert_snapshot!(events(&mut f, pager), @"
    workspace 4: state State(0x0)
    workspace 5: state State(Active)
    done
    ");
    assert_snapshot!(groups(&mut f, pager), @"
    group 0 of headless-1: headless-1:1*, headless-1:2, headless-1:3, headless-1:4
    group 1 of headless-2: headless-2:1, headless-2:2*, headless-2:3, headless-2:4
    ");
}

#[test]
fn activate_on_a_workspace_of_the_second_group_switches_the_second_output() {
    let (mut f, pager, left, _) = pager_on_two_outputs(true);
    let app = f.connect(Kind::Xdg);
    f.map(app, WindowSpec::plain((400, 300)));
    let focus = f.state.caminito.policy.focus(SEAT);
    assert_eq!(
        f.state.caminito.policy.active_output(SEAT),
        Some(output_id(&left))
    );

    // The second group's third workspace.
    f.client(pager).activate_workspace(6);
    f.client(pager).commit_workspaces();
    f.roundtrip(pager);

    // The left output is still the Active output, and its window has the focus still.
    let policy = &f.state.caminito.policy;
    assert_eq!(policy.active_output(SEAT), Some(output_id(&left)));
    assert_eq!(policy.focus(SEAT), focus);
    assert_snapshot!(groups(&mut f, pager), @"
    group 0 of headless-1: headless-1:1*, headless-1:2, headless-1:3, headless-1:4
    group 1 of headless-2: headless-2:1, headless-2:2, headless-2:3*, headless-2:4
    ");
}

#[test]
fn one_commit_activates_the_last_workspace_asked_for_in_each_group() {
    let (mut f, pager, _, _) = pager_on_two_outputs(true);

    f.client(pager).activate_workspace(5);
    f.client(pager).activate_workspace(1);
    f.client(pager).activate_workspace(7);
    f.client(pager).activate_workspace(3);
    f.client(pager).commit_workspaces();

    assert_snapshot!(groups(&mut f, pager), @"
    group 0 of headless-1: headless-1:1, headless-1:2, headless-1:3, headless-1:4*
    group 1 of headless-2: headless-2:1, headless-2:2, headless-2:3, headless-2:4*
    ");
}

#[test]
fn a_group_whose_handle_the_client_destroyed_goes_with_no_word_of_it() {
    let (mut f, pager, _, right) = pager_on_two_outputs(true);
    f.client(pager).destroy_workspace_group_handle(1);
    f.roundtrip(pager);

    f.unplug(&right);

    // Its workspaces are removed still: the client has their handles.
    assert_snapshot!(events(&mut f, pager), @"
    workspace 4: removed
    workspace 5: removed
    workspace 6: removed
    workspace 7: removed
    done
    ");
}

#[test]
fn an_output_that_goes_takes_its_group_and_workspaces_and_one_added_gets_a_group() {
    let (mut f, pager, _, right) = pager_on_two_outputs(true);

    // The workspaces leave the group, then the output, and then the group goes.
    f.unplug(&right);
    assert_snapshot!(events(&mut f, pager), @"
    group 1: workspace_leave workspace 4
    workspace 4: removed
    group 1: workspace_leave workspace 5
    workspace 5: removed
    group 1: workspace_leave workspace 6
    workspace 6: removed
    group 1: workspace_leave workspace 7
    workspace 7: removed
    group 1: output_leave headless-2
    group 1: removed
    done
    ");
    assert_snapshot!(groups(&mut f, pager), @"group 0 of headless-1: headless-1:1*, headless-1:2, headless-1:3, headless-1:4");

    f.add_output((1280, 720), 1.0);
    f.roundtrip(pager);
    f.client(pager).bind_outputs();
    assert_snapshot!(events(&mut f, pager), @"
    group 2: new
    group 2: capabilities GroupCapabilities(0x0)
    workspace 8: new
    workspace 8: id headless-3:1
    workspace 8: name Workspace 1
    workspace 8: coordinates [0, 0]
    workspace 8: state State(Active)
    workspace 8: capabilities WorkspaceCapabilities(Activate)
    group 2: workspace_enter workspace 8
    workspace 9: new
    workspace 9: id headless-3:2
    workspace 9: name Workspace 2
    workspace 9: coordinates [1, 0]
    workspace 9: state State(0x0)
    workspace 9: capabilities WorkspaceCapabilities(Activate)
    group 2: workspace_enter workspace 9
    workspace 10: new
    workspace 10: id headless-3:3
    workspace 10: name Workspace 3
    workspace 10: coordinates [2, 0]
    workspace 10: state State(0x0)
    workspace 10: capabilities WorkspaceCapabilities(Activate)
    group 2: workspace_enter workspace 10
    workspace 11: new
    workspace 11: id headless-3:4
    workspace 11: name Workspace 4
    workspace 11: coordinates [3, 0]
    workspace 11: state State(0x0)
    workspace 11: capabilities WorkspaceCapabilities(Activate)
    group 2: workspace_enter workspace 11
    done
    group 2: output_enter headless-3
    done
    ");
    assert_snapshot!(groups(&mut f, pager), @"
    group 0 of headless-1: headless-1:1*, headless-1:2, headless-1:3, headless-1:4
    group 2 of headless-3: headless-3:1*, headless-3:2, headless-3:3, headless-3:4
    ");
}

#[test]
fn an_output_that_s_disabled_takes_its_group_before_its_global_is_removed() {
    let (mut f, pager, _, _) = pager_on_two_outputs(true);
    f.client(pager).bind_output_management();
    f.roundtrip(pager);
    events(&mut f, pager);

    // As `wlr-randr --off` does (91).
    let off = [HeadChange::of("headless-2").disable()];
    f.client(pager).apply_outputs(&off);
    let events = events(&mut f, pager);
    assert!(events.contains("group 1: output_leave headless-2\ngroup 1: removed"));
    assert_snapshot!(groups(&mut f, pager), @"group 0 of headless-1: headless-1:1*, headless-1:2, headless-1:3, headless-1:4");
}

#[test]
fn with_the_key_off_two_outputs_are_in_the_one_group() {
    let (mut f, pager, _, _) = pager_on_two_outputs(false);

    assert_snapshot!(groups(&mut f, pager), @"group 0 of headless-1 and headless-2: 1*, 2, 3, 4");
}

#[test]
fn a_change_of_the_key_swaps_the_one_group_and_the_outputs_groups() {
    let (mut f, pager, _, _) = pager_on_two_outputs(false);
    f.chord(&[KEY_LEFTCTRL, KEY_LEFTALT, KEY_RIGHT]);
    events(&mut f, pager);

    // On: every output starts on the workspace that was active.
    f.set_setting(CAMINITO, "workspaces-per-output", Value::Bool(true));
    f.dispatch_until(|f| f.state.caminito.policy.workspaces_per_output());
    assert_snapshot!(events(&mut f, pager), @"
    group 0: workspace_leave workspace 0
    workspace 0: removed
    group 0: workspace_leave workspace 1
    workspace 1: removed
    group 0: workspace_leave workspace 2
    workspace 2: removed
    group 0: workspace_leave workspace 3
    workspace 3: removed
    group 0: output_leave headless-1
    group 0: output_leave headless-2
    group 0: removed
    group 1: new
    group 1: capabilities GroupCapabilities(0x0)
    group 1: output_enter headless-1
    workspace 4: new
    workspace 4: id headless-1:1
    workspace 4: name Workspace 1
    workspace 4: coordinates [0, 0]
    workspace 4: state State(0x0)
    workspace 4: capabilities WorkspaceCapabilities(Activate)
    group 1: workspace_enter workspace 4
    workspace 5: new
    workspace 5: id headless-1:2
    workspace 5: name Workspace 2
    workspace 5: coordinates [1, 0]
    workspace 5: state State(Active)
    workspace 5: capabilities WorkspaceCapabilities(Activate)
    group 1: workspace_enter workspace 5
    workspace 6: new
    workspace 6: id headless-1:3
    workspace 6: name Workspace 3
    workspace 6: coordinates [2, 0]
    workspace 6: state State(0x0)
    workspace 6: capabilities WorkspaceCapabilities(Activate)
    group 1: workspace_enter workspace 6
    workspace 7: new
    workspace 7: id headless-1:4
    workspace 7: name Workspace 4
    workspace 7: coordinates [3, 0]
    workspace 7: state State(0x0)
    workspace 7: capabilities WorkspaceCapabilities(Activate)
    group 1: workspace_enter workspace 7
    group 2: new
    group 2: capabilities GroupCapabilities(0x0)
    group 2: output_enter headless-2
    workspace 8: new
    workspace 8: id headless-2:1
    workspace 8: name Workspace 1
    workspace 8: coordinates [0, 0]
    workspace 8: state State(0x0)
    workspace 8: capabilities WorkspaceCapabilities(Activate)
    group 2: workspace_enter workspace 8
    workspace 9: new
    workspace 9: id headless-2:2
    workspace 9: name Workspace 2
    workspace 9: coordinates [1, 0]
    workspace 9: state State(Active)
    workspace 9: capabilities WorkspaceCapabilities(Activate)
    group 2: workspace_enter workspace 9
    workspace 10: new
    workspace 10: id headless-2:3
    workspace 10: name Workspace 3
    workspace 10: coordinates [2, 0]
    workspace 10: state State(0x0)
    workspace 10: capabilities WorkspaceCapabilities(Activate)
    group 2: workspace_enter workspace 10
    workspace 11: new
    workspace 11: id headless-2:4
    workspace 11: name Workspace 4
    workspace 11: coordinates [3, 0]
    workspace 11: state State(0x0)
    workspace 11: capabilities WorkspaceCapabilities(Activate)
    group 2: workspace_enter workspace 11
    done
    ");
    assert_snapshot!(groups(&mut f, pager), @"
    group 1 of headless-1: headless-1:1, headless-1:2*, headless-1:3, headless-1:4
    group 2 of headless-2: headless-2:1, headless-2:2*, headless-2:3, headless-2:4
    ");

    // The pager's requests are of the groups it has now.
    f.client(pager).activate_workspace(10);
    f.client(pager).commit_workspaces();
    assert_snapshot!(groups(&mut f, pager), @"
    group 1 of headless-1: headless-1:1, headless-1:2*, headless-1:3, headless-1:4
    group 2 of headless-2: headless-2:1, headless-2:2, headless-2:3*, headless-2:4
    ");
    events(&mut f, pager);

    // Off: the Active output's workspace, the left one's, becomes the one active one.
    f.set_setting(CAMINITO, "workspaces-per-output", Value::Bool(false));
    f.dispatch_until(|f| !f.state.caminito.policy.workspaces_per_output());
    assert_snapshot!(events(&mut f, pager), @"
    group 1: workspace_leave workspace 4
    workspace 4: removed
    group 1: workspace_leave workspace 5
    workspace 5: removed
    group 1: workspace_leave workspace 6
    workspace 6: removed
    group 1: workspace_leave workspace 7
    workspace 7: removed
    group 2: workspace_leave workspace 8
    workspace 8: removed
    group 2: workspace_leave workspace 9
    workspace 9: removed
    group 2: workspace_leave workspace 10
    workspace 10: removed
    group 2: workspace_leave workspace 11
    workspace 11: removed
    group 1: output_leave headless-1
    group 1: removed
    group 2: output_leave headless-2
    group 2: removed
    group 3: new
    group 3: capabilities GroupCapabilities(0x0)
    group 3: output_enter headless-1
    group 3: output_enter headless-2
    workspace 12: new
    workspace 12: id 1
    workspace 12: name Workspace 1
    workspace 12: coordinates [0, 0]
    workspace 12: state State(0x0)
    workspace 12: capabilities WorkspaceCapabilities(Activate)
    group 3: workspace_enter workspace 12
    workspace 13: new
    workspace 13: id 2
    workspace 13: name Workspace 2
    workspace 13: coordinates [1, 0]
    workspace 13: state State(Active)
    workspace 13: capabilities WorkspaceCapabilities(Activate)
    group 3: workspace_enter workspace 13
    workspace 14: new
    workspace 14: id 3
    workspace 14: name Workspace 3
    workspace 14: coordinates [2, 0]
    workspace 14: state State(0x0)
    workspace 14: capabilities WorkspaceCapabilities(Activate)
    group 3: workspace_enter workspace 14
    workspace 15: new
    workspace 15: id 4
    workspace 15: name Workspace 4
    workspace 15: coordinates [3, 0]
    workspace 15: state State(0x0)
    workspace 15: capabilities WorkspaceCapabilities(Activate)
    group 3: workspace_enter workspace 15
    done
    ");
    assert_snapshot!(groups(&mut f, pager), @"group 3 of headless-1 and headless-2: 1, 2*, 3, 4");

    f.client(pager).activate_workspace(15);
    f.client(pager).commit_workspaces();
    assert_snapshot!(groups(&mut f, pager), @"group 3 of headless-1 and headless-2: 1, 2, 3, 4*");
}

#[test]
fn an_activate_from_before_the_key_changed_is_dropped() {
    let (mut f, pager, _, _) = pager_on_two_outputs(false);
    f.client(pager).keep_removed_workspace_handles();

    f.client(pager).activate_workspace(2);
    f.roundtrip(pager);
    f.set_setting(CAMINITO, "workspaces-per-output", Value::Bool(true));
    f.dispatch_until(|f| f.state.caminito.policy.workspaces_per_output());
    f.client(pager).commit_workspaces();

    assert_snapshot!(groups(&mut f, pager), @"
    group 1 of headless-1: headless-1:1*, headless-1:2, headless-1:3, headless-1:4
    group 2 of headless-2: headless-2:1*, headless-2:2, headless-2:3, headless-2:4
    ");
}
