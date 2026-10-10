//! Edge resistance and snap-move (157) through an Alt+drag, which feeds policy's grab the
//! same way a titlebar drag or a client's move request does. Both run with an xdg toplevel
//! and an XWayland window (45).

use caminito_test_client::layer_shell::LayerSpec;
use smithay::utils::{Logical, Point};

use super::fixture::{
    BTN_LEFT, ClientWindow, Fixture, KEY_LEFTALT, KEY_LEFTSHIFT, Kind, WindowSpec, on_both_kinds,
};

const BAR_COLOR: u32 = 0xff00ff00;

/// Starts an Alt+drag of `window` from 50,50 into its frame, and returns where it pressed.
fn grab(f: &mut Fixture, window: ClientWindow) -> Point<f64, Logical> {
    let grip = (f.visible_frame(window).loc + Point::from((50, 50))).to_f64();
    f.pointer_move(grip.x, grip.y);
    f.key(KEY_LEFTALT, true);
    f.pointer_button(BTN_LEFT, true);
    grip
}

/// Moves the pointer of a drag to `to`, and returns where the window's frame is then.
fn drag_to(f: &mut Fixture, window: ClientWindow, to: Point<f64, Logical>) -> Point<i32, Logical> {
    f.pointer_move(to.x, to.y);
    f.sync(window);
    f.visible_frame(window).loc
}

fn release(f: &mut Fixture) {
    f.pointer_button(BTN_LEFT, false);
    f.key(KEY_LEFTALT, false);
}

on_both_kinds! {
    fn a_drag_stops_at_the_work_areas_edge_by_a_panel_then_goes_past_it(kind: Kind) {
        let (mut f, client) = Fixture::connected(kind);
        let panel = f.connect(Kind::Xdg);
        f.map_layer(panel, LayerSpec::top_bar(28), BAR_COLOR);
        let window = f.map(client, WindowSpec::plain((400, 300)));
        let start = f.visible_frame(window).loc;
        let grip = grab(&mut f, window);
        // How far up the frame's top is from the work area's.
        let up = f64::from(start.y - 28);

        // The pointer would take the frame's top 10 under the panel.
        let at = drag_to(&mut f, window, grip - Point::from((0., up + 10.)));
        assert_eq!(at, (start.x, 28).into());

        // 32 under: it goes on.
        let at = drag_to(&mut f, window, grip - Point::from((0., up + 32.)));
        assert_eq!(at, (start.x, -4).into());
        release(&mut f);
    }
}

on_both_kinds! {
    fn a_shift_drag_snaps_to_another_window(kind: Kind) {
        let (mut f, client) = Fixture::connected(kind);
        let other = f.map(client, WindowSpec::plain((400, 300)));
        let window = f.map(client, WindowSpec::plain((400, 300)));
        let o = f.visible_frame(other);
        let start = f.visible_frame(window).loc;
        let grip = grab(&mut f, window);
        f.key(KEY_LEFTSHIFT, true);

        // The pointer would take the frame 10 over the other window's right side and 5
        // below its top, which are the edges nearest.
        let target = Point::from((o.loc.x + o.size.w - 10, o.loc.y + 5));
        let at = drag_to(&mut f, window, grip + (target - start).to_f64());

        assert_eq!(at, (o.loc.x + o.size.w, o.loc.y).into());
        f.key(KEY_LEFTSHIFT, false);
        release(&mut f);
    }
}
