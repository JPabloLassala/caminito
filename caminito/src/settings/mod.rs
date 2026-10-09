//! Settings (09, 15): one GLib thread owns every `GSettings` object, and a pure
//! translation turns their values into typed settings.
//!
//! GSettings change notifications need an iterated GLib main context, so the thread runs a
//! GLib main loop. `start` waits for its first full reading, so the settings are complete
//! before anything maps. After that, each burst of changes reaches Caminito's loop as new
//! `Settings` over a calloop channel.
//!
//! The thread also reads the keymap the system names, for the parts of the keymap the
//! settings leave empty (54). localed is asked once, at start-up.

pub mod translate;

use std::cell::RefCell;
use std::fmt;
use std::sync::mpsc;
use std::thread::JoinHandle;

use gio::glib;
use gio::prelude::*;
use smithay::reexports::calloop::channel;

use translate::{Keymap, Logged, Reading, SCHEMAS};
pub use translate::{MissingSchemas, Settings, Value};

/// Where the settings come from.
pub enum Source {
    /// The installed schemas, on the default backend (dconf in a session).
    System,
    /// The schemas compiled into `schema_dir` alone, on a memory backend of their own, with
    /// `initial` written before the first reading. Each test gets its own.
    #[cfg(test)]
    Test {
        schema_dir: std::path::PathBuf,
        initial: Vec<(&'static str, String, Value)>,
    },
}

/// Why the settings thread couldn't start.
#[derive(Debug)]
pub enum Error {
    MissingSchemas(MissingSchemas),
    /// The thread couldn't be spawned.
    Spawn(std::io::Error),
    /// The thread ended before its first reading.
    Stopped,
}

impl fmt::Display for Error {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Error::MissingSchemas(missing) => missing.fmt(f),
            Error::Spawn(err) => write!(f, "can't start the settings thread: {err}"),
            Error::Stopped => f.write_str("the settings thread stopped before reading settings"),
        }
    }
}

impl std::error::Error for Error {}

/// The GLib thread. Dropping it stops the thread.
pub struct SettingsThread {
    context: glib::MainContext,
    main_loop: glib::MainLoop,
    thread: Option<JoinHandle<()>>,
}

/// Starts the GLib thread, and waits for its first reading. Later changes arrive on the
/// returned channel.
pub fn start(
    source: Source,
) -> Result<(Settings, SettingsThread, channel::Channel<Settings>), Error> {
    let context = glib::MainContext::new();
    let main_loop = glib::MainLoop::new(Some(&context), false);
    let (changes, channel) = channel::channel();
    let (reply, first) = mpsc::channel();

    let thread = {
        let context = context.clone();
        let main_loop = main_loop.clone();
        std::thread::Builder::new()
            .name("caminito-settings".into())
            .spawn(move || {
                context
                    .with_thread_default(|| run(source, &context, changes, reply, &main_loop))
                    .expect("the settings thread owns its context");
            })
            .map_err(Error::Spawn)?
    };
    let thread = SettingsThread {
        context,
        main_loop,
        thread: Some(thread),
    };
    match first.recv() {
        Ok(Ok(settings)) => Ok((settings, thread, channel)),
        Ok(Err(missing)) => Err(Error::MissingSchemas(missing)),
        Err(mpsc::RecvError) => Err(Error::Stopped),
    }
}

impl SettingsThread {
    /// Writes a key, on the GLib thread. A change comes back on the channel as any other.
    pub fn set(&self, schema: &'static str, key: &str, value: Value) {
        let key = key.to_owned();
        self.context.invoke(move || {
            let settings = WATCHER.with_borrow(|watcher| {
                watcher
                    .as_ref()
                    .and_then(|w| w.settings.iter().find(|(id, _)| *id == schema))
                    .map(|(_, settings)| settings.clone())
            });
            let Some(settings) = settings else {
                tracing::warn!("can't set {schema} {key}: the schema isn't installed");
                return;
            };
            // Outside the borrow: the change signal is emitted from inside `set_value`.
            let result = to_variant(&value).and_then(|v| settings.set_value(&key, &v));
            if let Err(err) = result {
                tracing::warn!("can't set {schema} {key} to {value}: {err}");
            }
        });
    }
}

impl Drop for SettingsThread {
    fn drop(&mut self) {
        // Through the context, so a quit can't come before the loop runs and be lost.
        let main_loop = self.main_loop.clone();
        self.context.invoke(move || main_loop.quit());
        if let Some(thread) = self.thread.take()
            && thread.join().is_err()
        {
            tracing::error!("the settings thread panicked");
        }
    }
}

/// The GLib thread's state. It isn't `Send`, so it lives in a thread-local that the
/// closures run on the thread reach it through.
struct Watcher {
    context: glib::MainContext,
    main_loop: glib::MainLoop,
    settings: Vec<(&'static str, gio::Settings)>,
    missing: Vec<&'static str>,
    system_keymap: Keymap,
    logged: Logged,
    last: Settings,
    changes: channel::Sender<Settings>,
    /// Whether a reading is queued for when the burst of changes ends.
    queued: bool,
}

thread_local! {
    static WATCHER: RefCell<Option<Watcher>> = const { RefCell::new(None) };
}

impl Watcher {
    fn translate(&mut self) -> Settings {
        read(
            &self.settings,
            &self.missing,
            &self.system_keymap,
            &mut self.logged,
        )
    }
}

/// Reads every key, translates them and logs what's newly wrong with them.
fn read(
    settings: &[(&'static str, gio::Settings)],
    missing: &[&'static str],
    system_keymap: &Keymap,
    logged: &mut Logged,
) -> Settings {
    {
        let mut reading = Reading::default();
        reading.set_system_keymap(system_keymap.clone());
        for schema in missing {
            reading.set_missing(schema);
        }
        for (id, settings) in settings {
            let schema = settings
                .settings_schema()
                .expect("settings made from a schema have one");
            for key in schema.list_keys() {
                let default = schema.key(&key).default_value();
                reading.insert(
                    id,
                    &key,
                    from_variant(&settings.value(&key)),
                    from_variant(&default),
                );
            }
        }
        let translation = translate::translate(&reading);
        for problem in logged.first_time(translation.problems) {
            tracing::warn!("{problem}");
        }
        translation.settings
    }
}

fn run(
    source: Source,
    context: &glib::MainContext,
    changes: channel::Sender<Settings>,
    reply: mpsc::Sender<Result<Settings, MissingSchemas>>,
    main_loop: &glib::MainLoop,
) {
    let (schemas, backend) = match &source {
        Source::System => (
            gio::SettingsSchemaSource::default(),
            None::<gio::SettingsBackend>,
        ),
        #[cfg(test)]
        Source::Test { schema_dir, .. } => (
            Some(
                gio::SettingsSchemaSource::from_directory(schema_dir, None, true)
                    .expect("the test schemas are compiled"),
            ),
            Some(gio::memory_settings_backend_new()),
        ),
    };
    let lookup = |id: &str| schemas.as_ref().and_then(|s| s.lookup(id, true));
    if let Err(missing) = translate::check_schemas(|id| lookup(id).is_some()) {
        let _ = reply.send(Err(missing));
        return;
    }

    let system_keymap = match &source {
        Source::System => Keymap::from_environment(|name| std::env::var(name).ok())
            .or(&localed_keymap().unwrap_or_default()),
        // Tests don't ask localed, so what they get doesn't depend on the machine's locale.
        #[cfg(test)]
        Source::Test { .. } => Keymap::default(),
    };
    let mut settings = Vec::new();
    let mut missing = Vec::new();
    for schema in SCHEMAS {
        match lookup(schema.id) {
            Some(found) => settings.push((
                schema.id,
                gio::Settings::new_full(&found, backend.as_ref(), None),
            )),
            None => missing.push(schema.id),
        }
    }
    #[cfg(test)]
    if let Source::Test { initial, .. } = source {
        for (schema, key, value) in initial {
            let (_, target) = settings
                .iter()
                .find(|(id, _)| *id == schema)
                .expect("a test schema");
            target
                .set_value(&key, &to_variant(&value).unwrap())
                .expect("a valid test value");
        }
    }
    for (_, s) in &settings {
        s.connect_changed(None, |_, _| queue_send());
    }

    let mut logged = Logged::default();
    let first = read(&settings, &missing, &system_keymap, &mut logged);
    WATCHER.set(Some(Watcher {
        context: context.clone(),
        main_loop: main_loop.clone(),
        settings,
        missing,
        system_keymap,
        logged,
        last: first.clone(),
        changes,
        queued: false,
    }));
    if reply.send(Ok(first)).is_err() {
        return;
    }
    main_loop.run();
    WATCHER.set(None);
}

/// localed's X11 keymap, which is what the X server would have used (09). Without it, the
/// keymap is xkbcommon's default.
fn localed_keymap() -> Option<Keymap> {
    let proxy = gio::DBusProxy::for_bus_sync(
        gio::BusType::System,
        gio::DBusProxyFlags::DO_NOT_CONNECT_SIGNALS,
        None,
        "org.freedesktop.locale1",
        "/org/freedesktop/locale1",
        "org.freedesktop.locale1",
        gio::Cancellable::NONE,
    );
    let proxy = match proxy {
        Ok(proxy) => proxy,
        Err(err) => {
            tracing::warn!("can't ask localed for the system keymap: {err}");
            return None;
        }
    };
    let property = |name| {
        proxy
            .cached_property(name)
            .and_then(|value| value.get::<String>())
            .unwrap_or_default()
    };
    let keymap = Keymap {
        model: property("X11Model"),
        layouts: property("X11Layout"),
        variants: property("X11Variant"),
        options: property("X11Options"),
    };
    tracing::debug!("localed's keymap: {keymap:?}");
    Some(keymap)
}

/// Queues sending the settings for when the current burst of changes is over, such as the many
/// keys a `gsettings reset-recursively` changes at once.
fn queue_send() {
    WATCHER.with_borrow_mut(|watcher| {
        let Some(watcher) = watcher.as_mut().filter(|w| !w.queued) else {
            return;
        };
        watcher.queued = true;
        glib::idle_source_new(None, glib::Priority::DEFAULT_IDLE, || {
            send_settings();
            glib::ControlFlow::Break
        })
        .attach(Some(&watcher.context));
    });
}

/// Sends the settings to Caminito's loop if they changed. Once the loop has gone, stops.
fn send_settings() {
    WATCHER.with_borrow_mut(|watcher| {
        let Some(watcher) = watcher.as_mut() else {
            return;
        };
        watcher.queued = false;
        let settings = watcher.translate();
        if settings == watcher.last {
            return;
        }
        watcher.last = settings.clone();
        if watcher.changes.send(settings).is_err() {
            watcher.main_loop.quit();
        }
    });
}

fn from_variant(variant: &glib::Variant) -> Value {
    if let Some(b) = variant.get::<bool>() {
        Value::Bool(b)
    } else if let Some(i) = variant.get::<i32>() {
        Value::Int(i)
    } else if let Some(u) = variant.get::<u32>() {
        Value::Uint(u)
    } else if let Some(i) = variant.get::<i64>() {
        Value::Int64(i)
    } else if let Some(d) = variant.get::<f64>() {
        Value::Double(d)
    } else if let Some(s) = variant.get::<String>() {
        Value::Str(s)
    } else if let Some(list) = variant.get::<Vec<String>>() {
        Value::StrList(list)
    } else if let Some(pair) = variant.get::<Option<(f64, f64)>>() {
        Value::MaybePair(pair)
    } else {
        Value::Other(variant.print(true).into())
    }
}

fn to_variant(value: &Value) -> Result<glib::Variant, glib::BoolError> {
    Ok(match value {
        Value::Bool(b) => b.to_variant(),
        Value::Int(i) => i.to_variant(),
        Value::Uint(u) => u.to_variant(),
        Value::Int64(i) => i.to_variant(),
        Value::Double(d) => d.to_variant(),
        Value::Str(s) => s.to_variant(),
        Value::StrList(list) => list.to_variant(),
        Value::MaybePair(pair) => pair.to_variant(),
        Value::Other(text) => glib::Variant::parse(None, text)
            .map_err(|err| glib::bool_error!("not GVariant text: {err}"))?,
    })
}
