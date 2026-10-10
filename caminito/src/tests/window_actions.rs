//! The window and workspace actions of Marco's keybindings (149), each from its default
//! binding, or from one the test sets where Marco's default is `disabled`. The commands'
//! are in `commands.rs`.

use super::fixture::{
    ClientId, Decoration, Fixture, KEY_E, KEY_F5, KEY_F9, KEY_F10, KEY_LEFT, KEY_LEFTALT,
    KEY_LEFTCTRL, KEY_LEFTMETA, KEY_LEFTSHIFT, KEY_Q, KEY_R, KEY_RIGHT, KEY_T, KEY_W, Kind,
    WindowSpec, on_both_kinds,
};
use crate::settings::Value;
use crate::settings::translate::{MARCO_GLOBAL_KEYBINDINGS, MARCO_WINDOW_KEYBINDINGS};
use crate::state::SEAT;

/// One 1920 × 1080 output and a connected client of `kind`, with these keys of
/// `window-keybindings` or `global-keybindings` set before Caminito reads them.
fn bound(kind: Kind, keys: &[(&'static str, &str, &str)]) -> (Fixture, ClientId) {
    let initial = keys
        .iter()
        .map(|(schema, key, value)| (*schema, (*key).to_owned(), Value::Str((*value).into())))
        .collect();
    let mut f = Fixture::with_settings(initial);
    f.add_output((1920, 1080), 1.0);
    let id = f.connect(kind);
    (f, id)
}

const WINDOW: &str = MARCO_WINDOW_KEYBINDINGS;
const GLOBAL: &str = MARCO_GLOBAL_KEYBINDINGS;

on_both_kinds! {
    fn alt_f9_minimises_and_alt_f5_unmaximises_the_focused_window(kind: Kind) {
        let (mut f, id) = Fixture::connected(kind);
        let window = f.map(id, WindowSpec::plain((400, 300)));
        let pid = f.pid(window);
        f.chord(&[KEY_LEFTALT, KEY_F10]);
        assert!(f.state.caminito.policy.is_maximized(pid));

        f.chord(&[KEY_LEFTALT, KEY_F5]);
        assert!(!f.state.caminito.policy.is_maximized(pid));
        f.chord(&[KEY_LEFTALT, KEY_F9]);
        assert!(f.state.caminito.policy.is_minimized(pid));
    }
}

on_both_kinds! {
    fn ctrl_shift_alt_right_and_left_move_the_focused_window_with_the_workspace_and_the_focus(
        kind: Kind
    ) {
        let (mut f, id) = Fixture::connected(kind);
        let a = f.map(id, WindowSpec::plain((400, 300)));
        let b = f.map(id, WindowSpec::plain((400, 300)));
        let (pa, pb) = (f.pid(a), f.pid(b));

        f.chord(&[KEY_LEFTCTRL, KEY_LEFTSHIFT, KEY_LEFTALT, KEY_RIGHT]);
        let policy = &f.state.caminito.policy;
        assert_eq!(policy.workspace_of(pb), Some(1));
        assert_eq!(policy.active_workspace(), 1);
        assert_eq!(policy.focus(SEAT), Some(pb));
        assert!(f.has_keyboard(b));

        f.chord(&[KEY_LEFTCTRL, KEY_LEFTSHIFT, KEY_LEFTALT, KEY_LEFT]);
        let policy = &f.state.caminito.policy;
        assert_eq!(policy.workspace_of(pb), Some(0));
        assert_eq!(policy.workspace_of(pa), Some(0));
        assert_eq!(policy.active_workspace(), 0);
        assert_eq!(policy.focus(SEAT), Some(pb));
    }
}

on_both_kinds! {
    fn the_state_actions_marco_leaves_disabled_act_once_bound(kind: Kind) {
        let (mut f, id) = bound(kind, &[
            (WINDOW, "maximize", "<Super>q"),
            (WINDOW, "maximize-horizontally", "<Super>w"),
            (WINDOW, "maximize-vertically", "<Super>e"),
            (WINDOW, "toggle-fullscreen", "<Super>r"),
        ]);
        let window = f.map(id, WindowSpec::plain((400, 300)));
        let pid = f.pid(window);

        f.chord(&[KEY_LEFTMETA, KEY_Q]);
        assert!(f.state.caminito.policy.is_maximized(pid));
        f.chord(&[KEY_LEFTMETA, KEY_W]);
        assert!(!f.state.caminito.policy.is_maximized_horizontally(pid));
        assert!(f.state.caminito.policy.is_maximized_vertically(pid));
        f.chord(&[KEY_LEFTMETA, KEY_E]);
        assert!(!f.state.caminito.policy.is_maximized_vertically(pid));

        f.chord(&[KEY_LEFTMETA, KEY_R]);
        assert!(f.state.caminito.policy.is_fullscreen(pid));
        f.chord(&[KEY_LEFTMETA, KEY_R]);
        assert!(!f.state.caminito.policy.is_fullscreen(pid));
    }
}

on_both_kinds! {
    fn toggle_shaded_shades_an_ssd_window_and_leaves_a_csd_one(kind: Kind) {
        let (mut f, id) = bound(kind, &[(WINDOW, "toggle-shaded", "<Super>q")]);
        let csd = f.map(id, WindowSpec {
            decoration: Decoration::Client,
            ..WindowSpec::default()
        });
        f.chord(&[KEY_LEFTMETA, KEY_Q]);
        assert!(!f.state.caminito.policy.is_shaded(f.pid(csd)));

        let ssd = f.map(id, WindowSpec::default());
        f.chord(&[KEY_LEFTMETA, KEY_Q]);
        assert!(f.state.caminito.policy.is_shaded(f.pid(ssd)));
        f.chord(&[KEY_LEFTMETA, KEY_Q]);
        assert!(!f.state.caminito.policy.is_shaded(f.pid(ssd)));
    }
}

on_both_kinds! {
    fn raise_lower_and_raise_or_lower_restack_the_focused_window_once_bound(kind: Kind) {
        let (mut f, id) = bound(kind, &[
            (WINDOW, "raise", "<Super>q"),
            (WINDOW, "lower", "<Super>w"),
            (WINDOW, "raise-or-lower", "<Super>e"),
        ]);
        // Too wide to sit side by side, so they overlap.
        let a = f.map(id, WindowSpec::plain((1200, 600)));
        let b = f.map(id, WindowSpec::plain((1200, 600)));
        let (pa, pb) = (f.pid(a), f.pid(b));
        let stacking = |f: &Fixture| f.state.caminito.policy.stacking(0).to_vec();

        f.chord(&[KEY_LEFTMETA, KEY_W]);
        assert_eq!(stacking(&f), [pb, pa]);
        assert_eq!(f.state.caminito.policy.focus(SEAT), Some(pb));
        f.chord(&[KEY_LEFTMETA, KEY_Q]);
        assert_eq!(stacking(&f), [pa, pb]);

        // On top, so lowered; then overlapped, so raised.
        f.chord(&[KEY_LEFTMETA, KEY_E]);
        assert_eq!(stacking(&f), [pb, pa]);
        f.chord(&[KEY_LEFTMETA, KEY_E]);
        assert_eq!(stacking(&f), [pa, pb]);
    }
}

on_both_kinds! {
    fn the_numbered_workspace_actions_go_by_the_number_and_do_nothing_past_the_count(
        kind: Kind
    ) {
        // Marco's four workspaces.
        let (mut f, id) = bound(kind, &[
            (GLOBAL, "switch-to-workspace-3", "<Super>q"),
            (GLOBAL, "switch-to-workspace-5", "<Super>w"),
            (WINDOW, "move-to-workspace-4", "<Super>e"),
            (WINDOW, "move-to-workspace-12", "<Super>r"),
            (GLOBAL, "switch-to-workspace-1", "<Super>t"),
        ]);
        let a = f.map(id, WindowSpec::plain((400, 300)));
        let b = f.map(id, WindowSpec::plain((400, 300)));
        let (pa, pb) = (f.pid(a), f.pid(b));

        f.chord(&[KEY_LEFTMETA, KEY_R]);
        f.chord(&[KEY_LEFTMETA, KEY_W]);
        assert_eq!(f.state.caminito.policy.workspace_of(pb), Some(0));
        assert_eq!(f.state.caminito.policy.active_workspace(), 0);

        // The window goes, and the workspace and the focus stay.
        f.chord(&[KEY_LEFTMETA, KEY_E]);
        assert_eq!(f.state.caminito.policy.workspace_of(pb), Some(3));
        assert_eq!(f.state.caminito.policy.active_workspace(), 0);
        assert_eq!(f.state.caminito.policy.focus(SEAT), Some(pa));

        f.chord(&[KEY_LEFTMETA, KEY_Q]);
        assert_eq!(f.state.caminito.policy.active_workspace(), 2);
        f.chord(&[KEY_LEFTMETA, KEY_T]);
        assert_eq!(f.state.caminito.policy.active_workspace(), 0);
        assert_eq!(f.state.caminito.policy.focus(SEAT), Some(pa));
    }
}
