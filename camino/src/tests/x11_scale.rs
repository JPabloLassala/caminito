//! The X11 scale (41) as X clients see it: XSETTINGS, the work area and frame extents in X
//! client space, and windows configured to the same logical size when it changes. It's
//! XWayland's alone, so these tests map XWayland windows only.

use camino_test_client::x11::{Names, XSetting};
use insta::assert_snapshot;

use super::fixture::{ClientId, ClientWindow, Fixture, Kind, WindowSpec};

/// The X11 scale's XSETTINGS, and the work area and a window's frame extents, one per
/// line.
fn as_x_sees_it(f: &mut Fixture, x: ClientId, window: ClientWindow) -> String {
    let id = window.x11();
    f.x11_call(x, move |x| {
        let table = x.xsettings()?.expect("Camino publishes XSETTINGS");
        let mut lines: Vec<String> = ["Gdk/WindowScalingFactor", "Xft/DPI"]
            .iter()
            .map(|name| match table.iter().find(|(n, _)| n == name) {
                Some((_, XSetting::Int(i))) => format!("{name} = {i}"),
                other => format!("{name} = {other:?}"),
            })
            .collect();
        let mut names = Names::new(x);
        lines.push(names.property(x, x.root(), "_NET_WORKAREA")?);
        lines.push(names.property(x, x.root(), "_GTK_WORKAREAS_D0")?);
        lines.push(names.property(x, id, "_NET_FRAME_EXTENTS")?);
        Ok::<_, x11rb::errors::ReplyError>(lines.join("\n"))
    })
    .unwrap()
}

/// An XWayland window's size, as its client sees it.
fn x_size(f: &mut Fixture, window: ClientWindow) -> (u16, u16) {
    let id = window.x11();
    f.x11_call(window.client(), move |x| x.size(id)).unwrap()
}

#[test]
fn an_output_going_from_scale_1_to_2_doubles_what_x_clients_see() {
    let mut f = Fixture::new();
    let output = f.add_output((1920, 1080), 1.0);
    let x = f.connect(Kind::X11);
    let window = f.map(x, WindowSpec::plain((300, 200)));

    assert_snapshot!(as_x_sees_it(&mut f, x, window), @"
    Gdk/WindowScalingFactor = 1
    Xft/DPI = 98304
    _NET_WORKAREA(CARDINAL) = 0, 0, 1920, 1080, 0, 0, 1920, 1080, 0, 0, 1920, 1080, 0, 0, 1920, 1080
    _GTK_WORKAREAS_D0(CARDINAL) = 0, 0, 1920, 1080
    _NET_FRAME_EXTENTS(CARDINAL) = 1, 1, 31, 1
    ");
    assert_eq!(x_size(&mut f, window), (300, 200));

    f.set_output_scale(&output, 2.0);
    f.sync(window);

    // The output is 960 × 540 logical pixels now, which is 1920 × 1080 in client space.
    assert_snapshot!(as_x_sees_it(&mut f, x, window), @"
    Gdk/WindowScalingFactor = 2
    Xft/DPI = 196608
    _NET_WORKAREA(CARDINAL) = 0, 0, 1920, 1080, 0, 0, 1920, 1080, 0, 0, 1920, 1080, 0, 0, 1920, 1080
    _GTK_WORKAREAS_D0(CARDINAL) = 0, 0, 1920, 1080
    _NET_FRAME_EXTENTS(CARDINAL) = 2, 2, 62, 2
    ");
    // The same logical size, in client space.
    assert_eq!(f.geometry(window).size, (300, 200).into());
    assert_eq!(x_size(&mut f, window), (600, 400));
    // Xwayland's screen is the output's size in client space.
    let root = f.x11_call(x, |x| x.size(x.root())).unwrap();
    assert_eq!(root, (1920, 1080));
}

#[test]
fn the_dump_shows_the_x11_scale() {
    let mut f = Fixture::new();
    let output = f.add_output((1920, 1080), 1.0);
    f.add_output((1920, 1080), 1.25);
    let x11_scale = |f: &Fixture| {
        let dump: serde_json::Value = serde_json::from_str(&f.state.camino.state_dump()).unwrap();
        dump["x11_scale"].clone()
    };
    assert_eq!(x11_scale(&f), 2);

    f.set_output_scale(&output, 3.0);
    assert_eq!(x11_scale(&f), 3);
}

/// The struts the dump shows for a window, as `side width span` lines.
fn dumped_struts(f: &Fixture, window: ClientWindow) -> String {
    let dump: serde_json::Value = serde_json::from_str(&f.state.camino.state_dump()).unwrap();
    let id = f.pid(window).0;
    let entry = dump["windows"]
        .as_array()
        .unwrap()
        .iter()
        .find(|entry| entry["id"] == id)
        .expect("the window is dumped");
    entry["x11"]["struts"]
        .as_array()
        .unwrap()
        .iter()
        .map(|strut| format!("{} {} {}", strut["side"], strut["width"], strut["span"]))
        .collect::<Vec<_>>()
        .join("\n")
}

#[test]
fn struts_are_divided_by_the_x11_scale_and_read_again_when_it_changes() {
    let mut f = Fixture::new();
    let output = f.add_output((1920, 1080), 2.0);
    let x = f.connect(Kind::X11);
    // A panel along the top of the output's left half, in client space at 2.
    let partial = vec![0, 0, 60, 0, 0, 0, 0, 0, 0, 959, 0, 0];
    let window = f.map(
        x,
        WindowSpec {
            types: vec!["_NET_WM_WINDOW_TYPE_DOCK"],
            cardinals: vec![("_NET_WM_STRUT_PARTIAL", partial)],
            ..WindowSpec::default()
        },
    );
    assert_snapshot!(dumped_struts(&f, window), @r#""top" 30 [0,479]"#);

    // The client hasn't rewritten them, so they're now at scale 1.
    f.set_output_scale(&output, 1.0);
    assert_snapshot!(dumped_struts(&f, window), @r#""top" 60 [0,959]"#);
}
