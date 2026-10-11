//! The sounds Caminito plays (170): the bell (105) and the workspace switch sounds, by their
//! names in the freedesktop sound naming spec, from the sound theme `org.mate.sound
//! theme-name` names. Marco played them through GTK's canberra integration; Caminito calls
//! libcanberra itself (ADR 0027), behind [`Player`], which the tests replace.

use std::ffi::{CStr, CString, c_char, c_int, c_void};
use std::ptr::{self, NonNull};

use caminito_policy::Direction;

use crate::state::Caminito;

/// A sound Caminito plays.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Sound {
    /// The bell's (`meta_bell_notify`, bell.c:281-364).
    Bell,
    /// A workspace switch's, by the way it went in the Workspace grid
    /// (`workspace_switch_sound`, workspace.c:310-362).
    Switch(Direction),
}

impl Sound {
    /// The sound's name in the sound naming spec, as Marco's canberra calls name it.
    pub(crate) fn event_id(self) -> &'static CStr {
        match self {
            Sound::Bell => c"bell-window-system",
            Sound::Switch(Direction::Left) => c"desktop-switch-left",
            Sound::Switch(Direction::Right) => c"desktop-switch-right",
            Sound::Switch(Direction::Up) => c"desktop-switch-up",
            Sound::Switch(Direction::Down) => c"desktop-switch-down",
        }
    }

    /// What the sound server's lists call it, in Marco's words (bell.c:297,
    /// workspace.c:356).
    fn description(self) -> &'static CStr {
        match self {
            Sound::Bell => c"Bell event",
            Sound::Switch(_) => c"Desktop switched",
        }
    }
}

/// What plays Caminito's sounds: libcanberra in a Session, and a recorder in the tests.
pub trait Player {
    /// Plays `sound` from the sound theme `theme`, without waiting for it.
    fn play(&mut self, theme: &str, sound: Sound);
}

impl Caminito {
    /// Plays `sound` from the sound theme the settings name.
    pub(crate) fn play_sound(&mut self, sound: Sound) {
        self.player.play(&self.settings.sound.theme_name, sound);
    }

    /// Plays the sound of each workspace switch since the last pass, if `event-sounds` is
    /// on: Marco plays one at every activation of a workspace, whatever asked for it
    /// (`meta_workspace_activate_with_focus`, workspace.c:382-383).
    pub(crate) fn refresh_sounds(&mut self) {
        for direction in self.policy.take_workspace_switches() {
            if self.settings.sound.event_sounds {
                self.play_sound(Sound::Switch(direction));
            }
        }
    }
}

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

/// libcanberra's callback for a sound that finished, which Caminito never gives.
type CaFinished = unsafe extern "C" fn(*mut CaContext, u32, c_int, *mut c_void);

#[link(name = "canberra")]
unsafe extern "C" {
    fn ca_context_create(context: *mut *mut CaContext) -> c_int;
    fn ca_context_destroy(context: *mut CaContext) -> c_int;
    #[cfg(test)]
    fn ca_context_set_driver(context: *mut CaContext, driver: *const c_char) -> c_int;
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

/// libcanberra, which finds a sound in the theme and plays it through the sound server
/// (ADR 0027). Its context is made at the first sound, so a Session with no sound server
/// starts as any other, and a sound that can't be played is logged and dropped.
#[derive(Default)]
pub(crate) struct Canberra {
    context: Option<NonNull<CaContext>>,
    /// Whether a failure was logged as a warning, which only the first is.
    warned: bool,
}

impl Canberra {
    /// The context, made at the first call.
    fn context(&mut self) -> Result<NonNull<CaContext>, c_int> {
        if let Some(context) = self.context {
            return Ok(context);
        }
        let mut context = ptr::null_mut();
        // Safety: `context` is a valid place for the new context's pointer.
        let code = unsafe { ca_context_create(&mut context) };
        let context = NonNull::new(context).ok_or(code)?;
        // Kept whether or not it takes its name: it plays without one, and `drop` destroys
        // it.
        self.context = Some(context);
        let named = Props::new(&[
            (c"application.name", c"Caminito"),
            (c"application.id", c"org.mate.Caminito"),
        ])
        .and_then(|props| {
            // Safety: the context and the list are live, and libcanberra copies the list.
            check(unsafe { ca_context_change_props_full(context.as_ptr(), props.0.as_ptr()) })
        });
        if let Err(code) = named {
            tracing::debug!("libcanberra's context has no name: {}", ca_error(code));
        }
        Ok(context)
    }

    /// Asks libcanberra for `sound` from `theme`. The sound is read from the theme each time
    /// and never kept in the sound server's cache, where Marco asks for `permanent`
    /// (bell.c:298, workspace.c:357): the cache has a sound by its name alone, so it would
    /// go on playing the last theme's after `theme-name` changes.
    fn try_play(&mut self, theme: &CStr, sound: Sound) -> Result<(), c_int> {
        let context = self.context()?;
        let props = Props::new(&[
            (c"event.id", sound.event_id()),
            (c"event.description", sound.description()),
            (c"canberra.cache-control", c"never"),
            (c"canberra.xdg-theme.name", theme),
        ])?;
        // Safety: the context and the list are live, libcanberra copies the list, and with
        // no callback it's given no data to keep.
        check(unsafe {
            ca_context_play_full(context.as_ptr(), 1, props.0.as_ptr(), None, ptr::null_mut())
        })
    }
}

impl Player for Canberra {
    fn play(&mut self, theme: &str, sound: Sound) {
        let Ok(theme) = CString::new(theme) else {
            tracing::debug!("can't play from the {theme:?} sound theme: its name has a NUL");
            return;
        };
        let Err(code) = self.try_play(&theme, sound) else {
            return;
        };
        let id = sound.event_id().to_string_lossy();
        let error = ca_error(code);
        if self.warned {
            tracing::debug!("can't play {id} from the {theme:?} sound theme: {error}");
        } else {
            self.warned = true;
            tracing::warn!("can't play {id} from the {theme:?} sound theme: {error}");
        }
    }
}

impl Drop for Canberra {
    fn drop(&mut self) {
        if let Some(context) = self.context.take() {
            // Safety: the context is live, and nothing uses it after this.
            unsafe { ca_context_destroy(context.as_ptr()) };
        }
    }
}

/// The tests' player: the sounds Caminito asked for, each with its sound theme and its name
/// in the sound naming spec, the oldest first.
#[cfg(test)]
#[derive(Debug, Clone, Default)]
pub(crate) struct Recorder(std::rc::Rc<std::cell::RefCell<Vec<(String, String)>>>);

#[cfg(test)]
impl Recorder {
    /// The sounds asked for since this was last called.
    pub(crate) fn take(&self) -> Vec<(String, String)> {
        self.0.take()
    }
}

#[cfg(test)]
impl Player for Recorder {
    fn play(&mut self, theme: &str, sound: Sound) {
        let id = sound.event_id().to_string_lossy().into_owned();
        self.0.borrow_mut().push((theme.to_owned(), id));
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    /// libcanberra with its `null` driver, which opens no sound server and plays nothing.
    fn silent() -> Canberra {
        let mut canberra = Canberra::default();
        let context = canberra.context().expect("libcanberra makes a context");
        // Safety: the context is live, and the driver's name is a NUL-terminated literal that
        // libcanberra copies.
        let code = unsafe { ca_context_set_driver(context.as_ptr(), c"null".as_ptr()) };
        assert_eq!(code, CA_SUCCESS, "{}", ca_error(code));
        canberra
    }

    #[test]
    fn each_sound_has_its_name_in_the_sound_naming_spec() {
        let names = [
            (Sound::Bell, c"bell-window-system"),
            (Sound::Switch(Direction::Left), c"desktop-switch-left"),
            (Sound::Switch(Direction::Right), c"desktop-switch-right"),
            (Sound::Switch(Direction::Up), c"desktop-switch-up"),
            (Sound::Switch(Direction::Down), c"desktop-switch-down"),
        ];
        for (sound, name) in names {
            assert_eq!(sound.event_id(), name);
        }
    }

    /// The calls into libcanberra, as far as a machine with no sound goes: a sound the
    /// library can't find or play is dropped, and Caminito goes on.
    #[test]
    fn a_sound_libcanberra_cant_play_is_dropped() {
        let mut canberra = silent();

        canberra.play("a theme that isn't installed", Sound::Bell);
        canberra.play("freedesktop", Sound::Switch(Direction::Left));
        canberra.play("with a\0NUL", Sound::Bell);

        assert!(canberra.context.is_some());
    }

    #[test]
    fn a_libcanberra_error_has_its_text() {
        assert_eq!(ca_error(CA_SUCCESS), "Success");
        assert_eq!(ca_error(1), "libcanberra's error 1");
    }
}
