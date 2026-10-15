//! At a fractional scale, what meets edge to edge in the logical space meets edge to edge on
//! the output: a frame and its window, a maximised window and the panels around it, with no
//! row of what's behind between them.

use caminito_test_client::layer_shell::{LayerAnchor, LayerSpec};
use smithay::output::Output;
use smithay::utils::{Physical, Point, Rectangle};

use super::fixture::{ClientWindow, Decoration, Fixture, Kind, WindowSpec, on_both_kinds};
use crate::backend::headless::argb8888;
use crate::scene::CLEAR_COLOR;

const BAR_COLOR: u32 = 0xff00_00ff;

/// The rows of `output`'s physical column `x`, from `top` to `bottom` logical, that show the
/// background.
fn clear_rows(f: &mut Fixture, output: &Output, x: i32, top: i32, bottom: i32) -> Vec<i32> {
    f.draw(output);
    let scale = output.current_scale().fractional_scale();
    let x = (f64::from(x) * scale).round() as i32;
    let top = (f64::from(top) * scale).round() as i32;
    let bottom = (f64::from(bottom) * scale).round() as i32;
    let clear = argb8888(CLEAR_COLOR) & 0xff_ffff;
    (top..bottom)
        .filter(|&y| {
            let pixel = f
                .state
                .backend
                .headless_mut()
                .pixel(output, Point::<i32, Physical>::from((x, y)));
            pixel.expect("the output is drawn") & 0xff_ffff == clear
        })
        .collect()
}

#[test]
fn a_maximised_window_meets_the_panels_around_it() {
    let mut f = Fixture::new();
    // 1536 × 864 logical. A window between a bar 1 high and one 26 high starts a quarter
    // pixel into a row and is a quarter pixel more than whole rows high.
    let output = f.add_output((1920, 1080), 1.25);
    let id = f.connect(Kind::Xdg);
    f.map_layer(id, LayerSpec::top_bar(1), BAR_COLOR);
    let bottom = LayerSpec {
        anchor: LayerAnchor::Bottom | LayerAnchor::Left | LayerAnchor::Right,
        ..LayerSpec::top_bar(26)
    };
    f.map_layer(id, bottom, BAR_COLOR);
    let window = f.map(
        id,
        WindowSpec {
            maximized: true,
            ..WindowSpec::plain((400, 300))
        },
    );
    f.draw_at_asked_size(window, (1536, 837));
    assert_eq!(
        f.geometry(window),
        Rectangle::new((0, 1).into(), (1536, 837).into())
    );

    assert_eq!(clear_rows(&mut f, &output, 700, 0, 864), Vec::<i32>::new());
}

on_both_kinds! {
    fn a_frame_meets_its_window(kind: Kind) {
        let mut f = Fixture::new();
        let output = f.add_output((1920, 1080), 1.25);
        let client = f.connect(kind);
        // Each height leaves the bottom of the window at another quarter of a pixel.
        for height in 300..304 {
            let window: ClientWindow = f.map(
                client,
                WindowSpec {
                    size: (400, height),
                    decoration: Decoration::Server,
                    ..WindowSpec::default()
                },
            );
            let visible = f.visible_frame(window);
            let middle = visible.loc.x + visible.size.w / 2;
            let rows = clear_rows(&mut f, &output, middle, visible.loc.y, visible.loc.y + visible.size.h);
            assert_eq!(rows, Vec::<i32>::new(), "{height} high, in {visible:?}");
        }
    }
}
