//! The secret agent against python-dbusmock's `networkmanager` template on a private bus, with
//! an AgentManager added to it. The mock never calls an agent, so the test calls it as
//! NetworkManager would, and the dialogs are a fake that the test answers. The Secret Service
//! is a fake too, on the same bus, which serves as the session bus.

mod mock;

use std::cell::RefCell;
use std::collections::HashMap;
use std::rc::Rc;
use std::time::{Duration, Instant};

use caminito_network::agent::{self, Agent, Answer, Dialogs, Medium, Question, Reply, Request};
use gio::glib::variant::{ObjectPath, ToVariant};
use gio::glib::{self, Variant};

use mock::Mock;
use mock::secret_service::{Item, SecretService};

const ALLOW_INTERACTION: u32 = 0x1;
const REQUEST_NEW: u32 = 0x2;
const WIFI_SECURITY: &str = "802-11-wireless-security";

/// The dialogs, as the agent drives them.
#[derive(Default)]
struct FakeDialogs {
    asked: RefCell<Vec<(Request, Option<Reply>)>>,
    /// The requests whose dialogs the agent closed.
    closed: RefCell<Vec<u64>>,
}

impl Dialogs for FakeDialogs {
    fn ask(&self, request: Request, reply: Reply) {
        self.asked.borrow_mut().push((request, Some(reply)));
    }

    fn close(&self, request: u64) {
        self.closed.borrow_mut().push(request);
    }
}

impl FakeDialogs {
    /// The user's answer to the last request.
    fn answer(&self, answer: Option<Answer>) {
        let reply = self
            .asked
            .borrow_mut()
            .last_mut()
            .and_then(|(_, reply)| reply.take())
            .expect("a request waits for its answer");
        match answer {
            Some(answer) => reply.send(answer),
            None => reply.cancel(),
        }
    }

    fn last_request(&self) -> Option<Request> {
        self.asked
            .borrow()
            .last()
            .map(|(request, _)| request.clone())
    }

    /// The number the last request's reply has.
    fn last_id(&self) -> u64 {
        self.asked
            .borrow()
            .last()
            .and_then(|(_, reply)| reply.as_ref().map(Reply::id))
            .expect("a request waits for its answer")
    }
}

/// The agent, on a connection of its own as the program's.
struct Running {
    /// The agent's unique name, which NetworkManager calls it at.
    name: String,
    dialogs: Rc<FakeDialogs>,
    _agent: Agent,
}

/// The registrations the AgentManager has had, as each one's identifier and capabilities.
fn registrations(mock: &Mock) -> Vec<(String, u32)> {
    // The mock gives each call's arguments as an array of variants.
    let argument = |call: &Variant, index| call.child_value(index).as_variant().unwrap();
    mock.calls(mock::AGENT_MANAGER, "RegisterWithCapabilities")
        .iter()
        .map(|call| {
            (
                argument(call, 0).str().unwrap().to_owned(),
                argument(call, 1).get::<u32>().unwrap(),
            )
        })
        .collect()
}

fn start(mock: &Mock) -> Running {
    let connection = mock.connect();
    let dialogs = Rc::new(FakeDialogs::default());
    let agent = Agent::new(&connection, &connection, dialogs.clone()).unwrap();
    mock.wait_until("the agent to register", || !registrations(mock).is_empty());
    Running {
        name: connection.unique_name().unwrap().to_string(),
        dialogs,
        _agent: agent,
    }
}

#[test]
fn the_agent_registers_with_its_identifier() {
    mock::run(|mock| {
        start(mock);
        assert_eq!(registrations(mock), [(agent::IDENTIFIER.to_owned(), 0)]);
    });
}

#[test]
fn the_agent_registers_again_when_network_manager_restarts() {
    mock::run(|mock| {
        let _running = start(mock);
        mock.stop_network_manager();
        // The new NetworkManager has heard of no agent: its calls start from none.
        mock.start_network_manager();
        mock.wait_until("the agent to register again", || {
            registrations(mock) == [(agent::IDENTIFIER.to_owned(), 0)]
        });
    });
}

/// A connection's settings, as NetworkManager sends them: each setting's name and its keys.
type Settings = HashMap<String, HashMap<String, Variant>>;

/// A Wi-Fi connection to `ssid` with the security `key_mgmt`, and its other keys `security`.
fn wifi(ssid: &str, key_mgmt: &str, security: &[(&str, Variant)]) -> Settings {
    let mut wireless_security = HashMap::from([("key-mgmt".to_owned(), key_mgmt.to_variant())]);
    wireless_security.extend(
        security
            .iter()
            .map(|(key, value)| ((*key).to_owned(), value.clone())),
    );
    HashMap::from([
        (
            "connection".to_owned(),
            HashMap::from([
                ("id".to_owned(), format!("{ssid} 1").to_variant()),
                ("type".to_owned(), "802-11-wireless".to_variant()),
            ]),
        ),
        (
            "802-11-wireless".to_owned(),
            HashMap::from([("ssid".to_owned(), ssid.as_bytes().to_variant())]),
        ),
        (WIFI_SECURITY.to_owned(), wireless_security),
    ])
}

const CONNECTION: &str = "/org/freedesktop/NetworkManager/Settings/1";

/// The answer to a call NetworkManager made, once it comes.
type Pending = Rc<RefCell<Option<Result<Variant, glib::Error>>>>;

/// Calls `method` on the agent as NetworkManager would, without waiting for the answer.
fn call(mock: &Mock, running: &Running, method: &str, parameters: Variant) -> Pending {
    let pending: Pending = Rc::default();
    let call = mock.connection.call_future(
        Some(&running.name),
        agent::PATH,
        "org.freedesktop.NetworkManager.SecretAgent",
        method,
        Some(&parameters),
        None,
        gio::DBusCallFlags::NONE,
        -1,
    );
    glib::spawn_future_local({
        let pending = pending.clone();
        async move { *pending.borrow_mut() = Some(call.await) }
    });
    pending
}

/// Asks the agent for `setting` of `connection`, as NetworkManager does.
fn get_secrets(
    mock: &Mock,
    running: &Running,
    connection: &Settings,
    setting: &str,
    flags: u32,
) -> Pending {
    let path = ObjectPath::try_from(CONNECTION.to_owned()).unwrap();
    call(
        mock,
        running,
        "GetSecrets",
        (connection, path, setting, Vec::<String>::new(), flags).to_variant(),
    )
}

/// The answer to `pending`, once it comes.
fn answer(mock: &Mock, pending: &Pending) -> Result<Variant, glib::Error> {
    mock.wait_until("the agent's answer", || pending.borrow().is_some());
    pending.borrow_mut().take().unwrap()
}

/// The secrets of an answer to GetSecrets.
fn secrets(result: Result<Variant, glib::Error>) -> HashMap<String, HashMap<String, Variant>> {
    result.unwrap().child_value(0).get().unwrap()
}

fn error(result: Result<Variant, glib::Error>) -> String {
    let err = result.unwrap_err();
    gio::DBusError::remote_error(&err).unwrap().to_string()
}

#[test]
fn a_wifi_password_is_asked_and_answered() {
    mock::run(|mock| {
        let running = start(mock);

        let pending = get_secrets(
            mock,
            &running,
            &wifi("Home", "wpa-psk", &[]),
            WIFI_SECURITY,
            ALLOW_INTERACTION,
        );
        mock.wait_until("the password's dialog", || {
            running.dialogs.last_request().is_some()
        });
        assert_eq!(
            running.dialogs.last_request().unwrap(),
            Request {
                network: "Home".to_owned(),
                medium: Medium::Wifi,
                question: Question::Psk,
                again: false,
            }
        );
        running
            .dialogs
            .answer(Some(Answer::Secret("correct horse".to_owned())));
        let secrets = secrets(answer(mock, &pending));
        assert_eq!(secrets.len(), 1);
        assert_eq!(secrets[WIFI_SECURITY]["psk"].str(), Some("correct horse"));
    });
}

#[test]
fn a_cancel_in_the_dialog_is_user_canceled() {
    mock::run(|mock| {
        let running = start(mock);
        let pending = get_secrets(
            mock,
            &running,
            &wifi("Home", "wpa-psk", &[]),
            WIFI_SECURITY,
            ALLOW_INTERACTION,
        );
        mock.wait_until("the password's dialog", || {
            running.dialogs.last_request().is_some()
        });
        running.dialogs.answer(None);
        assert_eq!(
            error(answer(mock, &pending)),
            "org.freedesktop.NetworkManager.SecretAgent.UserCanceled"
        );
    });
}

#[test]
fn cancel_get_secrets_closes_the_dialog_and_a_later_answer_is_ignored() {
    mock::run(|mock| {
        let running = start(mock);
        let pending = get_secrets(
            mock,
            &running,
            &wifi("Home", "wpa-psk", &[]),
            WIFI_SECURITY,
            ALLOW_INTERACTION,
        );
        mock.wait_until("the password's dialog", || {
            running.dialogs.last_request().is_some()
        });
        let id = running.dialogs.last_id();

        // Another connection's or another setting's cancel leaves it open.
        let other = ObjectPath::try_from("/org/freedesktop/NetworkManager/Settings/2").unwrap();
        let this = ObjectPath::try_from(CONNECTION.to_owned()).unwrap();
        for parameters in [(other, WIFI_SECURITY), (this.clone(), "802-1x")] {
            answer(
                mock,
                &call(mock, &running, "CancelGetSecrets", parameters.to_variant()),
            )
            .unwrap();
        }
        assert!(running.dialogs.closed.borrow().is_empty());

        answer(
            mock,
            &call(
                mock,
                &running,
                "CancelGetSecrets",
                (this, WIFI_SECURITY).to_variant(),
            ),
        )
        .unwrap();
        assert_eq!(*running.dialogs.closed.borrow(), [id]);
        assert_eq!(
            error(answer(mock, &pending)),
            "org.freedesktop.NetworkManager.SecretAgent.AgentCanceled"
        );

        // The dialog's answer, had it come before it closed, goes nowhere.
        running
            .dialogs
            .answer(Some(Answer::Secret("correct horse".to_owned())));
    });
}

#[test]
fn a_request_that_may_not_ask_has_no_secrets() {
    mock::run(|mock| {
        let running = start(mock);
        let pending = get_secrets(
            mock,
            &running,
            &wifi("Home", "wpa-psk", &[]),
            WIFI_SECURITY,
            0,
        );
        assert_eq!(
            error(answer(mock, &pending)),
            "org.freedesktop.NetworkManager.SecretAgent.NoSecrets"
        );
        assert!(running.dialogs.asked.borrow().is_empty());
    });
}

#[test]
fn a_wrong_password_is_asked_again() {
    mock::run(|mock| {
        let running = start(mock);
        let _pending = get_secrets(
            mock,
            &running,
            &wifi("Home", "wpa-psk", &[]),
            WIFI_SECURITY,
            ALLOW_INTERACTION | REQUEST_NEW,
        );
        mock.wait_until("the password's dialog", || {
            running.dialogs.last_request().is_some()
        });
        let request = running.dialogs.last_request().unwrap();
        assert_eq!(request.question, Question::Psk);
        assert!(request.again);
    });
}

#[test]
fn a_wep_key_goes_in_the_key_the_connection_transmits_with() {
    mock::run(|mock| {
        let running = start(mock);
        let connection = wifi("Old", "none", &[("wep-tx-keyidx", 2u32.to_variant())]);
        let pending = get_secrets(
            mock,
            &running,
            &connection,
            WIFI_SECURITY,
            ALLOW_INTERACTION,
        );
        mock.wait_until("the key's dialog", || {
            running.dialogs.last_request().is_some()
        });
        assert_eq!(
            running.dialogs.last_request().unwrap().question,
            Question::WepKey { passphrase: false }
        );
        running
            .dialogs
            .answer(Some(Answer::Secret("0123456789".to_owned())));
        assert_eq!(
            secrets(answer(mock, &pending))[WIFI_SECURITY]["wep-key2"].str(),
            Some("0123456789")
        );
    });
}

/// A wired connection with 802.1X, by `eap`, and the identity `identity`.
fn wired_8021x(eap: &str, identity: &str) -> Settings {
    HashMap::from([
        (
            "connection".to_owned(),
            HashMap::from([
                ("id".to_owned(), "Office".to_variant()),
                ("type".to_owned(), "802-3-ethernet".to_variant()),
            ]),
        ),
        (
            "802-1x".to_owned(),
            HashMap::from([
                ("eap".to_owned(), vec![eap].to_variant()),
                ("identity".to_owned(), identity.to_variant()),
            ]),
        ),
    ])
}

#[test]
fn an_8021x_password_is_asked_with_its_identity() {
    mock::run(|mock| {
        let running = start(mock);
        let pending = get_secrets(
            mock,
            &running,
            &wired_8021x("peap", "ana"),
            "802-1x",
            ALLOW_INTERACTION,
        );
        mock.wait_until("the password's dialog", || {
            running.dialogs.last_request().is_some()
        });
        assert_eq!(
            running.dialogs.last_request().unwrap(),
            Request {
                network: "Office".to_owned(),
                medium: Medium::Wired,
                question: Question::Password {
                    identity: "ana".to_owned()
                },
                again: false,
            }
        );
        running.dialogs.answer(Some(Answer::Login {
            identity: "ana.perez".to_owned(),
            password: "secret".to_owned(),
        }));
        let secrets = secrets(answer(mock, &pending));
        assert_eq!(secrets["802-1x"]["identity"].str(), Some("ana.perez"));
        assert_eq!(secrets["802-1x"]["password"].str(), Some("secret"));
    });
}

#[test]
fn a_private_key_passphrase_is_asked_for_tls() {
    mock::run(|mock| {
        let running = start(mock);
        let pending = get_secrets(
            mock,
            &running,
            &wired_8021x("tls", "ana"),
            "802-1x",
            ALLOW_INTERACTION,
        );
        mock.wait_until("the passphrase's dialog", || {
            running.dialogs.last_request().is_some()
        });
        assert_eq!(
            running.dialogs.last_request().unwrap().question,
            Question::PrivateKeyPassphrase
        );
        running
            .dialogs
            .answer(Some(Answer::Secret("passphrase".to_owned())));
        assert_eq!(
            secrets(answer(mock, &pending))["802-1x"]["private-key-password"].str(),
            Some("passphrase")
        );
    });
}

#[test]
fn a_password_too_short_for_wpa_is_canceled() {
    mock::run(|mock| {
        let running = start(mock);
        let pending = get_secrets(
            mock,
            &running,
            &wifi("Home", "wpa-psk", &[]),
            WIFI_SECURITY,
            ALLOW_INTERACTION,
        );
        mock.wait_until("the password's dialog", || {
            running.dialogs.last_request().is_some()
        });
        running
            .dialogs
            .answer(Some(Answer::Secret("short".to_owned())));
        assert_eq!(
            error(answer(mock, &pending)),
            "org.freedesktop.NetworkManager.SecretAgent.UserCanceled"
        );
    });
}

#[test]
fn saving_and_deleting_secrets_are_answered() {
    mock::run(|mock| {
        let running = start(mock);
        let connection = wifi("Home", "wpa-psk", &[]);
        let path = ObjectPath::try_from(CONNECTION.to_owned()).unwrap();
        for method in ["SaveSecrets", "DeleteSecrets"] {
            let pending = call(mock, &running, method, (&connection, &path).to_variant());
            answer(mock, &pending).unwrap();
        }
    });
}

#[test]
fn the_open_dialogs_close_when_network_manager_stops() {
    mock::run(|mock| {
        let running = start(mock);
        let _pending = get_secrets(
            mock,
            &running,
            &wifi("Home", "wpa-psk", &[]),
            WIFI_SECURITY,
            ALLOW_INTERACTION,
        );
        mock.wait_until("the password's dialog", || {
            running.dialogs.last_request().is_some()
        });
        let id = running.dialogs.last_id();
        mock.stop_network_manager();
        mock.wait_until("the dialog to close", || {
            *running.dialogs.closed.borrow() == [id]
        });
    });
}

#[test]
fn an_ssid_is_named_without_its_nul_bytes() {
    mock::run(|mock| {
        let running = start(mock);
        let mut connection = wifi("Home", "wpa-psk", &[]);
        // An SSID is any 32 bytes, and GVariant's bytestrings end in a NUL.
        connection
            .get_mut("802-11-wireless")
            .unwrap()
            .insert("ssid".to_owned(), b"Ho\0me\0".to_vec().to_variant());
        let _pending = get_secrets(
            mock,
            &running,
            &connection,
            WIFI_SECURITY,
            ALLOW_INTERACTION,
        );
        mock.wait_until("the password's dialog", || {
            running.dialogs.last_request().is_some()
        });
        assert_eq!(running.dialogs.last_request().unwrap().network, "Home");
    });
}

#[test]
fn a_wpa3_password_has_no_wpa2_length() {
    mock::run(|mock| {
        let running = start(mock);
        let pending = get_secrets(
            mock,
            &running,
            &wifi("Home", "sae", &[]),
            WIFI_SECURITY,
            ALLOW_INTERACTION,
        );
        mock.wait_until("the password's dialog", || {
            running.dialogs.last_request().is_some()
        });
        assert_eq!(
            running.dialogs.last_request().unwrap().question,
            Question::Sae
        );
        running
            .dialogs
            .answer(Some(Answer::Secret("short".to_owned())));
        assert_eq!(
            secrets(answer(mock, &pending))[WIFI_SECURITY]["psk"].str(),
            Some("short")
        );
    });
}

#[test]
fn a_phase_2_private_key_passphrase_goes_in_its_own_key() {
    mock::run(|mock| {
        let running = start(mock);
        let mut connection = wired_8021x("peap", "ana");
        connection
            .get_mut("802-1x")
            .unwrap()
            .insert("phase2-autheap".to_owned(), "tls".to_variant());
        let pending = get_secrets(mock, &running, &connection, "802-1x", ALLOW_INTERACTION);
        mock.wait_until("the passphrase's dialog", || {
            running.dialogs.last_request().is_some()
        });
        assert_eq!(
            running.dialogs.last_request().unwrap().question,
            Question::PrivateKeyPassphrase
        );
        running
            .dialogs
            .answer(Some(Answer::Secret("passphrase".to_owned())));
        assert_eq!(
            secrets(answer(mock, &pending))["802-1x"]["phase2-private-key-password"].str(),
            Some("passphrase")
        );
    });
}

const UUID: &str = "6d1c3a4e-0b9f-4d0a-9a1e-2f7c5b8e3d21";

/// `connection` with a UUID, and its secret `key` agent-owned: saved "for this user only".
fn agent_owned(mut connection: Settings, setting: &str, key: &str) -> Settings {
    connection
        .get_mut("connection")
        .unwrap()
        .insert("uuid".to_owned(), UUID.to_variant());
    connection
        .entry(setting.to_owned())
        .or_default()
        .insert(format!("{key}-flags"), 1u32.to_variant());
    connection
}

/// The item nm-applet writes for `key` of `setting`, through libsecret with its schema.
fn as_nm_applet_saves(setting: &str, key: &str, secret: &str) -> Item {
    Item {
        label: format!("Network secret for Home 1/{setting}/{key}"),
        attributes: HashMap::from([
            ("connection-uuid".to_owned(), UUID.to_owned()),
            ("setting-name".to_owned(), setting.to_owned()),
            ("setting-key".to_owned(), key.to_owned()),
            (
                "xdg:schema".to_owned(),
                "org.freedesktop.NetworkManager.Connection".to_owned(),
            ),
        ]),
        secret: secret.to_owned(),
    }
}

#[test]
fn a_secret_nm_applet_saved_is_answered_with_no_dialog() {
    mock::run(|mock| {
        let keyring = SecretService::start(mock);
        keyring.add(as_nm_applet_saves(WIFI_SECURITY, "psk", "correct horse"));
        let running = start(mock);
        let connection = agent_owned(wifi("Home", "wpa-psk", &[]), WIFI_SECURITY, "psk");

        let pending = get_secrets(
            mock,
            &running,
            &connection,
            WIFI_SECURITY,
            ALLOW_INTERACTION,
        );
        let secrets = secrets(answer(mock, &pending));
        assert_eq!(secrets[WIFI_SECURITY]["psk"].str(), Some("correct horse"));
        assert!(running.dialogs.asked.borrow().is_empty());
    });
}

#[test]
fn request_new_asks_though_the_keyring_has_the_secret() {
    mock::run(|mock| {
        let keyring = SecretService::start(mock);
        keyring.add(as_nm_applet_saves(WIFI_SECURITY, "psk", "old password"));
        let running = start(mock);
        let connection = agent_owned(wifi("Home", "wpa-psk", &[]), WIFI_SECURITY, "psk");

        let pending = get_secrets(
            mock,
            &running,
            &connection,
            WIFI_SECURITY,
            ALLOW_INTERACTION | REQUEST_NEW,
        );
        mock.wait_until("the password's dialog", || {
            running.dialogs.last_request().is_some()
        });
        assert!(running.dialogs.last_request().unwrap().again);
        running
            .dialogs
            .answer(Some(Answer::Secret("new password".to_owned())));
        assert_eq!(
            secrets(answer(mock, &pending))[WIFI_SECURITY]["psk"].str(),
            Some("new password")
        );
        // The new password takes the old one's place, as nm-applet would find it.
        mock.wait_until("the new password in the keyring", || {
            keyring.items() == [as_nm_applet_saves(WIFI_SECURITY, "psk", "new password")]
        });
    });
}

#[test]
fn an_agent_owned_answer_is_saved_as_nm_applet_saves_it() {
    mock::run(|mock| {
        let keyring = SecretService::start(mock);
        let running = start(mock);
        let connection = agent_owned(wifi("Home", "wpa-psk", &[]), WIFI_SECURITY, "psk");

        // Nothing is saved yet: the agent asks.
        let pending = get_secrets(
            mock,
            &running,
            &connection,
            WIFI_SECURITY,
            ALLOW_INTERACTION,
        );
        mock.wait_until("the password's dialog", || {
            running.dialogs.last_request().is_some()
        });
        running
            .dialogs
            .answer(Some(Answer::Secret("correct horse".to_owned())));
        answer(mock, &pending).unwrap();
        mock.wait_until("the password in the keyring", || {
            keyring.items() == [as_nm_applet_saves(WIFI_SECURITY, "psk", "correct horse")]
        });
    });
}

#[test]
fn a_secret_network_manager_keeps_is_not_saved() {
    mock::run(|mock| {
        let keyring = SecretService::start(mock);
        let running = start(mock);
        let mut connection = wifi("Home", "wpa-psk", &[]);
        connection
            .get_mut("connection")
            .unwrap()
            .insert("uuid".to_owned(), UUID.to_variant());

        let pending = get_secrets(
            mock,
            &running,
            &connection,
            WIFI_SECURITY,
            ALLOW_INTERACTION,
        );
        mock.wait_until("the password's dialog", || {
            running.dialogs.last_request().is_some()
        });
        running
            .dialogs
            .answer(Some(Answer::Secret("correct horse".to_owned())));
        answer(mock, &pending).unwrap();
        // Nothing is to happen: a save would take a few calls on the private bus, far less
        // than this.
        let until = Instant::now() + Duration::from_millis(300);
        mock.wait_until("a while", || Instant::now() >= until);
        assert!(keyring.items().is_empty());
    });
}

#[test]
fn save_secrets_keeps_the_agent_owned_ones_in_place_of_the_old() {
    mock::run(|mock| {
        let keyring = SecretService::start(mock);
        keyring.add(as_nm_applet_saves(WIFI_SECURITY, "psk", "old password"));
        keyring.add(as_nm_applet_saves("802-1x", "password", "gone"));
        let running = start(mock);
        let mut connection = agent_owned(
            wifi("Home", "wpa-psk", &[("psk", "new password".to_variant())]),
            WIFI_SECURITY,
            "psk",
        );
        // A secret NetworkManager keeps, and one asked each time, aren't the agent's.
        let security = connection.get_mut(WIFI_SECURITY).unwrap();
        security.insert("wep-key0".to_owned(), "system".to_variant());
        security.insert("wep-key-flags".to_owned(), 0u32.to_variant());
        security.insert("leap-password".to_owned(), "asked".to_variant());
        security.insert("leap-password-flags".to_owned(), 2u32.to_variant());

        let path = ObjectPath::try_from(CONNECTION.to_owned()).unwrap();
        let pending = call(
            mock,
            &running,
            "SaveSecrets",
            (&connection, &path).to_variant(),
        );
        answer(mock, &pending).unwrap();
        assert_eq!(
            keyring.items(),
            [as_nm_applet_saves(WIFI_SECURITY, "psk", "new password")]
        );
    });
}

#[test]
fn delete_secrets_deletes_the_connections_secrets() {
    mock::run(|mock| {
        let keyring = SecretService::start(mock);
        keyring.add(as_nm_applet_saves(WIFI_SECURITY, "psk", "password"));
        keyring.add(as_nm_applet_saves("802-1x", "password", "password"));
        let mut other = as_nm_applet_saves(WIFI_SECURITY, "psk", "another's");
        other
            .attributes
            .insert("connection-uuid".to_owned(), "another".to_owned());
        keyring.add(other.clone());
        let running = start(mock);
        let connection = agent_owned(wifi("Home", "wpa-psk", &[]), WIFI_SECURITY, "psk");

        let path = ObjectPath::try_from(CONNECTION.to_owned()).unwrap();
        let pending = call(
            mock,
            &running,
            "DeleteSecrets",
            (&connection, &path).to_variant(),
        );
        answer(mock, &pending).unwrap();
        assert_eq!(keyring.items(), [other]);
    });
}

#[test]
fn a_locked_keyring_is_unlocked_through_its_prompt() {
    mock::run(|mock| {
        let keyring = SecretService::start(mock);
        keyring.add(as_nm_applet_saves(WIFI_SECURITY, "psk", "correct horse"));
        keyring.lock(false);
        let running = start(mock);
        let connection = agent_owned(wifi("Home", "wpa-psk", &[]), WIFI_SECURITY, "psk");

        let pending = get_secrets(
            mock,
            &running,
            &connection,
            WIFI_SECURITY,
            ALLOW_INTERACTION,
        );
        assert_eq!(
            secrets(answer(mock, &pending))[WIFI_SECURITY]["psk"].str(),
            Some("correct horse")
        );
        assert_eq!(keyring.prompts(), 1);
        assert!(running.dialogs.asked.borrow().is_empty());
    });
}

#[test]
fn a_locked_keyring_the_user_refuses_to_unlock_falls_back_to_asking() {
    mock::run(|mock| {
        let keyring = SecretService::start(mock);
        keyring.add(as_nm_applet_saves(WIFI_SECURITY, "psk", "correct horse"));
        keyring.lock(true);
        let running = start(mock);
        let connection = agent_owned(wifi("Home", "wpa-psk", &[]), WIFI_SECURITY, "psk");

        let pending = get_secrets(
            mock,
            &running,
            &connection,
            WIFI_SECURITY,
            ALLOW_INTERACTION,
        );
        mock.wait_until("the password's dialog", || {
            running.dialogs.last_request().is_some()
        });
        assert_eq!(keyring.prompts(), 1);
        running
            .dialogs
            .answer(Some(Answer::Secret("typed again".to_owned())));
        assert_eq!(
            secrets(answer(mock, &pending))[WIFI_SECURITY]["psk"].str(),
            Some("typed again")
        );
        assert!(keyring.locked());
    });
}

#[test]
fn a_request_that_may_not_ask_has_no_secrets_the_keyring_lacks() {
    mock::run(|mock| {
        let _keyring = SecretService::start(mock);
        let running = start(mock);
        let connection = agent_owned(wifi("Home", "wpa-psk", &[]), WIFI_SECURITY, "psk");
        let pending = get_secrets(mock, &running, &connection, WIFI_SECURITY, 0);
        assert_eq!(
            error(answer(mock, &pending)),
            "org.freedesktop.NetworkManager.SecretAgent.NoSecrets"
        );
        assert!(running.dialogs.asked.borrow().is_empty());
    });
}

#[test]
fn a_new_password_replaces_one_saved_without_the_schema() {
    mock::run(|mock| {
        let keyring = SecretService::start(mock);
        let mut old = as_nm_applet_saves(WIFI_SECURITY, "psk", "old password");
        old.attributes.remove("xdg:schema");
        keyring.add(old);
        let running = start(mock);
        let connection = agent_owned(wifi("Home", "wpa-psk", &[]), WIFI_SECURITY, "psk");

        // Found by nm-applet's search, which leaves the schema out.
        let pending = get_secrets(
            mock,
            &running,
            &connection,
            WIFI_SECURITY,
            ALLOW_INTERACTION,
        );
        assert_eq!(
            secrets(answer(mock, &pending))[WIFI_SECURITY]["psk"].str(),
            Some("old password")
        );

        let _pending = get_secrets(
            mock,
            &running,
            &connection,
            WIFI_SECURITY,
            ALLOW_INTERACTION | REQUEST_NEW,
        );
        mock.wait_until("the password's dialog", || {
            running.dialogs.last_request().is_some()
        });
        running
            .dialogs
            .answer(Some(Answer::Secret("new password".to_owned())));
        mock.wait_until("the new password alone in the keyring", || {
            keyring.items() == [as_nm_applet_saves(WIFI_SECURITY, "psk", "new password")]
        });
    });
}

#[test]
fn a_request_that_may_not_ask_shows_no_unlock_prompt() {
    mock::run(|mock| {
        let keyring = SecretService::start(mock);
        keyring.add(as_nm_applet_saves(WIFI_SECURITY, "psk", "correct horse"));
        keyring.lock(false);
        let running = start(mock);
        let connection = agent_owned(wifi("Home", "wpa-psk", &[]), WIFI_SECURITY, "psk");

        let pending = get_secrets(mock, &running, &connection, WIFI_SECURITY, 0);
        assert_eq!(
            error(answer(mock, &pending)),
            "org.freedesktop.NetworkManager.SecretAgent.NoSecrets"
        );
        assert_eq!(keyring.prompts(), 0);
    });
}

#[test]
fn delete_secrets_fails_when_the_keyring_stays_locked() {
    mock::run(|mock| {
        let keyring = SecretService::start(mock);
        keyring.add(as_nm_applet_saves(WIFI_SECURITY, "psk", "password"));
        keyring.lock(true);
        let running = start(mock);
        let connection = agent_owned(wifi("Home", "wpa-psk", &[]), WIFI_SECURITY, "psk");

        let path = ObjectPath::try_from(CONNECTION.to_owned()).unwrap();
        let pending = call(
            mock,
            &running,
            "DeleteSecrets",
            (&connection, &path).to_variant(),
        );
        assert_eq!(
            error(answer(mock, &pending)),
            "org.freedesktop.NetworkManager.SecretAgent.Failed"
        );
        assert_eq!(keyring.items().len(), 1);
    });
}
