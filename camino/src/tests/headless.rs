//! `State` on the headless backend, with no clients.

use std::time::Duration;

use smithay::reexports::calloop::EventLoop;
use smithay::reexports::wayland_server::Display;
use smithay::utils::Rectangle;

use crate::backend::headless::argb8888;
use crate::scene::CLEAR_COLOR;
use crate::state::State;

fn headless() -> (EventLoop<'static, State>, State) {
    let event_loop = EventLoop::try_new().unwrap();
    let state = State::new_headless(&event_loop, Display::new().unwrap());
    (event_loop, state)
}

fn dispatch(event_loop: &mut EventLoop<State>, state: &mut State) {
    event_loop.dispatch(Duration::ZERO, state).unwrap();
    state.refresh_and_flush_clients();
}

#[test]
fn outputs_at_two_scales_sit_side_by_side_in_policy() {
    let (mut event_loop, mut state) = headless();
    let a = state.add_headless_output((1920, 1080).into(), 1.0);
    let b = state.add_headless_output((1920, 1080).into(), 1.5);
    dispatch(&mut event_loop, &mut state);

    let outputs = state.camino.policy.outputs();
    assert_eq!(outputs.len(), 2);
    assert_eq!(outputs[0].id, state.camino.output_id(&a));
    assert_eq!(
        outputs[0].geometry,
        Rectangle::new((0, 0).into(), (1920, 1080).into())
    );
    assert_eq!(outputs[0].scale, 1.0);
    assert_eq!(outputs[1].id, state.camino.output_id(&b));
    assert_eq!(
        outputs[1].geometry,
        Rectangle::new((1920, 0).into(), (1280, 720).into())
    );
    assert_eq!(outputs[1].scale, 1.5);
}

#[test]
fn dispatching_renders_each_output_cleared() {
    let (mut event_loop, mut state) = headless();
    let a = state.add_headless_output((64, 48).into(), 1.0);
    let b = state.add_headless_output((60, 30).into(), 1.5);
    dispatch(&mut event_loop, &mut state);

    for output in [&a, &b] {
        let pixels = state
            .backend
            .headless_mut()
            .pixels(output)
            .expect("output was rendered");
        let size = output.current_mode().unwrap().size;
        assert_eq!(pixels.len(), (size.w * size.h) as usize);
        assert!(pixels.iter().all(|p| *p == argb8888(CLEAR_COLOR)));
    }
}

#[test]
fn an_empty_output_has_no_scene_elements() {
    let (mut event_loop, mut state) = headless();
    let output = state.add_headless_output((64, 48).into(), 1.5);
    dispatch(&mut event_loop, &mut state);

    assert_eq!(state.scene_element_count(&output), 0);
}
