//! data-control (68): a clipboard manager is offered the selections and sets them without
//! the keyboard, which is how the clipboard outlives the client that copied. The windows are
//! xdg toplevels only: `selection` tests data-control with an X client's selection.

use camino_test_client::data_control::Protocol;
use camino_test_client::selection::{Selection, TEXT};

use super::fixture::{ClientId, Fixture, Kind, WindowSpec};

/// Expands one test body, which takes the data-control protocol and the selection it runs
/// on, into `<name>::ext_clipboard`, `ext_primary`, `wlr_clipboard` and `wlr_primary`.
macro_rules! on_both_protocols_and_selections {
    (fn $name:ident($protocol:ident: Protocol, $selection:ident: Selection) $body:block) => {
        mod $name {
            use super::*;

            fn body($protocol: Protocol, $selection: Selection) $body

            #[test]
            fn ext_clipboard() {
                body(Protocol::Ext, Selection::Clipboard);
            }

            #[test]
            fn ext_primary() {
                body(Protocol::Ext, Selection::Primary);
            }

            #[test]
            fn wlr_clipboard() {
                body(Protocol::Wlr, Selection::Clipboard);
            }

            #[test]
            fn wlr_primary() {
                body(Protocol::Wlr, Selection::Primary);
            }
        }
    };
}

/// A client with the one window, which has the keyboard, and a data-control client with no
/// window.
fn owner_and_manager(protocol: Protocol) -> (Fixture, ClientId, ClientId) {
    let (mut f, owner) = Fixture::connected(Kind::Xdg);
    let window = f.map(owner, WindowSpec::default());
    assert!(f.has_keyboard(window));
    let manager = f.connect(Kind::Xdg);
    f.client(manager).bind_data_control(protocol);
    f.roundtrip(manager);
    (f, owner, manager)
}

/// Whether data-control offers `manager` the selection as text.
fn is_offered_text(f: &Fixture, manager: ClientId, selection: Selection) -> bool {
    let offered = f
        .client_state(manager)
        .offered_through_data_control(selection);
    offered.is_some_and(|mime_types| mime_types.iter().any(|mime| mime == TEXT))
}

/// What `manager` reads of the selection through data-control.
fn read(f: &mut Fixture, manager: ClientId, selection: Selection) -> String {
    f.dispatch_until(|f| is_offered_text(f, manager, selection));
    let pipe = f
        .client(manager)
        .receive_through_data_control(selection, TEXT);
    String::from_utf8(f.read_to_end(pipe)).unwrap()
}

on_both_protocols_and_selections! {
    fn a_data_control_client_is_offered_what_another_client_copies(
        protocol: Protocol,
        selection: Selection
    ) {
        let (mut f, owner, manager) = owner_and_manager(protocol);
        assert!(!is_offered_text(&f, manager, selection), "nothing is copied yet");

        f.client(owner).set_selection(selection, "copied");
        f.roundtrip(owner);

        assert_eq!(read(&mut f, manager, selection), "copied");
    }
}

on_both_protocols_and_selections! {
    fn a_selection_a_data_control_client_took_over_outlives_its_first_owner(
        protocol: Protocol,
        selection: Selection
    ) {
        let (mut f, owner, manager) = owner_and_manager(protocol);
        f.client(owner).set_selection(selection, "kept");
        f.roundtrip(owner);

        let text = read(&mut f, manager, selection);
        f.client(manager).set_selection_through_data_control(selection, &text);
        f.roundtrip(manager);
        f.disconnect(owner);

        let paster = f.connect(Kind::Xdg);
        f.client(paster).bind_selections();
        let window = f.map(paster, WindowSpec::default());
        assert!(f.has_keyboard(window));
        f.dispatch_until(|f| f.client_state(paster).offered(selection).is_some());
        let pipe = f.client(paster).receive(selection, TEXT);
        assert_eq!(f.read_to_end(pipe), b"kept");
    }
}
