use std::fs;

use camino_theme::gdk_pixbuf::{Colorspace, Pixbuf};
use tempfile::TempDir;

use super::*;

/// Fake XDG data dirs: `data` is the first, `system` the second.
struct Dirs {
    root: TempDir,
}

impl Dirs {
    fn new() -> Self {
        Dirs {
            root: tempfile::tempdir().unwrap(),
        }
    }

    fn data(&self) -> PathBuf {
        self.root.path().join("data")
    }

    fn system(&self) -> PathBuf {
        self.root.path().join("system")
    }

    fn home(&self) -> PathBuf {
        self.root.path().join("home")
    }

    fn lookup(&self, theme: &str) -> IconLookup {
        IconLookup::new(vec![self.data(), self.system()], Some(self.home()), theme)
    }

    /// Writes `contents` at `path` under the root, making its directories.
    fn write(&self, path: &str, contents: &str) -> PathBuf {
        let path = self.root.path().join(path);
        fs::create_dir_all(path.parent().unwrap()).unwrap();
        fs::write(&path, contents).unwrap();
        path
    }

    /// An `index.theme` for `theme` in `system/icons`, with `directories` as
    /// `(name, group body)`.
    fn theme(&self, theme: &str, inherits: Option<&str>, directories: &[(&str, &str)]) {
        let names: Vec<&str> = directories.iter().map(|(name, _)| *name).collect();
        let mut text = format!(
            "[Icon Theme]\nName={theme}\nDirectories={}\n",
            names.join(",")
        );
        if let Some(inherits) = inherits {
            text += &format!("Inherits={inherits}\n");
        }
        for (name, body) in directories {
            text += &format!("\n[{name}]\n{body}\n");
        }
        self.write(&format!("system/icons/{theme}/index.theme"), &text);
    }

    /// An empty icon file under `system/icons/<theme>/`.
    fn icon(&self, theme: &str, path: &str) -> PathBuf {
        self.write(&format!("system/icons/{theme}/{path}"), "")
    }
}

const FIXED_16: (&str, &str) = ("16x16/apps", "Size=16\nType=Fixed");
const FIXED_24: (&str, &str) = ("24x24/apps", "Size=24\nType=Fixed");
const FIXED_24_AT_2: (&str, &str) = ("24x24@2/apps", "Size=24\nScale=2\nType=Fixed");
const FIXED_48: (&str, &str) = ("48x48/apps", "Size=48\nType=Fixed");
const SCALABLE: (&str, &str) = (
    "scalable/apps",
    "Size=48\nMinSize=8\nMaxSize=512\nType=Scalable",
);

#[test]
fn an_icon_in_a_directory_of_its_size_is_found() {
    let dirs = Dirs::new();
    dirs.theme("menta", None, &[FIXED_16, FIXED_24, FIXED_48]);
    dirs.icon("menta", "16x16/apps/caja.png");
    let wanted = dirs.icon("menta", "24x24/apps/caja.png");
    dirs.icon("menta", "48x48/apps/caja.png");

    assert_eq!(dirs.lookup("menta").find_icon("caja", 24, 1), Some(wanted));
}

#[test]
fn without_its_size_the_icon_of_the_closest_size_is_found() {
    let dirs = Dirs::new();
    dirs.theme("menta", None, &[FIXED_16, FIXED_48]);
    let wanted = dirs.icon("menta", "16x16/apps/caja.png");
    dirs.icon("menta", "48x48/apps/caja.png");

    assert_eq!(dirs.lookup("menta").find_icon("caja", 24, 1), Some(wanted));
}

#[test]
fn a_scalable_icon_matches_any_size_in_its_range() {
    let dirs = Dirs::new();
    dirs.theme("menta", None, &[FIXED_16, SCALABLE]);
    dirs.icon("menta", "16x16/apps/caja.png");
    let wanted = dirs.icon("menta", "scalable/apps/caja.svg");

    assert_eq!(dirs.lookup("menta").find_icon("caja", 32, 1), Some(wanted));
}

#[test]
fn a_threshold_directory_matches_sizes_within_its_threshold() {
    let dirs = Dirs::new();
    dirs.theme(
        "menta",
        None,
        &[FIXED_16, ("22x22/apps", "Size=22\nThreshold=2")],
    );
    dirs.icon("menta", "16x16/apps/caja.png");
    let wanted = dirs.icon("menta", "22x22/apps/caja.png");

    assert_eq!(dirs.lookup("menta").find_icon("caja", 24, 1), Some(wanted));
}

#[test]
fn png_is_preferred_to_svg_and_xpm_in_one_directory() {
    let dirs = Dirs::new();
    dirs.theme("menta", None, &[FIXED_24]);
    dirs.icon("menta", "24x24/apps/caja.xpm");
    dirs.icon("menta", "24x24/apps/caja.svg");
    let wanted = dirs.icon("menta", "24x24/apps/caja.png");

    assert_eq!(dirs.lookup("menta").find_icon("caja", 24, 1), Some(wanted));
}

#[test]
fn a_scale_2_directory_is_chosen_for_scale_2() {
    let dirs = Dirs::new();
    dirs.theme("menta", None, &[FIXED_24, FIXED_24_AT_2]);
    let at_1 = dirs.icon("menta", "24x24/apps/caja.png");
    let at_2 = dirs.icon("menta", "24x24@2/apps/caja.png");
    let mut lookup = dirs.lookup("menta");

    assert_eq!(lookup.find_icon("caja", 24, 2), Some(at_2));
    assert_eq!(lookup.find_icon("caja", 24, 1), Some(at_1));
}

#[test]
fn at_scale_2_a_directory_of_twice_the_size_is_closest() {
    let dirs = Dirs::new();
    dirs.theme("menta", None, &[FIXED_24, FIXED_48]);
    dirs.icon("menta", "24x24/apps/caja.png");
    let wanted = dirs.icon("menta", "48x48/apps/caja.png");

    assert_eq!(dirs.lookup("menta").find_icon("caja", 24, 2), Some(wanted));
}

#[test]
fn an_icon_missing_from_the_theme_is_found_in_the_themes_it_inherits_depth_first() {
    let dirs = Dirs::new();
    dirs.theme("menta", Some("mate,gnome"), &[FIXED_24]);
    dirs.theme("mate", Some("base"), &[FIXED_24]);
    dirs.theme("base", None, &[FIXED_24]);
    dirs.theme("gnome", None, &[FIXED_24]);
    dirs.theme("hicolor", None, &[FIXED_24]);
    let wanted = dirs.icon("base", "24x24/apps/caja.png");
    dirs.icon("gnome", "24x24/apps/caja.png");
    dirs.icon("hicolor", "24x24/apps/caja.png");

    assert_eq!(dirs.lookup("menta").find_icon("caja", 24, 1), Some(wanted));
}

#[test]
fn a_theme_without_the_icon_falls_back_to_hicolor() {
    let dirs = Dirs::new();
    dirs.theme("menta", Some("mate"), &[FIXED_24]);
    dirs.theme("mate", None, &[FIXED_24]);
    dirs.theme("hicolor", None, &[FIXED_48]);
    let wanted = dirs.icon("hicolor", "48x48/apps/caja.png");

    assert_eq!(dirs.lookup("menta").find_icon("caja", 24, 1), Some(wanted));
}

#[test]
fn themes_that_inherit_each_other_are_each_looked_in_once() {
    let dirs = Dirs::new();
    dirs.theme("menta", Some("mate"), &[FIXED_24]);
    dirs.theme("mate", Some("menta"), &[FIXED_24]);

    let mut lookup = dirs.lookup("menta");

    assert_eq!(lookup.find_icon("caja", 24, 1), None);
    assert_eq!(lookup.chain(), ["menta", "mate"]);
}

#[test]
fn an_icon_in_no_theme_is_found_in_pixmaps() {
    let dirs = Dirs::new();
    dirs.theme("menta", None, &[FIXED_24]);
    dirs.theme("hicolor", None, &[FIXED_24]);
    let wanted = dirs.write("system/pixmaps/xterm.xpm", "");

    assert_eq!(dirs.lookup("menta").find_icon("xterm", 24, 1), Some(wanted));
}

#[test]
fn a_theme_in_home_icons_comes_before_the_data_dirs() {
    let dirs = Dirs::new();
    dirs.theme("menta", None, &[FIXED_24]);
    dirs.icon("menta", "24x24/apps/caja.png");
    let wanted = dirs.write("home/.icons/menta/24x24/apps/caja.png", "");

    assert_eq!(dirs.lookup("menta").find_icon("caja", 24, 1), Some(wanted));
}

#[test]
fn an_absolute_path_that_exists_is_its_own_icon() {
    let dirs = Dirs::new();
    let path = dirs.write("opt/app/icon.png", "");
    let mut lookup = dirs.lookup("menta");

    assert_eq!(lookup.find_icon(path.to_str().unwrap(), 24, 1), Some(path));
    assert_eq!(lookup.find_icon("/nowhere/icon.png", 24, 1), None);
}

#[test]
fn a_name_with_an_extension_is_found_without_it() {
    let dirs = Dirs::new();
    dirs.theme("hicolor", None, &[FIXED_24]);
    let wanted = dirs.icon("hicolor", "24x24/apps/caja.png");

    assert_eq!(
        dirs.lookup("menta").find_icon("caja.png", 24, 1),
        Some(wanted)
    );
}

#[test]
fn set_theme_looks_in_the_new_theme() {
    let dirs = Dirs::new();
    dirs.theme("menta", None, &[FIXED_24]);
    dirs.theme("blue", None, &[FIXED_24]);
    let menta = dirs.icon("menta", "24x24/apps/caja.png");
    let blue = dirs.icon("blue", "24x24/apps/caja.png");
    let mut lookup = dirs.lookup("menta");
    assert_eq!(lookup.find_icon("caja", 24, 1), Some(menta));

    lookup.set_theme("blue");

    assert_eq!(lookup.find_icon("caja", 24, 1), Some(blue));
}

#[test]
fn the_desktop_entry_named_after_the_app_id_gives_the_icon() {
    let dirs = Dirs::new();
    dirs.write(
        "system/applications/org.mate.Caja.desktop",
        "# Caja\n[Desktop Entry]\nName=Caja\nIcon[de]=ordner\nIcon = system-file-manager\n\n\
         [Desktop Action New]\nIcon=window-new\n",
    );

    assert_eq!(
        dirs.lookup("menta").desktop_icon("org.mate.Caja"),
        Some("system-file-manager".into())
    );
}

#[test]
fn an_earlier_data_dir_shadows_a_later_ones_desktop_entry() {
    let dirs = Dirs::new();
    dirs.write(
        "system/applications/caja.desktop",
        "[Desktop Entry]\nIcon=system\n",
    );
    dirs.write(
        "data/applications/caja.desktop",
        "[Desktop Entry]\nIcon=user\n",
    );

    assert_eq!(
        dirs.lookup("menta").desktop_icon("caja"),
        Some("user".into())
    );
}

#[test]
fn the_desktop_entry_of_the_lowercase_app_id_gives_the_icon() {
    let dirs = Dirs::new();
    dirs.write(
        "system/applications/pluma.desktop",
        "[Desktop Entry]\nIcon=accessories-text-editor\n",
    );

    assert_eq!(
        dirs.lookup("menta").desktop_icon("Pluma"),
        Some("accessories-text-editor".into())
    );
}

#[test]
fn a_dash_in_the_app_id_can_be_a_subdirectory() {
    let dirs = Dirs::new();
    dirs.write(
        "system/applications/mate/terminal.desktop",
        "[Desktop Entry]\nIcon=utilities-terminal\n",
    );

    assert_eq!(
        dirs.lookup("menta").desktop_icon("mate-terminal"),
        Some("utilities-terminal".into())
    );
}

#[test]
fn a_desktop_entry_whose_startup_wm_class_is_the_app_id_gives_the_icon() {
    let dirs = Dirs::new();
    dirs.write(
        "system/applications/other.desktop",
        "[Desktop Entry]\nIcon=other\n",
    );
    dirs.write(
        "system/applications/firefox-esr.desktop",
        "[Desktop Entry]\nIcon=firefox-esr\nStartupWMClass=Firefox-esr\n",
    );

    assert_eq!(
        dirs.lookup("menta").desktop_icon("firefox-ESR"),
        Some("firefox-esr".into())
    );
}

#[test]
fn an_app_id_with_no_desktop_entry_has_no_desktop_icon() {
    let dirs = Dirs::new();
    dirs.write(
        "system/applications/caja.desktop",
        "[Desktop Entry]\nIcon=caja\n",
    );

    assert_eq!(dirs.lookup("menta").desktop_icon("unknown"), None);
}

#[test]
fn a_window_gets_its_desktop_entrys_icon_from_the_theme() {
    let dirs = Dirs::new();
    dirs.theme("hicolor", None, &[FIXED_16]);
    dirs.write(
        "system/applications/pluma.desktop",
        "[Desktop Entry]\nIcon=pluma\n",
    );
    let wanted = dirs.icon("hicolor", "16x16/apps/pluma.png");
    dirs.icon("hicolor", "16x16/apps/application-x-executable.png");

    assert_eq!(
        dirs.lookup("menta").window_icon(Some("pluma"), 16, 1),
        Some(wanted)
    );
}

#[test]
fn a_window_without_a_desktop_icon_gets_application_x_executable() {
    let dirs = Dirs::new();
    dirs.theme("hicolor", None, &[FIXED_16]);
    dirs.write(
        "system/applications/broken.desktop",
        "[Desktop Entry]\nIcon=not-installed\n",
    );
    let wanted = dirs.icon("hicolor", "16x16/apps/application-x-executable.png");
    let mut lookup = dirs.lookup("menta");

    assert_eq!(
        lookup.window_icon(Some("unknown"), 16, 1),
        Some(wanted.clone())
    );
    assert_eq!(
        lookup.window_icon(Some("broken"), 16, 1),
        Some(wanted.clone())
    );
    assert_eq!(lookup.window_icon(None, 16, 1), Some(wanted));
}

#[test]
fn an_icon_loads_at_the_size_asked_for_keeping_its_aspect() {
    let dirs = Dirs::new();
    let path = dirs.root.path().join("icon.png");
    let pixbuf = Pixbuf::new(Colorspace::Rgb, true, 8, 64, 32).unwrap();
    pixbuf.fill(0xff0000ff);
    pixbuf.savev(&path, "png", &[]).unwrap();

    let surface = load_icon(&path, 32).unwrap();

    assert_eq!((surface.width(), surface.height()), (32, 16));
}

#[test]
fn an_icon_that_doesnt_load_is_none() {
    let dirs = Dirs::new();
    let path = dirs.write("broken.png", "not a png");

    assert!(load_icon(&path, 16).is_none());
}
