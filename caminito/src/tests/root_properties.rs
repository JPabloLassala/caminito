//! Caminito's X connection and the root properties it publishes (38), as an X client reads
//! them. They're the root's, not a window's, so no test maps a window.

use std::ffi::OsStr;
use std::sync::mpsc;
use std::time::Duration;

use caminito_test_client::x11::{Names, PropertyValue, X11Client};
use insta::assert_snapshot;
use x11rb::protocol::Event;

use super::fixture::{ClientId, Fixture, KEY_LEFTALT, KEY_LEFTCTRL, KEY_RIGHT, Kind};
use crate::settings::Value;
use crate::settings::translate::{MARCO_GENERAL, MARCO_WORKSPACE_NAMES};

const DESKTOPS: &[&str] = &[
    "_NET_NUMBER_OF_DESKTOPS",
    "_NET_DESKTOP_NAMES",
    "_NET_CURRENT_DESKTOP",
];

/// The root properties `names`, one per line as `Names` writes them.
fn root_properties(f: &mut Fixture, x: ClientId, names: &'static [&'static str]) -> String {
    f.x11_call(x, move |x| {
        let mut formatter = Names::new(x);
        let lines = names
            .iter()
            .map(|name| formatter.property(x, x.root(), name))
            .collect::<Result<Vec<_>, _>>()?;
        Ok::<_, x11rb::errors::ReplyError>(lines.join("\n"))
    })
    .unwrap()
}

#[test]
fn the_root_lists_what_caminito_supports_and_its_check_window_is_named_caminito() {
    let mut f = Fixture::new();
    let x = f.connect(Kind::X11);

    let (supported, name) = f
        .x11_call(x, |x| {
            let Some(PropertyValue::Cardinals(atoms)) = x
                .property(x.root(), "_NET_SUPPORTED")?
                .map(|property| property.value)
            else {
                unreachable!("the root has an ATOM property");
            };
            let supported = atoms
                .iter()
                .map(|atom| x.atom_name(*atom))
                .collect::<Result<Vec<_>, _>>()?;
            let Some(PropertyValue::Cardinals(check)) = x
                .property(x.root(), "_NET_SUPPORTING_WM_CHECK")?
                .map(|property| property.value)
            else {
                unreachable!("the root has a WINDOW property");
            };
            let name = Names::new(x).property(x, check[0], "_NET_WM_NAME")?;
            Ok::<_, x11rb::errors::ReplyError>((supported.join("\n"), name))
        })
        .unwrap();

    // Smithay's, in its order, then Caminito's.
    assert_snapshot!(supported, @r"
    _NET_WM_STATE
    _NET_WM_STATE_MAXIMIZED_HORZ
    _NET_WM_STATE_MAXIMIZED_VERT
    _NET_WM_STATE_HIDDEN
    _NET_WM_STATE_FULLSCREEN
    _NET_WM_STATE_MODAL
    _NET_WM_STATE_FOCUSED
    _NET_WM_STATE_ABOVE
    _NET_WM_STATE_BELOW
    _NET_WM_STATE_SHADED
    _NET_WM_STATE_SKIP_TASKBAR
    _NET_WM_STATE_SKIP_PAGER
    _NET_WM_STATE_STICKY
    _NET_WM_STATE_DEMANDS_ATTENTION
    _NET_ACTIVE_WINDOW
    _NET_WM_MOVERESIZE
    _NET_CLIENT_LIST
    _NET_CLIENT_LIST_STACKING
    _NET_SHOWING_DESKTOP
    _NET_WM_OPAQUE_REGION
    _NET_WM_PING
    _GTK_FRAME_EXTENTS
    _NET_WM_SYNC_REQUEST
    _NET_WM_SYNC_REQUEST_COUNTER
    _NET_SUPPORTING_WM_CHECK
    _NET_WM_NAME
    _NET_NUMBER_OF_DESKTOPS
    _NET_DESKTOP_NAMES
    _NET_CURRENT_DESKTOP
    _NET_WORKAREA
    _GTK_WORKAREAS
    _NET_WM_DESKTOP
    _NET_FRAME_EXTENTS
    _NET_REQUEST_FRAME_EXTENTS
    _NET_CLOSE_WINDOW
    _NET_MOVERESIZE_WINDOW
    _NET_WM_ICON
    _NET_WM_STRUT
    _NET_WM_STRUT_PARTIAL
    _GTK_SHOW_WINDOW_MENU
    ");
    assert_snapshot!(name, @r#"_NET_WM_NAME(UTF8_STRING) = "Caminito""#);
}

#[test]
fn the_desktop_atoms_follow_the_workspace_count_their_names_and_switches() {
    let (mut f, x) = Fixture::connected(Kind::X11);
    assert_snapshot!(root_properties(&mut f, x, DESKTOPS), @r#"
    _NET_NUMBER_OF_DESKTOPS(CARDINAL) = 4
    _NET_DESKTOP_NAMES(UTF8_STRING) = "Workspace 1", "Workspace 2", "Workspace 3", "Workspace 4", ""
    _NET_CURRENT_DESKTOP(CARDINAL) = 0
    "#);

    f.set_setting(MARCO_GENERAL, "num-workspaces", Value::Int(3));
    f.dispatch_until(|f| f.state.caminito.policy.workspace_count() == 3);
    assert_snapshot!(root_properties(&mut f, x, DESKTOPS), @r#"
    _NET_NUMBER_OF_DESKTOPS(CARDINAL) = 3
    _NET_DESKTOP_NAMES(UTF8_STRING) = "Workspace 1", "Workspace 2", "Workspace 3", ""
    _NET_CURRENT_DESKTOP(CARDINAL) = 0
    "#);

    f.set_setting(MARCO_WORKSPACE_NAMES, "name-2", Value::Str("Mail".into()));
    f.dispatch_until(|f| f.state.caminito.policy.workspace_names()[1] == "Mail");
    assert_snapshot!(root_properties(&mut f, x, DESKTOPS), @r#"
    _NET_NUMBER_OF_DESKTOPS(CARDINAL) = 3
    _NET_DESKTOP_NAMES(UTF8_STRING) = "Workspace 1", "Mail", "Workspace 3", ""
    _NET_CURRENT_DESKTOP(CARDINAL) = 0
    "#);

    f.chord(&[KEY_LEFTCTRL, KEY_LEFTALT, KEY_RIGHT]);
    assert_snapshot!(root_properties(&mut f, x, DESKTOPS), @r#"
    _NET_NUMBER_OF_DESKTOPS(CARDINAL) = 3
    _NET_DESKTOP_NAMES(UTF8_STRING) = "Workspace 1", "Mail", "Workspace 3", ""
    _NET_CURRENT_DESKTOP(CARDINAL) = 1
    "#);
}

#[test]
fn a_current_desktop_message_switches_workspaces() {
    let (mut f, x) = Fixture::connected(Kind::X11);

    f.x11_call(x, |x| {
        x.send_client_message(x.root(), "_NET_CURRENT_DESKTOP", [2, 0, 0, 0, 0])
    })
    .unwrap();
    f.dispatch_until(|f| f.state.caminito.policy.active_workspace() == 2);

    assert_snapshot!(
        root_properties(&mut f, x, &["_NET_CURRENT_DESKTOP"]),
        @"_NET_CURRENT_DESKTOP(CARDINAL) = 2"
    );
}

#[test]
fn a_number_of_desktops_message_writes_num_workspaces_as_the_pager_does() {
    let (mut f, x) = Fixture::connected(Kind::X11);

    f.x11_call(x, |x| {
        x.send_client_message(x.root(), "_NET_NUMBER_OF_DESKTOPS", [6, 0, 0, 0, 0])
    })
    .unwrap();
    // What the settings thread read back, so GSettings has it.
    f.dispatch_until(|f| f.state.caminito.settings.policy.workspace_count == 6);

    assert_snapshot!(
        root_properties(&mut f, x, &["_NET_NUMBER_OF_DESKTOPS"]),
        @"_NET_NUMBER_OF_DESKTOPS(CARDINAL) = 6"
    );
}

#[test]
fn the_work_area_covers_both_outputs_and_each_desktop_lists_each_output() {
    let mut f = Fixture::new();
    f.add_output((1920, 1080), 1.0);
    let x = f.connect(Kind::X11);
    f.add_output((1280, 1024), 1.0);
    f.dispatch();

    const WORK_AREAS: &[&str] = &["_NET_WORKAREA", "_GTK_WORKAREAS_D0", "_GTK_WORKAREAS_D3"];
    // Marco's `_NET_WORKAREA` is the screen's, which spans both.
    assert_snapshot!(root_properties(&mut f, x, WORK_AREAS), @"
    _NET_WORKAREA(CARDINAL) = 0, 0, 3200, 1080, 0, 0, 3200, 1080, 0, 0, 3200, 1080, 0, 0, 3200, 1080
    _GTK_WORKAREAS_D0(CARDINAL) = 0, 0, 1920, 1080, 1920, 0, 1280, 1024
    _GTK_WORKAREAS_D3(CARDINAL) = 0, 0, 1920, 1080, 1920, 0, 1280, 1024
    ");

    f.set_setting(MARCO_GENERAL, "num-workspaces", Value::Int(2));
    f.dispatch_until(|f| f.state.caminito.policy.workspace_count() == 2);
    assert_snapshot!(root_properties(&mut f, x, WORK_AREAS), @"
    _NET_WORKAREA(CARDINAL) = 0, 0, 3200, 1080, 0, 0, 3200, 1080
    _GTK_WORKAREAS_D0(CARDINAL) = 0, 0, 1920, 1080, 1920, 0, 1280, 1024
    _GTK_WORKAREAS_D3 unset
    ");
}

#[test]
fn display_is_set_only_once_the_root_properties_and_xsettings_exist() {
    let mut f = Fixture::new();
    f.add_output((1920, 1080), 1.0);
    f.state.caminito.start_xwayland();
    let deadline = std::time::Instant::now() + Duration::from_secs(5);
    let display = loop {
        if let Some(display) = f.state.caminito.environment.get(OsStr::new("DISPLAY")) {
            break display
                .to_str()
                .unwrap()
                .trim_start_matches(':')
                .parse()
                .unwrap();
        }
        assert!(
            std::time::Instant::now() < deadline,
            "DISPLAY is set within 5 s"
        );
        f.dispatch_sources_only();
    };

    // Caminito isn't dispatched again, so the client sees the root as it was when DISPLAY was
    // set. Xwayland answers it alone.
    let (sender, receiver) = mpsc::channel();
    std::thread::spawn(move || {
        let read = || {
            let x = X11Client::connect(display).unwrap();
            let mut names = Names::new(&x);
            let mut lines = [
                "_NET_NUMBER_OF_DESKTOPS",
                "_NET_DESKTOP_NAMES",
                "_NET_CURRENT_DESKTOP",
                "_NET_WORKAREA",
                "_GTK_WORKAREAS_D0",
            ]
            .iter()
            .map(|name| names.property(&x, x.root(), name))
            .collect::<Result<Vec<_>, _>>()?;
            // GTK3 reads `_NET_SUPPORTED` once, as it starts.
            let Some(PropertyValue::Cardinals(supported)) = x
                .property(x.root(), "_NET_SUPPORTED")?
                .map(|property| property.value)
            else {
                unreachable!("the root has an ATOM property");
            };
            let gtk_workareas = x.atom("_GTK_WORKAREAS")?;
            lines.push(format!(
                "_NET_SUPPORTED lists _GTK_WORKAREAS: {}",
                supported.contains(&gtk_workareas)
            ));
            let Some(PropertyValue::Cardinals(check)) = x
                .property(x.root(), "_NET_SUPPORTING_WM_CHECK")?
                .map(|property| property.value)
            else {
                unreachable!("the root has a WINDOW property");
            };
            lines.push(names.property(&x, check[0], "_NET_WM_NAME")?);
            // GTK reads XSETTINGS as it starts, too (40).
            let theme = x.xsettings()?.and_then(|table| {
                table
                    .into_iter()
                    .find_map(|(name, value)| (name == "Net/ThemeName").then_some(value))
            });
            lines.push(format!("XSETTINGS Net/ThemeName: {theme:?}"));
            Ok::<_, x11rb::errors::ReplyError>(lines)
        };
        sender.send(read().unwrap().join("\n")).unwrap();
    });
    let root = receiver
        .recv_timeout(Duration::from_secs(5))
        .expect("Xwayland answers within 5 s");

    assert_snapshot!(root, @r#"
    _NET_NUMBER_OF_DESKTOPS(CARDINAL) = 4
    _NET_DESKTOP_NAMES(UTF8_STRING) = "Workspace 1", "Workspace 2", "Workspace 3", "Workspace 4", ""
    _NET_CURRENT_DESKTOP(CARDINAL) = 0
    _NET_WORKAREA(CARDINAL) = 0, 0, 1920, 1080, 0, 0, 1920, 1080, 0, 0, 1920, 1080, 0, 0, 1920, 1080
    _GTK_WORKAREAS_D0(CARDINAL) = 0, 0, 1920, 1080
    _NET_SUPPORTED lists _GTK_WORKAREAS: true
    _NET_WM_NAME(UTF8_STRING) = "Caminito"
    XSETTINGS Net/ThemeName: Some(Str("Menta"))
    "#);
}

#[test]
fn caminito_writes_none_of_the_root_properties_smithay_maintains() {
    let (mut f, x) = Fixture::connected(Kind::X11);
    f.x11_call(x, |x| x.watch_root()).unwrap();

    // Every change Caminito publishes, with no XWayland window for Smithay to write about.
    f.set_setting(MARCO_GENERAL, "num-workspaces", Value::Int(5));
    f.dispatch_until(|f| f.state.caminito.policy.workspace_count() == 5);
    f.set_setting(MARCO_WORKSPACE_NAMES, "name-1", Value::Str("Web".into()));
    f.dispatch_until(|f| f.state.caminito.policy.workspace_names()[0] == "Web");
    f.chord(&[KEY_LEFTCTRL, KEY_LEFTALT, KEY_RIGHT]);
    f.x11_call(x, |x| {
        x.send_client_message(x.root(), "_NET_CURRENT_DESKTOP", [3, 0, 0, 0, 0])
    })
    .unwrap();
    f.dispatch_until(|f| f.state.caminito.policy.active_workspace() == 3);
    f.add_output((1280, 1024), 1.0);
    f.dispatch();

    let written = f
        .x11_call(x, |x| {
            let mut names = x
                .events()?
                .iter()
                .filter_map(|event| match event {
                    Event::PropertyNotify(e) if e.window == x.root() => Some(x.atom_name(e.atom)),
                    _ => None,
                })
                .collect::<Result<Vec<_>, _>>()?;
            names.sort();
            names.dedup();
            Ok::<_, x11rb::errors::ReplyError>(names.join("\n"))
        })
        .unwrap();

    // None of `_NET_CLIENT_LIST`, `_NET_CLIENT_LIST_STACKING`, `_NET_ACTIVE_WINDOW`,
    // `_NET_WM_STATE`, `WM_STATE` or `_NET_SHOWING_DESKTOP` (11).
    assert_snapshot!(written, @"
    _GTK_WORKAREAS_D0
    _GTK_WORKAREAS_D1
    _GTK_WORKAREAS_D2
    _GTK_WORKAREAS_D3
    _GTK_WORKAREAS_D4
    _NET_CURRENT_DESKTOP
    _NET_DESKTOP_NAMES
    _NET_NUMBER_OF_DESKTOPS
    _NET_WORKAREA
    ");
}
