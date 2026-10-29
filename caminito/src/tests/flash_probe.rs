//! [DEBUG-flash] Throwaway probe: renders every frame of a Maximise animation and flags a
//! frame whose window-coloured region isn't between its neighbours'.

use std::time::Duration;

use smithay::utils::Point;

use super::fixture::{ClientWindow, Decoration, Fixture, Kind, WINDOW_COLOR, WindowSpec};
use super::maximise_animation::desk;
use crate::maximize_animation::LENGTH;

const STEP: Duration = Duration::from_millis(5);

const SHADOW_COLOR: u32 = 0x60000000;

/// Acks and commits the parent at the asked size with `shadow`, moving its content
/// subsurface inside its geometry. Returns the size.
fn draw_parent(f: &mut Fixture, window: ClientWindow, shadow: i32) -> (i32, i32) {
    let ClientWindow::Xdg(id, xdg) = window else {
        unreachable!()
    };
    f.roundtrip(id);
    let client = f.client(id);
    let (w, h) = client.window(xdg).configures.last().unwrap().size;
    client.ack_last_configure(xdg);
    client.set_window_geometry(xdg, (shadow, shadow, w, h));
    client.attach_buffer(xdg, (w + 2 * shadow, h + 2 * shadow), SHADOW_COLOR);
    client.set_subsurface_position(xdg, 0, (shadow, shadow));
    client.commit(xdg);
    f.roundtrip(id);
    (w, h)
}

macro_rules! draw_content {
    ($f:expr, $window:expr, $content:expr, $size:expr) => {{
        let ClientWindow::Xdg(id, xdg) = $window else {
            unreachable!()
        };
        $f.client(id)
            .commit_subsurface_buffer(xdg, $content, $size, WINDOW_COLOR);
        $f.roundtrip(id);
    }};
}

type Bbox = Option<(i32, i32, i32, i32)>;

fn green_bbox(f: &mut Fixture) -> Bbox {
    let output = f.state.caminito.outputs[0].clone();
    let pixels = f.state.backend.headless_mut().pixels(&output).unwrap();
    let width = 800;
    let mut bbox: Bbox = None;
    for (i, p) in pixels.iter().enumerate() {
        let (r, g, b) = ((p >> 16) & 0xff, (p >> 8) & 0xff, p & 0xff);
        if g > 0xc0 && r < 0x40 && b < 0x40 {
            let (x, y) = ((i % width) as i32, (i / width) as i32);
            bbox = Some(match bbox {
                None => (x, y, x, y),
                Some((l, t, r, b)) => (l.min(x), t.min(y), r.max(x), b.max(y)),
            });
        }
    }
    bbox
}

/// Frames of `maximize` (or unmaximise), with the client drawing its parent at the new
/// size `draw_after` into the animation, and its content subsurface `lag` after that.
fn frames(shadow: i32, maximize: bool, draw_after: Duration, lag: Duration) -> Vec<(u64, Bbox)> {
    let (mut f, id) = desk(Kind::Xdg, Vec::new());
    let spec = WindowSpec {
        decoration: Decoration::Client,
        ..WindowSpec::default()
    };
    let window = f.map(id, spec);
    let ClientWindow::Xdg(_, xdg) = window else {
        unreachable!()
    };
    let content = f.client(id).add_subsurface(xdg, (shadow, shadow), spec_size(), WINDOW_COLOR);
    let pid = f.pid(window);
    f.state
        .caminito
        .policy
        .request_geometry(pid, Some(Point::from((150, 120))), None);
    f.dispatch();
    let size = draw_parent(&mut f, window, shadow);
    draw_content!(f, window, &content, size);
    if !maximize {
        f.state.caminito.policy.set_maximized(pid, true);
        f.dispatch();
        let size = draw_parent(&mut f, window, 0);
        draw_content!(f, window, &content, size);
        f.advance_clock(LENGTH * 2);
    }
    let output = f.state.caminito.outputs[0].clone();
    f.draw(&output);
    let mut out = vec![(0, green_bbox(&mut f))];
    f.state.caminito.policy.set_maximized(pid, maximize);
    f.dispatch();
    let mut t = Duration::ZERO;
    let mut drew: Option<(i32, i32)> = None;
    let mut content_drawn = false;
    while t <= LENGTH + Duration::from_millis(150) {
        if drew.is_none() && t >= draw_after {
            drew = Some(draw_parent(&mut f, window, if maximize { 0 } else { shadow }));
        }
        if let Some(size) = drew
            && !content_drawn
            && t >= draw_after + lag
        {
            draw_content!(f, window, &content, size);
            content_drawn = true;
        }
        f.draw(&output);
        out.push((t.as_millis() as u64 + 1, green_bbox(&mut f)));
        f.advance_clock(STEP);
        t += STEP;
    }
    out
}

fn spec_size() -> (i32, i32) {
    WindowSpec::default().size
}

/// Frames where an edge goes back on itself (by more than 2 px).
fn flashes(frames: &[(u64, Bbox)]) -> Vec<String> {
    let mut bad = Vec::new();
    for w in frames.windows(3) {
        let [(_, a), (t, b), (_, c)] = w else { unreachable!() };
        let (Some(a), Some(b), Some(c)) = (a, b, c) else {
            bad.push(format!("t={t}: missing {a:?} {b:?} {c:?}"));
            continue;
        };
        let edges = |x: &(i32, i32, i32, i32)| [x.0, x.1, x.2, x.3];
        for i in 0..4 {
            let (a, b, c) = (edges(a)[i], edges(b)[i], edges(c)[i]);
            let between = (a.min(c) - 2..=a.max(c) + 2).contains(&b);
            if !between {
                bad.push(format!("t={t} edge {i}: {a} {b} {c}"));
            }
        }
    }
    bad
}

#[test]
fn probe() {
    let mut report = String::new();
    let mut any = false;
    for shadow in [0, 20] {
        for maximize in [true, false] {
            for (after, lag) in [(0, 0), (0, 5), (0, 20), (5, 0), (20, 0), (100, 0), (100, 10), (400, 0)] {
                let fr = frames(shadow, maximize, Duration::from_millis(after), Duration::from_millis(lag));
                let bad = flashes(&fr);
                any |= !bad.is_empty();
                report += &format!(
                    "shadow={shadow} maximize={maximize} draw_after={after}ms lag={lag}ms: {}\n",
                    if bad.is_empty() { "ok".into() } else { bad.join("; ") }
                );
                if !bad.is_empty() {
                    for (t, b) in &fr {
                        report += &format!("    {t:>4} {b:?}\n");
                    }
                }
            }
        }
    }
    println!("{report}");
    assert!(!any, "flashes:\n{report}");
}
