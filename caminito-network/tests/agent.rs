//! The secret agent against python-dbusmock's `networkmanager` template on a private bus, with
//! an AgentManager added to it. The mock never calls an agent, so the test calls it as
//! NetworkManager would, and the dialogs are a fake that the test answers.

mod mock;

use std::cell::RefCell;
use std::collections::HashMap;
use std::rc::Rc;

use caminito_network::agent::{self, Agent, Answer, Dialogs, Medium, Question, Reply, Request};
use gio::glib::variant::{ObjectPath, ToVariant};
use gio::glib::{self, Variant};

use mock::Mock;

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
    let agent = Agent::new(&connection, dialogs.clone()).unwrap();
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
            &wifi("Home", "sae", &[]),
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
