//! The headless backend, with no clients.

use smithay::utils::Rectangle;

use super::fixture::Fixture;
use crate::backend::headless::argb8888;
use crate::scene::CLEAR_COLOR;

#[test]
fn outputs_at_two_scales_sit_side_by_side_in_policy() {
    let mut f = Fixture::new();
    let a = f.add_output((1920, 1080), 1.0);
    let b = f.add_output((1920, 1080), 1.5);
    f.dispatch();

    let outputs = f.state.camino.policy.outputs();
    assert_eq!(outputs.len(), 2);
    assert_eq!(outputs[0].id, f.state.camino.output_id(&a));
    assert_eq!(
        outputs[0].geometry,
        Rectangle::new((0, 0).into(), (1920, 1080).into())
    );
    assert_eq!(outputs[0].scale, 1.0);
    assert_eq!(outputs[1].id, f.state.camino.output_id(&b));
    assert_eq!(
        outputs[1].geometry,
        Rectangle::new((1920, 0).into(), (1280, 720).into())
    );
    assert_eq!(outputs[1].scale, 1.5);
}

#[test]
fn dispatching_renders_each_output_cleared() {
    let mut f = Fixture::new();
    let a = f.add_output((64, 48), 1.0);
    let b = f.add_output((60, 30), 1.5);
    f.dispatch();

    for output in [&a, &b] {
        let pixels = f
            .state
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
    let mut f = Fixture::new();
    let output = f.add_output((64, 48), 1.5);
    f.dispatch();

    assert_eq!(f.state.scene_element_count(&output), 0);
}
