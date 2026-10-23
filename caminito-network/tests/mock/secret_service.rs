//! A fake Secret Service on the test's bus, as gnome-keyring is on the session bus: one
//! collection, the default, whose items the test reads and writes directly. It's locked or
//! not as a whole, and its unlock prompt is answered as the test says, at once.
//!
//! It has what the agent calls of the Secret Service API, and nothing else: OpenSession with
//! the plain algorithm, SearchItems, Unlock with its prompt, GetSecrets, ReadAlias, CreateItem
//! on the default collection with its Locked, and an item's Delete and Attributes.

use std::cell::RefCell;
use std::collections::{BTreeMap, HashMap};
use std::rc::Rc;

use gio::glib::Variant;
use gio::glib::variant::{ObjectPath, ToVariant};

use super::Mock;

const NAME: &str = "org.freedesktop.secrets";
const SERVICE_PATH: &str = "/org/freedesktop/secrets";
const COLLECTION_PATH: &str = "/org/freedesktop/secrets/collection/login";
const SESSION_PATH: &str = "/org/freedesktop/secrets/session/1";
const PROMPT_PATH: &str = "/org/freedesktop/secrets/prompt/";
const IS_LOCKED: &str = "org.freedesktop.Secret.Error.IsLocked";

const INTERFACES: &str = r#"<node>
  <interface name="org.freedesktop.Secret.Service">
    <method name="OpenSession">
      <arg name="algorithm" type="s" direction="in"/>
      <arg name="input" type="v" direction="in"/>
      <arg name="output" type="v" direction="out"/>
      <arg name="result" type="o" direction="out"/>
    </method>
    <method name="SearchItems">
      <arg name="attributes" type="a{ss}" direction="in"/>
      <arg name="unlocked" type="ao" direction="out"/>
      <arg name="locked" type="ao" direction="out"/>
    </method>
    <method name="Unlock">
      <arg name="objects" type="ao" direction="in"/>
      <arg name="unlocked" type="ao" direction="out"/>
      <arg name="prompt" type="o" direction="out"/>
    </method>
    <method name="ReadAlias">
      <arg name="name" type="s" direction="in"/>
      <arg name="collection" type="o" direction="out"/>
    </method>
    <method name="GetSecrets">
      <arg name="items" type="ao" direction="in"/>
      <arg name="session" type="o" direction="in"/>
      <arg name="secrets" type="a{o(oayays)}" direction="out"/>
    </method>
  </interface>
  <interface name="org.freedesktop.Secret.Collection">
    <method name="CreateItem">
      <arg name="properties" type="a{sv}" direction="in"/>
      <arg name="secret" type="(oayays)" direction="in"/>
      <arg name="replace" type="b" direction="in"/>
      <arg name="item" type="o" direction="out"/>
      <arg name="prompt" type="o" direction="out"/>
    </method>
    <property name="Locked" type="b" access="read"/>
  </interface>
  <interface name="org.freedesktop.Secret.Item">
    <method name="Delete">
      <arg name="prompt" type="o" direction="out"/>
    </method>
    <property name="Attributes" type="a{ss}" access="read"/>
  </interface>
  <interface name="org.freedesktop.Secret.Prompt">
    <method name="Prompt">
      <arg name="window_id" type="s" direction="in"/>
    </method>
    <signal name="Completed">
      <arg name="dismissed" type="b"/>
      <arg name="result" type="v"/>
    </signal>
  </interface>
</node>"#;

/// An item of the collection.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Item {
    pub label: String,
    pub attributes: HashMap<String, String>,
    pub secret: String,
}

#[derive(Default)]
struct Keyring {
    items: BTreeMap<u32, Item>,
    last_item: u32,
    locked: bool,
    /// The user dismisses the unlock prompt.
    refuse: bool,
    last_prompt: u32,
    /// Each prompt's objects to unlock.
    prompts: HashMap<u32, Vec<String>>,
}

/// The Secret Service, on a connection of its own, for as long as it lives.
pub struct SecretService {
    keyring: Rc<RefCell<Keyring>>,
    connection: gio::DBusConnection,
    registrations: Vec<gio::RegistrationId>,
    owner: Option<gio::OwnerId>,
}

impl Drop for SecretService {
    fn drop(&mut self) {
        for registration in self.registrations.drain(..) {
            let _ = self.connection.unregister_object(registration);
        }
        if let Some(owner) = self.owner.take() {
            gio::bus_unown_name(owner);
        }
    }
}

fn item_path(id: u32) -> String {
    format!("{COLLECTION_PATH}/{id}")
}

fn item_id(path: &str) -> u32 {
    path.rsplit('/').next().unwrap().parse().unwrap()
}

fn path(path: &str) -> ObjectPath {
    ObjectPath::try_from(path.to_owned()).expect("the fake's paths are valid")
}

fn paths(paths: &[String]) -> Vec<ObjectPath> {
    paths.iter().map(|p| path(p)).collect()
}

impl SecretService {
    /// Starts the Secret Service on `mock`'s bus, its collection empty and unlocked.
    pub fn start(mock: &Mock) -> SecretService {
        let connection = mock.connect();
        let keyring: Rc<RefCell<Keyring>> = Rc::default();
        let node = gio::DBusNodeInfo::for_xml(INTERFACES).expect("the fake's interfaces are valid");
        let interface = |name: &str| {
            node.lookup_interface(name)
                .expect("the node has the interface")
        };
        let mut registrations = Vec::new();
        let mut register = |object: &str, name: &str| {
            let weak = Rc::downgrade(&keyring);
            let keyring = keyring.clone();
            let registration = connection
                .register_object(object, &interface(name))
                .method_call(
                    move |connection, _, path, _, method, parameters, invocation| {
                        handle(&keyring, &connection, path, method, &parameters, invocation);
                    },
                )
                .property(move |_, _, path, _, property| {
                    let keyring = weak.upgrade().expect("the fake outlives its objects");
                    let keyring = keyring.borrow();
                    match property {
                        "Locked" => keyring.locked.to_variant(),
                        _ => keyring
                            .items
                            .get(&item_id(path))
                            .map(|item| item.attributes.clone())
                            .unwrap_or_default()
                            .to_variant(),
                    }
                })
                .build()
                .expect("the fake's object is free");
            registrations.push(registration);
        };
        register(SERVICE_PATH, "org.freedesktop.Secret.Service");
        register(COLLECTION_PATH, "org.freedesktop.Secret.Collection");
        // The items and prompts the test can reach: enough for any one test.
        for id in 1..=16 {
            register(&item_path(id), "org.freedesktop.Secret.Item");
            register(
                &format!("{PROMPT_PATH}{id}"),
                "org.freedesktop.Secret.Prompt",
            );
        }
        let owner = gio::bus_own_name_on_connection(
            &connection,
            NAME,
            gio::BusNameOwnerFlags::NONE,
            |_, _| {},
            |_, _| {},
        );
        mock.wait_for_name(NAME);
        SecretService {
            keyring,
            connection,
            registrations,
            owner: Some(owner),
        }
    }

    /// Adds `item` to the collection, as another program would have.
    pub fn add(&self, item: Item) {
        let mut keyring = self.keyring.borrow_mut();
        keyring.last_item += 1;
        let id = keyring.last_item;
        keyring.items.insert(id, item);
    }

    /// The collection's items, oldest first.
    pub fn items(&self) -> Vec<Item> {
        self.keyring.borrow().items.values().cloned().collect()
    }

    /// Locks the collection, and says whether its unlock prompt will be dismissed.
    pub fn lock(&self, refuse: bool) {
        let mut keyring = self.keyring.borrow_mut();
        keyring.locked = true;
        keyring.refuse = refuse;
    }

    pub fn locked(&self) -> bool {
        self.keyring.borrow().locked
    }

    /// How many prompts were shown.
    pub fn prompts(&self) -> u32 {
        self.keyring.borrow().last_prompt
    }
}

fn handle(
    keyring: &Rc<RefCell<Keyring>>,
    connection: &gio::DBusConnection,
    object: &str,
    method: &str,
    parameters: &Variant,
    invocation: gio::DBusMethodInvocation,
) {
    let mut state = keyring.borrow_mut();
    match method {
        "OpenSession" => {
            let algorithm = parameters.child_value(0);
            assert_eq!(algorithm.str(), Some("plain"), "the agent asks for plain");
            invocation.return_value(Some(&("".to_variant(), path(SESSION_PATH)).to_variant()));
        }
        "SearchItems" => {
            let wanted: HashMap<String, String> = parameters.child_value(0).get().unwrap();
            let found: Vec<String> = state
                .items
                .iter()
                .filter(|(_, item)| {
                    wanted
                        .iter()
                        .all(|(key, value)| item.attributes.get(key) == Some(value))
                })
                .map(|(id, _)| item_path(*id))
                .collect();
            let (unlocked, locked) = if state.locked {
                (Vec::new(), found)
            } else {
                (found, Vec::new())
            };
            invocation.return_value(Some(&(paths(&unlocked), paths(&locked)).to_variant()));
        }
        "Unlock" => {
            let objects: Vec<ObjectPath> = parameters.child_value(0).get().unwrap();
            let objects: Vec<String> = objects.iter().map(|o| o.as_str().to_owned()).collect();
            if state.locked {
                state.last_prompt += 1;
                let id = state.last_prompt;
                state.prompts.insert(id, objects);
                let prompt = format!("{PROMPT_PATH}{id}");
                invocation.return_value(Some(
                    &(Vec::<ObjectPath>::new(), path(&prompt)).to_variant(),
                ));
            } else {
                invocation.return_value(Some(&(paths(&objects), path("/")).to_variant()));
            }
        }
        "Prompt" => {
            let id: u32 = object[PROMPT_PATH.len()..].parse().unwrap();
            let objects = state.prompts.remove(&id).unwrap_or_default();
            invocation.return_value(None);
            let (dismissed, unlocked) = if state.refuse {
                (true, Vec::new())
            } else {
                state.locked = false;
                (false, objects)
            };
            connection
                .emit_signal(
                    None,
                    object,
                    "org.freedesktop.Secret.Prompt",
                    "Completed",
                    Some(&(dismissed, paths(&unlocked).to_variant()).to_variant()),
                )
                .unwrap();
        }
        "ReadAlias" => {
            invocation.return_value(Some(&(path(COLLECTION_PATH),).to_variant()));
        }
        "GetSecrets" => {
            if state.locked {
                invocation.return_dbus_error(IS_LOCKED, "the collection is locked");
                return;
            }
            let items: Vec<ObjectPath> = parameters.child_value(0).get().unwrap();
            let secrets: HashMap<ObjectPath, (ObjectPath, Vec<u8>, Vec<u8>, String)> = items
                .into_iter()
                .filter_map(|item| {
                    let secret = state.items.get(&item_id(item.as_str()))?.secret.clone();
                    Some((
                        item,
                        (
                            path(SESSION_PATH),
                            Vec::new(),
                            secret.into_bytes(),
                            "text/plain".to_owned(),
                        ),
                    ))
                })
                .collect();
            invocation.return_value(Some(&(secrets,).to_variant()));
        }
        "CreateItem" => {
            if state.locked {
                invocation.return_dbus_error(IS_LOCKED, "the collection is locked");
                return;
            }
            let properties: HashMap<String, Variant> = parameters.child_value(0).get().unwrap();
            let (_, _, secret, _): (ObjectPath, Vec<u8>, Vec<u8>, String) =
                parameters.child_value(1).get().unwrap();
            let replace: bool = parameters.child_value(2).get().unwrap();
            let item = Item {
                label: properties["org.freedesktop.Secret.Item.Label"]
                    .str()
                    .unwrap()
                    .to_owned(),
                attributes: properties["org.freedesktop.Secret.Item.Attributes"]
                    .get()
                    .unwrap(),
                secret: String::from_utf8(secret).unwrap(),
            };
            let same = state
                .items
                .iter()
                .find(|(_, other)| other.attributes == item.attributes)
                .map(|(id, _)| *id);
            let id = match same.filter(|_| replace) {
                Some(id) => id,
                None => {
                    state.last_item += 1;
                    state.last_item
                }
            };
            state.items.insert(id, item);
            invocation.return_value(Some(&(path(&item_path(id)), path("/")).to_variant()));
        }
        "Delete" => {
            if state.locked {
                invocation.return_dbus_error(IS_LOCKED, "the collection is locked");
                return;
            }
            state.items.remove(&item_id(object));
            invocation.return_value(Some(&(path("/"),).to_variant()));
        }
        _ => invocation.return_dbus_error(
            "org.freedesktop.DBus.Error.UnknownMethod",
            &format!("the fake has no method {method}"),
        ),
    }
}
