//! Caminito's cursors (13, 31): which one shows over frames and during grabs, and the size
//! each output loads it at.

use smithay::input::pointer::{CursorIcon, CursorImageStatus};
use smithay::utils::{Logical, Point};

use super::fixture::{BTN_LEFT, Fixture, Kind, WindowSpec, on_both_kinds};
use crate::backend::headless::argb8888;
use crate::cursor::{CursorSource, Cursors};
use crate::scene::CLEAR_COLOR;
use crate::settings::Value;
use crate::settings::translate::MOUSE;

fn named(f: &Fixture) -> Option<CursorIcon> {
    match f.state.caminito.cursor() {
        CursorImageStatus::Named(icon) => Some(icon),
        _ => None,
    }
}

on_both_kinds! {
    fn the_cursor_follows_the_frame_and_the_grab(kind: Kind) {
        let (mut f, client) = Fixture::connected(kind);
        let window = f.map(client, WindowSpec::default());
        let geometry = f.geometry(window).to_f64();
        let middle_y = geometry.loc.y + geometry.size.h / 2.;

        // On the left border, and dragging it.
        f.pointer_move(geometry.loc.x - 5., middle_y);
        assert_eq!(named(&f), Some(CursorIcon::WResize));
        f.pointer_button(BTN_LEFT, true);
        f.pointer_move(geometry.loc.x - 40., middle_y);
        assert_eq!(named(&f), Some(CursorIcon::WResize));
        f.pointer_button(BTN_LEFT, false);

        // On the client: the client's cursor, which is the default until it sets one.
        let geometry = f.geometry(window).to_f64();
        let inside = geometry.loc + geometry.size.downscale(2.).to_point();
        f.pointer_move(inside.x, inside.y);
        assert_eq!(named(&f), Some(CursorIcon::Default));

        // Moving by the titlebar.
        let title: Point<f64, Logical> = Point::from((inside.x, geometry.loc.y - 10.));
        f.pointer_move(title.x, title.y);
        assert_eq!(named(&f), Some(CursorIcon::Default));
        f.pointer_button(BTN_LEFT, true);
        f.pointer_move(title.x + 50., title.y + 50.);
        assert_eq!(named(&f), Some(CursorIcon::Move));
        f.pointer_button(BTN_LEFT, false);
        assert_eq!(named(&f), Some(CursorIcon::Default));
    }
}

#[test]
fn each_output_loads_the_cursor_at_its_rounded_up_scale_and_size_changes_apply_live() {
    let mut f = Fixture::new();
    let one = f.add_output((1920, 1080), 1.0);
    let fractional = f.add_output((1920, 1080), 1.5);
    let size_on = |f: &Fixture, output| {
        let image = f
            .state
            .caminito
            .cursors
            .image(CursorIcon::Default, output_scale(output));
        (image.size, image.scale)
    };
    fn output_scale(output: &smithay::output::Output) -> f64 {
        output.current_scale().fractional_scale()
    }

    assert_eq!(size_on(&f, &one), ((24, 24).into(), 1));
    assert_eq!(size_on(&f, &fractional), ((48, 48).into(), 2));

    f.set_setting(MOUSE, "cursor-size", Value::Int(32));
    f.dispatch_until(|f| size_on(f, &one).0.w == 32);
    assert_eq!(size_on(&f, &fractional), ((64, 64).into(), 2));
}

#[test]
fn the_cursor_is_drawn_at_the_pointer_over_everything() {
    let (mut f, _) = Fixture::connected(Kind::Xdg);
    let output = f.state.caminito.outputs[0].clone();

    f.pointer_move(100., 100.);

    let pixels = f.state.backend.headless_mut().pixels(&output).unwrap();
    let at = |x: usize, y: usize| pixels[y * 1920 + x];
    // Inside the built-in arrow, just below its tip: white.
    assert_eq!(at(103, 108), 0xffffffff);
    // Beside it, clear.
    assert_eq!(at(120, 103), argb8888(CLEAR_COLOR));
}

/// Writes an xcursor theme at `dir` whose cursors are 24 pixel squares of one ARGB8888
/// colour each, inheriting `inherits`.
fn write_theme(dir: &std::path::Path, inherits: Option<&std::path::Path>, cursors: &[(&str, u32)]) {
    std::fs::create_dir_all(dir.join("cursors")).unwrap();
    let inherits = inherits.map_or(String::new(), |theme| {
        format!("Inherits={}\n", theme.display())
    });
    std::fs::write(dir.join("index.theme"), format!("[Icon Theme]\n{inherits}")).unwrap();
    for (name, color) in cursors {
        // The file header, a table of contents with the one image, and the image.
        let words = [0x10, 0x1_0000, 1, 0xfffd_0002, 24, 28]
            .into_iter()
            .chain([36, 0xfffd_0002, 24, 1, 24, 24, 0, 0, 0])
            .chain(std::iter::repeat_n(*color, 24 * 24));
        let mut file = b"Xcur".to_vec();
        file.extend(words.flat_map(u32::to_le_bytes));
        std::fs::write(dir.join("cursors").join(name), file).unwrap();
    }
}

/// Oxygen's themes name their arrow `left_ptr` only, and Caminito looked `default` up in the
/// themes they inherit before it tried `left_ptr` in the theme itself. What it did find in
/// the theme it drew with red and blue swapped, so yellow was cyan (79).
#[test]
fn a_theme_without_a_default_cursor_shows_its_own_arrow_not_one_it_inherits() {
    const YELLOW: u32 = 0xffffff00;
    const BLACK: u32 = 0xff000000;
    let themes = tempfile::tempdir().unwrap();
    // xcursor joins a theme's name to each directory it searches, so a theme named by an
    // absolute path is found there, whatever is installed.
    let (inherited, yellow) = (
        themes.path().join("inherited"),
        themes.path().join("yellow"),
    );
    write_theme(&inherited, None, &[("default", BLACK)]);
    write_theme(&yellow, Some(&inherited), &[("left_ptr", YELLOW)]);

    let mut f = Fixture::new();
    f.add_output((1920, 1080), 1.0);
    f.state.caminito.cursors =
        Cursors::new(&f.state.caminito.settings.mouse, CursorSource::Xcursor);
    f.pointer_move(100., 100.);

    // The pointer stays where it is: the cursor on screen follows the setting.
    let theme = yellow.to_str().unwrap().to_owned();
    f.set_setting(MOUSE, "cursor-theme", Value::Str(theme));
    f.wait_for_pixel(105, 105, YELLOW & 0xff_ffff);

    // Xwayland's root window gets the same arrow. An X client can't read a cursor's
    // pixels back, so this asks for what Caminito would send.
    let root = f.state.caminito.cursors.x_root_cursor(1);
    assert_eq!(root.pixels[..4], YELLOW.to_le_bytes());
}
