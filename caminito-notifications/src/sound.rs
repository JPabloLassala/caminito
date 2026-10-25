//! The notifications' sounds, through libcanberra (CMN-258), as Caminito plays its own
//! (`caminito/src/sound.rs`, ADR 0027): a theme's sound by its name, from the sound theme
//! `org.mate.sound theme-name` names, or a file, and none while `event-sounds` is off.

use std::cell::{OnceCell, RefCell};
use std::ffi::{CStr, CString, c_char, c_int, c_void};
use std::ptr::{self, NonNull};

use caminito_notifications::DOMAIN;
use caminito_notifications::server::{Sound, Sounds};
use gtk::gio;
use gtk::glib;
use gtk::prelude::*;

/// MATE's sound settings, where the theme is.
const SOUND_SCHEMA: &str = "org.mate.sound";

/// libcanberra's context, which it hides.
#[repr(C)]
struct CaContext {
    _private: [u8; 0],
}

/// libcanberra's property list, which it hides.
#[repr(C)]
struct CaProplist {
    _private: [u8; 0],
}

/// libcanberra's callback for a sound that finished, which is never given.
type CaFinished = unsafe extern "C" fn(*mut CaContext, u32, c_int, *mut c_void);

#[link(name = "canberra")]
unsafe extern "C" {
    fn ca_context_create(context: *mut *mut CaContext) -> c_int;
    fn ca_context_destroy(context: *mut CaContext) -> c_int;
    fn ca_context_change_props_full(context: *mut CaContext, props: *mut CaProplist) -> c_int;
    fn ca_context_play_full(
        context: *mut CaContext,
        id: u32,
        props: *mut CaProplist,
        finished: Option<CaFinished>,
        userdata: *mut c_void,
    ) -> c_int;
    fn ca_proplist_create(props: *mut *mut CaProplist) -> c_int;
    fn ca_proplist_destroy(props: *mut CaProplist) -> c_int;
    fn ca_proplist_sets(props: *mut CaProplist, key: *const c_char, value: *const c_char) -> c_int;
    fn ca_strerror(code: c_int) -> *const c_char;
}

/// libcanberra's `CA_SUCCESS`.
const CA_SUCCESS: c_int = 0;
/// libcanberra's `CA_ERROR_INVALID`.
const CA_ERROR_INVALID: c_int = -2;

/// `code` as a result: libcanberra's `CA_SUCCESS`, or an error.
fn check(code: c_int) -> Result<(), c_int> {
    if code == CA_SUCCESS {
        Ok(())
    } else {
        Err(code)
    }
}

/// A libcanberra error's text.
fn ca_error(code: c_int) -> String {
    // Safety: `ca_strerror` takes any code, and returns a NUL-terminated static string or,
    // for a code that isn't one of its own, NULL, which is looked for before it's read.
    let text = unsafe { ca_strerror(code) };
    if text.is_null() {
        return format!("libcanberra's error {code}");
    }
    // Safety: as above, the string is NUL-terminated and static.
    unsafe { CStr::from_ptr(text) }
        .to_string_lossy()
        .into_owned()
}

/// A libcanberra property list, destroyed as it's dropped.
struct Props(NonNull<CaProplist>);

impl Props {
    /// A list of `props`, each a key and its value.
    fn new(props: &[(&CStr, &CStr)]) -> Result<Self, c_int> {
        let mut list = ptr::null_mut();
        // Safety: `list` is a valid place for the new list's pointer.
        let code = unsafe { ca_proplist_create(&mut list) };
        let list = Self(NonNull::new(list).ok_or(code)?);
        for (key, value) in props {
            // Safety: the list is live, and the key and the value are NUL-terminated strings
            // that libcanberra copies before it returns.
            check(unsafe { ca_proplist_sets(list.0.as_ptr(), key.as_ptr(), value.as_ptr()) })?;
        }
        Ok(list)
    }
}

impl Drop for Props {
    fn drop(&mut self) {
        // Safety: the list is live, and nothing uses it after this.
        unsafe { ca_proplist_destroy(self.0.as_ptr()) };
    }
}

/// libcanberra, through the sound server. Its context is made at the first sound, and a sound
/// that can't be played is logged and dropped.
#[derive(Default)]
pub(crate) struct Canberra {
    context: RefCell<Option<NonNull<CaContext>>>,
    /// MATE's sound settings, looked up at the first sound, if their schema is installed.
    settings: OnceCell<Option<gio::Settings>>,
}

impl Canberra {
    /// The context, made at the first call.
    fn context(&self) -> Result<NonNull<CaContext>, c_int> {
        if let Some(context) = *self.context.borrow() {
            return Ok(context);
        }
        let mut context = ptr::null_mut();
        // Safety: `context` is a valid place for the new context's pointer.
        let code = unsafe { ca_context_create(&mut context) };
        let context = NonNull::new(context).ok_or(code)?;
        // Kept whether or not it takes its name: it plays without one.
        *self.context.borrow_mut() = Some(context);
        let named = Props::new(&[
            (c"application.name", c"Notifications"),
            (c"application.id", c"org.mate.Caminito.Notifications"),
        ])
        .and_then(|props| {
            // Safety: the context and the list are live, and libcanberra copies the list.
            check(unsafe { ca_context_change_props_full(context.as_ptr(), props.0.as_ptr()) })
        });
        if let Err(code) = named {
            glib::g_debug!(
                DOMAIN,
                "libcanberra's context has no name: {}",
                ca_error(code)
            );
        }
        Ok(context)
    }

    /// MATE's sound settings, if their schema is installed.
    fn settings(&self) -> Option<&gio::Settings> {
        self.settings
            .get_or_init(|| {
                gio::SettingsSchemaSource::default()
                    .and_then(|source| source.lookup(SOUND_SCHEMA, true))
                    .map(|_| gio::Settings::new(SOUND_SCHEMA))
            })
            .as_ref()
    }

    /// The sound theme MATE's settings name, or libcanberra's default.
    fn theme(&self) -> CString {
        let theme = self
            .settings()
            .map(|settings| settings.string("theme-name").to_string())
            .filter(|theme| !theme.is_empty())
            .unwrap_or_else(|| "freedesktop".to_owned());
        CString::new(theme).unwrap_or_else(|_| c"freedesktop".to_owned())
    }

    fn try_play(&self, sound: &Sound) -> Result<(), c_int> {
        let context = self.context()?;
        let theme = self.theme();
        let (key, value) = match sound {
            Sound::Name(name) => (c"event.id", name.as_bytes()),
            Sound::File(path) => (c"media.filename", path.as_os_str().as_encoded_bytes()),
        };
        // A name or path with a NUL in it names nothing there is.
        let value = CString::new(value).map_err(|_| CA_ERROR_INVALID)?;
        let props = Props::new(&[
            (key, &value),
            (c"event.description", c"Notification"),
            (c"media.role", c"event"),
            (c"canberra.xdg-theme.name", &theme),
        ])?;
        // Safety: the context and the list are live, libcanberra copies the list, and with
        // no callback it's given no data to keep.
        check(unsafe {
            ca_context_play_full(context.as_ptr(), 1, props.0.as_ptr(), None, ptr::null_mut())
        })
    }
}

impl Sounds for Canberra {
    fn play(&self, sound: &Sound) {
        // MATE's switch for every event sound, which Caminito's own follow too.
        if self
            .settings()
            .is_some_and(|settings| !settings.boolean("event-sounds"))
        {
            return;
        }
        if let Err(code) = self.try_play(sound) {
            glib::g_debug!(DOMAIN, "can't play {sound:?}: {}", ca_error(code));
        }
    }
}

impl Drop for Canberra {
    fn drop(&mut self) {
        if let Some(context) = self.context.get_mut().take() {
            // Safety: the context is live, and nothing uses it after this.
            unsafe { ca_context_destroy(context.as_ptr()) };
        }
    }
}
