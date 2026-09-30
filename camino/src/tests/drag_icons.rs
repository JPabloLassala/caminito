//! A Wayland drag's icon (48): drawn under the pointer at its offset, following it, and
//! gone once the drag ends. X clients draw their own drag icons in override-redirect
//! windows, so only an xdg client is tested.

use camino_test_client::selection::DragIconSpec;
use smithay::utils::{Logical, Point};

use super::fixture::{BTN_LEFT, ClientId, ClientWindow, Fixture, Kind, WindowSpec};
use crate::scene::{LAYERS, SceneLayer};

/// A 16 × 16 blue icon, its top left corner 20 pixels up and left of the pointer, so the
/// cursor, which hangs down and right of it, doesn't cover it.
const BLUE_ICON: DragIconSpec = DragIconSpec {
    size: (16, 16),
    argb: 0xff0000ff,
    offset: (-20, -20),
};

/// A red icon left of [`BLUE_ICON`], clear of it and of the cursor.
const RED_ICON: DragIconSpec = DragIconSpec {
    argb: 0xffff0000,
    offset: (-50, -20),
    ..BLUE_ICON
};

/// An xdg client with a window, the pointer pressed in the window's middle, and where the
/// test has put the pointer since.
struct Press {
    f: Fixture,
    client: ClientId,
    window: ClientWindow,
    /// The press's serial, as the client saw it.
    serial: u32,
    pointer: Point<i32, Logical>,
}

impl Press {
    fn in_a_window(mut f: Fixture, client: ClientId) -> Self {
        f.client(client).bind_input();
        let window = f.map(client, WindowSpec::plain((400, 300)));
        let geometry = f.geometry(window);
        let pointer = geometry.loc + geometry.size.downscale(2).to_point();
        f.pointer_move(pointer.x.into(), pointer.y.into());
        f.pointer_button(BTN_LEFT, true);
        f.roundtrip(client);
        let serial = f.client_state(client).last_press_serial.unwrap();
        Press {
            f,
            client,
            window,
            serial,
            pointer,
        }
    }

    /// A press in a window of a new xdg client on one 1920 × 1080 output, dragging `icon`.
    fn dragging(icon: DragIconSpec) -> Self {
        let (f, client) = Fixture::connected(Kind::Xdg);
        let mut press = Self::in_a_window(f, client);
        press.start_drag(icon);
        press
    }

    /// The client starts a drag with `icon` from the press.
    fn start_drag(&mut self, icon: DragIconSpec) {
        let (window, serial) = (self.window.xdg(), self.serial);
        self.f
            .client(self.client)
            .start_drag(window, serial, "dragged", Some(icon));
        self.f.roundtrip(self.client);
    }

    fn move_by(&mut self, dx: i32, dy: i32) {
        self.f.pointer_move_by(dx.into(), dy.into());
        self.pointer += Point::from((dx, dy));
    }

    /// The first output's pixel at the logical point `at`.
    fn pixel(&mut self, at: Point<i32, Logical>) -> u32 {
        let output = self.f.state.camino.outputs[0].clone();
        let at = at.to_physical_precise_round(output.current_scale().fractional_scale());
        self.f
            .state
            .backend
            .headless_mut()
            .pixel(&output, at)
            .expect("the output is rendered")
    }

    /// The pixel in the middle of where `icon` is with the pointer where it is now.
    fn middle_of(&mut self, icon: DragIconSpec) -> u32 {
        let (w, h) = icon.size;
        let at = self.pointer + Point::from(icon.offset) + Point::from((w / 2, h / 2));
        self.pixel(at)
    }
}

#[test]
fn a_drag_icon_is_drawn_at_the_pointer_plus_its_offset_and_follows_it_until_the_drop() {
    let (f, client) = Fixture::connected(Kind::Xdg);
    let mut p = Press::in_a_window(f, client);
    assert_ne!(
        p.middle_of(BLUE_ICON),
        BLUE_ICON.argb,
        "nothing's blue before the drag"
    );

    p.start_drag(BLUE_ICON);
    assert_eq!(p.middle_of(BLUE_ICON), BLUE_ICON.argb);

    let before = p.pointer;
    p.move_by(60, 40);
    assert_eq!(p.middle_of(BLUE_ICON), BLUE_ICON.argb);
    let left_behind = before + Point::from(BLUE_ICON.offset) + Point::from((8, 8));
    assert_ne!(
        p.pixel(left_behind),
        BLUE_ICON.argb,
        "the icon moved with the pointer"
    );

    // The icon's own offset moves it too, from where it was.
    p.f.client(client).offset_drag_icon(-30, 10);
    p.f.roundtrip(client);
    let moved = DragIconSpec {
        offset: (-50, -10),
        ..BLUE_ICON
    };
    assert_eq!(p.middle_of(moved), BLUE_ICON.argb);
    assert_ne!(p.middle_of(BLUE_ICON), BLUE_ICON.argb);

    p.f.pointer_button(BTN_LEFT, false);
    assert_ne!(
        p.middle_of(moved),
        BLUE_ICON.argb,
        "the drop took the icon away"
    );
}

#[test]
fn a_drag_icon_is_drawn_over_everything_but_the_cursor() {
    let mut p = Press::dragging(BLUE_ICON);

    let output = p.f.state.camino.outputs[0].clone();
    let mut layers = p.f.state.scene_layers(&output);
    layers.dedup();
    assert_eq!(
        layers,
        [
            SceneLayer::Cursor,
            SceneLayer::DragIcon,
            SceneLayer::Windows
        ]
    );
    assert_eq!(LAYERS[..2], [SceneLayer::Cursor, SceneLayer::DragIcon]);
}

#[test]
fn a_cancelled_drag_takes_its_icon_away() {
    let mut p = Press::dragging(BLUE_ICON);
    assert_eq!(p.middle_of(BLUE_ICON), BLUE_ICON.argb);

    // A second drag from the same press cancels the first, and shows its own icon.
    p.start_drag(RED_ICON);
    assert_ne!(
        p.middle_of(BLUE_ICON),
        BLUE_ICON.argb,
        "the cancel took the icon away"
    );
    assert_eq!(p.middle_of(RED_ICON), RED_ICON.argb);
}

#[test]
fn a_drag_icon_gets_frame_callbacks() {
    let mut p = Press::dragging(BLUE_ICON);
    let client = p.client;

    p.f.client(client).request_drag_icon_frame();
    p.f.dispatch_until(|f| f.client_state(client).drag_icon_frames_done > 0);
}

#[test]
fn a_drag_icon_is_drawn_at_the_scale_of_the_output_under_the_pointer() {
    let mut f = Fixture::new();
    let output = f.add_output((1920, 1080), 2.0);
    let client = f.connect(Kind::Xdg);
    let mut p = Press::in_a_window(f, client);
    p.start_drag(BLUE_ICON);

    // The icon's 16 logical pixels are 32 physical ones, from twice its logical corner.
    // Its outermost physical pixels are blended with the window by the upscaling filter.
    let corner = (p.pointer + Point::from(BLUE_ICON.offset)).to_physical(2);
    let mut physical = |dx: i32, dy: i32| {
        let at = corner + Point::from((dx, dy));
        p.f.state.backend.headless_mut().pixel(&output, at).unwrap()
    };
    assert_eq!(physical(1, 1), BLUE_ICON.argb);
    assert_eq!(physical(30, 30), BLUE_ICON.argb);
    assert_ne!(physical(32, 32), BLUE_ICON.argb);
    assert_ne!(physical(-1, -1), BLUE_ICON.argb);
}
