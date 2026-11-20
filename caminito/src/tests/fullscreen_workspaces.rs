//! Fullscreen workspaces (ADR 0057, CMN-304) through the headless fixture, for both kinds of
//! client: with `fullscreen-workspaces` on, a focused window that goes fullscreen or opens
//! fullscreen is on a workspace of its own after its Home workspace, which its output slides
//! to, and the output slides back as the window leaves fullscreen or closes. The debug dump
//! names the window whose Fullscreen workspace an output shows.

use super::dynamic_workspaces::destroy;
use super::fixture::{
    ClientId, ClientWindow, Fixture, KEY_LEFT, KEY_LEFTALT, KEY_LEFTCTRL, KEY_RIGHT, Kind,
    WindowSpec, on_both_kinds,
};
use super::workspace_slide::{OUTPUT, slides};
use crate::settings::Value as Setting;
use crate::settings::translate::CAMINITO;
use crate::state::SEAT;

/// One output with `workspace-slide` on and `fullscreen-workspaces` as `on` says, and a
/// client of `kind`.
fn desk(kind: Kind, on: bool) -> (Fixture, ClientId) {
    let mut f = Fixture::with_settings(vec![
        (CAMINITO, "workspace-slide".into(), Setting::Bool(true)),
        (CAMINITO, "fullscreen-workspaces".into(), Setting::Bool(on)),
    ]);
    f.add_output(OUTPUT, 1.0);
    let client = f.connect(kind);
    (f, client)
}

/// The window, by policy's id, whose Fullscreen workspace the output shows, as the debug
/// dump has it.
fn fullscreen_workspace(f: &Fixture) -> Option<u64> {
    let dump: serde_json::Value = serde_json::from_str(&f.state.caminito.state_dump()).unwrap();
    dump["outputs"][0]["fullscreen_workspace"].as_u64()
}

/// Whether a slide runs on the output and moves `window`.
fn slides_with(f: &Fixture, window: ClientWindow) -> bool {
    let pid = f.pid(window).0;
    slides(f)
        .iter()
        .any(|slide| slide.windows.iter().any(|(id, _)| *id == pid))
}

/// Has `window`'s client ask for fullscreen, or to leave it, as its kind asks, and has it
/// draw at the size asked.
fn set_fullscreen(f: &mut Fixture, window: ClientWindow, fullscreen: bool) {
    match window {
        ClientWindow::Xdg(client, xdg) if fullscreen => f.client(client).set_fullscreen(xdg, None),
        ClientWindow::Xdg(client, xdg) => f.client(client).unset_fullscreen(xdg),
        ClientWindow::X11(client, id) => {
            f.x11_call(client, move |x| {
                let atom = x.atom("_NET_WM_STATE_FULLSCREEN")?;
                let action = u32::from(fullscreen);
                x.send_client_message(id, "_NET_WM_STATE", [action, atom, 0, 1, 0])
            })
            .unwrap();
        }
    }
    let pid = f.pid(window);
    f.dispatch_until(|f| f.state.caminito.policy.is_fullscreen(pid) == fullscreen);
    let size = if fullscreen { OUTPUT } else { (400, 300) };
    f.draw_at_asked_size(window, size);
}

on_both_kinds! {
    fn a_focused_window_that_goes_fullscreen_has_a_workspace_to_switch_from_and_back_to(
        kind: Kind
    ) {
        let (mut f, client) = desk(kind, true);
        let other = f.map(client, WindowSpec::plain((400, 300)));
        let window = f.map(client, WindowSpec::plain((400, 300)));

        set_fullscreen(&mut f, window, true);

        assert_eq!(fullscreen_workspace(&f), Some(f.pid(window).0));
        assert!(slides_with(&f, other), "{:?}", slides(&f));
        let policy = &f.state.caminito.policy;
        assert!(!policy.is_showing(f.pid(other)));
        assert_eq!(policy.active_workspace(), 0);

        f.chord(&[KEY_LEFTCTRL, KEY_LEFTALT, KEY_LEFT]);
        assert_eq!(fullscreen_workspace(&f), None);
        assert!(f.state.caminito.policy.is_showing(f.pid(other)));
        assert!(!f.state.caminito.policy.is_showing(f.pid(window)));

        f.chord(&[KEY_LEFTCTRL, KEY_LEFTALT, KEY_RIGHT]);
        assert_eq!(fullscreen_workspace(&f), Some(f.pid(window).0));

        set_fullscreen(&mut f, window, false);

        assert_eq!(fullscreen_workspace(&f), None);
        assert!(slides_with(&f, other), "{:?}", slides(&f));
        let policy = &f.state.caminito.policy;
        assert!(policy.is_showing(f.pid(other)));
        assert_eq!(policy.focus(SEAT), Some(f.pid(window)));
    }
}

on_both_kinds! {
    fn a_window_that_opens_fullscreen_has_a_workspace_until_it_closes(kind: Kind) {
        let (mut f, client) = desk(kind, true);
        let other = f.map(client, WindowSpec::plain((400, 300)));

        let window = f.map(client, WindowSpec { fullscreen: true, ..WindowSpec::plain(OUTPUT) });

        assert!(f.state.caminito.policy.is_fullscreen(f.pid(window)));
        assert_eq!(fullscreen_workspace(&f), Some(f.pid(window).0));
        assert!(!f.state.caminito.policy.is_showing(f.pid(other)));

        destroy(&mut f, window);

        assert_eq!(fullscreen_workspace(&f), None);
        assert!(slides_with(&f, other), "{:?}", slides(&f));
        assert_eq!(f.state.caminito.policy.focus(SEAT), Some(f.pid(other)));
    }
}

on_both_kinds! {
    fn the_key_is_read_live_and_off_a_fullscreen_window_stays_on_its_workspace(kind: Kind) {
        let (mut f, client) = desk(kind, false);
        let other = f.map(client, WindowSpec::plain((400, 300)));
        let window = f.map(client, WindowSpec::plain((400, 300)));

        set_fullscreen(&mut f, window, true);

        assert_eq!(fullscreen_workspace(&f), None);
        assert!(slides(&f).is_empty());
        assert!(f.state.caminito.policy.is_showing(f.pid(other)));

        set_fullscreen(&mut f, window, false);
        f.set_setting(CAMINITO, "fullscreen-workspaces", Setting::Bool(true));
        f.dispatch_until(|f| f.state.caminito.settings.policy.fullscreen_workspaces);
        set_fullscreen(&mut f, window, true);

        assert_eq!(fullscreen_workspace(&f), Some(f.pid(window).0));
    }
}

on_both_kinds! {
    fn a_window_opened_from_a_fullscreen_workspace_opens_on_its_home_workspace(kind: Kind) {
        let (mut f, client) = desk(kind, true);
        let other = f.map(client, WindowSpec::plain((400, 300)));
        let game = f.map(client, WindowSpec { fullscreen: true, ..WindowSpec::plain(OUTPUT) });
        assert_eq!(fullscreen_workspace(&f), Some(f.pid(game).0));

        let window = f.map(client, WindowSpec::plain((400, 300)));

        assert_eq!(fullscreen_workspace(&f), None);
        assert!(slides_with(&f, other), "{:?}", slides(&f));
        let policy = &f.state.caminito.policy;
        assert!(policy.is_showing(f.pid(window)));
        assert!(!policy.is_showing(f.pid(game)));
        assert_eq!(policy.focus(SEAT), Some(f.pid(window)));
    }
}
