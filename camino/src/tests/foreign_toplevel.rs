//! wlr-foreign-toplevel-management and ext-foreign-toplevel-list (65): the windows a panel's
//! window list sees, what it's told as they change, and what its requests do to them.

use camino_test_client::foreign_toplevel::format_toplevels;
use insta::{allow_duplicates, assert_snapshot};
use smithay::utils::Rectangle;

use super::fixture::{
    ClientId, ClientWindow, Fixture, KEY_LEFTALT, KEY_LEFTCTRL, KEY_RIGHT, KEY_TAB, Kind,
    WindowSpec, on_both_kinds,
};

/// A window titled `title` whose app is `app_id`.
fn titled(title: &'static str, app_id: &'static str) -> WindowSpec {
    WindowSpec {
        title: Some(title),
        app_id: Some(app_id),
        ..WindowSpec::default()
    }
}

/// A client that has bound the outputs and foreign toplevel management, as a panel does,
/// and has seen the toplevels there are.
fn panel(f: &mut Fixture) -> ClientId {
    let client = f.connect(Kind::Xdg);
    f.client(client).bind_outputs();
    f.client(client).bind_foreign_toplevels();
    f.roundtrip(client);
    client
}

/// One output, a client of `kind` with a window of Caja's and then one of Pluma's, which
/// has the focus, and a panel that has seen both and was told nothing since.
fn two_windows_and_a_panel(kind: Kind) -> (Fixture, ClientWindow, ClientWindow, ClientId) {
    let (mut f, client) = Fixture::connected(kind);
    let caja = f.map(client, titled("Caja", "caja"));
    let pluma = f.map(client, titled("Pluma", "pluma"));
    let panel = panel(&mut f);
    f.client(panel).take_toplevel_events();
    (f, caja, pluma, panel)
}

/// The toplevels as the panel knows them now.
fn toplevels(f: &mut Fixture, panel: ClientId) -> String {
    f.roundtrip(panel);
    format_toplevels(&f.client(panel).foreign_toplevels().toplevels)
}

/// What the panel was told since this was last asked.
fn events(f: &mut Fixture, panel: ClientId) -> String {
    f.roundtrip(panel);
    f.client(panel).take_toplevel_events()
}

/// The panel's index for the toplevel titled `title`.
fn toplevel(f: &mut Fixture, panel: ClientId, title: &str) -> usize {
    f.client(panel).foreign_toplevels().titled(title)
}

/// Has a window's client change its title, and waits until Camino knows.
fn set_title(f: &mut Fixture, window: ClientWindow, title: &'static str) {
    match window {
        ClientWindow::Xdg(id, xdg) => {
            f.client(id).set_title(xdg, title);
            f.client(id).commit(xdg);
            f.roundtrip(id);
        }
        ClientWindow::X11(id, x11) => {
            f.x11_call(id, move |x| x.set_string(x11, "_NET_WM_NAME", title))
                .unwrap();
            f.dispatch_until(|f| f.mapped(window).title() == title);
        }
    }
}

/// Has a window's client destroy it, and waits until Camino no longer manages it.
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

on_both_kinds! {
    fn a_client_that_binds_late_gets_every_window(kind: Kind) {
        let (mut f, client) = Fixture::connected(kind);
        f.map(client, titled("Caja", "caja"));
        f.map(client, titled("Pluma", "pluma"));

        let panel = panel(&mut f);

        // The newest window has the focus. An XWayland window's app id is its `WM_CLASS`.
        allow_duplicates! {
            assert_snapshot!(f.client(panel).take_toplevel_events(), @"
            toplevel 0: new
            toplevel 0: title Caja
            toplevel 0: app_id caja
            toplevel 0: output_enter headless-1
            toplevel 0: state []
            toplevel 0: done
            toplevel 1: new
            toplevel 1: title Pluma
            toplevel 1: app_id pluma
            toplevel 1: output_enter headless-1
            toplevel 1: state [Activated]
            toplevel 1: done
            ");
        }
    }
}

on_both_kinds! {
    fn a_new_window_is_announced_to_a_client_that_s_bound(kind: Kind) {
        let (mut f, client) = Fixture::connected(kind);
        let panel = panel(&mut f);

        f.map(client, titled("Caja", "caja"));

        allow_duplicates! {
            assert_snapshot!(toplevels(&mut f, panel), @r#"Caja (caja): [Activated] on ["headless-1"]"#);
        }
    }
}

on_both_kinds! {
    fn a_title_change_sends_one_update(kind: Kind) {
        let (mut f, caja, _, panel) = two_windows_and_a_panel(kind);

        set_title(&mut f, caja, "Home");

        allow_duplicates! {
            assert_snapshot!(events(&mut f, panel), @"
            toplevel 0: title Home
            toplevel 0: done
            ");
        }
    }
}

on_both_kinds! {
    fn a_focus_change_sends_each_window_one_update(kind: Kind) {
        let (mut f, _, _, panel) = two_windows_and_a_panel(kind);

        // Alt+Tab, to Caja.
        f.chord(&[KEY_LEFTALT, KEY_TAB]);

        allow_duplicates! {
            assert_snapshot!(events(&mut f, panel), @"
            toplevel 0: state [Activated]
            toplevel 0: done
            toplevel 1: state []
            toplevel 1: done
            ");
        }
    }
}

on_both_kinds! {
    fn minimising_and_maximising_each_send_one_update(kind: Kind) {
        let (mut f, caja, _, panel) = two_windows_and_a_panel(kind);

        f.client(panel).set_toplevel_maximized(0, true);
        allow_duplicates! {
            assert_snapshot!(events(&mut f, panel), @"
            toplevel 0: state [Maximized]
            toplevel 0: done
            ");
        }
        assert!(f.state.camino.policy.is_maximized(f.pid(caja)));

        f.client(panel).set_toplevel_minimized(0, true);
        allow_duplicates! {
            assert_snapshot!(events(&mut f, panel), @"
            toplevel 0: state [Maximized, Minimized]
            toplevel 0: done
            ");
        }
        assert!(f.state.camino.policy.is_minimized(f.pid(caja)));

        f.client(panel).set_toplevel_maximized(0, false);
        allow_duplicates! {
            assert_snapshot!(events(&mut f, panel), @"
            toplevel 0: state [Minimized]
            toplevel 0: done
            ");
        }
        assert!(!f.state.camino.policy.is_maximized(f.pid(caja)));
    }
}

on_both_kinds! {
    fn a_burst_of_changes_sends_one_update(kind: Kind) {
        let (mut f, _, _, panel) = two_windows_and_a_panel(kind);

        // Pluma has the focus, and loses it as it's minimised.
        f.client(panel).set_toplevel_maximized(1, true);
        f.client(panel).set_toplevel_minimized(1, true);

        allow_duplicates! {
            assert_snapshot!(events(&mut f, panel), @"
            toplevel 0: state [Activated]
            toplevel 0: done
            toplevel 1: state [Maximized, Minimized]
            toplevel 1: done
            ");
        }
    }
}

on_both_kinds! {
    fn a_closed_window_s_handle_gets_closed(kind: Kind) {
        let (mut f, _, pluma, panel) = two_windows_and_a_panel(kind);

        destroy(&mut f, pluma);

        // Caja takes the focus.
        allow_duplicates! {
            assert_snapshot!(events(&mut f, panel), @"
            toplevel 1: closed
            toplevel 0: state [Activated]
            toplevel 0: done
            ");
        }
    }
}

on_both_kinds! {
    fn activating_a_minimised_window_on_another_workspace_brings_it_back(kind: Kind) {
        let (mut f, caja, _, panel) = two_windows_and_a_panel(kind);
        f.client(panel).set_toplevel_minimized(0, true);
        f.roundtrip(panel);
        f.chord(&[KEY_LEFTCTRL, KEY_LEFTALT, KEY_RIGHT]);
        assert_eq!(f.state.camino.policy.active_workspace(), 1);
        f.client(panel).take_toplevel_events();

        f.client(panel).activate_toplevel(0);

        allow_duplicates! {
            assert_snapshot!(events(&mut f, panel), @"
            toplevel 0: state [Activated]
            toplevel 0: done
            ");
        }
        let policy = &f.state.camino.policy;
        assert_eq!(policy.active_workspace(), 0);
        assert!(!policy.is_minimized(f.pid(caja)));
        f.sync(caja);
        assert!(f.has_keyboard(caja));
    }
}

on_both_kinds! {
    fn unminimising_restores_and_focuses_the_window(kind: Kind) {
        let (mut f, caja, _, panel) = two_windows_and_a_panel(kind);
        f.client(panel).set_toplevel_minimized(0, true);
        f.roundtrip(panel);

        f.client(panel).set_toplevel_minimized(0, false);

        assert_eq!(
            toplevels(&mut f, panel),
            "Caja (caja): [Activated] on [\"headless-1\"]\n\
             Pluma (pluma): [] on [\"headless-1\"]"
        );
        assert!(!f.state.camino.policy.is_minimized(f.pid(caja)));
    }
}

on_both_kinds! {
    fn close_asks_the_window_s_client_to_close_it(kind: Kind) {
        let (mut f, caja, pluma, panel) = two_windows_and_a_panel(kind);

        f.client(panel).close_toplevel(0);
        f.roundtrip(panel);
        f.sync(caja);

        assert!(f.close_requested(caja));
        assert!(!f.close_requested(pluma));
    }
}

on_both_kinds! {
    fn set_fullscreen_fills_the_output_named(kind: Kind) {
        let (mut f, client) = Fixture::connected(kind);
        let second = f.add_output((1280, 720), 1.0);
        let caja = f.map(client, titled("Caja", "caja"));
        let panel = panel(&mut f);
        f.client(panel).take_toplevel_events();

        // The panel's second `wl_output`.
        f.client(panel).set_toplevel_fullscreen(0, Some(1));

        allow_duplicates! {
            assert_snapshot!(events(&mut f, panel), @"
            toplevel 0: output_leave headless-1
            toplevel 0: output_enter headless-2
            toplevel 0: state [Activated, Fullscreen]
            toplevel 0: done
            ");
        }
        f.draw_at_asked_size(caja, (1280, 720));
        assert_eq!(f.geometry(caja), crate::output::output_geometry(&second));

        f.client(panel).unset_toplevel_fullscreen(0);
        f.roundtrip(panel);
        assert!(!f.state.camino.policy.is_fullscreen(f.pid(caja)));
    }
}

on_both_kinds! {
    fn a_second_client_gets_its_own_handles_and_outlives_the_first(kind: Kind) {
        let (mut f, client) = Fixture::connected(kind);
        f.map(client, titled("Caja", "caja"));
        let first = panel(&mut f);
        let second = panel(&mut f);
        let expected = "Caja (caja): [Activated] on [\"headless-1\"]";
        assert_eq!(toplevels(&mut f, first), expected);
        assert_eq!(toplevels(&mut f, second), expected);

        // Each acts through its own handle.
        f.client(second).set_toplevel_maximized(0, true);
        let expected = "Caja (caja): [Maximized, Activated] on [\"headless-1\"]";
        assert_eq!(toplevels(&mut f, second), expected);
        assert_eq!(toplevels(&mut f, first), expected);

        f.disconnect(first);
        f.map(client, titled("Pluma", "pluma"));

        assert_eq!(
            toplevels(&mut f, second),
            "Caja (caja): [Maximized] on [\"headless-1\"]\n\
             Pluma (pluma): [Activated] on [\"headless-1\"]"
        );
    }
}

on_both_kinds! {
    fn a_toplevel_enters_an_output_the_client_binds_later(kind: Kind) {
        let (mut f, client) = Fixture::connected(kind);
        f.map(client, titled("Caja", "caja"));
        let panel = f.connect(Kind::Xdg);
        f.client(panel).bind_foreign_toplevels();
        allow_duplicates! {
            assert_snapshot!(events(&mut f, panel), @"
            toplevel 0: new
            toplevel 0: title Caja
            toplevel 0: app_id caja
            toplevel 0: state [Activated]
            toplevel 0: done
            ");
        }

        f.client(panel).bind_outputs();

        allow_duplicates! {
            assert_snapshot!(events(&mut f, panel), @"
            toplevel 0: output_enter headless-1
            toplevel 0: done
            ");
        }
    }
}

on_both_kinds! {
    fn the_rectangle_a_client_shows_a_window_at_is_kept(kind: Kind) {
        let (mut f, client) = Fixture::connected(kind);
        let caja = f.map(client, titled("Caja", "caja"));
        let panel = panel(&mut f);
        let bar = f.map(panel, WindowSpec::plain((800, 30)));
        f.roundtrip(panel);
        let index = toplevel(&mut f, panel, "Caja");
        let rectangle = |f: &Fixture| f.mapped(caja).minimize_rectangle().map(|(_, r)| r);

        f.client(panel)
            .set_toplevel_rectangle(index, bar.xdg(), (10, 0, 100, 30));
        f.roundtrip(panel);
        assert_eq!(
            rectangle(&f),
            Some(Rectangle::new((10, 0).into(), (100, 30).into()))
        );

        // An empty rectangle takes it back.
        f.client(panel)
            .set_toplevel_rectangle(index, bar.xdg(), (0, 0, 0, 0));
        f.roundtrip(panel);
        assert_eq!(rectangle(&f), None);
    }
}

on_both_kinds! {
    fn the_foreign_toplevel_list_names_each_window_by_a_stable_identifier(kind: Kind) {
        let (mut f, client) = Fixture::connected(kind);
        let caja = f.map(client, titled("Caja", "caja"));
        let lister = f.connect(Kind::Xdg);
        f.client(lister).bind_foreign_toplevel_list();
        f.roundtrip(lister);

        let listed = &f.client(lister).foreign_toplevels().listed[0];
        assert_eq!(listed.title.as_deref(), Some("Caja"));
        assert_eq!(listed.app_id.as_deref(), Some("caja"));
        assert_eq!(listed.updates, 1);
        let identifier = listed.identifier.clone().expect("it has an identifier");
        let camino = &f.state.camino;
        assert_eq!(camino.toplevel_identifier(f.pid(caja)), Some(identifier.clone()));
        assert_eq!(
            camino.window_for_toplevel_identifier(&identifier),
            Some(f.pid(caja))
        );

        set_title(&mut f, caja, "Home");
        f.roundtrip(lister);
        let listed = &f.client(lister).foreign_toplevels().listed[0];
        assert_eq!(listed.title.as_deref(), Some("Home"));
        assert_eq!(listed.identifier.as_deref(), Some(identifier.as_str()));
        assert_eq!(listed.updates, 2);

        destroy(&mut f, caja);
        f.roundtrip(lister);
        assert!(f.client(lister).foreign_toplevels().listed[0].closed);
        assert_eq!(f.state.camino.window_for_toplevel_identifier(&identifier), None);
    }
}

/// An xdg toplevel with a parent is a dialog (35), which skips the taskbar by Marco's rules,
/// as an X11 dialog does.
#[test]
fn a_dialog_of_a_window_isn_t_listed() {
    let (mut f, client) = Fixture::connected(Kind::Xdg);
    let caja = f.map(client, titled("Caja", "caja"));
    let dialog = WindowSpec {
        parent: Some(caja),
        ..titled("Properties", "caja")
    };
    f.map(client, dialog);

    let panel = panel(&mut f);

    // The dialog has the focus.
    assert_snapshot!(toplevels(&mut f, panel), @r#"Caja (caja): [] on ["headless-1"]"#);
}

/// Only an XWayland window can be transient for another and still be a normal window, which
/// the taskbar shows: it says so with `_NET_WM_WINDOW_TYPE`.
#[test]
fn a_transient_is_told_its_parent() {
    let (mut f, client) = Fixture::connected(Kind::X11);
    let caja = f.map(client, titled("Caja", "caja"));
    let transient = WindowSpec {
        parent: Some(caja),
        types: vec!["_NET_WM_WINDOW_TYPE_NORMAL"],
        ..titled("Search", "caja")
    };
    f.map(client, transient);

    let panel = panel(&mut f);

    assert_snapshot!(f.client(panel).take_toplevel_events(), @"
    toplevel 0: new
    toplevel 0: title Caja
    toplevel 0: app_id caja
    toplevel 0: output_enter headless-1
    toplevel 0: state []
    toplevel 0: done
    toplevel 1: new
    toplevel 1: title Search
    toplevel 1: app_id caja
    toplevel 1: output_enter headless-1
    toplevel 1: state [Activated]
    toplevel 1: parent toplevel 0
    toplevel 1: done
    ");

    // Its parent goes, and it has none.
    destroy(&mut f, caja);
    assert_snapshot!(events(&mut f, panel), @"
    toplevel 0: closed
    toplevel 1: parent none
    toplevel 1: done
    ");
}

/// Only an XWayland window can ask to skip the taskbar.
#[test]
fn an_xwayland_window_that_skips_the_taskbar_isn_t_listed() {
    let (mut f, client) = Fixture::connected(Kind::X11);
    f.map(client, titled("Caja", "Caja"));
    let skipping = WindowSpec {
        states: vec!["_NET_WM_STATE_SKIP_TASKBAR"],
        ..titled("Clock", "Mate-panel")
    };
    f.map(client, skipping);

    let panel = panel(&mut f);

    // The app id is `WM_CLASS`'s class, as it's written.
    assert_snapshot!(toplevels(&mut f, panel), @r#"Caja (Caja): [] on ["headless-1"]"#);
}

/// Only an XWayland window can start to skip the taskbar while it's managed.
#[test]
fn a_handle_that_s_closed_is_inert_though_its_window_stays() {
    let (mut f, client) = Fixture::connected(Kind::X11);
    let caja = f.map(client, titled("Caja", "Caja"));
    let panel = panel(&mut f);
    f.client(panel).keep_closed_toplevel_handles();
    f.client(panel).take_toplevel_events();

    let id = caja.x11();
    f.x11_call(client, move |x| {
        let taskbar = x.atom("_NET_WM_STATE_SKIP_TASKBAR")?;
        x.send_client_message(id, "_NET_WM_STATE", [1, taskbar, 0, 1, 0])
    })
    .unwrap();
    f.dispatch_until(|f| f.mapped(caja).skips().taskbar);
    assert_snapshot!(events(&mut f, panel), @"toplevel 0: closed");

    f.client(panel).set_toplevel_minimized(0, true);
    f.client(panel).close_toplevel(0);
    f.roundtrip(panel);
    f.sync(caja);

    assert!(!f.state.camino.policy.is_minimized(f.pid(caja)));
    assert!(!f.close_requested(caja));
}

on_both_kinds! {
    fn a_client_that_stops_keeps_its_toplevels_and_gets_no_new_ones(kind: Kind) {
        let (mut f, caja, pluma, panel) = two_windows_and_a_panel(kind);

        f.client(panel).stop_foreign_toplevels();
        f.roundtrip(panel);
        f.map(caja.client(), titled("Atril", "atril"));
        set_title(&mut f, caja, "Home");
        destroy(&mut f, pluma);

        // Atril takes the focus from Pluma, and the client isn't told of it.
        allow_duplicates! {
            assert_snapshot!(events(&mut f, panel), @"
            toplevel 1: state []
            toplevel 1: done
            toplevel 0: title Home
            toplevel 0: done
            toplevel 1: closed
            ");
        }
    }
}

/// An xdg toplevel with a parent is a dialog, which the taskbar doesn't show. The foreign
/// toplevel list has it, since M6 names every window by its identifier there.
#[test]
fn the_foreign_toplevel_list_has_a_dialog_too() {
    let (mut f, client) = Fixture::connected(Kind::Xdg);
    let caja = f.map(client, titled("Caja", "caja"));
    let dialog = WindowSpec {
        parent: Some(caja),
        ..titled("Properties", "caja")
    };
    let dialog = f.map(client, dialog);
    let lister = f.connect(Kind::Xdg);
    f.client(lister).bind_foreign_toplevel_list();
    f.roundtrip(lister);

    let listed = &f.client(lister).foreign_toplevels().listed;
    let titles: Vec<_> = listed.iter().map(|t| t.title.as_deref().unwrap()).collect();
    assert_eq!(titles, ["Caja", "Properties"]);
    assert!(f.state.camino.toplevel_identifier(f.pid(dialog)).is_some());
}
