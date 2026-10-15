//! The pairing agent (256) against python-dbusmock's `bluez5` template on a private bus. The
//! template registers agents but never calls one, so the test calls the agent as BlueZ
//! would, and the dialogs are a fake that the test answers.

mod mock;

use std::cell::{Cell, RefCell};
use std::rc::Rc;

use caminito_bluetooth::agent::{self, Agent, Answer, Code, Dialogs, Question, Reply};
use caminito_bluetooth::bluez::{Bluez, Device};
use gio::glib::variant::{ObjectPath, ToVariant};
use gio::glib::{self, Variant};

use mock::Mock;

const KEYBOARD: &str = "11:22:33:44:55:66";
const PHONE: &str = "AA:BB:CC:DD:EE:FF";

/// The dialogs, as the agent drives them.
#[derive(Default)]
struct FakeDialogs {
    asked: RefCell<Vec<(Device, Question, Option<Reply>)>>,
    shown: RefCell<Vec<(String, Code)>>,
    closed: Cell<usize>,
}

impl Dialogs for FakeDialogs {
    fn ask(&self, device: &Device, question: Question, reply: Reply) {
        self.asked
            .borrow_mut()
            .push((device.clone(), question, Some(reply)));
    }

    fn show(&self, device: &Device, code: Code) {
        self.shown.borrow_mut().push((device.path.clone(), code));
    }

    fn close(&self) {
        self.closed.set(self.closed.get() + 1);
    }
}

impl FakeDialogs {
    /// The user's answer to the last question.
    fn answer(&self, answer: Answer) {
        let reply = self
            .asked
            .borrow_mut()
            .last_mut()
            .and_then(|(_, _, reply)| reply.take())
            .expect("a question waits for its answer");
        reply.send(answer);
    }

    fn last_question(&self) -> Option<Question> {
        self.asked
            .borrow()
            .last()
            .map(|(_, question, _)| question.clone())
    }
}

/// The agent, on a connection of its own as the program's, once BlueZ's objects are read
/// and the agent is registered.
struct Running {
    /// The agent's unique name, which BlueZ calls it at.
    name: String,
    dialogs: Rc<FakeDialogs>,
    _agent: Agent,
}

fn start(mock: &Mock) -> Running {
    let connection = mock.connect();
    let bluez = Bluez::new(&connection);
    mock.wait_until("BlueZ's objects", || bluez.is_ready());
    let dialogs = Rc::new(FakeDialogs::default());
    let agent = Agent::new(&bluez, dialogs.clone()).unwrap();
    mock.wait_until("the agent to be the default", || {
        !mock.calls("/org/bluez", "RequestDefaultAgent").is_empty()
    });
    Running {
        name: connection.unique_name().unwrap().to_string(),
        dialogs,
        _agent: agent,
    }
}

/// The answer to a call BlueZ made, once it comes.
type Pending = Rc<RefCell<Option<Result<Variant, glib::Error>>>>;

/// Calls `method` on the agent as BlueZ would, without waiting for the answer.
fn request(mock: &Mock, running: &Running, method: &str, parameters: Variant) -> Pending {
    let pending: Pending = Rc::default();
    let call = mock.connection.call_future(
        Some(&running.name),
        agent::PATH,
        "org.bluez.Agent1",
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

/// The answer to `pending`, once it comes.
fn answer(mock: &Mock, pending: &Pending) -> Result<Variant, glib::Error> {
    mock.wait_until("the agent's answer", || pending.borrow().is_some());
    pending.borrow_mut().take().unwrap()
}

fn rejection(result: Result<Variant, glib::Error>) -> String {
    let err = result.unwrap_err();
    gio::DBusError::remote_error(&err).unwrap().to_string()
}

fn path(device: &str) -> ObjectPath {
    ObjectPath::try_from(device.to_owned()).unwrap()
}

fn trusted(mock: &Mock, device: &str) -> bool {
    mock.property(device, "org.bluez.Device1", "Trusted")
        .get::<bool>()
        .unwrap()
}

#[test]
fn the_agent_registers_as_the_default_with_a_keyboard_and_a_display() {
    mock::run(|mock| {
        mock.add_adapter("hci0", "laptop");
        start(mock);

        // The mock gives each call's arguments as an array of variants.
        let argument = |call: &Variant, index| {
            let argument = call.child_value(index).as_variant().unwrap();
            argument.str().unwrap().to_owned()
        };
        let registered = mock.calls("/org/bluez", "RegisterAgent");
        assert_eq!(registered.len(), 1);
        assert_eq!(argument(&registered[0], 0), agent::PATH);
        assert_eq!(argument(&registered[0], 1), "KeyboardDisplay");
        let default = mock.calls("/org/bluez", "RequestDefaultAgent");
        assert_eq!(argument(&default[0], 0), agent::PATH);
    });
}

#[test]
fn a_pin_code_and_a_passkey_are_asked_and_answered() {
    mock::run(|mock| {
        mock.add_adapter("hci0", "laptop");
        let keyboard = mock.add_device("hci0", KEYBOARD, "Keyboard");
        let running = start(mock);

        let pending = request(
            mock,
            &running,
            "RequestPinCode",
            (path(&keyboard),).to_variant(),
        );
        mock.wait_until("the PIN's dialog", || {
            running.dialogs.last_question() == Some(Question::PinCode)
        });
        assert_eq!(running.dialogs.asked.borrow()[0].0.alias, "Keyboard");
        running.dialogs.answer(Answer::PinCode("0000".to_owned()));
        assert_eq!(
            answer(mock, &pending).unwrap().child_value(0).str(),
            Some("0000")
        );

        let pending = request(
            mock,
            &running,
            "RequestPasskey",
            (path(&keyboard),).to_variant(),
        );
        mock.wait_until("the passkey's dialog", || {
            running.dialogs.last_question() == Some(Question::Passkey)
        });
        running.dialogs.answer(Answer::Passkey(123_456));
        assert_eq!(
            answer(mock, &pending).unwrap().child_value(0).get::<u32>(),
            Some(123_456)
        );
    });
}

#[test]
fn a_declined_or_impossible_answer_is_rejected() {
    mock::run(|mock| {
        mock.add_adapter("hci0", "laptop");
        let phone = mock.add_device("hci0", PHONE, "Phone");
        let running = start(mock);

        let pending = request(
            mock,
            &running,
            "RequestConfirmation",
            (path(&phone), 4321u32).to_variant(),
        );
        mock.wait_until("the confirmation's dialog", || {
            running.dialogs.last_question() == Some(Question::Confirmation { passkey: 4321 })
        });
        running.dialogs.answer(Answer::No);
        assert_eq!(
            rejection(answer(mock, &pending)),
            "org.bluez.Error.Rejected"
        );

        // A passkey has six digits at most, and a PIN 16 characters.
        let pending = request(
            mock,
            &running,
            "RequestPasskey",
            (path(&phone),).to_variant(),
        );
        mock.wait_until("the passkey's dialog", || {
            running.dialogs.asked.borrow().len() == 2
        });
        running.dialogs.answer(Answer::Passkey(1_000_000));
        assert_eq!(
            rejection(answer(mock, &pending)),
            "org.bluez.Error.Rejected"
        );

        let pending = request(
            mock,
            &running,
            "RequestPinCode",
            (path(&phone),).to_variant(),
        );
        mock.wait_until("the PIN's dialog", || {
            running.dialogs.asked.borrow().len() == 3
        });
        running.dialogs.answer(Answer::PinCode(String::new()));
        assert_eq!(
            rejection(answer(mock, &pending)),
            "org.bluez.Error.Rejected"
        );
    });
}

#[test]
fn a_confirmed_pairing_ends_with_the_device_trusted() {
    mock::run(|mock| {
        mock.add_adapter("hci0", "laptop");
        let phone = mock.add_device("hci0", PHONE, "Phone");
        let running = start(mock);

        let pending = request(
            mock,
            &running,
            "RequestConfirmation",
            (path(&phone), 123_456u32).to_variant(),
        );
        mock.wait_until("the confirmation's dialog", || {
            running.dialogs.last_question().is_some()
        });
        running.dialogs.answer(Answer::Yes);
        answer(mock, &pending).unwrap();
        assert!(!trusted(mock, &phone));

        mock.pair("hci0", PHONE);
        mock.wait_until("the phone to be trusted", || trusted(mock, &phone));
    });
}

#[test]
fn a_pairing_the_device_started_is_authorized() {
    mock::run(|mock| {
        mock.add_adapter("hci0", "laptop");
        let phone = mock.add_device("hci0", PHONE, "Phone");
        let running = start(mock);

        let pending = request(
            mock,
            &running,
            "RequestAuthorization",
            (path(&phone),).to_variant(),
        );
        mock.wait_until("the authorization's dialog", || {
            running.dialogs.last_question() == Some(Question::Authorization)
        });
        running.dialogs.answer(Answer::Yes);
        answer(mock, &pending).unwrap();

        mock.pair("hci0", PHONE);
        mock.wait_until("the phone to be trusted", || trusted(mock, &phone));
    });
}

#[test]
fn a_code_shown_for_the_device_is_updated_then_closed_once_it_has_paired() {
    mock::run(|mock| {
        mock.add_adapter("hci0", "laptop");
        let keyboard = mock.add_device("hci0", KEYBOARD, "Keyboard");
        let running = start(mock);

        // BlueZ has its answer at once, and shows the code again as each digit is typed.
        answer(
            mock,
            &request(
                mock,
                &running,
                "DisplayPasskey",
                (path(&keyboard), 42u32, 0u16).to_variant(),
            ),
        )
        .unwrap();
        answer(
            mock,
            &request(
                mock,
                &running,
                "DisplayPasskey",
                (path(&keyboard), 42u32, 3u16).to_variant(),
            ),
        )
        .unwrap();
        assert_eq!(
            *running.dialogs.shown.borrow(),
            [
                (
                    keyboard.clone(),
                    Code::Passkey {
                        passkey: 42,
                        entered: 0
                    }
                ),
                (
                    keyboard.clone(),
                    Code::Passkey {
                        passkey: 42,
                        entered: 3
                    }
                ),
            ]
        );
        assert_eq!(running.dialogs.closed.get(), 0);

        mock.pair("hci0", KEYBOARD);
        mock.wait_until("the code to close", || running.dialogs.closed.get() == 1);
        mock.wait_until("the keyboard to be trusted", || trusted(mock, &keyboard));

        answer(
            mock,
            &request(
                mock,
                &running,
                "DisplayPinCode",
                (path(&keyboard), "1234").to_variant(),
            ),
        )
        .unwrap();
        assert_eq!(
            running.dialogs.shown.borrow().last().unwrap().1,
            Code::PinCode("1234".to_owned())
        );
    });
}

#[test]
fn cancel_closes_the_dialog_and_a_later_answer_is_ignored() {
    mock::run(|mock| {
        mock.add_adapter("hci0", "laptop");
        let keyboard = mock.add_device("hci0", KEYBOARD, "Keyboard");
        let running = start(mock);

        let pending = request(
            mock,
            &running,
            "RequestPinCode",
            (path(&keyboard),).to_variant(),
        );
        mock.wait_until("the PIN's dialog", || {
            running.dialogs.last_question().is_some()
        });
        answer(mock, &request(mock, &running, "Cancel", ().to_variant())).unwrap();
        assert_eq!(running.dialogs.closed.get(), 1);
        assert_eq!(
            rejection(answer(mock, &pending)),
            "org.bluez.Error.Canceled"
        );

        // The dialog's answer, had it come before it closed, goes nowhere.
        running.dialogs.answer(Answer::PinCode("0000".to_owned()));
    });
}

#[test]
fn a_new_request_cancels_the_open_one() {
    mock::run(|mock| {
        mock.add_adapter("hci0", "laptop");
        let keyboard = mock.add_device("hci0", KEYBOARD, "Keyboard");
        let phone = mock.add_device("hci0", PHONE, "Phone");
        let running = start(mock);

        let first = request(
            mock,
            &running,
            "RequestPinCode",
            (path(&keyboard),).to_variant(),
        );
        mock.wait_until("the PIN's dialog", || {
            running.dialogs.asked.borrow().len() == 1
        });
        let second = request(
            mock,
            &running,
            "RequestAuthorization",
            (path(&phone),).to_variant(),
        );
        mock.wait_until("the authorization's dialog", || {
            running.dialogs.asked.borrow().len() == 2
        });
        assert_eq!(running.dialogs.closed.get(), 1);
        assert_eq!(rejection(answer(mock, &first)), "org.bluez.Error.Canceled");

        running.dialogs.answer(Answer::Yes);
        answer(mock, &second).unwrap();
    });
}

#[test]
fn a_service_is_allowed_silently_for_a_trusted_device_and_asked_otherwise() {
    mock::run(|mock| {
        const AUDIO_SINK: &str = "0000110b-0000-1000-8000-00805f9b34fb";
        mock.add_adapter("hci0", "laptop");
        let headset = mock.add_device("hci0", KEYBOARD, "Headset");
        let phone = mock.add_device("hci0", PHONE, "Phone");
        mock.pair("hci0", KEYBOARD);
        mock.set_property(&headset, "org.bluez.Device1", "Trusted", true);
        mock.pair("hci0", PHONE);
        let running = start(mock);

        answer(
            mock,
            &request(
                mock,
                &running,
                "AuthorizeService",
                (path(&headset), AUDIO_SINK).to_variant(),
            ),
        )
        .unwrap();
        assert!(running.dialogs.asked.borrow().is_empty());

        let pending = request(
            mock,
            &running,
            "AuthorizeService",
            (path(&phone), AUDIO_SINK).to_variant(),
        );
        mock.wait_until("the service's dialog", || {
            running.dialogs.last_question()
                == Some(Question::Service {
                    uuid: AUDIO_SINK.to_owned(),
                })
        });
        running.dialogs.answer(Answer::Yes);
        answer(mock, &pending).unwrap();
        // A service allowed once doesn't trust the device.
        assert!(!trusted(mock, &phone));
    });
}
