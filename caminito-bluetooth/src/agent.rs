//! The pairing agent (256): an `org.bluez.Agent1` that BlueZ asks for every PIN, passkey and
//! consent in the session, including for a pairing a device starts. It's registered as the
//! default agent each time BlueZ comes on the bus, and every device paired through it is
//! marked trusted, since pairing is the one consent the settings window asks for (the spec's
//! "The settings window": no trust toggle).
//!
//! The dialogs are behind [`Dialogs`], which the binary draws in GTK and the tests replace.
//! One dialog is open at a time: a request that comes while one is open cancels it, as
//! BlueZ's `Cancel` closes it.

use std::cell::RefCell;
use std::collections::BTreeSet;
use std::fmt;
use std::rc::{Rc, Weak};

use gio::glib::variant::{ObjectPath, ToVariant};
use gio::glib::{self, Variant};

use crate::DOMAIN;
use crate::bluez::{self, Bluez, Device};

/// Where the agent is exported, on the connection BlueZ is followed on.
pub const PATH: &str = "/org/mate/Caminito/Bluetooth/Agent";

const BLUEZ: &str = "org.bluez";
const AGENT: &str = "org.bluez.Agent1";
const AGENT_MANAGER: &str = "org.bluez.AgentManager1";
/// It can both show a code and take one typed, so BlueZ picks the safest pairing that the
/// device allows.
const CAPABILITY: &str = "KeyboardDisplay";
const REJECTED: &str = "org.bluez.Error.Rejected";
const CANCELED: &str = "org.bluez.Error.Canceled";

/// `org.bluez.Agent1`, as BlueZ's doc/org.bluez.Agent.rst gives it.
const INTERFACE: &str = r#"<node>
  <interface name="org.bluez.Agent1">
    <method name="Release"/>
    <method name="RequestPinCode">
      <arg name="device" type="o" direction="in"/>
      <arg name="pincode" type="s" direction="out"/>
    </method>
    <method name="DisplayPinCode">
      <arg name="device" type="o" direction="in"/>
      <arg name="pincode" type="s" direction="in"/>
    </method>
    <method name="RequestPasskey">
      <arg name="device" type="o" direction="in"/>
      <arg name="passkey" type="u" direction="out"/>
    </method>
    <method name="DisplayPasskey">
      <arg name="device" type="o" direction="in"/>
      <arg name="passkey" type="u" direction="in"/>
      <arg name="entered" type="q" direction="in"/>
    </method>
    <method name="RequestConfirmation">
      <arg name="device" type="o" direction="in"/>
      <arg name="passkey" type="u" direction="in"/>
    </method>
    <method name="RequestAuthorization">
      <arg name="device" type="o" direction="in"/>
    </method>
    <method name="AuthorizeService">
      <arg name="device" type="o" direction="in"/>
      <arg name="uuid" type="s" direction="in"/>
    </method>
    <method name="Cancel"/>
  </interface>
</node>"#;

/// What BlueZ asks the user about a device.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum Question {
    /// `RequestPinCode`: the PIN to pair with, 1 to 16 characters, which an older device
    /// either prints or lets the user choose.
    PinCode,
    /// `RequestPasskey`: the passkey the device shows, 0 to 999999.
    Passkey,
    /// `RequestConfirmation`: whether the device shows `passkey` too.
    Confirmation {
        /// The six digits to compare, as a number.
        passkey: u32,
    },
    /// `RequestAuthorization`: whether the device, which started the pairing, may pair.
    Authorization,
    /// `AuthorizeService`: whether the device, which isn't trusted, may use a service.
    Service {
        /// The service's UUID, as `0000110d-0000-1000-8000-00805f9b34fb`.
        uuid: String,
    },
}

/// A code BlueZ shows the user to type on a device.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum Code {
    /// `DisplayPinCode`: a PIN for the device's keyboard.
    PinCode(String),
    /// `DisplayPasskey`: a passkey for the device's keyboard, and how many of its digits
    /// have been typed so far. BlueZ shows it again as each one is.
    Passkey {
        /// The six digits to type, as a number.
        passkey: u32,
        /// How many have been typed.
        entered: u16,
    },
}

/// The user's answer to a [`Question`].
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum Answer {
    /// The PIN typed, for [`Question::PinCode`].
    PinCode(String),
    /// The passkey typed, for [`Question::Passkey`].
    Passkey(u32),
    /// Yes, for a confirmation, an authorization or a service.
    Yes,
    /// No, to any question. BlueZ hears `org.bluez.Error.Rejected`.
    No,
}

/// The agent's dialogs: GTK's in the binary, a fake in the tests.
pub trait Dialogs {
    /// Asks `question` about `device`. The answer goes to `reply`, once, unless
    /// [`Dialogs::close`] comes first.
    fn ask(&self, device: &Device, question: Question, reply: Reply);
    /// Shows `code` to type on `device`, in place of the open dialog when that one shows
    /// this device's code already.
    fn show(&self, device: &Device, code: Code);
    /// Closes the open dialog: BlueZ cancelled it, a new request replaced it, or the pairing
    /// it was for ended. A reply sent after it is ignored.
    fn close(&self);
}

/// Where the answer to one question goes.
pub struct Reply {
    state: Weak<RefCell<State>>,
    request: u64,
}

impl Reply {
    /// Answers BlueZ, unless the question was closed since. An answer of the wrong kind
    /// for the question, or a PIN or passkey out of range, is a no.
    pub fn send(self, answer: Answer) {
        let Some(state) = self.state.upgrade() else {
            return;
        };
        let open = {
            let mut state = state.borrow_mut();
            match &state.open {
                Some(open) if open.request == self.request => state.open.take(),
                _ => None,
            }
        };
        let Some(Open {
            device,
            question: Some(question),
            invocation: Some(invocation),
            ..
        }) = open
        else {
            return;
        };
        let value = match (&question, answer) {
            (Question::PinCode, Answer::PinCode(pin)) if (1..=16).contains(&pin.len()) => {
                Some(Some((pin,).to_variant()))
            }
            (Question::Passkey, Answer::Passkey(passkey)) if passkey <= 999_999 => {
                Some(Some((passkey,).to_variant()))
            }
            (
                Question::Confirmation { .. } | Question::Authorization | Question::Service { .. },
                Answer::Yes,
            ) => Some(None),
            _ => None,
        };
        match value {
            Some(value) => {
                if !matches!(question, Question::Service { .. }) {
                    state.borrow_mut().pairing.insert(device);
                }
                invocation.return_value(value.as_ref());
            }
            None => invocation.return_dbus_error(REJECTED, "the user declined"),
        }
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
            Error::Export(err) => write!(f, "can't export the pairing agent: {err}"),
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

/// The dialog open, and the request it's for.
struct Open {
    request: u64,
    device: String,
    /// None for a code shown, which BlueZ had its answer to at once.
    question: Option<Question>,
    invocation: Option<gio::DBusMethodInvocation>,
}

struct State {
    bluez: Bluez,
    dialogs: Rc<dyn Dialogs>,
    /// The last request's number, which tells a late [`Reply`] from the current one.
    requests: u64,
    open: Option<Open>,
    /// The devices a pairing went through the agent for, to trust once BlueZ has paired
    /// them.
    pairing: BTreeSet<String>,
}

/// The pairing agent, exported and registered with BlueZ for as long as it lives.
pub struct Agent {
    connection: gio::DBusConnection,
    registration: Option<gio::RegistrationId>,
    /// Stops watching BlueZ's name; a closure for the reason [`Bluez`]'s is.
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
    /// Exports the agent on `bluez`'s connection, and registers it as BlueZ's default agent
    /// now and whenever BlueZ comes back on the bus.
    pub fn new(bluez: &Bluez, dialogs: Rc<dyn Dialogs>) -> Result<Agent, Error> {
        let state = Rc::new(RefCell::new(State {
            bluez: bluez.clone(),
            dialogs,
            requests: 0,
            open: None,
            pairing: BTreeSet::new(),
        }));
        let connection = bluez.connection();

        let node = gio::DBusNodeInfo::for_xml(INTERFACE).expect("the agent's interface is valid");
        let interface = node
            .lookup_interface(AGENT)
            .expect("the node has the agent's interface");
        let registration = connection
            .register_object(PATH, &interface)
            .method_call({
                let state = Rc::downgrade(&state);
                move |_, _, _, _, method, parameters, invocation| match state.upgrade() {
                    Some(state) => handle(&state, method, &parameters, invocation),
                    None => invocation.return_dbus_error(REJECTED, "the agent has stopped"),
                }
            })
            .build()
            .map_err(Error::Export)?;

        bluez.connect_changed({
            let state = Rc::downgrade(&state);
            move |bluez| {
                if let Some(state) = state.upgrade() {
                    trust_the_paired(&state, bluez);
                }
            }
        });

        let appeared = bluez.clone();
        let vanished = Rc::downgrade(&state);
        let watcher = gio::bus_watch_name_on_connection(
            &connection,
            BLUEZ,
            gio::BusNameWatcherFlags::NONE,
            move |_, _, _| {
                let bluez = appeared.clone();
                glib::spawn_future_local(async move {
                    if let Err(err) = register(&bluez).await {
                        glib::g_warning!(DOMAIN, "{err}");
                    }
                });
            },
            move |_, _| {
                if let Some(state) = vanished.upgrade() {
                    state.borrow_mut().pairing.clear();
                    dismiss(&state, CANCELED);
                }
            },
        );

        Ok(Agent {
            connection,
            registration: Some(registration),
            unwatch: Some(Box::new(move || gio::bus_unwatch_name(watcher))),
            _state: state,
        })
    }
}

/// Registers the agent with a BlueZ that has just come on the bus, and makes it the
/// default, so it answers the pairings a device starts too.
async fn register(bluez: &Bluez) -> Result<(), bluez::Error> {
    let path = ObjectPath::try_from(PATH.to_owned()).expect("the agent's path is valid");
    bluez
        .call(
            "/org/bluez",
            AGENT_MANAGER,
            "RegisterAgent",
            Some(&(path.clone(), CAPABILITY).to_variant()),
            "register the pairing agent with",
        )
        .await?;
    bluez
        .call(
            "/org/bluez",
            AGENT_MANAGER,
            "RequestDefaultAgent",
            Some(&(path,).to_variant()),
            "make the pairing agent the default of",
        )
        .await
}

/// Answers one of BlueZ's calls. gio has checked its arguments against [`INTERFACE`].
fn handle(
    state: &Rc<RefCell<State>>,
    method: &str,
    parameters: &Variant,
    invocation: gio::DBusMethodInvocation,
) {
    let device = || {
        parameters
            .child_value(0)
            .str()
            .expect("the first argument is the device's path")
            .to_owned()
    };
    let u32_at = |index| {
        parameters
            .child_value(index)
            .get::<u32>()
            .expect("the passkey is a u32")
    };
    let string_at = |index| {
        parameters
            .child_value(index)
            .str()
            .expect("the argument is a string")
            .to_owned()
    };
    match method {
        "RequestPinCode" => ask(state, device(), Question::PinCode, invocation),
        "RequestPasskey" => ask(state, device(), Question::Passkey, invocation),
        "RequestConfirmation" => ask(
            state,
            device(),
            Question::Confirmation { passkey: u32_at(1) },
            invocation,
        ),
        "RequestAuthorization" => ask(state, device(), Question::Authorization, invocation),
        "AuthorizeService" => {
            let path = device();
            let bluez = state.borrow().bluez.clone();
            if find(&bluez, &path).is_some_and(|device| device.paired && device.trusted) {
                invocation.return_value(None);
            } else {
                let uuid = string_at(1);
                ask(state, path, Question::Service { uuid }, invocation);
            }
        }
        "DisplayPinCode" => {
            invocation.return_value(None);
            show(state, device(), Code::PinCode(string_at(1)));
        }
        "DisplayPasskey" => {
            let entered = parameters
                .child_value(2)
                .get::<u16>()
                .expect("the count is a u16");
            invocation.return_value(None);
            let passkey = u32_at(1);
            show(state, device(), Code::Passkey { passkey, entered });
        }
        "Cancel" => {
            // The pairing failed or was given up, so there's nothing to trust.
            let device = state.borrow().open.as_ref().map(|open| open.device.clone());
            if let Some(device) = device {
                state.borrow_mut().pairing.remove(&device);
            }
            dismiss(state, CANCELED);
            invocation.return_value(None);
        }
        "Release" => {
            dismiss(state, CANCELED);
            invocation.return_value(None);
        }
        _ => invocation.return_dbus_error(
            "org.freedesktop.DBus.Error.UnknownMethod",
            &format!("the agent has no method {method}"),
        ),
    }
}

/// Opens a dialog with `question` for the call `invocation`, in place of any open one.
fn ask(
    state: &Rc<RefCell<State>>,
    path: String,
    question: Question,
    invocation: gio::DBusMethodInvocation,
) {
    let request = open(
        state,
        Open {
            request: 0,
            device: path.clone(),
            question: Some(question.clone()),
            invocation: Some(invocation),
        },
    );
    let (bluez, dialogs) = parts(state);
    let reply = Reply {
        state: Rc::downgrade(state),
        request,
    };
    dialogs.ask(&describe(&bluez, &path), question, reply);
}

/// Shows `code` for the device at `path`, in the dialog that shows its code already or in
/// place of any open one.
fn show(state: &Rc<RefCell<State>>, path: String, code: Code) {
    let shown = matches!(
        &state.borrow().open,
        Some(open) if open.question.is_none() && open.device == path
    );
    if !shown {
        open(
            state,
            Open {
                request: 0,
                device: path.clone(),
                question: None,
                invocation: None,
            },
        );
    }
    state.borrow_mut().pairing.insert(path.clone());
    let (bluez, dialogs) = parts(state);
    dialogs.show(&describe(&bluez, &path), code);
}

/// Makes `open` the open dialog, with a new request number, which it returns. One that was
/// open is cancelled and closed.
fn open(state: &Rc<RefCell<State>>, mut open: Open) -> u64 {
    let (request, previous) = {
        let mut state = state.borrow_mut();
        state.requests += 1;
        open.request = state.requests;
        (state.requests, state.open.replace(open))
    };
    if let Some(previous) = previous {
        finish(previous, CANCELED);
        parts(state).1.close();
    }
    request
}

/// Closes the open dialog, if there's one, and answers its call with `error`.
fn dismiss(state: &Rc<RefCell<State>>, error: &str) {
    let open = state.borrow_mut().open.take();
    if let Some(open) = open {
        finish(open, error);
        parts(state).1.close();
    }
}

fn finish(open: Open, error: &str) {
    if let Some(invocation) = open.invocation {
        invocation.return_dbus_error(error, "the request was closed");
    }
}

/// Trusts each device the agent paired that BlueZ has now paired, and closes the code shown
/// for it.
fn trust_the_paired(state: &Rc<RefCell<State>>, bluez: &Bluez) {
    let devices = bluez.devices();
    let (paired, close) = {
        let mut state = state.borrow_mut();
        let paired: Vec<Device> = devices
            .iter()
            .filter(|device| device.paired && state.pairing.contains(&device.path))
            .cloned()
            .collect();
        state.pairing.retain(|path| {
            devices
                .iter()
                .any(|device| &device.path == path && !device.paired)
        });
        let close = matches!(
            &state.open,
            Some(open) if open.question.is_none()
                && !state.pairing.contains(&open.device)
        );
        (paired, close)
    };
    if close {
        dismiss(state, CANCELED);
    }
    for device in paired.into_iter().filter(|device| !device.trusted) {
        let bluez = bluez.clone();
        glib::spawn_future_local(async move {
            if let Err(err) = bluez.set_trusted(&device.path, true).await {
                glib::g_warning!(DOMAIN, "{err}");
            }
        });
    }
}

/// The model and the dialogs, cloned so that nothing is borrowed while the dialogs run:
/// a fake answers at once.
fn parts(state: &Rc<RefCell<State>>) -> (Bluez, Rc<dyn Dialogs>) {
    let state = state.borrow();
    (state.bluez.clone(), state.dialogs.clone())
}

fn find(bluez: &Bluez, path: &str) -> Option<Device> {
    bluez
        .devices()
        .into_iter()
        .find(|device| device.path == path)
}

/// The device at `path`, or what its path tells of one the model hasn't read yet.
fn describe(bluez: &Bluez, path: &str) -> Device {
    find(bluez, path).unwrap_or_else(|| {
        let (adapter, name) = path.rsplit_once('/').unwrap_or(("", path));
        let address = name.trim_start_matches("dev_").replace('_', ":");
        Device {
            path: path.to_owned(),
            adapter: adapter.to_owned(),
            name: None,
            alias: address.clone(),
            address,
            icon: None,
            paired: false,
            trusted: false,
            connected: false,
            battery: None,
        }
    })
}
