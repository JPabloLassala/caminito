//! A window's functions (177): a client's request, a Pending window's states and a
//! keybinding are all refused a state the window has no function for, by Policy's setters.
//! The table of the functions is in `caminito-policy/src/tests/functions.rs`.

use caminito_theme::FrameType;

use super::fixture::{
    ClientId, ClientWindow, Decoration, Fixture, KEY_LEFTMETA, KEY_Q, Kind, WindowSpec,
    on_both_kinds,
};
use crate::settings::Value;
use crate::settings::translate::MARCO_WINDOW_KEYBINDINGS;

/// A window whose size hints fix its size.
fn fixed_size() -> WindowSpec {
    WindowSpec {
        fixed_size: true,
        ..WindowSpec::default()
    }
}

/// Sends a `_NET_WM_STATE` request to add `states`, as an X client does.
fn add_net_wm_state(f: &mut Fixture, window: ClientWindow, states: [&'static str; 2]) {
    let id = window.x11();
    f.x11_call(window.client(), move |x| {
        let first = x.atom(states[0])?;
        let second = x.atom(states[1])?;
        x.send_client_message(id, "_NET_WM_STATE", [1, first, second, 1, 0])
    })
    .unwrap();
}

/// Has `window`'s client ask to maximise it, and lets Caminito answer.
fn ask_to_maximize(f: &mut Fixture, window: ClientWindow) {
    match window {
        ClientWindow::Xdg(id, xdg) => f.client(id).set_maximized(xdg),
        ClientWindow::X11(..) => add_net_wm_state(
            f,
            window,
            [
                "_NET_WM_STATE_MAXIMIZED_VERT",
                "_NET_WM_STATE_MAXIMIZED_HORZ",
            ],
        ),
    }
    f.sync(window);
}

/// Has `window`'s client ask to make it fullscreen, and lets Caminito answer.
fn ask_for_fullscreen(f: &mut Fixture, window: ClientWindow) {
    match window {
        ClientWindow::Xdg(id, xdg) => f.client(id).set_fullscreen(xdg, None),
        ClientWindow::X11(..) => add_net_wm_state(
            f,
            window,
            ["_NET_WM_STATE_FULLSCREEN", "_NET_WM_STATE_FULLSCREEN"],
        ),
    }
    f.sync(window);
}

on_both_kinds! {
    fn a_clients_request_to_maximise_a_fixed_size_window_is_refused(kind: Kind) {
        let (mut f, id) = Fixture::connected(kind);
        let window = f.map(id, fixed_size());
        // Its Frame has no maximise button, and doesn't resize.
        let flags = f.frame_flags(window);
        assert!(!flags.allows_maximize);
        assert!(!flags.allows_horizontal_resize && !flags.allows_vertical_resize);
        assert!(flags.allows_minimize);

        ask_to_maximize(&mut f, window);

        let policy = &f.state.caminito.policy;
        assert!(!policy.is_maximized_horizontally(f.pid(window)));
        assert!(!policy.is_maximized_vertically(f.pid(window)));
        assert_eq!(f.geometry(window).size, (400, 300).into());

        // A window that resizes is maximised by the same request.
        let resizable = f.map(id, WindowSpec::default());
        ask_to_maximize(&mut f, resizable);
        f.dispatch_until(|f| f.state.caminito.policy.is_maximized(f.pid(resizable)));
    }
}

on_both_kinds! {
    fn a_fixed_size_window_that_asks_to_map_maximised_maps_unmaximised(kind: Kind) {
        let (mut f, id) = Fixture::connected(kind);

        let window = f.map(id, WindowSpec {
            maximized: true,
            ..fixed_size()
        });

        let policy = &f.state.caminito.policy;
        assert!(!policy.is_maximized_horizontally(f.pid(window)));
        assert!(!policy.is_maximized_vertically(f.pid(window)));
        assert_eq!(f.geometry(window).size, (400, 300).into());
    }
}

on_both_kinds! {
    fn a_fixed_size_window_that_asks_for_fullscreen_gets_it(kind: Kind) {
        let (mut f, id) = Fixture::connected(kind);
        let window = f.map(id, fixed_size());

        ask_for_fullscreen(&mut f, window);

        // Not as in Marco, which refuses a window that can't resize (177).
        f.dispatch_until(|f| f.state.caminito.policy.is_fullscreen(f.pid(window)));
    }
}

/// A modal dialog of `parent`, as each kind of client says it's modal: gtk-shell's
/// `set_modal`, or `_NET_WM_STATE_MODAL`.
fn modal_dialog(f: &mut Fixture, id: ClientId, kind: Kind, parent: ClientWindow) -> ClientWindow {
    let spec = WindowSpec {
        size: (200, 100),
        parent: Some(parent),
        ..WindowSpec::default()
    };
    match kind {
        Kind::Xdg => {
            let dialog = f.map(id, spec);
            f.client(id).set_modal(dialog.xdg());
            f.sync(dialog);
            dialog
        }
        Kind::X11 => f.map(
            id,
            WindowSpec {
                types: vec!["_NET_WM_WINDOW_TYPE_DIALOG"],
                states: vec!["_NET_WM_STATE_MODAL"],
                ..spec
            },
        ),
    }
}

on_both_kinds! {
    fn toggle_shaded_does_nothing_on_a_csd_window_and_on_an_attached_modal_dialog(kind: Kind) {
        let mut f = Fixture::with_settings(vec![
            (MARCO_WINDOW_KEYBINDINGS, "toggle-shaded".into(), Value::Str("<Super>q".into())),
        ]);
        f.set_attach_modal_dialogs(true);
        f.add_output((1920, 1080), 1.0);
        let id = f.connect(kind);

        let csd = f.map(id, WindowSpec {
            decoration: Decoration::Client,
            ..WindowSpec::default()
        });
        f.chord(&[KEY_LEFTMETA, KEY_Q]);
        assert!(!f.state.caminito.policy.is_shaded(f.pid(csd)));

        let parent = f.map(id, WindowSpec::default());
        let dialog = modal_dialog(&mut f, id, kind, parent);
        f.dispatch_until(|f| f.frame_type(dialog) == Some(FrameType::Attached));
        assert!(f.has_keyboard(dialog));
        f.chord(&[KEY_LEFTMETA, KEY_Q]);
        assert!(!f.state.caminito.policy.is_shaded(f.pid(dialog)));
        assert!(!f.state.caminito.policy.is_shaded(f.pid(parent)));

        // Let go, the dialog has a titlebar again, and shades.
        f.set_attach_modal_dialogs(false);
        f.dispatch_until(|f| f.frame_type(dialog) == Some(FrameType::ModalDialog));
        f.chord(&[KEY_LEFTMETA, KEY_Q]);
        assert!(f.state.caminito.policy.is_shaded(f.pid(dialog)));
    }
}
