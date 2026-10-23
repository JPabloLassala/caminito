//! A VPN plugin's `auth-dialog` (ADR 0041), run as nm-applet runs it for a `GetSecrets` on a
//! `vpn` setting: found through the plugin's `.name` file by its service type, given the
//! connection's data and secrets on its stdin, and read back from its stdout.
//!
//! An auth-dialog that supports the external UI mode prints its fields as a key file, and the
//! agent asks them in a dialog of its own. One that doesn't draws its own window and prints
//! the secrets as lines of a key and its value, ended by an empty pair, then waits for `QUIT`.
//! The protocol is nm-applet's, in network-manager-applet's `src/applet-vpn-request.c`, and
//! the input's format is the one libnm's `nm_vpn_service_plugin_read_vpn_details` reads.

use std::collections::{BTreeMap, HashMap};
use std::ffi::{OsStr, OsString};
use std::path::{Path, PathBuf};

use gio::glib;
use gio::prelude::*;

use crate::agent::Field;

/// Where NetworkManager's VPN plugins put their `.name` files.
pub const PLUGINS: &str = "/usr/lib/NetworkManager/VPN";

/// The group of an external UI's key file that describes the dialog, before its fields.
const UI_GROUP: &str = "VPN Plugin UI";

/// A VPN plugin's auth-dialog, from its `.name` file.
#[derive(Debug)]
pub(crate) struct Plugin {
    auth_dialog: PathBuf,
    /// It prints its fields for the agent to ask (`--external-ui-mode`).
    pub(crate) external_ui: bool,
    /// It takes NetworkManager's hints, as `-t`.
    hints: bool,
}

/// The plugin of `service`, from the `.name` files in `dir`, by its service type or one of
/// its aliases, as libnm finds it. None when no plugin there has that service, or it has no
/// auth-dialog.
pub(crate) fn find(dir: &Path, service: &str) -> Option<Plugin> {
    let mut files: Vec<PathBuf> = std::fs::read_dir(dir)
        .ok()?
        .filter_map(|entry| Some(entry.ok()?.path()))
        .filter(|path| path.extension() == Some(OsStr::new("name")))
        .collect();
    // The first by name wins, as each run must find the same one.
    files.sort();
    files.iter().find_map(|path| {
        let file = glib::KeyFile::new();
        file.load_from_file(path, glib::KeyFileFlags::NONE).ok()?;
        let named = file
            .string("VPN Connection", "service")
            .is_ok_and(|named| named == service)
            || file
                .string_list("VPN Connection", "aliases")
                .is_ok_and(|aliases| aliases.iter().any(|alias| alias.as_str() == service));
        if !named {
            return None;
        }
        let flag = |key| file.boolean("GNOME", key).unwrap_or(false);
        Some(Plugin {
            auth_dialog: PathBuf::from(file.string("GNOME", "auth-dialog").ok()?.as_str()),
            external_ui: flag("supports-external-ui-mode"),
            hints: flag("supports-hints"),
        })
    })
}

/// What the auth-dialog is told of the request, besides its input.
pub(crate) struct Run<'a> {
    pub(crate) uuid: &'a str,
    /// The connection's name.
    pub(crate) id: &'a str,
    pub(crate) service: &'a str,
    pub(crate) hints: &'a [String],
    /// It may ask the user.
    pub(crate) interactive: bool,
    /// The secrets it has are wrong, or there are none.
    pub(crate) again: bool,
}

/// The auth-dialog's command line, in nm-applet's order.
pub(crate) fn command(plugin: &Plugin, run: &Run) -> Vec<OsString> {
    let mut argv: Vec<OsString> = vec![plugin.auth_dialog.clone().into()];
    argv.extend(["-u", run.uuid, "-n", run.id, "-s", run.service].map(OsString::from));
    if run.interactive {
        argv.push("-i".into());
    }
    if run.again {
        argv.push("-r".into());
    }
    if plugin.external_ui {
        argv.push("--external-ui-mode".into());
    }
    if plugin.hints {
        for hint in run.hints {
            argv.extend(["-t".into(), hint.into()]);
        }
    }
    argv
}

/// Starts the auth-dialog with its stdin and stdout piped to the agent.
pub(crate) fn spawn(argv: &[OsString]) -> Result<gio::Subprocess, glib::Error> {
    let launcher = gio::SubprocessLauncher::new(
        gio::SubprocessFlags::STDIN_PIPE | gio::SubprocessFlags::STDOUT_PIPE,
    );
    // GLib's debug messages would go to its stdout, which the agent reads, as nm-applet says.
    launcher.unsetenv("G_MESSAGES_DEBUG");
    let argv: Vec<&OsStr> = argv.iter().map(OsString::as_os_str).collect();
    launcher.spawn(&argv)
}

/// The auth-dialog's input: the connection's data and its secrets, then `DONE`.
pub(crate) fn input(data: &HashMap<String, String>, secrets: &HashMap<String, String>) -> String {
    let mut input = String::new();
    // Sorted, so that a dialog sees its keys in the same order each time.
    for (prefix, keys) in [("DATA", data), ("SECRET", secrets)] {
        for (key, value) in keys.iter().collect::<BTreeMap<_, _>>() {
            input.push_str(&format!("{prefix}_KEY={key}\n{prefix}_VAL={value}\n\n"));
        }
    }
    input.push_str("DONE\n\n");
    input
}

/// What an auth-dialog that draws its own window has printed so far.
pub(crate) struct Printed {
    pub(crate) secrets: HashMap<String, String>,
    /// It has ended its secrets with an empty pair, and waits for QUIT.
    pub(crate) ended: bool,
}

/// The secrets an auth-dialog that draws its own window has printed in `output`, and whether it
/// has ended them with an empty pair.
pub(crate) fn printed(output: &str) -> Printed {
    // Only whole lines: the last piece is a line still being printed.
    let mut lines: Vec<&str> = output.split('\n').collect();
    lines.pop();
    let mut secrets = HashMap::new();
    for pair in lines.as_chunks::<2>().0 {
        if pair[0].is_empty() {
            return Printed {
                secrets,
                ended: true,
            };
        }
        secrets.insert(pair[0].to_owned(), pair[1].to_owned());
    }
    Printed {
        secrets,
        ended: false,
    }
}

/// What an auth-dialog in the external UI mode asks.
#[derive(Debug, PartialEq, Eq)]
pub(crate) struct Prompt {
    pub(crate) title: String,
    pub(crate) description: String,
    /// The fields the user is to fill in.
    pub(crate) fields: Vec<Field>,
    /// The secrets it gave without asking, as one the keyring had.
    pub(crate) known: HashMap<String, String>,
}

/// The external UI's key file, as nm-applet and GNOME Shell read it: a group of
/// [`UI_GROUP`] and a group for each secret.
pub(crate) fn prompt(output: &str) -> Option<Prompt> {
    let file = glib::KeyFile::new();
    file.load_from_data(output, glib::KeyFileFlags::NONE).ok()?;
    if !file.has_group(UI_GROUP) {
        return None;
    }
    let string = |group: &str, key| file.string(group, key).map(|value| value.to_string());
    let mut prompt = Prompt {
        title: string(UI_GROUP, "Title").unwrap_or_default(),
        description: string(UI_GROUP, "Description").unwrap_or_default(),
        fields: Vec::new(),
        known: HashMap::new(),
    };
    for group in file.groups().iter().map(|group| group.as_str()) {
        if group == UI_GROUP {
            continue;
        }
        let value = string(group, "Value").unwrap_or_default();
        if file.boolean(group, "ShouldAsk").unwrap_or(false) {
            prompt.fields.push(Field {
                key: group.to_owned(),
                label: string(group, "Label").unwrap_or_else(|_| group.to_owned()),
                value,
                secret: file.boolean(group, "IsSecret").unwrap_or(false),
            });
        } else if !value.is_empty() {
            prompt.known.insert(group.to_owned(), value);
        }
    }
    Some(prompt)
}

/// How an auth-dialog's run ended.
#[derive(Debug, PartialEq, Eq)]
pub(crate) enum Outcome {
    /// It gave its secrets.
    Secrets(HashMap<String, String>),
    /// In the external UI mode, it asks these fields of the user.
    Prompt(Prompt),
    /// It exited with a failure, as when the user cancelled its window.
    Canceled,
    /// A signal killed it.
    Crashed,
    /// Its external UI isn't one the agent reads.
    Unreadable,
}

/// Gives the auth-dialog its `input`, reads what it prints until it exits, and tells it to
/// quit once it has printed its secrets.
pub(crate) async fn run(process: gio::Subprocess, input: String, external_ui: bool) -> Outcome {
    let stdin = process
        .stdin_pipe()
        .expect("the auth-dialog's stdin is piped");
    let stdout = process
        .stdout_pipe()
        .expect("the auth-dialog's stdout is piped");
    // One that exits without reading it all closes the pipe: what it printed still counts.
    let _ = stdin
        .write_all_future(input.into_bytes(), glib::Priority::DEFAULT)
        .await;

    let mut output = Vec::new();
    let mut told_to_quit = false;
    while let Ok(bytes) = stdout
        .read_bytes_future(4096, glib::Priority::DEFAULT)
        .await
    {
        if bytes.is_empty() {
            break;
        }
        output.extend_from_slice(&bytes);
        // An auth-dialog of its own window waits for QUIT after its secrets, as long as 20 s.
        if !external_ui && !told_to_quit && printed(&String::from_utf8_lossy(&output)).ended {
            told_to_quit = true;
            let _ = stdin
                .write_all_future(b"QUIT\n\n", glib::Priority::DEFAULT)
                .await;
        }
    }
    // Its status is read below, whatever ended the wait.
    let _ = process.wait_future().await;

    if process.has_signaled() {
        return Outcome::Crashed;
    }
    if !process.has_exited() || process.exit_status() != 0 {
        return Outcome::Canceled;
    }
    let output = String::from_utf8_lossy(&output);
    if external_ui {
        prompt(&output).map_or(Outcome::Unreadable, Outcome::Prompt)
    } else {
        Outcome::Secrets(printed(&output).secrets)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn the_input_lists_the_data_then_the_secrets_then_done() {
        let data = HashMap::from([
            ("gateway".to_owned(), "vpn.example.com".to_owned()),
            ("cookie-flags".to_owned(), "2".to_owned()),
        ]);
        let secrets = HashMap::from([("password".to_owned(), "saved".to_owned())]);
        assert_eq!(
            input(&data, &secrets),
            "DATA_KEY=cookie-flags\nDATA_VAL=2\n\n\
             DATA_KEY=gateway\nDATA_VAL=vpn.example.com\n\n\
             SECRET_KEY=password\nSECRET_VAL=saved\n\n\
             DONE\n\n"
        );
    }

    #[test]
    fn printed_secrets_end_with_an_empty_pair() {
        let Printed { secrets, ended } = printed("password\nhunter2\ncode\n");
        assert_eq!(
            secrets,
            HashMap::from([("password".to_owned(), "hunter2".to_owned())])
        );
        assert!(!ended);
        // One empty line is a value still to come, or the end's first half.
        assert!(!printed("password\nhunter2\n\n").ended);
        // An empty value, then the end.
        let Printed { secrets, ended } = printed("password\nhunter2\ncode\n\n\n\n");
        assert_eq!(secrets.len(), 2);
        assert_eq!(secrets["code"], "");
        assert!(ended);
        assert!(printed("password\nhunter2\ncode\n123\n\n\n").ended);
    }

    #[test]
    fn an_external_ui_without_its_group_is_unreadable() {
        assert_eq!(prompt("[password]\nValue=x\n"), None);
        assert_eq!(prompt("not a key file"), None);
    }

    #[test]
    fn an_external_ui_asks_what_it_should_and_keeps_the_rest() {
        let prompt = prompt(
            "[VPN Plugin UI]\nVersion=2\nDescription=Log in\nTitle=Authenticate VPN\n\n\
             [password]\nValue=saved\nLabel=Password:\nIsSecret=true\nShouldAsk=false\n\n\
             [empty]\nValue=\nShouldAsk=false\n\n\
             [otp]\nLabel=Code:\nIsSecret=false\nShouldAsk=true\n",
        )
        .unwrap();
        assert_eq!(prompt.title, "Authenticate VPN");
        assert_eq!(prompt.description, "Log in");
        assert_eq!(
            prompt.fields,
            [Field {
                key: "otp".to_owned(),
                label: "Code:".to_owned(),
                value: String::new(),
                secret: false,
            }]
        );
        assert_eq!(
            prompt.known,
            HashMap::from([("password".to_owned(), "saved".to_owned())])
        );
    }

    #[test]
    fn a_plugin_is_found_by_its_service_or_an_alias() {
        let dir = tempfile::tempdir().unwrap();
        std::fs::write(
            dir.path().join("nm-fake-service.name"),
            "[VPN Connection]\nname=fake\nservice=org.example.Fake\naliases=org.example.Old;\n\n\
             [GNOME]\nauth-dialog=/usr/lib/fake-auth-dialog\nsupports-hints=true\n",
        )
        .unwrap();
        std::fs::write(
            dir.path().join("nm-bare-service.name"),
            "[VPN Connection]\nname=bare\nservice=org.example.Bare\n",
        )
        .unwrap();
        for service in ["org.example.Fake", "org.example.Old"] {
            let plugin = find(dir.path(), service).unwrap();
            assert_eq!(plugin.auth_dialog, Path::new("/usr/lib/fake-auth-dialog"));
            assert!(plugin.hints);
            assert!(!plugin.external_ui);
        }
        // A plugin with no auth-dialog has none to run.
        assert!(find(dir.path(), "org.example.Bare").is_none());
        assert!(find(dir.path(), "org.example.Other").is_none());
        assert!(find(&dir.path().join("missing"), "org.example.Fake").is_none());
    }
}
