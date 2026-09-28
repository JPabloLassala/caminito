//! Finding a theme's file the way Marco does: newest format first, then each data
//! directory in order.

use std::ffi::OsString;
use std::fs;
use std::path::{Path, PathBuf};

use camino_theme::{Theme, ThemeDirs, ThemeError};

/// The smallest theme Marco accepts in `major`'s format, described as `description`.
fn theme(major: u32, description: &str) -> String {
    let mut functions = vec!["close", "maximize", "minimize", "menu"];
    if major >= 2 {
        functions.extend(["shade", "above", "stick", "unshade", "unabove", "unstick"]);
    }
    let buttons: String = functions
        .iter()
        .flat_map(|function| {
            ["normal", "pressed"].map(|state| {
                format!(r#"<button function="{function}" state="{state}" draw_ops="empty"/>"#)
            })
        })
        .collect();
    let windows: String = [
        "normal",
        "dialog",
        "modal_dialog",
        "menu",
        "utility",
        "border",
    ]
    .iter()
    .map(|frame_type| format!(r#"<window type="{frame_type}" style_set="set"/>"#))
    .collect();
    format!(
        r#"<metacity_theme>
<info><name>T</name><author>A</author><copyright>C</copyright><date>D</date>
<description>{description}</description></info>
<frame_geometry name="g">
<distance name="left_width" value="1"/><distance name="right_width" value="1"/>
<distance name="bottom_height" value="1"/><distance name="title_vertical_pad" value="1"/>
<distance name="left_titlebar_edge" value="1"/><distance name="right_titlebar_edge" value="1"/>
<aspect_ratio name="button" value="1.0"/>
<border name="title_border" left="1" right="1" top="1" bottom="1"/>
<border name="button_border" left="1" right="1" top="1" bottom="1"/>
</frame_geometry>
<draw_ops name="empty"/>
<frame_style name="s" geometry="g">{buttons}</frame_style>
<frame_style_set name="set">
<frame focus="yes" state="normal" resize="both" style="s"/>
<frame focus="no" state="normal" resize="both" style="s"/>
<frame focus="yes" state="maximized" style="s"/><frame focus="no" state="maximized" style="s"/>
<frame focus="yes" state="shaded" style="s"/><frame focus="no" state="shaded" style="s"/>
<frame focus="yes" state="maximized_and_shaded" style="s"/>
<frame focus="no" state="maximized_and_shaded" style="s"/>
</frame_style_set>
{windows}
</metacity_theme>"#
    )
}

/// Writes `text` as `theme`'s `metacity-theme-<major>.xml` under data directory `dir`.
fn install(dir: &Path, theme: &str, major: u32, text: &str) -> PathBuf {
    let theme_dir = dir.join("themes").join(theme).join("metacity-1");
    fs::create_dir_all(&theme_dir).unwrap();
    let path = theme_dir.join(format!("metacity-theme-{major}.xml"));
    fs::write(&path, text).unwrap();
    path
}

fn description(theme: &Theme) -> &str {
    &theme.info.description
}

#[test]
fn the_newest_format_a_theme_ships_is_loaded() {
    let data = tempfile::tempdir().unwrap();
    install(data.path(), "T", 1, &theme(1, "v1"));
    let v2 = install(data.path(), "T", 2, &theme(2, "v2"));
    let theme = Theme::load("T", &ThemeDirs::new(vec![data.path().to_owned()])).unwrap();
    assert_eq!(description(&theme), "v2");
    assert_eq!(theme.path, v2);
    assert_eq!(theme.name, "T");
}

#[test]
fn a_newer_format_wins_over_an_earlier_directory() {
    let home = tempfile::tempdir().unwrap();
    let system = tempfile::tempdir().unwrap();
    install(home.path(), "T", 1, &theme(1, "home v1"));
    install(system.path(), "T", 1, &theme(1, "system v1"));
    let dirs = ThemeDirs::new(vec![home.path().to_owned(), system.path().to_owned()]);
    assert_eq!(description(&Theme::load("T", &dirs).unwrap()), "home v1");

    install(system.path(), "T", 2, &theme(2, "system v2"));
    assert_eq!(description(&Theme::load("T", &dirs).unwrap()), "system v2");
}

#[test]
fn a_file_needing_a_newer_marco_falls_back_to_an_older_file() {
    let data = tempfile::tempdir().unwrap();
    install(data.path(), "T", 2, &theme(2, "v2"));
    install(
        data.path(),
        "T",
        3,
        &theme(3, "v3").replace("<metacity_theme>", r#"<metacity_theme version=">= 3.6">"#),
    );
    let theme = Theme::load("T", &ThemeDirs::new(vec![data.path().to_owned()])).unwrap();
    assert_eq!(description(&theme), "v2");
}

#[test]
fn an_invalid_file_stops_the_search() {
    let data = tempfile::tempdir().unwrap();
    install(data.path(), "T", 2, &theme(2, "v2"));
    let v3 = install(data.path(), "T", 3, "<metacity_theme>");
    let error = Theme::load("T", &ThemeDirs::new(vec![data.path().to_owned()])).unwrap_err();
    let ThemeError::Invalid { path, .. } = error else {
        panic!("{error:?}")
    };
    assert_eq!(path, v3);
}

#[test]
fn a_theme_in_no_directory_is_not_found() {
    let data = tempfile::tempdir().unwrap();
    let error = Theme::load("Nope", &ThemeDirs::new(vec![data.path().to_owned()])).unwrap_err();
    assert_eq!(
        error,
        ThemeError::NotFound {
            name: "Nope".to_owned()
        }
    );
}

#[test]
fn data_directories_follow_the_xdg_base_directory_spec() {
    let dirs = ThemeDirs::from_vars(
        Some(OsString::from("/data/home")),
        Some(OsString::from("/home/me")),
        Some(OsString::from("/opt/share:/usr/share")),
    );
    assert_eq!(
        dirs.dirs(),
        [
            PathBuf::from("/data/home"),
            PathBuf::from("/opt/share"),
            PathBuf::from("/usr/share")
        ]
    );
    let defaults = ThemeDirs::from_vars(
        Some(OsString::new()),
        Some(OsString::from("/home/me")),
        None,
    );
    assert_eq!(
        defaults.dirs(),
        [
            PathBuf::from("/home/me/.local/share"),
            PathBuf::from("/usr/local/share"),
            PathBuf::from("/usr/share")
        ]
    );
}
