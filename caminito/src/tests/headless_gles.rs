//! The headless backend on GLES, for the tests of a shader's pixels (CMN-230). Each test
//! here is skipped where surfaceless EGL can't be created.

use caminito_test_client::layer_shell::LayerSpec;

use super::fixture::{Fixture, Kind};
use crate::backend::headless::argb8888;
use crate::scene::CLEAR_COLOR;

const BAR_COLOR: u32 = 0xff_20_80_c0;

#[test]
fn a_top_bar_is_drawn_over_the_background_on_gles() {
    let Some(mut f) = Fixture::on_gles() else {
        return;
    };
    let output = f.add_output((64, 48), 1.0);
    let id = f.connect(Kind::Xdg);
    f.map_layer(id, LayerSpec::top_bar(8), BAR_COLOR);
    f.wait_for_pixel(40, 2, BAR_COLOR & 0xff_ffff);

    let pixels = f
        .state
        .backend
        .headless_mut()
        .pixels(&output)
        .expect("the output is drawn");
    assert_eq!(pixels.len(), 64 * 48);
    // Right of the 24-pixel cursor at the top left corner: the bar's 8 rows at the top, and
    // the background under them, so that rows read upside down would show.
    for (i, &pixel) in pixels.iter().enumerate() {
        let (x, y) = (i % 64, i / 64);
        if x < 24 {
            continue;
        }
        let expected = if y < 8 {
            BAR_COLOR
        } else {
            argb8888(CLEAR_COLOR)
        };
        assert_eq!(pixel, expected, "the pixel at ({x}, {y})");
    }
}
