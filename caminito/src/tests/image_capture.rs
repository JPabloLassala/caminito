//! Image capture (103): what a screenshot tool copies of an output and of a window through
//! ext-image-capture-source and ext-image-copy-capture, and what its sessions are told as
//! their sources change.

use caminito_test_client::image_capture::{
    Constraints, FailureReason, Format, Image, Outcome, SessionId,
};
use smithay::utils::{Logical, Point, Rectangle};

use super::fixture::{
    BTN_LEFT, ClientId, ClientWindow, Fixture, KEY_LEFTALT, KEY_LEFTCTRL, KEY_RIGHT, Kind,
    WINDOW_COLOR, WindowSpec, on_both_kinds,
};
use crate::backend::headless::argb8888;
use crate::scene::{CLEAR_COLOR, LOCK_BACKGROUND};
use crate::settings::Value;
use crate::settings::translate::MARCO_GENERAL;

const RED: u32 = 0xffff0000;
const WHITE: u32 = 0xffffffff;

/// From the pointer to a pixel inside the built-in arrow, just below its tip, which is
/// white.
const IN_THE_ARROW: (i32, i32) = (3, 8);

/// A client that has bound what a screenshot tool binds, the outputs, the foreign toplevel
/// list and image capture, and has seen the outputs and toplevels there are.
fn screenshot_tool(f: &mut Fixture) -> ClientId {
    let tool = f.connect(Kind::Xdg);
    f.client(tool).bind_outputs();
    f.client(tool).bind_foreign_toplevel_list();
    f.client(tool).bind_image_capture();
    f.roundtrip(tool);
    tool
}

/// Opens a session on the first output, and returns it once it has its constraints.
fn output_session(f: &mut Fixture, tool: ClientId, paint_cursors: bool) -> SessionId {
    let session = f.client(tool).capture_output(0, paint_cursors);
    f.roundtrip(tool);
    session
}

/// Opens a session on a window, which the tool finds in the foreign toplevel list, and
/// returns it once it has its constraints.
fn window_session(
    f: &mut Fixture,
    tool: ClientId,
    window: ClientWindow,
    paint_cursors: bool,
) -> SessionId {
    f.roundtrip(tool);
    let identifier = f.state.caminito.toplevel_identifier(f.pid(window));
    let listed = &f.client_state(tool).foreign_toplevels.listed;
    let listed = listed
        .iter()
        .position(|toplevel| !toplevel.closed && toplevel.identifier == identifier)
        .expect("the window is in the foreign toplevel list");
    let session = f.client(tool).capture_toplevel(listed, paint_cursors);
    f.roundtrip(tool);
    session
}

/// How the session's last frame ended, if it has.
fn outcome(f: &Fixture, tool: ClientId, session: SessionId) -> Option<Outcome> {
    let session = f.client_state(tool).capture_session(session);
    session.last_frame().outcome
}

/// Asks for a frame, and returns its pixels once it's ready.
#[track_caller]
fn frame(f: &mut Fixture, tool: ClientId, session: SessionId) -> Image {
    f.client(tool).request_capture_frame(session);
    f.dispatch_until(|f| outcome(f, tool, session).is_some());
    assert_eq!(outcome(f, tool, session), Some(Outcome::Ready));
    let session = f.client_state(tool).capture_session(session);
    session.last_frame().image()
}

/// One frame of the first output, as a screenshot of it is.
#[track_caller]
fn capture_output(f: &mut Fixture, tool: ClientId, paint_cursors: bool) -> Image {
    let session = output_session(f, tool, paint_cursors);
    frame(f, tool, session)
}

/// One frame of a window, as a screenshot of it is.
#[track_caller]
fn capture_window(
    f: &mut Fixture,
    tool: ClientId,
    window: ClientWindow,
    paint_cursors: bool,
) -> Image {
    let session = window_session(f, tool, window, paint_cursors);
    frame(f, tool, session)
}

/// The sizes a session has asked its buffers to be, oldest first.
fn sizes(f: &Fixture, tool: ClientId, session: SessionId) -> Vec<(i32, i32)> {
    let session = f.client_state(tool).capture_session(session);
    session.constraints.iter().map(|c| c.size).collect()
}

/// What the first output shows.
fn output_pixels(f: &mut Fixture) -> Image {
    let output = f.state.caminito.outputs[0].clone();
    let size = output.current_mode().unwrap().size;
    let pixels = f.state.backend.headless_mut().pixels(&output);
    Image {
        size: (size.w, size.h),
        pixels: pixels.expect("the output was drawn"),
    }
}

fn center(rect: Rectangle<i32, Logical>) -> Point<i32, Logical> {
    rect.loc + rect.size.downscale(2).to_point()
}

/// A point on a window's titlebar, globally: the middle of the frame's top.
fn title_point(f: &Fixture, window: ClientWindow) -> Point<i32, Logical> {
    let geometry = f.geometry(window);
    let top = f.frame_extents(window).top;
    Point::from((
        geometry.loc.x + geometry.size.w / 2,
        geometry.loc.y - top / 2,
    ))
}

fn pointer_to(f: &mut Fixture, point: Point<i32, Logical>) {
    f.pointer_move(f64::from(point.x), f64::from(point.y));
}

/// Drags a window by its titlebar until the middle of its geometry is at `to`.
fn drag_center_to(f: &mut Fixture, window: ClientWindow, to: Point<i32, Logical>) {
    let grip = title_point(f, window);
    let delta = to - center(f.geometry(window));
    pointer_to(f, grip);
    f.pointer_button(BTN_LEFT, true);
    pointer_to(f, grip + delta);
    f.pointer_button(BTN_LEFT, false);
    f.sync(window);
}

/// Has a window's client draw it at `size`, and waits until Caminito has it at that size.
fn resize(f: &mut Fixture, window: ClientWindow, size: (i32, i32)) {
    match window {
        ClientWindow::Xdg(id, xdg) => {
            f.client(id).ack_and_commit_buffer(xdg, size, WINDOW_COLOR);
            f.roundtrip(id);
        }
        ClientWindow::X11(id, x11) => {
            let (w, h) = (size.0 as u32, size.1 as u32);
            f.x11_call(id, move |x| {
                let at = x.position(x11)?;
                x.configure(x11, at, (w, h))
            })
            .unwrap();
            f.dispatch_until(|f| f.geometry(window).size == size.into());
        }
    }
}

/// Has a window's client destroy it, and waits until Caminito no longer manages it.
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

/// Where the middle of a window's content is in a capture of the window, which starts at
/// its frame's top left corner.
fn content_center(f: &Fixture, window: ClientWindow) -> (i32, i32) {
    let extents = f.frame_extents(window);
    let size = f.geometry(window).size;
    (extents.left + size.w / 2, extents.top + size.h / 2)
}

on_both_kinds! {
    fn an_output_capture_is_what_the_output_shows(kind: Kind) {
        let (mut f, client) = Fixture::connected(kind);
        let window = f.map(client, WindowSpec::default());
        let tool = screenshot_tool(&mut f);

        let capture = capture_output(&mut f, tool, true);

        assert_eq!(capture.size, (1920, 1080));
        let inside = center(f.geometry(window));
        assert_eq!(capture.at(inside.x, inside.y), WINDOW_COLOR);
        assert_eq!(capture.at(1919, 1079), argb8888(CLEAR_COLOR));
        assert!(
            capture == output_pixels(&mut f),
            "the capture is the output's pixels"
        );
    }
}

on_both_kinds! {
    /// The size is the frame's, which M8's shadow mustn't grow.
    fn a_window_capture_is_the_window_with_its_frame(kind: Kind) {
        let (mut f, client) = Fixture::connected(kind);
        let window = f.map(client, WindowSpec::default());
        let tool = screenshot_tool(&mut f);
        // The cursor is clear of the window on the output the capture is compared with.
        f.pointer_move(1900., 1000.);

        let capture = capture_window(&mut f, tool, window, false);

        let frame = f.visible_frame(window);
        assert!(f.frame_extents(window).top > 0, "the window has a frame");
        assert_eq!(capture.size, (frame.size.w, frame.size.h));
        let (x, y) = content_center(&f, window);
        assert_eq!(capture.at(x, y), WINDOW_COLOR);
        assert_eq!(capture.at(frame.size.w / 2, 2) >> 24, 0xff, "the titlebar is drawn");
        // Where the capture is opaque it's what the output shows of the window. Elsewhere,
        // as at the frame's rounded corners, it shows nothing through.
        let output = output_pixels(&mut f);
        for y in 0..frame.size.h {
            for x in 0..frame.size.w {
                let pixel = capture.at(x, y);
                if pixel >> 24 == 0xff {
                    let shown = output.at(frame.loc.x + x, frame.loc.y + y);
                    assert_eq!(pixel, shown, "at ({x}, {y})");
                }
            }
        }
    }
}

on_both_kinds! {
    fn a_window_capture_leaves_the_shadow_out(kind: Kind) {
        let (mut f, client) = Fixture::connected(kind);
        let window = f.map(client, WindowSpec::default());
        let tool = screenshot_tool(&mut f);
        f.pointer_move(1900., 1000.);

        let capture = capture_window(&mut f, tool, window, false);

        // Menta's top corners are rounded. The output shows the shadow in the pixel the
        // corner cuts out and beside the frame, and the capture shows nothing there (171).
        let frame = f.visible_frame(window);
        assert!(f.mapped(window).frame().shadow_rect().is_some());
        let output = output_pixels(&mut f);
        let clear = argb8888(CLEAR_COLOR);
        assert_ne!(output.at(frame.loc.x, frame.loc.y), clear, "the shadow is drawn");
        assert_ne!(output.at(frame.loc.x + frame.size.w + 2, frame.loc.y + 40), clear);
        assert_eq!(capture.at(0, 0), 0);
        assert_eq!(capture.size, (frame.size.w, frame.size.h));
    }
}

on_both_kinds! {
    fn a_window_capture_is_the_same_when_another_window_covers_it(kind: Kind) {
        let (mut f, client) = Fixture::connected(kind);
        let window = f.map(client, WindowSpec::default());
        let other = f.connect(Kind::Xdg);
        let spec = WindowSpec {
            size: (600, 500),
            ..WindowSpec::default()
        };
        let cover = f.map(other, spec);
        f.client(other)
            .ack_and_commit_buffer(cover.xdg(), (600, 500), RED);
        f.roundtrip(other);
        let tool = screenshot_tool(&mut f);
        let middle = center(f.geometry(window));

        drag_center_to(&mut f, cover, (1500, 700).into());
        assert!(!f.visible_frame(cover).overlaps(f.visible_frame(window)));
        let uncovered = capture_window(&mut f, tool, window, false);

        drag_center_to(&mut f, cover, middle);
        assert_eq!(output_pixels(&mut f).at(middle.x, middle.y), RED);
        let covered = capture_window(&mut f, tool, window, false);

        let (x, y) = content_center(&f, window);
        assert_eq!(covered.at(x, y), WINDOW_COLOR);
        assert!(covered == uncovered, "the cover isn't in the capture");
    }
}

on_both_kinds! {
    fn a_capture_at_an_output_scale_of_2_has_twice_the_pixels(kind: Kind) {
        let mut f = Fixture::new();
        f.add_output((1920, 1080), 2.0);
        let client = f.connect(kind);
        let window = f.map(client, WindowSpec::default());
        let tool = screenshot_tool(&mut f);

        let of_window = capture_window(&mut f, tool, window, false);
        let of_output = capture_output(&mut f, tool, false);

        let frame = f.visible_frame(window);
        assert_eq!(of_window.size, (frame.size.w * 2, frame.size.h * 2));
        let (x, y) = content_center(&f, window);
        assert_eq!(of_window.at(x * 2, y * 2), WINDOW_COLOR);
        // An output's capture is of its mode's size, with the window drawn twice as large.
        assert_eq!(of_output.size, (1920, 1080));
        let inside = center(f.geometry(window));
        assert_eq!(of_output.at(inside.x * 2, inside.y * 2), WINDOW_COLOR);
    }
}

#[test]
fn an_output_capture_has_the_cursor_only_if_the_session_paints_cursors() {
    let (mut f, _) = Fixture::connected(Kind::Xdg);
    let tool = screenshot_tool(&mut f);
    f.pointer_move(100., 100.);
    let (x, y) = (100 + IN_THE_ARROW.0, 100 + IN_THE_ARROW.1);

    let with = capture_output(&mut f, tool, true);
    let without = capture_output(&mut f, tool, false);

    assert_eq!(with.at(x, y), WHITE);
    assert_eq!(without.at(x, y), argb8888(CLEAR_COLOR));
}

on_both_kinds! {
    /// The pointer is on the titlebar, where the cursor is Caminito's own for both kinds.
    fn a_window_capture_has_the_cursor_only_if_the_session_paints_cursors(kind: Kind) {
        let (mut f, client) = Fixture::connected(kind);
        let window = f.map(client, WindowSpec::default());
        let tool = screenshot_tool(&mut f);
        let pointer = title_point(&f, window);
        pointer_to(&mut f, pointer);
        let in_capture = pointer - f.visible_frame(window).loc;
        let (x, y) = (in_capture.x + IN_THE_ARROW.0, in_capture.y + IN_THE_ARROW.1);

        let with = capture_window(&mut f, tool, window, true);
        let without = capture_window(&mut f, tool, window, false);

        assert_eq!(with.at(x, y), WHITE);
        assert_ne!(without.at(x, y), WHITE);
    }
}

on_both_kinds! {
    fn a_hidden_window_s_capture_has_no_cursor(kind: Kind) {
        let (mut f, client) = Fixture::connected(kind);
        let window = f.map(client, WindowSpec::default());
        let tool = screenshot_tool(&mut f);
        let pointer = title_point(&f, window);
        pointer_to(&mut f, pointer);
        let in_capture = pointer - f.visible_frame(window).loc;
        let (x, y) = (in_capture.x + IN_THE_ARROW.0, in_capture.y + IN_THE_ARROW.1);

        f.chord(&[KEY_LEFTCTRL, KEY_LEFTALT, KEY_RIGHT]);
        assert!(!f.state.caminito.policy.is_showing(f.pid(window)));
        let capture = capture_window(&mut f, tool, window, true);

        assert_ne!(capture.at(x, y), WHITE);
    }
}

on_both_kinds! {
    fn resizing_the_window_sends_the_session_new_constraints(kind: Kind) {
        let (mut f, client) = Fixture::connected(kind);
        let window = f.map(client, WindowSpec::default());
        let tool = screenshot_tool(&mut f);
        let session = window_session(&mut f, tool, window, false);
        let before = f.visible_frame(window).size;
        let constraints = &f.client_state(tool).capture_session(session).constraints;
        assert_eq!(
            constraints[..],
            [Constraints {
                size: (before.w, before.h),
                shm_formats: vec![Format::Argb8888, Format::Xrgb8888],
                dmabuf: false,
            }]
        );

        resize(&mut f, window, (500, 350));
        f.roundtrip(tool);

        let after = f.visible_frame(window).size;
        assert_eq!(after, before + (100, 50).into());
        assert_eq!(
            sizes(&f, tool, session),
            [(before.w, before.h), (after.w, after.h)]
        );
        assert_eq!(frame(&mut f, tool, session).size, (after.w, after.h));
    }
}

on_both_kinds! {
    fn closing_the_window_stops_the_session(kind: Kind) {
        let (mut f, client) = Fixture::connected(kind);
        let window = f.map(client, WindowSpec::default());
        let tool = screenshot_tool(&mut f);
        let session = window_session(&mut f, tool, window, false);
        frame(&mut f, tool, session);
        // The session's next frame waits for the window to change.
        f.client(tool).request_capture_frame(session);
        f.roundtrip(tool);

        destroy(&mut f, window);
        f.roundtrip(tool);

        assert!(f.client_state(tool).capture_session(session).stopped);
        assert_eq!(
            outcome(&f, tool, session),
            Some(Outcome::Failed(FailureReason::Stopped))
        );
    }
}

#[test]
fn unplugging_the_output_stops_the_session() {
    let mut f = Fixture::new();
    f.add_output((1920, 1080), 1.0);
    let other = f.add_output((1280, 720), 1.0);
    let tool = screenshot_tool(&mut f);
    let first = f.client(tool).capture_output(0, false);
    let second = f.client(tool).capture_output(1, false);
    f.roundtrip(tool);
    assert_eq!(sizes(&f, tool, second), [(1280, 720)]);

    f.unplug(&other);
    f.roundtrip(tool);

    assert!(f.client_state(tool).capture_session(second).stopped);
    assert!(!f.client_state(tool).capture_session(first).stopped);
}

#[test]
fn changing_the_output_s_mode_sends_the_session_new_constraints() {
    let mut f = Fixture::new();
    let output = f.plug_with_modes((0, 0), &[(1920, 1080), (1280, 720)]);
    let tool = screenshot_tool(&mut f);
    let session = output_session(&mut f, tool, false);

    f.change_mode(&output, (1280, 720));
    f.roundtrip(tool);

    assert_eq!(sizes(&f, tool, session), [(1920, 1080), (1280, 720)]);
}

/// xdg only: the change is a subsurface's commit, which says what part of the window it
/// damages, and Xwayland's surfaces have none.
#[test]
fn a_session_s_next_frame_waits_for_the_window_to_change_and_says_what_did() {
    let (mut f, client) = Fixture::connected(Kind::Xdg);
    let window = f.map(client, WindowSpec::plain((400, 300)));
    let part = f
        .client(client)
        .add_subsurface(window.xdg(), (10, 20), (50, 40), WHITE);
    f.client(client).commit(window.xdg());
    let tool = screenshot_tool(&mut f);
    let session = window_session(&mut f, tool, window, false);
    let damage = |f: &Fixture| {
        let session = f.client_state(tool).capture_session(session);
        session.last_frame().damage.clone()
    };

    let first = frame(&mut f, tool, session);
    assert_eq!(first.size, (400, 300));
    assert_eq!(first.at(10, 20), WHITE);
    assert_eq!(damage(&f), [(0, 0, 400, 300)]);

    // Caminito has handled the request, and drawn, by the second roundtrip.
    f.client(tool).request_capture_frame(session);
    f.roundtrip(tool);
    f.roundtrip(tool);
    assert_eq!(outcome(&f, tool, session), None);

    f.client(client)
        .commit_subsurface_buffer(window.xdg(), &part, (50, 40), RED);
    f.dispatch_until(|f| outcome(f, tool, session).is_some());

    assert_eq!(outcome(&f, tool, session), Some(Outcome::Ready));
    assert_eq!(damage(&f), [(10, 20, 50, 40)]);
    let session = f.client_state(tool).capture_session(session);
    let second = session.last_frame().image();
    assert_eq!(second.at(10, 20), RED);
    assert_eq!(second.at(9, 20), WINDOW_COLOR);
}

on_both_kinds! {
    fn a_minimised_window_is_captured_as_it_was_last_drawn(kind: Kind) {
        let (mut f, client) = Fixture::connected(kind);
        let window = f.map(client, WindowSpec::default());
        let tool = screenshot_tool(&mut f);
        let session = window_session(&mut f, tool, window, false);
        let frame_size = f.visible_frame(window).size;
        let panel = f.connect(Kind::Xdg);
        f.client(panel).bind_foreign_toplevels();
        f.roundtrip(panel);

        f.client(panel).set_toplevel_minimized(0, true);
        f.roundtrip(panel);
        assert!(f.state.caminito.policy.is_minimized(f.pid(window)));
        let capture = frame(&mut f, tool, session);

        assert!(!f.client_state(tool).capture_session(session).stopped);
        assert_eq!(capture.size, (frame_size.w, frame_size.h));
        let (x, y) = content_center(&f, window);
        assert_eq!(capture.at(x, y), WINDOW_COLOR);
    }
}

on_both_kinds! {
    fn a_window_on_another_workspace_is_captured(kind: Kind) {
        let (mut f, client) = Fixture::connected(kind);
        let window = f.map(client, WindowSpec::default());
        let tool = screenshot_tool(&mut f);

        f.chord(&[KEY_LEFTCTRL, KEY_LEFTALT, KEY_RIGHT]);
        assert!(!f.state.caminito.policy.is_showing(f.pid(window)));
        let capture = capture_window(&mut f, tool, window, false);

        let frame = f.visible_frame(window).size;
        assert_eq!(capture.size, (frame.w, frame.h));
        let (x, y) = content_center(&f, window);
        assert_eq!(capture.at(x, y), WINDOW_COLOR);
    }
}

on_both_kinds! {
    fn a_shaded_window_is_captured_as_it_shows_its_titlebar_alone(kind: Kind) {
        let settings = vec![(
            MARCO_GENERAL,
            "action-double-click-titlebar".into(),
            Value::Str("toggle_shade".into()),
        )];
        let mut f = Fixture::with_settings(settings);
        f.add_output((1920, 1080), 1.0);
        let client = f.connect(kind);
        let window = f.map(client, WindowSpec::default());
        let tool = screenshot_tool(&mut f);
        let session = window_session(&mut f, tool, window, false);
        let unshaded = f.visible_frame(window).size;

        let title = title_point(&f, window);
        pointer_to(&mut f, title);
        for _ in 0..2 {
            f.pointer_button(BTN_LEFT, true);
            f.pointer_button(BTN_LEFT, false);
        }
        f.sync(window);
        assert!(f.state.caminito.policy.is_shaded(f.pid(window)));
        f.roundtrip(tool);

        let extents = f.frame_extents(window);
        let width = f.geometry(window).size.w + extents.left + extents.right;
        assert_eq!(
            sizes(&f, tool, session),
            [(unshaded.w, unshaded.h), (width, extents.top)]
        );
        let capture = frame(&mut f, tool, session);
        assert_eq!(capture.size, (width, extents.top));
        assert_eq!(capture.at(width / 2, 2) >> 24, 0xff, "the titlebar is drawn");
        assert!(!capture.pixels.contains(&WINDOW_COLOR));
    }
}

#[test]
fn a_frame_whose_buffer_isn_t_of_the_size_asked_fails() {
    let (mut f, _) = Fixture::connected(Kind::Xdg);
    let tool = screenshot_tool(&mut f);
    let session = output_session(&mut f, tool, false);

    f.client(tool)
        .request_capture_frame_of_size(session, (1930, 1090));
    f.dispatch_until(|f| outcome(f, tool, session).is_some());

    assert_eq!(
        outcome(&f, tool, session),
        Some(Outcome::Failed(FailureReason::BufferConstraints))
    );
}

/// xdg only: nothing here depends on the window's kind. No client can see that Caminito
/// forgot a session, so this reads its sessions.
#[test]
fn a_tool_that_quits_with_a_frame_waiting_is_forgotten() {
    let (mut f, client) = Fixture::connected(Kind::Xdg);
    let window = f.map(client, WindowSpec::default());
    let tool = screenshot_tool(&mut f);
    let session = window_session(&mut f, tool, window, false);
    frame(&mut f, tool, session);
    f.client(tool).request_capture_frame(session);
    f.roundtrip(tool);

    f.disconnect(tool);
    resize(&mut f, window, (500, 350));

    // Caminito hears of the hang-up when it next reads the socket, which a loaded machine
    // can put after the resize's dispatch.
    f.dispatch_until(|f| f.state.caminito.captures.is_empty());
}

on_both_kinds! {
    /// While the session is locked (133), an output's capture is what the lock shows and a
    /// window's is the lock background alone, without the cursor, before the locker's first
    /// frame and after it. The sessions go on, and after the unlock have the window again.
    fn a_capture_shows_the_lock_never_the_session(kind: Kind) {
        const LOCKER_COLOR: u32 = 0xff12_3456;
        let lock_background = argb8888(LOCK_BACKGROUND);
        let (mut f, client) = Fixture::connected(kind);
        let window = f.map(client, WindowSpec::default());
        let inside = center(f.geometry(window));
        pointer_to(&mut f, inside);
        let tool = screenshot_tool(&mut f);
        let of_output = output_session(&mut f, tool, false);
        let of_window = window_session(&mut f, tool, window, true);
        assert!(frame(&mut f, tool, of_output).pixels.contains(&WINDOW_COLOR));
        let unlocked = frame(&mut f, tool, of_window);
        assert!(unlocked.pixels.contains(&WINDOW_COLOR));
        let locker = f.connect(Kind::Xdg);
        f.client(locker).bind_outputs();

        let lock = f.client(locker).lock_session();
        f.roundtrip(locker);
        assert_eq!(f.lock_state(), "locked");
        let output = frame(&mut f, tool, of_output);
        assert!(output.pixels.iter().all(|p| *p == lock_background));
        let captured = frame(&mut f, tool, of_window);
        assert_eq!(captured.size, unlocked.size);
        assert!(captured.pixels.iter().all(|p| *p == lock_background));

        let surface = f.client(locker).create_lock_surface(lock, 0);
        f.roundtrip(locker);
        f.client(locker).map_lock_surface(surface, LOCKER_COLOR);
        f.roundtrip(locker);
        f.wait_for_pixel(0, 0, LOCKER_COLOR & 0xff_ffff);
        let output = frame(&mut f, tool, of_output);
        assert!(output.pixels.iter().all(|p| *p == LOCKER_COLOR));
        // The window's session has nothing new to show, so a new one is asked.
        let captured = capture_window(&mut f, tool, window, true);
        assert!(captured.pixels.iter().all(|p| *p == lock_background));

        f.client(locker).unlock_and_destroy(lock);
        f.roundtrip(locker);
        assert_eq!(f.lock_state(), "unlocked");
        assert!(frame(&mut f, tool, of_output).pixels.contains(&WINDOW_COLOR));
        assert!(frame(&mut f, tool, of_window).pixels.contains(&WINDOW_COLOR));
    }
}
