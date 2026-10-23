//! The secret agent (ADR 0041): an `org.freedesktop.NetworkManager.SecretAgent` that
//! NetworkManager asks for the secrets a connection needs and doesn't have, as a Wi-Fi
//! network's password. It's registered with NetworkManager's AgentManager each time
//! NetworkManager comes on the bus, since NetworkManager forgets its agents when it stops.
//!
//! It asks for a Wi-Fi network's password or WEP key, and for an 802.1X password or private
//! key's passphrase, Wi-Fi or wired. A secret whose flags say it's agent-owned, saved "for
//! this user only", is looked for in the Secret Service first, through [`Keyring`], and asked
//! only when it isn't there or NetworkManager asks for a new one (CMN-197). The answer to such
//! a question is saved there, as nm-applet saves it, and so is what `SaveSecrets` sends;
//! `DeleteSecrets` deletes them.
//!
//! The dialogs are behind [`Dialogs`], which the binary draws in GTK and the tests replace.
//! Each request has its dialog, and NetworkManager may have several open at once, one for
//! each connection that is coming up.

use std::cell::RefCell;
use std::collections::HashMap;
use std::fmt;
use std::rc::{Rc, Weak};

use gio::glib::variant::ToVariant;
use gio::glib::{self, Variant};

use crate::DOMAIN;
use crate::keyring::{self, Keyring};

/// Where NetworkManager calls every secret agent.
pub const PATH: &str = "/org/freedesktop/NetworkManager/SecretAgent";
/// The agent's name to NetworkManager.
pub const IDENTIFIER: &str = "org.mate.Caminito.Network";

const NETWORK_MANAGER: &str = "org.freedesktop.NetworkManager";
const AGENT: &str = "org.freedesktop.NetworkManager.SecretAgent";
const AGENT_MANAGER_PATH: &str = "/org/freedesktop/NetworkManager/AgentManager";
const AGENT_MANAGER: &str = "org.freedesktop.NetworkManager.AgentManager";
/// No VPN hints: the agent asks for no VPN's secrets yet.
const CAPABILITIES: u32 = 0;

/// `GetSecrets`'s flags, NetworkManager's `NMSecretAgentGetSecretsFlags`. The agent may ask
/// the user.
const ALLOW_INTERACTION: u32 = 0x1;
/// The secrets NetworkManager has are wrong, or it has none: they're to be asked again.
const REQUEST_NEW: u32 = 0x2;

/// A secret's flags, NetworkManager's `NMSettingSecretFlags`, in the key named for it with
/// `-flags`. The agent keeps the secret, not NetworkManager.
const AGENT_OWNED: u32 = 0x1;
/// The secret is asked each time, and kept by no one.
const NOT_SAVED: u32 = 0x2;

/// The errors of NetworkManager's `NMSecretAgentError`, as their D-Bus names.
const USER_CANCELED: &str = "org.freedesktop.NetworkManager.SecretAgent.UserCanceled";
const AGENT_CANCELED: &str = "org.freedesktop.NetworkManager.SecretAgent.AgentCanceled";
const NO_SECRETS: &str = "org.freedesktop.NetworkManager.SecretAgent.NoSecrets";
const FAILED: &str = "org.freedesktop.NetworkManager.SecretAgent.Failed";

const WIFI_SECURITY: &str = "802-11-wireless-security";
const IEEE_8021X: &str = "802-1x";

/// `org.freedesktop.NetworkManager.SecretAgent`, as NetworkManager's
/// introspection/org.freedesktop.NetworkManager.SecretAgent.xml gives it.
const INTERFACE: &str = r#"<node>
  <interface name="org.freedesktop.NetworkManager.SecretAgent">
    <method name="GetSecrets">
      <arg name="connection" type="a{sa{sv}}" direction="in"/>
      <arg name="connection_path" type="o" direction="in"/>
      <arg name="setting_name" type="s" direction="in"/>
      <arg name="hints" type="as" direction="in"/>
      <arg name="flags" type="u" direction="in"/>
      <arg name="secrets" type="a{sa{sv}}" direction="out"/>
    </method>
    <method name="CancelGetSecrets">
      <arg name="connection_path" type="o" direction="in"/>
      <arg name="setting_name" type="s" direction="in"/>
    </method>
    <method name="SaveSecrets">
      <arg name="connection" type="a{sa{sv}}" direction="in"/>
      <arg name="connection_path" type="o" direction="in"/>
    </method>
    <method name="DeleteSecrets">
      <arg name="connection" type="a{sa{sv}}" direction="in"/>
      <arg name="connection_path" type="o" direction="in"/>
    </method>
  </interface>
</node>"#;

/// How the connection that needs a secret reaches its network.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Medium {
    Wifi,
    Wired,
}

/// The secret NetworkManager asks for.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum Question {
    /// A WPA Personal network's password: 8 to 63 characters, or 64 hexadecimal digits.
    Psk,
    /// A WPA3 Personal (SAE) network's password, of any length.
    Sae,
    /// A WEP network's key: 5 or 13 characters, or 10 or 26 hexadecimal digits, or a
    /// passphrase of up to 64 characters where the connection says it's one.
    WepKey { passphrase: bool },
    /// An 802.1X password, with the identity it goes with, which the user may change.
    Password {
        /// The connection's identity, empty when it has none.
        identity: String,
    },
    /// The passphrase of an 802.1X private key, for EAP-TLS or a phase 2 of TLS.
    PrivateKeyPassphrase,
}

impl Question {
    /// Whether `answer` is one NetworkManager takes for this question.
    pub fn accepts(&self, answer: &Answer) -> bool {
        let hex = |secret: &str| secret.bytes().all(|byte| byte.is_ascii_hexdigit());
        match (self, answer) {
            // NetworkManager counts bytes, as WPA does.
            (Question::Psk, Answer::Secret(psk)) => {
                (8..=63).contains(&psk.len()) || (psk.len() == 64 && hex(psk))
            }
            (Question::Sae, Answer::Secret(password)) => !password.is_empty(),
            (Question::WepKey { passphrase: true }, Answer::Secret(key)) => {
                (1..=64).contains(&key.len())
            }
            (Question::WepKey { passphrase: false }, Answer::Secret(key)) => {
                (key.is_ascii() && matches!(key.len(), 5 | 13))
                    || (matches!(key.len(), 10 | 26) && hex(key))
            }
            (Question::Password { .. }, Answer::Login { identity, password }) => {
                !identity.is_empty() && !password.is_empty()
            }
            (Question::PrivateKeyPassphrase, Answer::Secret(passphrase)) => !passphrase.is_empty(),
            _ => false,
        }
    }
}

/// What NetworkManager asks the user, and of which network.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Request {
    /// The network's name: a Wi-Fi network's SSID, or the connection's name for a wired
    /// one.
    pub network: String,
    pub medium: Medium,
    pub question: Question,
    /// The secret was asked before and didn't work, or was never given.
    pub again: bool,
}

/// The user's answer to a [`Request`].
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum Answer {
    /// A password, a key or a passphrase.
    Secret(String),
    /// An 802.1X identity and its password, for [`Question::Password`].
    Login { identity: String, password: String },
}

/// The agent's dialogs: GTK's in the binary, a fake in the tests.
pub trait Dialogs {
    /// Asks `request` of the user. The answer goes to `reply`, once, unless
    /// [`Dialogs::close`] comes first for its [`Reply::id`].
    fn ask(&self, request: Request, reply: Reply);
    /// Closes the dialog of the request `id`: NetworkManager cancelled it, or went away. A
    /// reply sent after it is ignored.
    fn close(&self, id: u64);
}

/// Where the answer to one request goes.
pub struct Reply {
    state: Weak<RefCell<State>>,
    id: u64,
}

impl Reply {
    /// The request's number, which [`Dialogs::close`] names it by.
    pub fn id(&self) -> u64 {
        self.id
    }

    /// Answers NetworkManager with the secret, unless the request was closed since. An
    /// answer the question doesn't accept is a cancel.
    pub fn send(self, answer: Answer) {
        let Some(pending) = self.take() else {
            return;
        };
        if !pending.question.accepts(&answer) {
            pending
                .invocation
                .return_dbus_error(USER_CANCELED, "the answer isn't a valid secret");
            return;
        }
        let (secret, identity) = match answer {
            Answer::Secret(secret) => (secret, None),
            Answer::Login { identity, password } => (password, Some(identity)),
        };
        let mut keys = HashMap::from([(pending.key.clone(), secret.to_variant())]);
        if let Some(identity) = identity {
            keys.insert("identity".to_owned(), identity.to_variant());
        }
        let secrets = HashMap::from([(pending.setting.clone(), keys)]);
        pending
            .invocation
            .return_value(Some(&(secrets,).to_variant()));

        // An agent-owned secret is the agent's to keep, as nm-applet keeps the answers to its
        // dialogs.
        let Some(owner) = pending.owner else {
            return;
        };
        let Some(keyring) = self
            .state
            .upgrade()
            .map(|state| state.borrow().keyring.clone())
        else {
            return;
        };
        let entry = keyring::Entry {
            label: label(&owner.name, &pending.setting, &pending.key),
            uuid: owner.uuid,
            setting: pending.setting,
            key: pending.key,
            secret,
        };
        // Without a prompt: the keyring is open if the user just unlocked it, and a user who
        // just refused isn't asked again.
        glib::spawn_future_local(async move { save(&keyring, entry, false).await });
    }

    /// Answers NetworkManager that the user cancelled, unless the request was closed since.
    pub fn cancel(self) {
        if let Some(pending) = self.take() {
            pending
                .invocation
                .return_dbus_error(USER_CANCELED, "the user cancelled");
        }
    }

    fn take(&self) -> Option<Pending> {
        let state = self.state.upgrade()?;
        state.borrow_mut().pending.remove(&self.id)
    }
}

/// Exporting the agent failed.
#[derive(Debug)]
pub enum Error {
    /// The connection refused the object, as when another is at its path.
    Export(glib::Error),
}

impl fmt::Display for Error {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Error::Export(err) => write!(f, "can't export the secret agent: {err}"),
        }
    }
}

impl std::error::Error for Error {
    fn source(&self) -> Option<&(dyn std::error::Error + 'static)> {
        match self {
            Error::Export(err) => Some(err),
        }
    }
}

/// A request waiting for the keyring or its dialog's answer.
struct Pending {
    connection_path: String,
    setting: String,
    question: Question,
    /// The key of the secret in the setting.
    key: String,
    /// The connection whose secret is agent-owned, which the answer is saved for.
    owner: Option<Owner>,
    invocation: gio::DBusMethodInvocation,
}

/// The connection an agent-owned secret is kept for.
struct Owner {
    uuid: String,
    /// The connection's name, which the keyring's items are labelled with.
    name: String,
}

impl Owner {
    /// The owner of `connection`'s secrets, if it has the UUID the keyring knows it by.
    fn of(connection: &Settings) -> Option<Owner> {
        Some(Owner {
            uuid: string(connection, "connection", "uuid")?,
            name: string(connection, "connection", "id").unwrap_or_default(),
        })
    }
}

struct State {
    dialogs: Rc<dyn Dialogs>,
    keyring: Rc<Keyring>,
    /// The last request's number.
    last_request: u64,
    pending: HashMap<u64, Pending>,
}

/// The secret agent, exported and registered with NetworkManager for as long as it lives.
pub struct Agent {
    connection: gio::DBusConnection,
    registration: Option<gio::RegistrationId>,
    /// Stops watching NetworkManager's name. A closure, since the watcher's id is of a type
    /// gio doesn't export.
    unwatch: Option<Box<dyn FnOnce()>>,
    _state: Rc<RefCell<State>>,
}

impl Drop for Agent {
    fn drop(&mut self) {
        if let Some(registration) = self.registration.take() {
            let _ = self.connection.unregister_object(registration);
        }
        if let Some(unwatch) = self.unwatch.take() {
            unwatch();
        }
    }
}

impl Agent {
    /// Exports the agent on `connection`, the system bus, and registers it with
    /// NetworkManager now and whenever NetworkManager comes back on the bus. The agent-owned
    /// secrets are kept by the Secret Service on `session`, the session bus.
    pub fn new(
        connection: &gio::DBusConnection,
        session: &gio::DBusConnection,
        dialogs: Rc<dyn Dialogs>,
    ) -> Result<Agent, Error> {
        let state = Rc::new(RefCell::new(State {
            dialogs,
            keyring: Rc::new(Keyring::new(session)),
            last_request: 0,
            pending: HashMap::new(),
        }));

        let node = gio::DBusNodeInfo::for_xml(INTERFACE).expect("the agent's interface is valid");
        let interface = node
            .lookup_interface(AGENT)
            .expect("the node has the agent's interface");
        let registration = connection
            .register_object(PATH, &interface)
            .method_call({
                let state = Rc::downgrade(&state);
                move |_, _, _, _, method, parameters, invocation| {
                    handle(&state, method, &parameters, invocation);
                }
            })
            .build()
            .map_err(Error::Export)?;

        let watcher = gio::bus_watch_name_on_connection(
            connection,
            NETWORK_MANAGER,
            gio::BusNameWatcherFlags::NONE,
            |connection, _, _| {
                let connection = connection.clone();
                glib::spawn_future_local(async move {
                    if let Err(err) = register(&connection).await {
                        glib::g_warning!(DOMAIN, "can't register the secret agent: {err}");
                    }
                });
            },
            {
                // NetworkManager won't hear the answers: its requests went with it.
                let state = Rc::downgrade(&state);
                move |_, _| {
                    if let Some(state) = state.upgrade() {
                        let open: Vec<u64> = state.borrow().pending.keys().copied().collect();
                        for id in open {
                            dismiss(&state, id);
                        }
                    }
                }
            },
        );

        Ok(Agent {
            connection: connection.clone(),
            registration: Some(registration),
            unwatch: Some(Box::new(move || gio::bus_unwatch_name(watcher))),
            _state: state,
        })
    }
}

/// Registers the agent with a NetworkManager that has just come on the bus.
async fn register(connection: &gio::DBusConnection) -> Result<Variant, glib::Error> {
    connection
        .call_future(
            Some(NETWORK_MANAGER),
            AGENT_MANAGER_PATH,
            AGENT_MANAGER,
            "RegisterWithCapabilities",
            Some(&(IDENTIFIER, CAPABILITIES).to_variant()),
            None,
            gio::DBusCallFlags::NONE,
            -1,
        )
        .await
}

/// Answers one of NetworkManager's calls. gio has checked its arguments against
/// [`INTERFACE`].
fn handle(
    state: &Weak<RefCell<State>>,
    method: &str,
    parameters: &Variant,
    invocation: gio::DBusMethodInvocation,
) {
    let Some(state) = state.upgrade() else {
        invocation.return_dbus_error(AGENT_CANCELED, "the agent has stopped");
        return;
    };
    let string_at = |index| {
        parameters
            .child_value(index)
            .str()
            .expect("the argument is a string")
            .to_owned()
    };
    match method {
        "GetSecrets" => {
            let connection: Settings = parameters
                .child_value(0)
                .get()
                .expect("the connection is a{sa{sv}}");
            let flags = parameters
                .child_value(4)
                .get::<u32>()
                .expect("the flags are a u32");
            get_secrets(
                &state,
                &connection,
                string_at(1),
                string_at(2),
                flags,
                invocation,
            );
        }
        "CancelGetSecrets" => {
            let (connection, setting) = (string_at(0), string_at(1));
            let canceled: Vec<u64> = state
                .borrow()
                .pending
                .iter()
                .filter(|(_, pending)| {
                    pending.connection_path == connection && pending.setting == setting
                })
                .map(|(id, _)| *id)
                .collect();
            for id in canceled {
                dismiss(&state, id);
            }
            invocation.return_value(None);
        }
        "SaveSecrets" | "DeleteSecrets" => {
            let connection: Settings = parameters
                .child_value(0)
                .get()
                .expect("the connection is a{sa{sv}}");
            let keyring = state.borrow().keyring.clone();
            if method == "SaveSecrets" {
                glib::spawn_future_local(save_secrets(keyring, connection, invocation));
            } else {
                glib::spawn_future_local(delete_secrets(keyring, connection, invocation));
            }
        }
        _ => invocation.return_dbus_error(
            "org.freedesktop.DBus.Error.UnknownMethod",
            &format!("the agent has no method {method}"),
        ),
    }
}

/// A connection's settings, as NetworkManager sends them: each setting's name and its keys.
type Settings = HashMap<String, HashMap<String, Variant>>;

/// Answers NetworkManager with the secret it asks for, from the keyring where it's
/// agent-owned and saved there, else from a dialog, or answers that there's none.
fn get_secrets(
    state: &Rc<RefCell<State>>,
    connection: &Settings,
    path: String,
    setting: String,
    flags: u32,
    invocation: gio::DBusMethodInvocation,
) {
    let Some((request, key)) = request_for(connection, &setting, flags) else {
        invocation.return_dbus_error(
            NO_SECRETS,
            &format!("the agent doesn't ask for the secrets of {setting}"),
        );
        return;
    };
    let interactive = flags & ALLOW_INTERACTION != 0;
    let owner =
        Owner::of(connection).filter(|_| is_agent_owned(secret_flags(connection, &setting, &key)));
    // REQUEST_NEW: the saved secret is wrong, so the keyring isn't asked for it.
    let saved_uuid = owner
        .as_ref()
        .filter(|_| flags & REQUEST_NEW == 0)
        .map(|owner| owner.uuid.clone());
    if saved_uuid.is_none() && !interactive {
        invocation.return_dbus_error(NO_SECRETS, "the agent may not ask, and keeps no secret");
        return;
    }

    let (id, keyring) = {
        let mut state = state.borrow_mut();
        state.last_request += 1;
        let id = state.last_request;
        state.pending.insert(
            id,
            Pending {
                connection_path: path,
                setting: setting.clone(),
                question: request.question.clone(),
                key: key.clone(),
                owner,
                invocation,
            },
        );
        (id, state.keyring.clone())
    };
    let Some(uuid) = saved_uuid else {
        ask(state, id, request);
        return;
    };

    let state = Rc::downgrade(state);
    glib::spawn_future_local(async move {
        // The keyring's unlock prompt is a question too, for a request that may ask.
        let found = keyring
            .find(&uuid, &setting, interactive)
            .await
            .unwrap_or_else(|err| {
                glib::g_warning!(DOMAIN, "can't read the keyring's secrets: {err}");
                HashMap::new()
            });
        let Some(state) = state.upgrade() else {
            return;
        };
        if !state.borrow().pending.contains_key(&id) {
            // NetworkManager cancelled the request meanwhile.
            return;
        }
        if interactive && !found.contains_key(&key) {
            ask(&state, id, request);
            return;
        }
        let pending = state.borrow_mut().pending.remove(&id);
        let pending = pending.expect("the request is pending");
        if found.contains_key(&key) {
            let keys: HashMap<String, Variant> = found
                .into_iter()
                .map(|(key, secret)| (key, secret.to_variant()))
                .collect();
            let secrets = HashMap::from([(setting, keys)]);
            pending
                .invocation
                .return_value(Some(&(secrets,).to_variant()));
        } else {
            pending
                .invocation
                .return_dbus_error(NO_SECRETS, "the keyring has no such secret");
        }
    });
}

/// Opens the dialog of the pending request `id`.
fn ask(state: &Rc<RefCell<State>>, id: u64, request: Request) {
    let dialogs = state.borrow().dialogs.clone();
    // Nothing is borrowed while the dialogs run: a fake answers at once.
    let reply = Reply {
        state: Rc::downgrade(state),
        id,
    };
    dialogs.ask(request, reply);
}

/// Saves the agent-owned secrets of `connection` in place of those saved for it before, as
/// nm-applet does.
async fn save_secrets(
    keyring: Rc<Keyring>,
    connection: Settings,
    invocation: gio::DBusMethodInvocation,
) {
    if let Some(owner) = Owner::of(&connection) {
        if let Err(err) = keyring.clear(&owner.uuid).await {
            glib::g_warning!(DOMAIN, "can't delete the keyring's old secrets: {err}");
        }
        for entry in agent_owned(&connection, &owner) {
            save(&keyring, entry, true).await;
        }
    }
    invocation.return_value(None);
}

/// Saves `entry` in the keyring, or warns that it can't. `may_prompt` lets it unlock a
/// locked keyring through its prompt.
async fn save(keyring: &Keyring, entry: keyring::Entry, may_prompt: bool) {
    if let Err(err) = keyring.store(&entry, may_prompt).await {
        glib::g_warning!(DOMAIN, "can't save the secret in the keyring: {err}");
    }
}

/// Deletes every secret the keyring has for `connection`.
async fn delete_secrets(
    keyring: Rc<Keyring>,
    connection: Settings,
    invocation: gio::DBusMethodInvocation,
) {
    let Some(uuid) = string(&connection, "connection", "uuid") else {
        invocation.return_value(None);
        return;
    };
    match keyring.clear(&uuid).await {
        Ok(()) => invocation.return_value(None),
        Err(err) => {
            invocation.return_dbus_error(FAILED, &format!("can't delete the secrets: {err}"))
        }
    }
}

/// The agent-owned secrets of `connection`, whose owner is `owner`, as nm-applet saves them
/// (network-manager-applet's src/applet-agent.c, line 606, `save_one_secret`, and line 642,
/// `vpn_secret_iter_cb`).
fn agent_owned(connection: &Settings, owner: &Owner) -> Vec<keyring::Entry> {
    let name = &owner.name;
    let entry = |setting: &str, key: &str, label, secret: &str| keyring::Entry {
        uuid: owner.uuid.clone(),
        setting: setting.to_owned(),
        key: key.to_owned(),
        label,
        secret: secret.to_owned(),
    };
    let mut secrets = Vec::new();
    for (setting, keys) in connection {
        if setting == "vpn" {
            // A VPN's secrets are a dictionary of their own, and their flags are in its data.
            let dictionary = |key| {
                keys.get(key)
                    .and_then(Variant::get::<HashMap<String, String>>)
                    .unwrap_or_default()
            };
            let data = dictionary("data");
            let service = keys
                .get("service-type")
                .and_then(|value| value.str().map(str::to_owned))
                .unwrap_or_default();
            for (key, secret) in dictionary("secrets") {
                let flags = data
                    .get(&format!("{key}-flags"))
                    .and_then(|flags| flags.parse::<u32>().ok())
                    .unwrap_or(0);
                if !secret.is_empty() && is_agent_owned(flags) {
                    let label = format!("VPN {key} secret for {name}/{service}/vpn");
                    secrets.push(entry(setting, &key, label, &secret));
                }
            }
            continue;
        }
        for (key, value) in keys {
            let Some(secret) = value.str().filter(|secret| !secret.is_empty()) else {
                continue;
            };
            if is_agent_owned(secret_flags(connection, setting, key)) {
                secrets.push(entry(setting, key, label(name, setting, key), secret));
            }
        }
    }
    secrets
}

/// Whether a secret with `flags` is the agent's to keep: agent-owned, and not one asked each
/// time.
fn is_agent_owned(flags: u32) -> bool {
    flags & (AGENT_OWNED | NOT_SAVED) == AGENT_OWNED
}

/// The label nm-applet gives a secret's item.
fn label(name: &str, setting: &str, key: &str) -> String {
    format!("Network secret for {name}/{setting}/{key}")
}

/// The flags of the secret `key` of `setting`: none where the connection gives none, which
/// leaves it to NetworkManager.
fn secret_flags(connection: &Settings, setting: &str, key: &str) -> u32 {
    // The four WEP keys share their flags.
    let flags = if key
        .strip_prefix("wep-key")
        .is_some_and(|index| index.len() == 1 && index.as_bytes()[0].is_ascii_digit())
    {
        "wep-key-flags".to_owned()
    } else {
        format!("{key}-flags")
    };
    connection
        .get(setting)
        .and_then(|keys| keys.get(&flags))
        .and_then(Variant::get::<u32>)
        .unwrap_or(0)
}

/// The string `key` of `setting`, if the connection has it.
fn string(connection: &Settings, setting: &str, key: &str) -> Option<String> {
    connection
        .get(setting)
        .and_then(|keys| keys.get(key))
        .and_then(|value| value.str().map(str::to_owned))
}

/// The request NetworkManager's call makes of the user, and the key its secret goes in, or
/// None for a secret the agent doesn't ask for.
fn request_for(connection: &Settings, setting: &str, flags: u32) -> Option<(Request, String)> {
    let string = |setting: &str, key: &str| string(connection, setting, key);
    let medium = match string("connection", "type")?.as_str() {
        "802-11-wireless" => Medium::Wifi,
        "802-3-ethernet" => Medium::Wired,
        _ => return None,
    };
    let ssid = connection
        .get("802-11-wireless")
        .and_then(|keys| keys.get("ssid"))
        .and_then(|ssid| ssid.get::<Vec<u8>>())
        .filter(|ssid| !ssid.is_empty())
        // An SSID is any bytes: a NUL among them would end the name in a C string.
        .map(|ssid| String::from_utf8_lossy(&ssid).replace('\0', ""));
    let network = ssid
        .or_else(|| string("connection", "id"))
        .unwrap_or_default();

    let (question, key) = match setting {
        WIFI_SECURITY => match string(WIFI_SECURITY, "key-mgmt")?.as_str() {
            "wpa-psk" => (Question::Psk, "psk".to_owned()),
            "sae" => (Question::Sae, "psk".to_owned()),
            // Static WEP: a key-mgmt of none with a key.
            "none" => {
                let keys = connection.get(WIFI_SECURITY)?;
                let index = keys
                    .get("wep-tx-keyidx")
                    .and_then(Variant::get::<u32>)
                    .unwrap_or(0);
                // NM_WEP_KEY_TYPE_PASSPHRASE.
                let passphrase = keys.get("wep-key-type").and_then(Variant::get::<u32>) == Some(2);
                (Question::WepKey { passphrase }, format!("wep-key{index}"))
            }
            _ => return None,
        },
        IEEE_8021X => {
            let methods = connection
                .get(IEEE_8021X)
                .and_then(|keys| keys.get("eap"))
                .and_then(Variant::get::<Vec<String>>)
                .unwrap_or_default();
            // TLS inside PEAP or TTLS has a private key of its own, phase 2's.
            let phase2 = ["phase2-auth", "phase2-autheap"]
                .iter()
                .any(|key| string(IEEE_8021X, key).as_deref() == Some("tls"));
            if methods.first().is_some_and(|method| method == "tls") {
                (
                    Question::PrivateKeyPassphrase,
                    "private-key-password".to_owned(),
                )
            } else if phase2 {
                (
                    Question::PrivateKeyPassphrase,
                    "phase2-private-key-password".to_owned(),
                )
            } else {
                let identity = string(IEEE_8021X, "identity").unwrap_or_default();
                (Question::Password { identity }, "password".to_owned())
            }
        }
        _ => return None,
    };
    let request = Request {
        network,
        medium,
        question,
        again: flags & REQUEST_NEW != 0,
    };
    Some((request, key))
}

/// Closes the dialog of the request `id`, if it's still open, and answers NetworkManager
/// that the agent cancelled it.
fn dismiss(state: &Rc<RefCell<State>>, id: u64) {
    let pending = state.borrow_mut().pending.remove(&id);
    if let Some(pending) = pending {
        pending
            .invocation
            .return_dbus_error(AGENT_CANCELED, "the request was cancelled");
        let dialogs = state.borrow().dialogs.clone();
        dialogs.close(id);
    }
}
