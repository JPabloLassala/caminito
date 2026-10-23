//! The agent-owned secrets (ADR 0041): the ones a user saved "for this user only", which
//! NetworkManager doesn't keep and asks its agent for. They live in the Secret Service, as
//! gnome-keyring serves it on the session bus, under nm-applet's attributes, so the secrets
//! nm-applet saved are found and nm-applet finds the ones saved here.
//!
//! nm-applet saves through libsecret, with its schema `org.freedesktop.NetworkManager.Connection`
//! (network-manager-applet's src/applet-agent.c, line 21, `network_manager_secret_schema`): an item for
//! each secret, in the default collection, with the attributes `connection-uuid`,
//! `setting-name` and `setting-key`, and `xdg:schema` that libsecret adds. It searches by the
//! first two alone, as libsecret does for a schema with `SECRET_SCHEMA_DONT_MATCH_NAME`, so an
//! item without `xdg:schema` is found too.
//!
//! The Secret Service API is called over gio's D-Bus, with no libsecret. Secrets travel in the
//! plain, which the API allows on the session bus. A locked collection is unlocked through the
//! Secret Service's own prompt.

use std::cell::RefCell;
use std::collections::HashMap;
use std::fmt;

use gio::glib::variant::{ObjectPath, ToVariant};
use gio::glib::{self, Variant};

const NAME: &str = "org.freedesktop.secrets";
const SERVICE_PATH: &str = "/org/freedesktop/secrets";
const SERVICE: &str = "org.freedesktop.Secret.Service";
const COLLECTION: &str = "org.freedesktop.Secret.Collection";
const ITEM: &str = "org.freedesktop.Secret.Item";
const PROMPT: &str = "org.freedesktop.Secret.Prompt";
const PROPERTIES: &str = "org.freedesktop.DBus.Properties";
/// What the API answers for no prompt, and for no default collection.
const NONE: &str = "/";

/// nm-applet's attributes (network-manager-applet's src/applet-agent.c).
const UUID_TAG: &str = "connection-uuid";
const SETTING_NAME_TAG: &str = "setting-name";
const SETTING_KEY_TAG: &str = "setting-key";
/// The schema's name, which libsecret writes as the attribute `xdg:schema`.
const SCHEMA: &str = "org.freedesktop.NetworkManager.Connection";

/// The Secret Service's secret, as `(oayays)`: the session, the parameters, the value and its
/// content type.
type Secret = (ObjectPath, Vec<u8>, Vec<u8>, String);

/// A call to the Secret Service failed.
#[derive(Debug)]
pub(crate) enum Error {
    /// The Secret Service isn't there, or refused the call.
    Call(glib::Error),
    /// The default collection is locked, and the user didn't unlock it.
    Locked,
    /// There's no default collection to save in.
    NoCollection,
}

impl fmt::Display for Error {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Error::Call(err) => write!(f, "the secret service refused: {err}"),
            Error::Locked => write!(f, "the keyring stayed locked"),
            Error::NoCollection => write!(f, "the secret service has no default collection"),
        }
    }
}

impl std::error::Error for Error {
    fn source(&self) -> Option<&(dyn std::error::Error + 'static)> {
        match self {
            Error::Call(err) => Some(err),
            Error::Locked | Error::NoCollection => None,
        }
    }
}

impl From<glib::Error> for Error {
    fn from(err: glib::Error) -> Error {
        Error::Call(err)
    }
}

/// One secret to save.
pub(crate) struct Entry {
    /// The connection's UUID.
    pub(crate) uuid: String,
    /// The setting the secret is in, as `802-11-wireless-security`.
    pub(crate) setting: String,
    /// The secret's key in the setting, as `psk`.
    pub(crate) key: String,
    /// The item's name, as the keyring's manager shows it.
    pub(crate) label: String,
    pub(crate) secret: String,
}

/// The Secret Service, on the session bus.
pub(crate) struct Keyring {
    connection: gio::DBusConnection,
    /// The session the secrets travel in, opened at the first need of one.
    session: RefCell<Option<ObjectPath>>,
}

impl Keyring {
    /// The Secret Service on `connection`, the session bus. Nothing is called until it's
    /// needed.
    pub(crate) fn new(connection: &gio::DBusConnection) -> Keyring {
        Keyring {
            connection: connection.clone(),
            session: RefCell::default(),
        }
    }

    /// The secrets saved for `setting` of the connection `uuid`, by their keys. Locked ones
    /// are unlocked through the keyring's prompt if `may_prompt`; those that stay locked are
    /// left out.
    pub(crate) async fn find(
        &self,
        uuid: &str,
        setting: &str,
        may_prompt: bool,
    ) -> Result<HashMap<String, String>, Error> {
        let (items, _) = self
            .search(&[(UUID_TAG, uuid), (SETTING_NAME_TAG, setting)], may_prompt)
            .await?;
        if items.is_empty() {
            return Ok(HashMap::new());
        }
        let session = self.session().await?;
        let reply = self
            .call(
                SERVICE_PATH,
                SERVICE,
                "GetSecrets",
                (items.clone(), session).to_variant(),
            )
            .await
            .inspect_err(|_| self.forget_session())?;
        let secrets: HashMap<ObjectPath, Secret> = reply
            .child_value(0)
            .get()
            .expect("GetSecrets answers a{o(oayays)}");
        let mut found = HashMap::new();
        for (item, (_, _, value, _)) in secrets {
            let attributes = self.property(item.as_str(), ITEM, "Attributes").await?;
            let attributes: HashMap<String, String> =
                attributes.get().expect("an item's attributes are a{ss}");
            if let Some(key) = attributes.get(SETTING_KEY_TAG) {
                // A secret that isn't text isn't one NetworkManager takes as a string.
                if let Ok(value) = String::from_utf8(value) {
                    found.insert(key.clone(), value);
                }
            }
        }
        Ok(found)
    }

    /// Saves `entry` in the default collection, in place of what was saved for its key. A
    /// locked collection is unlocked through the keyring's prompt if `may_prompt`.
    pub(crate) async fn store(&self, entry: &Entry, may_prompt: bool) -> Result<(), Error> {
        let collection = self.default_collection().await?;
        let locked = self
            .property(collection.as_str(), COLLECTION, "Locked")
            .await?;
        if locked.get::<bool>() == Some(true)
            && self
                .unlock(vec![collection.clone()], may_prompt)
                .await?
                .is_empty()
        {
            return Err(Error::Locked);
        }
        // CreateItem replaces only an item with the very same attributes, and one saved
        // without `xdg:schema` has others: it would stay, and be found beside this one.
        let key = [
            (UUID_TAG, entry.uuid.as_str()),
            (SETTING_NAME_TAG, entry.setting.as_str()),
            (SETTING_KEY_TAG, entry.key.as_str()),
        ];
        let (old, _) = self.search(&key, false).await?;
        self.delete(old).await?;

        let mut attributes: HashMap<&str, &str> = key.into_iter().collect();
        attributes.insert("xdg:schema", SCHEMA);
        let properties = HashMap::from([
            (format!("{ITEM}.Label"), entry.label.to_variant()),
            (format!("{ITEM}.Attributes"), attributes.to_variant()),
        ]);
        let secret: Secret = (
            self.session().await?,
            Vec::new(),
            entry.secret.as_bytes().to_vec(),
            "text/plain".to_owned(),
        );
        let reply = self
            .call(
                collection.as_str(),
                COLLECTION,
                "CreateItem",
                (properties, secret, true).to_variant(),
            )
            .await
            .inspect_err(|_| self.forget_session())?;
        let prompt: ObjectPath = reply.child_value(1).get().expect("CreateItem answers (oo)");
        if prompt.as_str() != NONE && self.prompt(&prompt).await?.is_none() {
            return Err(Error::Locked);
        }
        Ok(())
    }

    /// Deletes every secret saved for the connection `uuid`, unlocking them through the
    /// keyring's prompt. It fails if some stay locked.
    pub(crate) async fn clear(&self, uuid: &str) -> Result<(), Error> {
        let (items, locked) = self.search(&[(UUID_TAG, uuid)], true).await?;
        self.delete(items).await?;
        if locked > 0 {
            return Err(Error::Locked);
        }
        Ok(())
    }

    async fn delete(&self, items: Vec<ObjectPath>) -> Result<(), Error> {
        for item in items {
            let reply = self
                .call(item.as_str(), ITEM, "Delete", ().to_variant())
                .await?;
            let prompt: ObjectPath = reply.child_value(0).get().expect("Delete answers (o)");
            if prompt.as_str() != NONE {
                self.prompt(&prompt).await?;
            }
        }
        Ok(())
    }

    /// The unlocked items with `attributes`, those that were locked unlocked first if
    /// `may_prompt`, and how many stayed locked.
    async fn search(
        &self,
        attributes: &[(&str, &str)],
        may_prompt: bool,
    ) -> Result<(Vec<ObjectPath>, usize), Error> {
        let attributes: HashMap<&str, &str> = attributes.iter().copied().collect();
        let reply = self
            .call(
                SERVICE_PATH,
                SERVICE,
                "SearchItems",
                (attributes,).to_variant(),
            )
            .await?;
        let (mut unlocked, locked): (Vec<ObjectPath>, Vec<ObjectPath>) =
            reply.get().expect("SearchItems answers (aoao)");
        let mut still_locked = locked.len();
        if !locked.is_empty() {
            let now_unlocked = self.unlock(locked, may_prompt).await?;
            still_locked -= now_unlocked.len().min(still_locked);
            unlocked.extend(now_unlocked);
        }
        Ok((unlocked, still_locked))
    }

    /// Unlocks `objects`, through the Secret Service's prompt where it has one and
    /// `may_prompt`, and gives those that are unlocked: none when the user dismissed the
    /// prompt, or there was one and no leave to show it.
    async fn unlock(
        &self,
        objects: Vec<ObjectPath>,
        may_prompt: bool,
    ) -> Result<Vec<ObjectPath>, Error> {
        if !may_prompt {
            // Unlock would hand back a prompt to show, or to dismiss: there's no asking it
            // without the risk of one.
            return Ok(Vec::new());
        }
        let reply = self
            .call(SERVICE_PATH, SERVICE, "Unlock", (objects,).to_variant())
            .await?;
        let (mut unlocked, prompt): (Vec<ObjectPath>, ObjectPath) =
            reply.get().expect("Unlock answers (aoo)");
        if prompt.as_str() != NONE
            && let Some(result) = self.prompt(&prompt).await?
        {
            unlocked.extend(result.get::<Vec<ObjectPath>>().unwrap_or_default());
        }
        Ok(unlocked)
    }

    /// Shows the Secret Service's prompt and waits for the user: its result, or None when
    /// they dismissed it.
    async fn prompt(&self, prompt: &ObjectPath) -> Result<Option<Variant>, Error> {
        let (sender, completed) = futures_channel::oneshot::channel::<Variant>();
        let sender = RefCell::new(Some(sender));
        let _subscription = self.connection.subscribe_to_signal(
            None,
            Some(PROMPT),
            Some("Completed"),
            Some(prompt.as_str()),
            None,
            gio::DBusSignalFlags::NONE,
            move |signal| {
                if let Some(sender) = sender.borrow_mut().take() {
                    let _ = sender.send(signal.parameters.clone());
                }
            },
        );
        // No parent window: the agent's dialogs aren't open when the keyring asks.
        self.call(prompt.as_str(), PROMPT, "Prompt", ("",).to_variant())
            .await?;
        let Ok(completed) = completed.await else {
            return Ok(None);
        };
        let (dismissed, result): (bool, Variant) = completed.get().expect("Completed sends (bv)");
        Ok((!dismissed).then_some(result))
    }

    /// The default collection, where libsecret saves what names no collection.
    async fn default_collection(&self) -> Result<ObjectPath, Error> {
        let reply = self
            .call(
                SERVICE_PATH,
                SERVICE,
                "ReadAlias",
                ("default",).to_variant(),
            )
            .await?;
        let collection: ObjectPath = reply.child_value(0).get().expect("ReadAlias answers (o)");
        if collection.as_str() == NONE {
            return Err(Error::NoCollection);
        }
        Ok(collection)
    }

    async fn session(&self) -> Result<ObjectPath, Error> {
        if let Some(session) = self.session.borrow().clone() {
            return Ok(session);
        }
        let reply = self
            .call(
                SERVICE_PATH,
                SERVICE,
                "OpenSession",
                ("plain", "".to_variant()).to_variant(),
            )
            .await?;
        let session: ObjectPath = reply
            .child_value(1)
            .get()
            .expect("OpenSession answers (vo)");
        *self.session.borrow_mut() = Some(session.clone());
        Ok(session)
    }

    /// Forgets the session after a failed call, which may be for a Secret Service that
    /// restarted since it was opened.
    fn forget_session(&self) {
        self.session.borrow_mut().take();
    }

    async fn property(&self, path: &str, interface: &str, name: &str) -> Result<Variant, Error> {
        let reply = self
            .call(path, PROPERTIES, "Get", (interface, name).to_variant())
            .await?;
        let value = reply.child_value(0);
        Ok(value.as_variant().unwrap_or(value))
    }

    async fn call(
        &self,
        path: &str,
        interface: &str,
        method: &str,
        parameters: Variant,
    ) -> Result<Variant, glib::Error> {
        self.connection
            .call_future(
                Some(NAME),
                path,
                interface,
                method,
                Some(&parameters),
                None,
                gio::DBusCallFlags::NONE,
                -1,
            )
            .await
    }
}
