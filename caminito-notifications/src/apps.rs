//! The Notifying apps the Apps page lists (CMN-262): the known apps, which have sent a
//! notification, and the installed apps whose desktop entry says they send them, before they
//! have, by `X-GNOME-UsesNotifications` or a KNotifications `.notifyrc` file naming the entry.

use std::path::{Path, PathBuf};

use gio::glib;

use crate::notification::icon;
use crate::server::Icon;

const GROUP: &str = "Desktop Entry";

/// A Notifying app as the Apps page shows it.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct App {
    /// Its key: its desktop entry's id without `.desktop`, or the name it gives.
    pub key: String,
    /// Its desktop entry's name, or else its key.
    pub name: String,
    /// Its desktop entry's icon.
    pub icon: Option<Icon>,
}

/// The apps of `known`, the known apps' keys, and the installed ones that say they send
/// notifications, by their names.
pub fn listed(known: &[String]) -> Vec<App> {
    let mut dirs = vec![glib::user_data_dir()];
    dirs.extend(glib::system_data_dirs());
    listed_in(&dirs, known)
}

/// [`listed`], with the XDG data directories `dirs`, the first one's entries first.
fn listed_in(dirs: &[PathBuf], known: &[String]) -> Vec<App> {
    let entries = entries(dirs);
    let mut declared = notifyrc_entries(dirs);
    declared.extend(
        entries
            .iter()
            .filter(|entry| entry.uses_notifications)
            .map(|entry| entry.app.key.clone()),
    );
    let mut apps: Vec<App> = known
        .iter()
        .map(
            |key| match entries.iter().find(|entry| entry.app.key == *key) {
                Some(entry) => entry.app.clone(),
                None => App {
                    key: key.clone(),
                    name: key.clone(),
                    icon: None,
                },
            },
        )
        .collect();
    for entry in &entries {
        let shown = !entry.no_display && declared.contains(&entry.app.key);
        if shown && !apps.iter().any(|app| app.key == entry.app.key) {
            apps.push(entry.app.clone());
        }
    }
    apps.sort_by_key(|app| app.name.to_lowercase());
    apps
}

/// An installed desktop entry.
struct Entry {
    app: App,
    no_display: bool,
    uses_notifications: bool,
}

/// The desktop entries in `dirs`' `applications/`, the first of each id, without the hidden.
fn entries(dirs: &[PathBuf]) -> Vec<Entry> {
    let mut entries: Vec<Entry> = Vec::new();
    let mut seen: Vec<String> = Vec::new();
    for (path, id) in files(dirs, "applications", ".desktop") {
        if seen.contains(&id) {
            continue;
        }
        seen.push(id.clone());
        let file = glib::KeyFile::new();
        if file
            .load_from_file(&path, glib::KeyFileFlags::NONE)
            .is_err()
            || file.boolean(GROUP, "Hidden").unwrap_or(false)
        {
            continue;
        }
        let Ok(name) = file.locale_string(GROUP, "Name", None) else {
            continue;
        };
        entries.push(Entry {
            app: App {
                key: id,
                name: name.to_string(),
                icon: file.string(GROUP, "Icon").ok().and_then(|text| icon(&text)),
            },
            no_display: file.boolean(GROUP, "NoDisplay").unwrap_or(false),
            uses_notifications: file
                .boolean(GROUP, "X-GNOME-UsesNotifications")
                .unwrap_or(false),
        });
    }
    entries
}

/// The desktop entries' ids KNotifications' `.notifyrc` files in `dirs` name in their
/// `[Global]` group's `DesktopEntry`, or by their own name without one.
fn notifyrc_entries(dirs: &[PathBuf]) -> Vec<String> {
    ["knotifications6", "knotifications5"]
        .into_iter()
        .flat_map(|subdir| files(dirs, subdir, ".notifyrc"))
        .map(|(path, name)| {
            let file = glib::KeyFile::new();
            file.load_from_file(&path, glib::KeyFileFlags::NONE)
                .ok()
                .and_then(|()| file.string("Global", "DesktopEntry").ok())
                .map_or(name, |entry| entry.trim_end_matches(".desktop").to_owned())
        })
        .collect()
}

/// The files of `dirs`' `subdir` whose names end in `suffix`, with their names without it,
/// in the order of `dirs`.
fn files(dirs: &[PathBuf], subdir: &str, suffix: &str) -> Vec<(PathBuf, String)> {
    dirs.iter()
        .flat_map(|dir| sorted(&dir.join(subdir)))
        .filter_map(|path| {
            let name = path.file_name()?.to_str()?.strip_suffix(suffix)?.to_owned();
            Some((path, name))
        })
        .collect()
}

fn sorted(dir: &Path) -> Vec<PathBuf> {
    let mut paths: Vec<PathBuf> = std::fs::read_dir(dir)
        .into_iter()
        .flatten()
        .flatten()
        .map(|file| file.path())
        .collect();
    paths.sort();
    paths
}

#[cfg(test)]
mod tests {
    use super::*;

    fn write(dir: &Path, file: &str, text: &str) {
        let path = dir.join(file);
        std::fs::create_dir_all(path.parent().unwrap()).unwrap();
        std::fs::write(path, text).unwrap();
    }

    fn entry(name: &str, extra: &str) -> String {
        format!("[Desktop Entry]\nType=Application\nName={name}\nIcon=icon-{name}\n{extra}")
    }

    #[test]
    fn the_known_apps_and_those_that_say_they_send_notifications_are_listed_by_name() {
        let user = tempfile::tempdir().unwrap();
        let system = tempfile::tempdir().unwrap();
        let (user, system) = (user.path(), system.path());
        write(
            system,
            "applications/mail.desktop",
            &entry("Mail", "X-GNOME-UsesNotifications=true"),
        );
        write(
            user,
            "applications/mail.desktop",
            &entry("My mail", "X-GNOME-UsesNotifications=true"),
        );
        write(system, "applications/chat.desktop", &entry("chat", ""));
        write(
            system,
            "knotifications6/chat.notifyrc",
            "[Global]\nIconName=chat\n",
        );
        write(
            system,
            "applications/org.kde.kmail2.desktop",
            &entry("KMail", ""),
        );
        write(
            system,
            "knotifications5/kmail2.notifyrc",
            "[Global]\nDesktopEntry=org.kde.kmail2\n",
        );
        write(system, "applications/editor.desktop", &entry("Editor", ""));
        write(
            system,
            "applications/daemon.desktop",
            &entry("Daemon", "NoDisplay=true\nX-GNOME-UsesNotifications=true"),
        );
        write(
            system,
            "applications/hidden.desktop",
            &entry("Hidden", "Hidden=true\nX-GNOME-UsesNotifications=true"),
        );
        write(
            system,
            "applications/tool.desktop",
            &entry("Tool", "NoDisplay=true"),
        );

        let known = ["Script".to_owned(), "tool".to_owned()];
        let apps = listed_in(&[user.to_owned(), system.to_owned()], &known);

        let names: Vec<(&str, &str)> = apps
            .iter()
            .map(|app| (app.key.as_str(), app.name.as_str()))
            .collect();
        assert_eq!(
            names,
            [
                ("chat", "chat"),
                ("org.kde.kmail2", "KMail"),
                ("mail", "My mail"),
                ("Script", "Script"),
                ("tool", "Tool"),
            ]
        );
        assert_eq!(apps[2].icon, Some(Icon::Name("icon-My mail".to_owned())));
        assert_eq!(apps[3].icon, None);
    }
}
