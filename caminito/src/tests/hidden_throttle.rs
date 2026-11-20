//! Windows that aren't showing are held to the throttled rate (CMN-316): a game on a
//! workspace the user has left has its fifo and commit-timing barriers released once a
//! second, not at each frame of its output, and at each frame again once it shows. One
//! that only the Overview's bar shows has them at the bar's rate.

use std::time::Duration;

use super::fixture::{
    ClientId, ClientWindow, Fixture, KEY_LEFT, KEY_LEFTALT, KEY_LEFTCTRL, KEY_RIGHT, Kind,
    WindowSpec,
};
use super::overview::{is_open, open};

/// Caminito's clock moves on by this at each step: a frame of a 50 Hz output.
const STEP: Duration = Duration::from_millis(20);

/// A game and the window the user is looking at, on one client.
struct Scene {
    f: Fixture,
    id: ClientId,
    game: ClientWindow,
    other: ClientWindow,
    /// The game's frame callbacks when it last drew.
    asked: usize,
    steps: usize,
}

impl Scene {
    /// A game on workspace 0 and a window on workspace 1, with the user on workspace 1 and
    /// Caminito's clock stopped.
    fn left() -> Self {
        let (mut f, id) = Fixture::connected(Kind::Xdg);
        let game = f.map(id, WindowSpec::plain((400, 300)));
        let other = f.map(id, WindowSpec::plain((300, 200)));
        let other_id = f.pid(other);
        f.state.caminito.policy.move_to_workspace(other_id, 1);
        f.dispatch();
        f.chord(&[KEY_LEFTCTRL, KEY_LEFTALT, KEY_RIGHT]);
        f.stop_clock();
        Self {
            f,
            id,
            game,
            other,
            asked: 0,
            steps: 0,
        }
    }

    fn game_frames(&mut self) -> usize {
        self.f.client(self.id).window(self.game.xdg()).frames_done
    }

    /// One frame of the output: the game draws again if its last frame is through, as a
    /// game presenting in FIFO mode does, waiting on the barrier its last frame set, with
    /// a frame callback that comes once the frame is through. The window the user is
    /// looking at draws too, so the output draws a frame.
    fn step(&mut self) {
        let done = self.game_frames();
        let client = self.f.client(self.id);
        if done == self.asked {
            self.asked += 1;
            client.request_frame(self.game.xdg());
            client.fifo_commit(self.game.xdg());
        }
        let colour = [0xff0000ff, 0xff00ff00][self.steps % 2];
        self.steps += 1;
        let client = self.f.client(self.id);
        client.ack_and_commit_buffer(self.other.xdg(), (300, 200), colour);
        self.f.roundtrip(self.id);
        self.f.advance_clock(STEP);
        self.f.roundtrip(self.id);
    }

    /// How many of the game's frames are through in a second of Caminito's clock.
    fn game_frames_in_a_second(&mut self) -> usize {
        self.asked = self.game_frames();
        let start = self.asked;
        for _ in 0..50 {
            self.step();
        }
        self.game_frames() - start
    }
}

#[test]
fn a_window_on_a_workspace_the_user_left_gets_its_fifo_barriers_released_once_a_second() {
    let mut scene = Scene::left();

    let got = scene.game_frames_in_a_second();

    assert!((1..=2).contains(&got), "{got} frames in a second");
}

#[test]
fn a_window_that_shows_again_gets_its_fifo_barriers_released_at_each_frame_at_once() {
    let mut scene = Scene::left();
    scene.game_frames_in_a_second();
    // Its last frame waits on a barrier.
    assert_eq!(scene.game_frames(), scene.asked - 1);

    scene.f.chord(&[KEY_LEFTCTRL, KEY_LEFTALT, KEY_LEFT]);
    // The next frame lets it through, and the one after tells the game so.
    scene.step();
    scene.step();
    assert_eq!(scene.game_frames(), scene.asked);

    let got = scene.game_frames_in_a_second();
    assert!(got >= 20, "{got} frames in a second");
}

#[test]
fn a_timed_commit_of_a_window_that_is_not_showing_waits_for_the_throttle() {
    let mut scene = Scene::left();
    // A first step, at which the throttle is due.
    scene.step();
    let start = scene.game_frames();

    let time = Duration::from(scene.f.state.caminito.clock.now()) + Duration::from_millis(30);
    let client = scene.f.client(scene.id);
    client.set_commit_time(scene.game.xdg(), time);
    client.request_frame(scene.game.xdg());
    client.commit(scene.game.xdg());
    // So that the step doesn't draw the game again.
    scene.asked = usize::MAX;
    for _ in 0..10 {
        scene.step();
    }
    assert_eq!(scene.game_frames(), start, "held past its time");

    for _ in 0..45 {
        scene.step();
    }
    assert_eq!(scene.game_frames(), start + 1, "through within a second");
}

#[test]
fn a_window_only_the_overview_bar_shows_gets_its_fifo_barriers_released_ten_times_a_second() {
    let mut scene = Scene::left();
    open(&mut scene.f);

    let got = scene.game_frames_in_a_second();

    assert!((8..=12).contains(&got), "{got} frames in a second");
    assert!(is_open(&scene.f));
}
