//! XSETTINGS (40) as an X11 GTK app reads them: the table on the `_XSETTINGS_S0` owner, and
//! the Xft resources (47) as an Xft app reads them, from `RESOURCE_MANAGER`. Neither is a
//! window's, so no test maps one.

use camino_test_client::x11::{PropertyValue, XSetting};
use insta::assert_snapshot;

use super::fixture::{ClientId, Fixture, Kind};
use crate::settings::Value;
use crate::settings::translate::{FONT_RENDERING, INTERFACE};

/// The XSETTINGS `names`, one per line, as `name = value`.
fn xsettings(f: &mut Fixture, x: ClientId, names: &'static [&'static str]) -> String {
    let table = f
        .x11_call(x, |x| x.xsettings())
        .unwrap()
        .expect("Camino publishes XSETTINGS");
    names
        .iter()
        .map(|name| match table.iter().find(|(n, _)| n == name) {
            Some((_, XSetting::Int(i))) => format!("{name} = {i}"),
            Some((_, XSetting::Str(s))) => format!("{name} = {s:?}"),
            Some((_, XSetting::Color(c))) => format!("{name} = {c:?}"),
            None => format!("{name} unset"),
        })
        .collect::<Vec<_>>()
        .join("\n")
}

#[test]
fn x_clients_read_mates_theme_fonts_and_dpi() {
    let mut f = Fixture::with_settings(vec![
        (
            INTERFACE,
            "gtk-theme".into(),
            Value::Str("BlackMATE".into()),
        ),
        (INTERFACE, "icon-theme".into(), Value::Str("mate".into())),
        (
            INTERFACE,
            "font-name".into(),
            Value::Str("Cantarell 11".into()),
        ),
        (FONT_RENDERING, "hinting".into(), Value::Str("full".into())),
        (FONT_RENDERING, "dpi".into(), Value::Double(120.0)),
    ]);
    f.add_output((1920, 1080), 1.0);
    let x = f.connect(Kind::X11);

    assert_snapshot!(
        xsettings(
            &mut f,
            x,
            &[
                "Net/ThemeName",
                "Net/IconThemeName",
                "Gtk/FontName",
                "Xft/HintStyle",
                "Xft/DPI",
                "Gdk/UnscaledDPI",
                "Gdk/WindowScalingFactor",
            ]
        ),
        @r#"
    Net/ThemeName = "BlackMATE"
    Net/IconThemeName = "mate"
    Gtk/FontName = "Cantarell 11"
    Xft/HintStyle = "hintfull"
    Xft/DPI = 122880
    Gdk/UnscaledDPI = 122880
    Gdk/WindowScalingFactor = 1
    "#
    );
}

/// `Xft.dpi` in `RESOURCE_MANAGER`, as an Xft client reads it.
fn xft_dpi(f: &mut Fixture, x: ClientId) -> String {
    let property = f
        .x11_call(x, |x| x.property(x.root(), "RESOURCE_MANAGER"))
        .unwrap()
        .expect("Camino writes RESOURCE_MANAGER");
    assert_eq!(property.type_, "STRING");
    let PropertyValue::Bytes(bytes) = property.value else {
        panic!("RESOURCE_MANAGER has format 8");
    };
    String::from_utf8(bytes)
        .unwrap()
        .lines()
        .filter(|line| line.starts_with("Xft.dpi:"))
        .collect::<Vec<_>>()
        .join("\n")
}

#[test]
fn xft_clients_read_the_dpi_at_the_x11_scale_from_the_resources_and_again_after_a_restart() {
    let mut f = Fixture::new();
    f.restarts_xwayland();
    let output = f.add_output((1920, 1080), 1.0);
    let x = f.connect(Kind::X11);
    assert_eq!(xft_dpi(&mut f, x), "Xft.dpi:\t96");

    f.set_output_scale(&output, 2.0);
    assert_eq!(xft_dpi(&mut f, x), "Xft.dpi:\t192");

    f.crash_xwayland();
    f.wait_for_xwayland();
    let x = f.connect_x11();
    assert_eq!(xft_dpi(&mut f, x), "Xft.dpi:\t192");
}

#[test]
fn a_gtk_theme_change_reaches_x_clients_live() {
    let (mut f, x) = Fixture::connected(Kind::X11);
    assert_snapshot!(xsettings(&mut f, x, &["Net/ThemeName"]), @r#"Net/ThemeName = "Menta""#);

    f.set_setting(INTERFACE, "gtk-theme", Value::Str("TraditionalOk".into()));
    f.dispatch_until(|f| f.state.camino.settings.interface.gtk_theme == "TraditionalOk");

    assert_snapshot!(
        xsettings(&mut f, x, &["Net/ThemeName"]),
        @r#"Net/ThemeName = "TraditionalOk""#
    );
}
