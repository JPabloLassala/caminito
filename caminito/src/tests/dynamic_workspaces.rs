//! `dynamic-workspaces` (151), Marco master's key, as a pager's ext-workspace sees it:
//! workspaces are added and removed as windows come and go. Its tests run on Marco master's
//! schema, which has the key; on 1.29.0's, which Arch ships, nothing changes.

use insta::{allow_duplicates, assert_snapshot};

use super::fixture::{
    ClientId, ClientWindow, Fixture, KEY_LEFTALT, KEY_LEFTCTRL, KEY_RIGHT, Kind, WindowSpec,
    on_both_kinds,
};
use crate::settings::Value;
use crate::settings::translate::MARCO_GENERAL;

/// One output, and a client that has bound the outputs and ext-workspace, as a pager does,
/// has seen the workspaces there are, and was told nothing since.
fn pager(mut f: Fixture) -> (Fixture, ClientId) {
    f.add_output((1920, 1080), 1.0);
    let pager = f.connect(Kind::Xdg);
    f.client(pager).bind_outputs();
    f.client(pager).bind_workspaces();
    f.roundtrip(pager);
    f.client(pager).take_workspace_events();
    (f, pager)
}

/// A pager, on Marco master's schema with `dynamic-workspaces` on.
fn dynamic_pager() -> (Fixture, ClientId) {
    pager(Fixture::with_marco_master_settings(vec![(
        MARCO_GENERAL,
        "dynamic-workspaces".into(),
        Value::Bool(true),
    )]))
}

/// What the pager was told since this was last asked.
fn events(f: &mut Fixture, pager: ClientId) -> String {
    f.roundtrip(pager);
    f.client(pager).take_workspace_events()
}

/// The names of the workspaces the pager has, each with its coordinates, the active one
/// starred.
fn workspaces(f: &mut Fixture, pager: ClientId) -> String {
    f.roundtrip(pager);
    let workspaces = &f.client(pager).workspaces().workspaces;
    workspaces
        .iter()
        .filter(|workspace| !workspace.removed)
        .map(|workspace| {
            let star = if workspace.active { "*" } else { "" };
            let name = workspace.name.as_deref().unwrap_or("(none)");
            format!("{name}{star} at {:?}", workspace.coordinates)
        })
        .collect::<Vec<_>>()
        .join(", ")
}

/// `num-workspaces` as the settings last read it.
fn num_workspaces(f: &Fixture) -> usize {
    f.state.caminito.settings.policy.workspace_count
}

/// Has a window's client destroy it, and waits until Caminito no longer manages it.
fn destroy(f: &mut Fixture, window: ClientWindow) {
    match window {
        ClientWindow::Xdg(id, xdg) => {
            f.client(id).destroy_window(xdg);
            f.roundtrip(id);
        }
        ClientWindow::X11(id, x11) => {
            f.x11_call(id, move |x| x.destroy(x11)).unwrap();
            f.dispatch_until(|f| f.policy_id(window).is_none());
        }
    }
}

#[test]
fn with_no_window_there_is_one_workspace_and_num_workspaces_says_so() {
    let (mut f, pager) = dynamic_pager();

    assert_snapshot!(workspaces(&mut f, pager), @"Workspace 1* at [0, 0]");
    f.dispatch_until(|f| num_workspaces(f) == 1);
}

on_both_kinds! {
    fn a_window_coming_and_going_adds_a_workspace_and_removes_it(kind: Kind) {
        let (mut f, pager) = dynamic_pager();
        let client = f.connect(kind);

        let window = f.map(client, WindowSpec::default());

        allow_duplicates! {
            assert_snapshot!(events(&mut f, pager), @"
            workspace 1: new
            workspace 1: id 2
            workspace 1: name Workspace 2
            workspace 1: coordinates [1, 0]
            workspace 1: state State(0x0)
            workspace 1: capabilities WorkspaceCapabilities(Activate)
            group 0: workspace_enter workspace 1
            done
            ");
            assert_snapshot!(workspaces(&mut f, pager), @"Workspace 1* at [0, 0], Workspace 2 at [1, 0]");
        }
        f.dispatch_until(|f| num_workspaces(f) == 2);

        destroy(&mut f, window);

        allow_duplicates! {
            assert_snapshot!(events(&mut f, pager), @"
            group 0: workspace_leave workspace 1
            workspace 1: removed
            done
            ");
            assert_snapshot!(workspaces(&mut f, pager), @"Workspace 1* at [0, 0]");
        }
        f.dispatch_until(|f| num_workspaces(f) == 1);
    }
}

on_both_kinds! {
    fn the_windows_after_an_emptied_workspace_move_down_with_the_view(kind: Kind) {
        let (mut f, pager) = dynamic_pager();
        let client = f.connect(kind);
        let first = f.map(client, WindowSpec::default());
        f.chord(&[KEY_LEFTCTRL, KEY_LEFTALT, KEY_RIGHT]);
        let second = f.map(client, WindowSpec::default());
        assert_eq!(f.state.caminito.policy.workspace_count(), 3);
        events(&mut f, pager);

        destroy(&mut f, first);

        allow_duplicates! {
            assert_snapshot!(events(&mut f, pager), @"
            group 0: workspace_leave workspace 2
            workspace 2: removed
            workspace 0: state State(Active)
            workspace 1: state State(0x0)
            done
            ");
            assert_snapshot!(workspaces(&mut f, pager), @"Workspace 1* at [0, 0], Workspace 2 at [1, 0]");
        }
        assert!(f.has_keyboard(second));
    }
}

on_both_kinds! {
    fn without_the_key_in_the_schema_windows_leave_the_workspaces_as_they_are(kind: Kind) {
        let (mut f, pager) = pager(Fixture::new());
        let client = f.connect(kind);

        let window = f.map(client, WindowSpec::default());
        destroy(&mut f, window);

        allow_duplicates! {
            assert_snapshot!(events(&mut f, pager), @"");
            assert_snapshot!(workspaces(&mut f, pager), @"Workspace 1* at [0, 0], Workspace 2 at [1, 0], Workspace 3 at [2, 0], Workspace 4 at [3, 0]");
        }
        assert_eq!(num_workspaces(&f), 4);
    }
}
