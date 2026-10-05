//! The clipboard and primary selection between Wayland and X clients (11, 37). X clients
//! only reach a selection while one of Xwayland's windows has the keyboard, so every test
//! has one window of each kind, side by side, and focuses them by clicking.

use camino_test_client::data_control::Protocol;
use camino_test_client::selection::{Selection, TEXT};

use super::fixture::{ClientId, ClientWindow, Fixture, Kind, WindowSpec};

/// A Wayland client and an X client, each with a window, and neither focused.
struct Desk {
    f: Fixture,
    wayland: ClientId,
    wayland_window: ClientWindow,
    xwayland: ClientId,
    xwayland_window: ClientWindow,
}

fn desk() -> Desk {
    let (mut f, wayland) = Fixture::connected(Kind::Xdg);
    f.client(wayland).bind_selections();
    let wayland_window = f.map(wayland, WindowSpec::default());
    let xwayland = f.connect(Kind::X11);
    let xwayland_window = f.map(
        xwayland,
        WindowSpec {
            position: Some((1000, 600)),
            ..WindowSpec::default()
        },
    );
    assert!(
        !f.geometry(wayland_window)
            .overlaps(f.geometry(xwayland_window)),
        "the windows are side by side"
    );
    Desk {
        f,
        wayland,
        wayland_window,
        xwayland,
        xwayland_window,
    }
}

impl Desk {
    /// Clicks in the middle of `window`, which focuses it.
    fn focus(&mut self, window: ClientWindow) {
        let geometry = self.f.geometry(window);
        let middle = geometry.loc + geometry.size.downscale(2).to_point();
        self.f.click(middle.x.into(), middle.y.into());
        assert!(self.f.has_keyboard(window), "{window:?} is focused");
    }

    /// The Wayland client sets `selection` to `text`.
    fn wayland_copies(&mut self, selection: Selection, text: &str) {
        self.f.client(self.wayland).set_selection(selection, text);
        self.f.roundtrip(self.wayland);
    }

    /// The X client asks for `selection` as text into its window, as a paste does.
    fn x_pastes(&mut self, selection: Selection) -> Option<String> {
        let window = self.xwayland_window.x11();
        self.x_pastes_into(window, selection)
    }

    /// The X client asks for `selection` as text into `window`, one of its own.
    fn x_pastes_into(&mut self, window: u32, selection: Selection) -> Option<String> {
        let pasted = self
            .f
            .x11_call(self.xwayland, move |x| {
                x.convert_selection(window, x_name(selection), "UTF8_STRING")
            })
            .expect("the X client asks for the selection");
        pasted.map(|bytes| String::from_utf8(bytes).unwrap())
    }

    /// The X selection's owner.
    fn x_owner(&mut self, selection: Selection) -> u32 {
        self.f
            .x11_call(self.xwayland, move |x| x.selection_owner(x_name(selection)))
            .expect("the X client asks for the owner")
    }
}

/// The X selection's name.
fn x_name(selection: Selection) -> &'static str {
    match selection {
        Selection::Clipboard => "CLIPBOARD",
        Selection::Primary => "PRIMARY",
    }
}

/// Expands one test body, which takes the selection it runs on, into `<name>::clipboard` and
/// `<name>::primary`.
macro_rules! on_both_selections {
    (fn $name:ident($selection:ident: Selection) $body:block) => {
        mod $name {
            use super::*;

            fn body($selection: Selection) $body

            #[test]
            fn clipboard() {
                body(Selection::Clipboard);
            }

            #[test]
            fn primary() {
                body(Selection::Primary);
            }
        }
    };
}

on_both_selections! {
    fn a_wayland_selection_reaches_an_x_client_once_one_of_its_windows_is_focused(
        selection: Selection
    ) {
        let mut d = desk();
        d.focus(d.wayland_window);
        d.wayland_copies(selection, "from Wayland");

        assert_eq!(d.x_owner(selection), x11rb::NONE, "X hasn't been given it yet");

        d.focus(d.xwayland_window);
        assert_eq!(d.x_pastes(selection).as_deref(), Some("from Wayland"));
    }
}

on_both_selections! {
    fn an_x_selection_is_readable_by_a_wayland_client(selection: Selection) {
        let mut d = desk();
        d.focus(d.xwayland_window);
        let window = d.xwayland_window.x11();
        d.f.x11_call(d.xwayland, move |x| {
            x.set_selection_owner(window, x_name(selection))?;
            // Camino asks what it offers as soon as it hears it owns the selection.
            x.serve_selection("from X", "TARGETS")
        })
        .expect("the X client owns the selection");

        d.focus(d.wayland_window);
        let wayland = d.wayland;
        d.f.dispatch_until(|f| f.client_state(wayland).offered(selection).is_some());
        let offered = d.f.client_state(wayland).offered(selection).unwrap();
        assert!(offered.iter().any(|mime| mime == TEXT), "{offered:?}");

        let pipe = d.f.client(wayland).receive(selection, TEXT);
        d.f.x11_call(d.xwayland, |x| x.serve_selection("from X", "UTF8_STRING"))
            .expect("the X client writes the selection");
        assert_eq!(d.f.read_to_end(pipe), b"from X");
    }
}

on_both_selections! {
    fn an_x_client_without_the_keyboard_gets_nothing_from_the_selection(selection: Selection) {
        let mut d = desk();
        d.focus(d.wayland_window);
        d.wayland_copies(selection, "secret");
        d.focus(d.xwayland_window);
        d.focus(d.wayland_window);

        assert_eq!(d.x_pastes(selection), None);
    }
}

on_both_selections! {
    fn an_x_menu_that_grabs_the_keyboard_can_paste_the_selection(selection: Selection) {
        let mut d = desk();
        d.focus(d.wayland_window);
        d.wayland_copies(selection, "from Wayland");
        // The menu's window was clicked first, which gave X the selection; while the menu
        // holds the keyboard, its window doesn't.
        d.focus(d.xwayland_window);

        let menu = d.f.map_override_redirect(d.xwayland, (1000, 100), (100, 100), 0xff0000);
        d.f.x11_call(d.xwayland, move |x| x.grab_keyboard(menu))
            .expect("the menu grabs the keyboard");
        d.f.dispatch_until(|f| f.override_redirect_with_keyboard() == Some(menu));

        assert_eq!(d.x_pastes_into(menu, selection).as_deref(), Some("from Wayland"));
    }
}

on_both_selections! {
    fn a_data_control_client_keeps_an_x_selection_after_its_x_client_exits(selection: Selection) {
        let mut d = desk();
        let manager = d.f.connect(Kind::Xdg);
        d.f.client(manager).bind_data_control(Protocol::Ext);
        d.f.roundtrip(manager);

        d.focus(d.xwayland_window);
        let window = d.xwayland_window.x11();
        d.f.x11_call(d.xwayland, move |x| {
            x.set_selection_owner(window, x_name(selection))?;
            x.serve_selection("from X", "TARGETS")
        })
        .expect("the X client owns the selection");
        let offered = |f: &Fixture, id| f.client_state(id).offered_through_data_control(selection);
        d.f.dispatch_until(|f| offered(f, manager).is_some_and(|mimes| mimes.contains(&TEXT.into())));

        // The manager takes the selection over, as msd's clipboard plugin does.
        let pipe = d.f.client(manager).receive_through_data_control(selection, TEXT);
        d.f.x11_call(d.xwayland, |x| x.serve_selection("from X", "UTF8_STRING"))
            .expect("the X client writes the selection");
        let text = String::from_utf8(d.f.read_to_end(pipe)).unwrap();
        assert_eq!(text, "from X");
        d.f.client(manager).set_selection_through_data_control(selection, &text);
        d.f.roundtrip(manager);

        d.f.disconnect(d.xwayland);
        let x_window = d.xwayland_window;
        d.f.dispatch_until(|f| f.policy_id(x_window).is_none());
        d.focus(d.wayland_window);
        let wayland = d.wayland;
        d.f.dispatch_until(|f| f.client_state(wayland).offered(selection).is_some());
        let pipe = d.f.client(wayland).receive(selection, TEXT);
        assert_eq!(d.f.read_to_end(pipe), b"from X");
    }
}

on_both_selections! {
    fn a_selection_set_through_data_control_reaches_x_once_an_x_window_is_focused(
        selection: Selection
    ) {
        let mut d = desk();
        d.focus(d.wayland_window);
        let manager = d.f.connect(Kind::Xdg);
        d.f.client(manager).bind_data_control(Protocol::Wlr);
        d.f.client(manager).set_selection_through_data_control(selection, "from a manager");
        d.f.roundtrip(manager);

        assert_eq!(d.x_owner(selection), x11rb::NONE, "X hasn't been given it yet");

        d.focus(d.xwayland_window);
        assert_eq!(d.x_pastes(selection).as_deref(), Some("from a manager"));
    }
}
