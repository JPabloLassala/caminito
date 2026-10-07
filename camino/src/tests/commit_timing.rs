//! commit-timing: a commit held back until its timestamp (93). xdg only: Xwayland sets no
//! timestamps here.

use std::time::Duration;

use super::fixture::{Fixture, Kind, WindowSpec};

#[test]
fn a_commit_is_shown_at_its_timestamp_with_nothing_else_drawing() {
    let (mut f, id) = Fixture::connected(Kind::Xdg);
    let window = f.map(id, WindowSpec::plain((200, 100)));
    let at = f.geometry(window).loc;
    let at = (at.x + 5, at.y + 5);
    f.wait_for_pixel(at.0, at.1, 0x00ff00);

    // As a client pacing itself does: the frame is for the next refresh, and nothing
    // else gives Camino a reason to draw before then, or after.
    let time = Duration::from(f.state.camino.clock.now()) + Duration::from_millis(30);
    let xdg = window.xdg();
    let client = f.client(id);
    client.set_commit_time(xdg, time);
    client.ack_and_commit_buffer(xdg, (200, 100), 0xff0000ff);

    f.wait_for_pixel(at.0, at.1, 0x0000ff);
}
