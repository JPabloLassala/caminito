//! `caminito_toplevel_identifier_manager_v1` (CMN-243, ADR 0049): the Dock applet names a
//! window to Caminito by the handle libxfce4windowing gives it, and is answered with the
//! identifier the foreign toplevel list gives the same window.

use caminito_test_client::toplevel_identifier::IdentifierAnswer;

use super::fixture::{ClientId, ClientWindow, Fixture, Kind, WindowSpec, on_both_kinds};

/// A window titled `title` whose app is `app_id`.
fn titled(title: &'static str, app_id: &'static str) -> WindowSpec {
    WindowSpec {
        title: Some(title),
        app_id: Some(app_id),
        ..WindowSpec::default()
    }
}

/// A client that has bound foreign toplevel management and the foreign toplevel list, as
/// the applet has, and has seen the toplevels there are.
fn applet(f: &mut Fixture) -> ClientId {
    let applet = f.connect(Kind::Xdg);
    f.client(applet).bind_foreign_toplevels();
    f.client(applet).bind_foreign_toplevel_list();
    f.roundtrip(applet);
    applet
}

/// What Caminito answers the applet for the toplevel at `index`.
fn answer(f: &mut Fixture, applet: ClientId, index: usize) -> IdentifierAnswer {
    let asked = f.client(applet).ask_toplevel_identifier(index);
    f.roundtrip(applet);
    f.client(applet).toplevel_identifier(asked).clone()
}

#[test]
fn the_compositor_offers_the_protocol() {
    let (mut f, _) = Fixture::connected(Kind::Xdg);
    let applet = applet(&mut f);

    assert!(f.client(applet).has_toplevel_identifier_manager());
}

on_both_kinds! {
    fn the_answer_is_the_identifier_the_list_gives_the_window(kind: Kind) {
        let (mut f, client) = Fixture::connected(kind);
        f.map(client, titled("Caja", "caja"));
        let applet = applet(&mut f);

        let listed = f.client(applet).foreign_toplevels().listed[0].identifier.clone();
        let identifier = listed.expect("the list gives it an identifier");

        assert_eq!(
            answer(&mut f, applet, 0),
            IdentifierAnswer::Identifier(identifier.clone())
        );
    }
}

on_both_kinds! {
    fn two_windows_with_one_title_get_their_own_identifiers(kind: Kind) {
        let (mut f, client) = Fixture::connected(kind);
        f.map(client, titled("Terminal", "mate-terminal"));
        f.map(client, titled("Terminal", "mate-terminal"));
        let applet = applet(&mut f);

        let identifiers: Vec<_> = (0..2)
            .map(|index| match answer(&mut f, applet, index) {
                IdentifierAnswer::Identifier(identifier) => identifier,
                other => panic!("toplevel {index} got {other:?}"),
            })
            .collect();

        assert_ne!(identifiers[0], identifiers[1]);
        // The toplevels are announced in the order the windows mapped, and the list names
        // them the same.
        let listed = &f.client(applet).foreign_toplevels().listed;
        assert_eq!(listed[0].identifier.as_deref(), Some(identifiers[0].as_str()));
        assert_eq!(listed[1].identifier.as_deref(), Some(identifiers[1].as_str()));
    }
}

on_both_kinds! {
    fn the_handle_of_a_closed_window_is_answered_gone(kind: Kind) {
        let (mut f, client) = Fixture::connected(kind);
        let caja = f.map(client, titled("Caja", "caja"));
        let applet = applet(&mut f);
        f.client(applet).keep_closed_toplevel_handles();

        close(&mut f, caja);
        f.roundtrip(applet);

        assert!(f.client(applet).foreign_toplevels().toplevels[0].closed);
        assert_eq!(answer(&mut f, applet, 0), IdentifierAnswer::Gone);
    }
}

/// Has a window's client destroy it, and waits until Caminito no longer manages it.
fn close(f: &mut Fixture, window: ClientWindow) {
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
