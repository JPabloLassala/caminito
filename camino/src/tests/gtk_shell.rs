//! `gtk_shell1` (67): what GTK3 says through it of a window's modality, its bell, and the
//! requests Camino takes and ignores. Only xdg toplevels are mapped, because an XWayland window
//! says it's modal with `_NET_WM_STATE_MODAL`, which `xwayland_windows` tests.

use camino_policy::{PolicyWindow, WindowType};
use camino_theme::FrameType;

use super::fixture::{ClientId, ClientWindow, Fixture, Kind, WindowSpec};

/// One output, and a framed dialog over its framed 600 × 400 parent, as pluma's *Open* is
/// over its window. The `attached` style isn't tested here: no MATE schema has
/// `attach-modal-dialogs`, and `decoration::style`'s tests have it.
fn a_dialog() -> (Fixture, ClientId, ClientWindow, ClientWindow) {
    let mut f = Fixture::new();
    f.add_output((1920, 1080), 1.0);
    let client = f.connect(Kind::Xdg);
    let parent = f.map(
        client,
        WindowSpec {
            size: (600, 400),
            ..WindowSpec::default()
        },
    );
    let dialog = f.map(
        client,
        WindowSpec {
            size: (200, 100),
            parent: Some(parent),
            ..WindowSpec::default()
        },
    );
    (f, client, parent, dialog)
}

/// Clicks a corner of `parent`, which a dialog centred over it doesn't cover.
fn click_corner(f: &mut Fixture, parent: ClientWindow) {
    let corner = f.geometry(parent).loc;
    f.click(f64::from(corner.x + 10), f64::from(corner.y + 10));
}

#[test]
fn set_modal_makes_a_transient_a_modal_dialog_and_unset_modal_takes_it_back() {
    let (mut f, client, _, dialog) = a_dialog();
    assert_eq!(f.mapped(dialog).window_type(), WindowType::Dialog);
    assert_eq!(f.frame_type(dialog), Some(FrameType::Dialog));

    f.client(client).set_modal(dialog.xdg());
    f.sync(dialog);

    assert_eq!(f.mapped(dialog).window_type(), WindowType::ModalDialog);
    assert_eq!(f.frame_type(dialog), Some(FrameType::ModalDialog));

    f.client(client).unset_modal(dialog.xdg());
    f.sync(dialog);

    assert_eq!(f.mapped(dialog).window_type(), WindowType::Dialog);
    assert_eq!(f.frame_type(dialog), Some(FrameType::Dialog));
}

#[test]
fn a_window_that_is_modal_before_it_maps_maps_as_a_modal_dialog() {
    let mut f = Fixture::new();
    f.add_output((1920, 1080), 1.0);
    let client = f.connect(Kind::Xdg);
    let parent = f.map(client, WindowSpec::default());

    // GTK3 says a window is modal when it makes the toplevel, before its first buffer.
    let window = f.client(client).create_window();
    f.client(client).set_parent(window, Some(parent.xdg()));
    f.client(client).set_modal(window);
    f.client(client).commit(window);
    f.roundtrip(client);
    f.client(client)
        .ack_and_commit_buffer(window, (200, 100), 0xff00ff00);
    f.roundtrip(client);

    let dialog = ClientWindow::Xdg(client, window);
    assert_eq!(f.mapped(dialog).window_type(), WindowType::ModalDialog);

    click_corner(&mut f, parent);

    assert!(f.has_keyboard(dialog));
}

#[test]
fn the_parent_of_a_modal_dialog_cant_be_focused_over_it() {
    let (mut f, client, parent, dialog) = a_dialog();
    f.client(client).set_modal(dialog.xdg());
    f.sync(dialog);

    click_corner(&mut f, parent);

    assert!(f.has_keyboard(dialog));
    assert!(!f.has_keyboard(parent));

    f.client(client).unset_modal(dialog.xdg());
    f.sync(dialog);
    click_corner(&mut f, parent);

    assert!(f.has_keyboard(parent));
}

#[test]
fn the_bell_is_accepted_for_a_window_and_for_none() {
    let (mut f, client, _, dialog) = a_dialog();

    f.client(client).system_bell(Some(dialog.xdg()));
    f.client(client).system_bell(None);

    // A protocol error would end the client here.
    f.roundtrip(client);
    assert_eq!(f.mapped(dialog).window_type(), WindowType::Dialog);
}

#[test]
fn the_requests_camino_ignores_raise_no_protocol_error() {
    let (mut f, client, parent, dialog) = a_dialog();
    f.client(client).set_modal(dialog.xdg());

    f.client(client).send_other_gtk_requests(dialog.xdg());
    f.client(client).send_other_gtk_requests(parent.xdg());

    // A protocol error would end the client here.
    f.sync(dialog);
    // The window is as it was.
    assert_eq!(f.mapped(dialog).window_type(), WindowType::ModalDialog);
    assert_eq!(f.mapped(parent).window_type(), WindowType::Normal);
}
