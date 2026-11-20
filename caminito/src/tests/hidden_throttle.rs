//! Windows that aren't showing are held to the throttled rate (CMN-316): a game on a
//! workspace the user has left has its fifo barriers released once a second, not at each
//! frame of its output.

use std::time::Duration;

use super::fixture::{
    ClientId, ClientWindow, Fixture, KEY_LEFTALT, KEY_LEFTCTRL, KEY_RIGHT, Kind, WindowSpec,
};

/// A game on workspace 0 and a window on workspace 1 that draws at each step.
fn game_and_other(f: &mut Fixture, id: ClientId) -> [ClientWindow; 2] {
    let game = f.map(id, WindowSpec::plain((400, 300)));
    let other = f.map(id, WindowSpec::plain((300, 200)));
    let other_id = f.pid(other);
    f.state.caminito.policy.move_to_workspace(other_id, 1);
    f.dispatch();
    [game, other]
}

/// How many of the game's frames are let through in a second of Caminito's clock, at
/// 20 ms steps, with `other` drawing at each one: the game draws again as soon as its last
/// frame is through, as a game presenting in FIFO mode does, waiting on the barrier the
/// last frame set. A frame callback goes with each, which comes once the frame is through.
fn game_frames_in_a_second(
    f: &mut Fixture,
    id: ClientId,
    game: ClientWindow,
    other: ClientWindow,
) -> usize {
    let start = f.client(id).window(game.xdg()).frames_done;
    let mut asked = start;
    for step in 0..50 {
        let client = f.client(id);
        if client.window(game.xdg()).frames_done == asked {
            asked += 1;
            client.request_frame(game.xdg());
            client.fifo_commit(game.xdg());
        }
        let colour = if step % 2 == 0 {
            0xff0000ff
        } else {
            0xff00ff00
        };
        f.client(id)
            .ack_and_commit_buffer(other.xdg(), (300, 200), colour);
        f.roundtrip(id);
        f.advance_clock(Duration::from_millis(20));
        f.roundtrip(id);
    }
    f.client(id).window(game.xdg()).frames_done - start
}

#[test]
fn a_window_on_a_workspace_the_user_left_gets_its_fifo_barriers_released_at_each_frame() {
    let (mut f, id) = Fixture::connected(Kind::Xdg);
    let [game, other] = game_and_other(&mut f, id);
    f.chord(&[KEY_LEFTCTRL, KEY_LEFTALT, KEY_RIGHT]);
    f.stop_clock();

    let got = game_frames_in_a_second(&mut f, id, game, other);
    assert!(got >= 20, "{got} frames in a second");
}
