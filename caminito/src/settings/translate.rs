//! GSettings values into Caminito's typed settings, with 09's rules for every key. Nothing
//! here touches GLib, so it's tested with plain values.

use std::collections::{BTreeMap, HashSet};
use std::fmt;
use std::time::Duration;

use caminito_policy::{
    Config, FocusMode, FocusNewWindows, MAX_WORKSPACES, MinimizedPlacement, Orientation,
    PlacementMode, ShowDesktopSkipList, UrgentPlacement, WorkspaceLayout, WrapStyle,
    default_workspace_name,
};
use caminito_theme::ButtonLayout;

use crate::input::bindings::Bindings;

pub const MARCO_GENERAL: &str = "org.mate.Marco.general";
pub const MARCO_WORKSPACE_NAMES: &str = "org.mate.Marco.workspace-names";
pub const MARCO_KEYBINDING_COMMANDS: &str = "org.mate.Marco.keybinding-commands";
/// mate-desktop's preferred terminal, which `run-command-terminal` runs (09, 149).
pub const TERMINAL: &str = "org.mate.applications-terminal";
/// The bindings of actions that aren't a window's (148).
pub const MARCO_GLOBAL_KEYBINDINGS: &str = "org.mate.Marco.global-keybindings";
/// The bindings of actions on the focused window (148).
pub const MARCO_WINDOW_KEYBINDINGS: &str = "org.mate.Marco.window-keybindings";
pub const INTERFACE: &str = "org.mate.interface";
pub const FONT_RENDERING: &str = "org.mate.font-rendering";
pub const MOUSE: &str = "org.mate.peripherals-mouse";
pub const TOUCHPAD: &str = "org.mate.peripherals-touchpad";
pub const KEYBOARD: &str = "org.mate.peripherals-keyboard";
pub const KEYBOARD_XKB: &str = "org.mate.peripherals-keyboard-xkb.kbd";
pub const SOUND: &str = "org.mate.sound";
pub const A11Y_KEYBOARD: &str = "org.mate.accessibility-keyboard";
/// mate-desktop's lockdown keys, of which Caminito reads `disable-lock-screen` (126).
pub const LOCKDOWN: &str = "org.mate.lockdown";
/// mate-session's, of which Caminito reads `idle-delay` (128).
pub const SESSION: &str = "org.mate.session";
/// mate-screensaver's, of which Caminito reads the keys of idle and the lock after it (09,
/// 128). caminito-locker reads the rest itself (12).
pub const SCREENSAVER: &str = "org.mate.screensaver";
/// mate-power-manager's, of which Caminito reads the outputs' power-off, `sleep-display-ac`
/// and `sleep-display-battery` (09, 129).
pub const POWER_MANAGER: &str = "org.mate.power-manager";
/// Caminito's own schema (09), which Caminito installs. A development run may not have it.
pub const CAMINITO: &str = "org.mate.Caminito";

/// A schema Caminito reads.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Schema {
    pub id: &'static str,
    /// Marco's schemas must be installed (09). Without another, its keys are defaults.
    pub required: bool,
}

/// Every schema Caminito reads. Each is read whole, so an unsupported key needs only its
/// schema here.
pub const SCHEMAS: &[Schema] = &[
    Schema {
        id: MARCO_GENERAL,
        required: true,
    },
    Schema {
        id: MARCO_WORKSPACE_NAMES,
        required: true,
    },
    Schema {
        id: MARCO_KEYBINDING_COMMANDS,
        required: true,
    },
    Schema {
        id: MARCO_GLOBAL_KEYBINDINGS,
        required: true,
    },
    Schema {
        id: MARCO_WINDOW_KEYBINDINGS,
        required: true,
    },
    Schema {
        id: TERMINAL,
        required: false,
    },
    Schema {
        id: INTERFACE,
        required: false,
    },
    Schema {
        id: FONT_RENDERING,
        required: false,
    },
    Schema {
        id: MOUSE,
        required: false,
    },
    Schema {
        id: SOUND,
        required: false,
    },
    Schema {
        id: TOUCHPAD,
        required: false,
    },
    Schema {
        id: KEYBOARD,
        required: false,
    },
    Schema {
        id: KEYBOARD_XKB,
        required: false,
    },
    Schema {
        id: A11Y_KEYBOARD,
        required: false,
    },
    Schema {
        id: LOCKDOWN,
        required: false,
    },
    Schema {
        id: SESSION,
        required: false,
    },
    Schema {
        id: SCREENSAVER,
        required: false,
    },
    Schema {
        id: POWER_MANAGER,
        required: false,
    },
    Schema {
        id: CAMINITO,
        required: false,
    },
];

/// Keys Caminito doesn't support yet, whose defaults it doesn't match (09). Each is logged
/// once per run, and only when set away from its default. Entries arrive with the
/// milestones that read their neighbours.
///
/// Keys that are *never* supported aren't here: they're ignored without a word (09, 53).
/// Of the input schemas, those are `motion-threshold` and `tap-button-*-finger`, which msd
/// uses only for non-libinput drivers, and the keyboard's `bell-mode`, `click`,
/// `click-volume`, `bell-pitch`, `bell-duration` and `bell-custom-file`.
///
/// `org.mate.peripherals-keyboard-xkb.general`'s `group-per-window` and `default-group` are
/// unsupported, as v0.1 has one global group, but they're documented rather than logged
/// (09, 54). Caminito behaves as if `group-per-window` were off, so a log line would reach
/// only the users who turned it off, whom Caminito already matches.
pub const UNSUPPORTED: &[(&str, &str)] = &[
    // Caminito will draw the ripple itself (53); 147 put it on the later roadmap.
    (MOUSE, "locate-pointer"),
    // M8's keys a user can set to something Caminito doesn't do yet (147), each with the
    // ticket that honours it. Caminito does what each one's default asks, except
    // `resize-with-right-button`, whose values it matches none of. A key whose other value is
    // what Caminito already does (`audible-bell`, `reduced-resources`) isn't here. Master's
    // keys are logged only where the installed schema has them. The keybindings' schemas have
    // no key here: an action with no behaviour yet is read and binds nothing, and its
    // default reaches the client (148).
    // 162.
    (MARCO_GENERAL, "mouse-button-modifier"),
    (MARCO_GENERAL, "resize-with-right-button"),
    (MARCO_GENERAL, "action-scroll-up-titlebar"),
    (MARCO_GENERAL, "action-scroll-down-titlebar"),
    // Mouse keys are later (13): they need pointer emulation with msd's acceleration curve.
    (A11Y_KEYBOARD, "mousekeys-enable"),
    // 13 had these in v0.1, and the AccessX filter doesn't apply them yet (134); 147 put
    // them on the later roadmap.
    // `timeout` only matters with `timeout-enable` on, so it isn't logged.
    (A11Y_KEYBOARD, "timeout-enable"),
    (A11Y_KEYBOARD, "capslock-beep-enable"),
    (A11Y_KEYBOARD, "feature-state-change-beep"),
];

/// Marco's schemas that aren't installed, which stop Caminito starting (09).
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct MissingSchemas(pub Vec<&'static str>);

impl fmt::Display for MissingSchemas {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(
            f,
            "Marco's GSettings schemas {} aren't installed; Caminito reads Marco's settings, \
             so install marco",
            self.0.join(", ")
        )
    }
}

impl std::error::Error for MissingSchemas {}

/// Checks that every required schema is installed.
pub fn check_schemas(installed: impl Fn(&str) -> bool) -> Result<(), MissingSchemas> {
    let missing: Vec<&'static str> = SCHEMAS
        .iter()
        .filter(|schema| schema.required && !installed(schema.id))
        .map(|schema| schema.id)
        .collect();
    if missing.is_empty() {
        Ok(())
    } else {
        Err(MissingSchemas(missing))
    }
}

/// A GSettings value, as far as Caminito reads it. Enums are their nicks.
#[derive(Debug, Clone, PartialEq)]
pub enum Value {
    Bool(bool),
    Int(i32),
    Uint(u32),
    Int64(i64),
    Double(f64),
    Str(String),
    StrList(Vec<String>),
    /// A maybe pair of doubles, `m(dd)`.
    MaybePair(Option<(f64, f64)>),
    /// Any other type, printed as GVariant text.
    Other(String),
}

impl fmt::Display for Value {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Value::Bool(b) => write!(f, "{b}"),
            Value::Int(i) => write!(f, "{i}"),
            Value::Uint(u) => write!(f, "{u}"),
            Value::Int64(i) => write!(f, "{i}"),
            Value::Double(d) => write!(f, "{d}"),
            Value::Str(s) => write!(f, "'{s}'"),
            Value::StrList(list) => write!(f, "{list:?}"),
            Value::MaybePair(None) => f.write_str("nothing"),
            Value::MaybePair(Some((a, b))) => write!(f, "({a}, {b})"),
            Value::Other(text) => f.write_str(text),
        }
    }
}

/// One key: its value, and its schema default.
#[derive(Debug, Clone, PartialEq)]
pub struct Entry {
    pub value: Value,
    pub default: Value,
}

/// Every key of the schemas Caminito found, as GSettings has them.
#[derive(Debug, Clone, Default, PartialEq)]
pub struct Reading {
    entries: BTreeMap<(String, String), Entry>,
    /// Optional schemas that aren't installed.
    missing: Vec<&'static str>,
    /// The keymap the system names, for the parts the settings leave empty.
    system_keymap: Keymap,
}

impl Reading {
    pub fn insert(&mut self, schema: &str, key: &str, value: Value, default: Value) {
        self.entries
            .insert((schema.into(), key.into()), Entry { value, default });
    }

    /// Notes that an optional schema isn't installed.
    pub fn set_missing(&mut self, schema: &'static str) {
        self.missing.push(schema);
    }

    /// Sets the keymap the system names: `XKB_DEFAULT_*`, then localed's X11 keymap.
    pub fn set_system_keymap(&mut self, keymap: Keymap) {
        self.system_keymap = keymap;
    }

    #[cfg(test)]
    fn remove(&mut self, schema: &str, key: &str) {
        self.entries.remove(&(schema.into(), key.into()));
    }

    fn get(&self, schema: &str, key: &str) -> Option<&Entry> {
        self.entries.get(&(schema.into(), key.into()))
    }

    /// The keys of `schema` whose values are strings, with their values.
    fn strings_of<'a>(&'a self, schema: &'a str) -> impl Iterator<Item = (&'a str, &'a str)> {
        self.entries
            .iter()
            .filter(move |((id, _), _)| id == schema)
            .filter_map(|((_, key), entry)| match &entry.value {
                Value::Str(value) => Some((key.as_str(), value.as_str())),
                _ => None,
            })
    }
}

/// Something wrong with the settings, logged once (09).
#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub enum Problem {
    /// A value outside its range, clamped.
    OutOfRange {
        schema: String,
        key: String,
        value: String,
        used: String,
    },
    /// A value Caminito can't use, replaced by the schema default.
    Invalid {
        schema: String,
        key: String,
        value: String,
        used: String,
    },
    /// A key Caminito doesn't support, set away from its default.
    Unsupported {
        schema: String,
        key: String,
        value: String,
    },
    /// A keybinding that isn't a combination Caminito can bind, which binds nothing
    /// (148), and why.
    Unbindable {
        schema: String,
        key: String,
        value: String,
        reason: String,
    },
    /// An optional schema that isn't installed, so its keys are defaults.
    MissingSchema(String),
}

impl fmt::Display for Problem {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Problem::OutOfRange {
                schema,
                key,
                value,
                used,
            } => write!(f, "{schema} {key} is {value}, out of range; using {used}"),
            Problem::Invalid {
                schema,
                key,
                value,
                used,
            } => write!(
                f,
                "{schema} {key} is {value}, which isn't valid; using {used}"
            ),
            Problem::Unsupported { schema, key, value } => write!(
                f,
                "{schema} {key} is {value}, but Caminito doesn't support it yet; ignoring it"
            ),
            Problem::Unbindable {
                schema,
                key,
                value,
                reason,
            } => write!(
                f,
                "{schema} {key} is {value}, which isn't a key combination Caminito can bind: \
                 {reason}; binding nothing"
            ),
            Problem::MissingSchema(schema) => {
                write!(f, "the {schema} schema isn't installed; using its defaults")
            }
        }
    }
}

/// The problems logged so far, so each is logged once per run.
#[derive(Debug, Default)]
pub struct Logged(HashSet<Problem>);

impl Logged {
    /// The problems not logged before, which are then counted as logged. An unsupported
    /// key is logged once whatever values it's later set to.
    pub fn first_time(&mut self, problems: Vec<Problem>) -> Vec<Problem> {
        problems
            .into_iter()
            .filter(|problem| {
                let identity = match problem {
                    Problem::Unsupported { schema, key, .. } => Problem::Unsupported {
                        schema: schema.clone(),
                        key: key.clone(),
                        value: String::new(),
                    },
                    other => other.clone(),
                };
                self.0.insert(identity)
            })
            .collect()
    }
}

/// Caminito's settings, typed.
#[derive(Debug, Clone, PartialEq)]
pub struct Settings {
    /// Policy's settings: the workspace count and names, and the grid they're in.
    pub policy: Config,
    /// The keybindings in force (148).
    pub bindings: Bindings,
    pub commands: Commands,
    pub frames: Frames,
    pub switcher: SwitcherSettings,
    pub interface: Interface,
    pub font_rendering: FontRendering,
    pub mouse: Mouse,
    pub sound: Sound,
    pub bell: Bell,
    pub devices: Devices,
    pub keyboard: Keyboard,
    pub accessx: AccessX,
    pub keymap: Keymap,
    pub locker: LockerSettings,
    pub idle: IdleSettings,
    pub night_light: NightLightSettings,
}

/// Night Light (ADR 0023), from `org.mate.Caminito`'s `night-light-*` keys.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct NightLightSettings {
    /// `night-light-enabled`.
    pub enabled: bool,
    /// `night-light-schedule`.
    pub schedule: Schedule,
    /// `night-light-noon-temperature`, in kelvin, 1000 to 6500.
    pub noon_temperature: u32,
    /// `night-light-night-temperature`, in kelvin, 1000 to 6500.
    pub night_temperature: u32,
    /// `night-light-constant-temperature`, in kelvin, 1000 to 6500.
    pub constant_temperature: u32,
    /// `night-light-paused-until`: the Unix time a pause ends, or 0 when there's none.
    pub paused_until: i64,
    /// `night-light-last-location`: the latitude and longitude geoclue last gave.
    pub last_location: Option<(f64, f64)>,
}

/// Night Light's schedule (`org.mate.Caminito.NightLightSchedule`).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Schedule {
    /// "Follow the sun": the temperature follows the sun's elevation.
    Sun,
    /// One temperature all the time.
    Constant,
}

impl Schedule {
    /// The schedule's nick, as the schema and the state dump name it.
    pub fn name(self) -> &'static str {
        self.nick()
    }
}

/// The screen locker (12, 126).
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct LockerSettings {
    /// `org.mate.Caminito locker-command`: the locker's argv, run without a shell. It may
    /// be empty, which is a failed start.
    pub command: Vec<String>,
    /// `org.mate.lockdown disable-lock-screen`: no lock request is accepted.
    pub lock_disabled: bool,
}

/// Idle, the blank it brings and the lock after it (09, 128).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct IdleSettings {
    /// `org.mate.session idle-delay`: how long the session goes without activity before
    /// it's idle. Zero is never, as in mate-session.
    pub delay: Duration,
    /// `org.mate.screensaver idle-activation-enabled`: whether idle blanks the outputs.
    pub blank: bool,
    /// `org.mate.screensaver lock-delay`, while `lock-enabled` is on: how long after the
    /// blank starts the session is locked.
    pub lock: Option<Duration>,
    /// When the outputs are powered off (129).
    pub sleep_display: SleepDisplay,
}

/// How long the session goes without activity before the outputs are powered off, on each
/// power source (09, 129). Zero is never, as in mate-power-manager. UPower's `OnBattery`
/// picks one; a UPS counts as AC, so `sleep-display-ups` is never read.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct SleepDisplay {
    /// `org.mate.power-manager sleep-display-ac`.
    pub ac: Duration,
    /// `org.mate.power-manager sleep-display-battery`.
    pub battery: Duration,
}

/// What the Window switcher is drawn with (108), from `org.mate.Marco.general`.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct SwitcherSettings {
    /// `alt-tab-max-columns`, 1 to 64.
    pub max_columns: usize,
    /// Whether cells show thumbnails: `compositing-manager` on and
    /// `compositing-fast-alt-tab` off. Otherwise they show icons alone.
    pub thumbnails: bool,
    /// `icon-size`, 8 to 256: the icons of the icons-only grid, in logical pixels.
    pub icon_size: i32,
}

/// The command lines Marco's `run-command-*` bindings run, from
/// `org.mate.Marco.keybinding-commands` (104, 149), and the terminal
/// `run-command-terminal` runs.
#[derive(Debug, Clone, PartialEq)]
pub struct Commands {
    /// `command-screenshot`.
    pub screenshot: String,
    /// `command-window-screenshot`.
    pub window_screenshot: String,
    /// `command-area-screenshot`. Marco 1.29.0's schema has no key for it, so it's often
    /// the default of the Marco that has (`org.mate.marco.gschema.xml:780-781` at `8522ed1`).
    pub area_screenshot: String,
    /// `command-1` to `-12`, from the first.
    pub numbered: Vec<String>,
    /// `org.mate.applications-terminal exec`, as Marco's `meta_prefs_get_terminal_command`.
    pub terminal: String,
}

/// What SSD frames are drawn with and do (05, 07).
#[derive(Debug, Clone, PartialEq)]
pub struct Frames {
    /// The Marco theme's name.
    pub theme: String,
    /// The titlebar's Pango font description: `titlebar-font`, or `org.mate.interface
    /// font-name` when `titlebar-uses-system-font` is on (09).
    pub titlebar_font: String,
    pub button_layout: ButtonLayout,
    pub double_click: TitlebarAction,
    pub middle_click: TitlebarAction,
    pub right_click: TitlebarAction,
    /// Marco reads no key for this, so it's off unless a schema has one.
    pub attach_modal_dialogs: bool,
    /// Whether frames have shadows: `compositing-manager`, which is all that turns Marco's
    /// off (171).
    pub shadows: bool,
}

/// A titlebar click's action (`org.mate.Marco.ActionTitlebar`).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum TitlebarAction {
    Close,
    Minimize,
    ToggleMaximize,
    ToggleMaximizeHorizontally,
    ToggleMaximizeVertically,
    ToggleShade,
    Shade,
    Unshade,
    Raise,
    Lower,
    ToggleStick,
    ToggleAbove,
    Menu,
    None,
}

/// An enum's nicks, as its schema names them.
trait Nick: Copy + PartialEq + 'static {
    const NICKS: &'static [(&'static str, Self)];

    fn from_nick(nick: &str) -> Option<Self> {
        Self::NICKS
            .iter()
            .find(|(n, _)| *n == nick)
            .map(|(_, v)| *v)
    }

    fn nick(self) -> &'static str {
        Self::NICKS.iter().find(|(_, v)| *v == self).unwrap().0
    }
}

impl Nick for TitlebarAction {
    // `last`, the enum's end marker, isn't an action.
    const NICKS: &'static [(&'static str, Self)] = &[
        ("close", Self::Close),
        ("minimize", Self::Minimize),
        ("toggle_maximize", Self::ToggleMaximize),
        (
            "toggle_maximize_horizontally",
            Self::ToggleMaximizeHorizontally,
        ),
        ("toggle_maximize_vertically", Self::ToggleMaximizeVertically),
        ("toggle_shade", Self::ToggleShade),
        ("shade", Self::Shade),
        ("unshade", Self::Unshade),
        ("raise", Self::Raise),
        ("lower", Self::Lower),
        ("toggle_stick", Self::ToggleStick),
        ("toggle_above", Self::ToggleAbove),
        ("menu", Self::Menu),
        ("none", Self::None),
    ];
}

/// `org.mate.Caminito.NightLightSchedule`.
impl Nick for Schedule {
    const NICKS: &'static [(&'static str, Self)] =
        &[("sun", Self::Sun), ("constant", Self::Constant)];
}

/// `org.mate.Marco.FocusMode`.
impl Nick for FocusMode {
    const NICKS: &'static [(&'static str, Self)] = &[
        ("click", Self::Click),
        ("sloppy", Self::Sloppy),
        ("mouse", Self::Mouse),
    ];
}

/// `org.mate.Marco.FocusNewWindows`.
impl Nick for FocusNewWindows {
    const NICKS: &'static [(&'static str, Self)] =
        &[("smart", Self::Smart), ("strict", Self::Strict)];
}

/// Marco master's `org.mate.Marco.UrgentWindowPlacement`.
impl Nick for UrgentPlacement {
    const NICKS: &'static [(&'static str, Self)] = &[("start", Self::Start), ("mru", Self::Mru)];
}

/// `org.mate.Marco.placement_type`.
impl Nick for PlacementMode {
    const NICKS: &'static [(&'static str, Self)] = &[
        ("automatic", Self::Automatic),
        ("pointer", Self::Pointer),
        ("manual", Self::Manual),
    ];
}

/// Marco master's `org.mate.Marco.MinimizedWindowPlacement`.
impl Nick for MinimizedPlacement {
    const NICKS: &'static [(&'static str, Self)] = &[
        ("mru", Self::Mru),
        ("end", Self::End),
        ("hidden", Self::Hidden),
    ];
}

/// `org.mate.Marco.WrapStyle`.
impl Nick for WrapStyle {
    const NICKS: &'static [(&'static str, Self)] = &[
        ("no wrap", Self::NoWrap),
        ("classic", Self::Classic),
        ("toroidal", Self::Toroidal),
    ];
}

/// `org.mate.Caminito.WorkspaceLayoutOrientation`.
impl Nick for Orientation {
    const NICKS: &'static [(&'static str, Self)] = &[
        ("horizontal", Self::Horizontal),
        ("vertical", Self::Vertical),
    ];
}

impl Nick for Antialiasing {
    const NICKS: &'static [(&'static str, Self)] = &[
        ("none", Self::None),
        ("grayscale", Self::Grayscale),
        ("rgba", Self::Rgba),
    ];
}

impl Nick for Hinting {
    const NICKS: &'static [(&'static str, Self)] = &[
        ("none", Self::None),
        ("slight", Self::Slight),
        ("medium", Self::Medium),
        ("full", Self::Full),
    ];
}

impl Nick for RgbaOrder {
    const NICKS: &'static [(&'static str, Self)] = &[
        ("rgb", Self::Rgb),
        ("bgr", Self::Bgr),
        ("vrgb", Self::Vrgb),
        ("vbgr", Self::Vbgr),
    ];
}

impl Nick for AccelProfile {
    const NICKS: &'static [(&'static str, Self)] = &[
        ("default", Self::Default),
        ("adaptive", Self::Adaptive),
        ("flat", Self::Flat),
    ];
}

/// `org.mate.peripherals-touchpad.Handedness`.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum Handedness {
    Right,
    Left,
    /// Whatever the mouse's `left-handed` says.
    Mouse,
}

impl Nick for Handedness {
    const NICKS: &'static [(&'static str, Self)] = &[
        ("right", Self::Right),
        ("left", Self::Left),
        ("mouse", Self::Mouse),
    ];
}

/// `org.mate.peripherals-keyboard.NumLockState`.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum NumLockState {
    Off,
    On,
    Unknown,
}

impl Nick for NumLockState {
    const NICKS: &'static [(&'static str, Self)] = &[
        ("off", Self::Off),
        ("on", Self::On),
        ("unknown", Self::Unknown),
    ];
}

/// `org.mate.interface`.
#[derive(Debug, Clone, PartialEq)]
pub struct Interface {
    pub gtk_theme: String,
    pub font_name: String,
    pub icon_theme: String,
    pub gtk: GtkOptions,
    pub window_scaling_factor: WindowScalingFactor,
}

/// `window-scaling-factor` under Caminito (13): which default scale outputs get. On X11 it
/// was GTK's own scale, but here it forces the output scale instead.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum WindowScalingFactor {
    /// 0: KWin's rule picks each output's scale.
    Auto,
    /// 1 or 2: every output gets that integer.
    Forced(u32),
}

/// The `org.mate.interface` keys Caminito only passes on to X11 GTK apps through XSETTINGS,
/// as they are (40). Each is its key without a `gtk-` prefix.
#[derive(Debug, Clone, PartialEq)]
pub struct GtkOptions {
    pub key_theme: String,
    pub toolbar_style: String,
    pub toolbar_icons_size: String,
    pub cursor_blink: bool,
    pub cursor_blink_time: i32,
    pub color_scheme: String,
    pub im_preedit_style: String,
    pub im_status_style: String,
    pub im_module: String,
    pub file_chooser_backend: String,
    pub decoration_layout: String,
    pub shell_shows_app_menu: bool,
    pub shell_shows_menubar: bool,
    pub menus_have_icons: bool,
    pub buttons_have_icons: bool,
    pub menubar_accel: String,
    pub show_input_method_menu: bool,
    pub show_unicode_menu: bool,
    pub automatic_mnemonics: bool,
    pub enable_primary_paste: bool,
    pub enable_animations: bool,
    pub dialogs_use_header: bool,
}

/// `org.mate.font-rendering`.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct FontRendering {
    pub antialiasing: Antialiasing,
    pub hinting: Hinting,
    pub rgba_order: RgbaOrder,
    /// As the key has it: 0 and below mean "not set".
    pub dpi: f64,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Antialiasing {
    None,
    Grayscale,
    Rgba,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Hinting {
    None,
    Slight,
    Medium,
    Full,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum RgbaOrder {
    Rgb,
    Bgr,
    Vrgb,
    Vbgr,
}

/// The `org.mate.peripherals-mouse` keys Caminito itself uses (09).
#[derive(Debug, Clone, PartialEq)]
pub struct Mouse {
    /// Empty for the default theme.
    pub cursor_theme: String,
    pub cursor_size: u32,
    /// The longest time between a double-click's clicks.
    pub double_click_time: Duration,
    /// How far, in logical pixels, a press must move to become a drag.
    pub drag_threshold: u32,
}

/// `org.mate.sound`.
#[derive(Debug, Clone, PartialEq)]
pub struct Sound {
    pub theme_name: String,
    pub event_sounds: bool,
    pub input_feedback_sounds: bool,
}

/// Marco's bell keys, from `org.mate.Marco.general` (09). The bell event reads them (105),
/// and flashes by the visual bell's (169).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Bell {
    /// `audible-bell`: whether a bell plays the sound theme's bell.
    pub audible: bool,
    /// `visual-bell`: whether a bell flashes.
    pub visual: bool,
    /// `visual-bell-type`: what flashes.
    pub visual_type: VisualBellType,
}

/// `org.mate.Marco.VisualBellType`.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum VisualBellType {
    /// Every output.
    Fullscreen,
    /// The window's frame, or an output where there's none.
    FrameFlash,
}

impl Nick for VisualBellType {
    const NICKS: &'static [(&'static str, Self)] = &[
        ("fullscreen", Self::Fullscreen),
        ("frame_flash", Self::FrameFlash),
    ];
}

/// What msd's mouse plugin sets on each kind of pointer under X11, which Caminito sets on
/// libinput devices instead (09, 53).
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct Devices {
    /// Pointers that aren't touchpads.
    pub mouse: DeviceSettings,
    /// Pointers libinput can tap on.
    pub touchpad: DeviceSettings,
}

/// One kind of device's settings. A `None` is one msd doesn't set on this kind, so the
/// device keeps its own.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct DeviceSettings {
    /// Whether the device sends events at all.
    pub send_events: Option<bool>,
    /// Whether the primary and secondary buttons are swapped.
    pub left_handed: bool,
    /// libinput's pointer speed, in [−1, 1], or `None` for the device's default.
    pub accel_speed: Option<f64>,
    /// How the pointer speeds up with the device's motion.
    pub accel_profile: AccelProfile,
    /// Whether scrolling is reversed.
    pub natural_scroll: bool,
    /// Whether pressing left and right together is a middle click.
    pub middle_emulation: Option<bool>,
    /// Whether a tap is a click.
    pub tap: Option<bool>,
    /// Whether the device ignores touches while the keyboard is typed on.
    pub disable_while_typing: Option<bool>,
    /// How clicks tell the buttons apart.
    pub click_method: Option<ClickMethod>,
    /// How the device scrolls.
    pub scroll_method: Option<ScrollMethod>,
    /// Whether the device's horizontal scrolling reaches clients. libinput has no switch
    /// for it, so Caminito drops the device's horizontal axis events itself.
    pub horizontal_scroll: bool,
}

/// libinput's pointer acceleration profile.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum AccelProfile {
    /// The device's own default profile.
    Default,
    /// Faster motion moves the pointer further.
    Adaptive,
    /// Motion is scaled by one factor.
    Flat,
}

/// How a touchpad without physical buttons tells the buttons apart.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ClickMethod {
    /// By where on the pad the click is.
    ButtonAreas,
    /// By how many fingers are down.
    Clickfinger,
}

/// How a touchpad scrolls.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ScrollMethod {
    /// It doesn't.
    NoScroll,
    /// By moving two fingers.
    TwoFinger,
    /// By moving along the pad's edge.
    Edge,
}

/// `org.mate.peripherals-keyboard`, for the seat's keyboard on every backend (09, 53).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Keyboard {
    /// Repeats per second, where 0 turns repeat off, as `wl_keyboard.repeat_info` has it.
    pub repeat_rate: i32,
    /// Milliseconds before a held key repeats, at least 1.
    pub repeat_delay: i32,
    /// Whether NumLock is restored at start-up and its state written back on each change.
    pub remember_numlock: bool,
    /// NumLock's remembered state, or `None` when it's `unknown`.
    pub numlock: Option<bool>,
}

/// `org.mate.accessibility-keyboard`: AccessX, which Caminito's keyboard filter applies as
/// the X server does under X11 (13, 134). `togglekeys-backend` isn't read: both of its
/// values mean Caminito's filter. `mousekeys-*` is read by nothing (13).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct AccessX {
    /// `enable`: whether the keyboard gestures turn sticky and slow keys on and off.
    pub gestures: bool,
    /// `stickykeys-enable`.
    pub sticky_keys: bool,
    /// `stickykeys-latch-to-lock`: whether a second press of a latched modifier locks it.
    pub sticky_latch_to_lock: bool,
    /// `stickykeys-two-key-off`: whether two keys pressed at once turn sticky keys off.
    pub sticky_two_key_off: bool,
    /// `stickykeys-modifier-beep`: whether a modifier's press beeps.
    pub sticky_modifier_beep: bool,
    /// `slowkeys-enable`.
    pub slow_keys: bool,
    /// `slowkeys-delay`: how long a key is held before it counts as pressed.
    pub slow_delay: Duration,
    /// `slowkeys-beep-press`.
    pub slow_beep_press: bool,
    /// `slowkeys-beep-accept`.
    pub slow_beep_accept: bool,
    /// `slowkeys-beep-reject`.
    pub slow_beep_reject: bool,
    /// `bouncekeys-enable`.
    pub bounce_keys: bool,
    /// `bouncekeys-delay`: how long after a key's release its next press is ignored.
    pub bounce_delay: Duration,
    /// `bouncekeys-beep-reject`.
    pub bounce_beep_reject: bool,
    /// `togglekeys-enable`: whether a lock key's press beeps.
    pub toggle_keys: bool,
}

/// A keymap by its RMLVO names, for xkbcommon with rules `evdev` (09, 54). An empty part is
/// left to xkbcommon's own default.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct Keymap {
    pub model: String,
    /// Comma-joined, one per group.
    pub layouts: String,
    /// Comma-joined, one per layout, each possibly empty.
    pub variants: String,
    /// Comma-joined.
    pub options: String,
}

impl Keymap {
    /// The keymap `XKB_DEFAULT_*` names, as `lookup` reads them.
    pub fn from_environment(lookup: impl Fn(&str) -> Option<String>) -> Self {
        let var = |name| lookup(name).unwrap_or_default();
        Self {
            model: var("XKB_DEFAULT_MODEL"),
            layouts: var("XKB_DEFAULT_LAYOUT"),
            variants: var("XKB_DEFAULT_VARIANT"),
            options: var("XKB_DEFAULT_OPTIONS"),
        }
    }

    /// This keymap, with each empty part taken from `fallback`: the model, the layouts and
    /// the options each on their own, as libmatekbd merges its defaults. Variants belong to
    /// their layouts, so they come from wherever the layouts do, as in xkbcommon.
    pub fn or(self, fallback: &Keymap) -> Self {
        let or = |part: String, fallback: &String| {
            if part.is_empty() {
                fallback.clone()
            } else {
                part
            }
        };
        let (layouts, variants) = if self.layouts.is_empty() {
            (fallback.layouts.clone(), fallback.variants.clone())
        } else {
            (self.layouts, self.variants)
        };
        Self {
            model: or(self.model, &fallback.model),
            layouts,
            variants,
            options: or(self.options, &fallback.options),
        }
    }
}

/// Settings from a reading, and what was wrong with it.
#[derive(Debug, Clone, PartialEq)]
pub struct Translation {
    pub settings: Settings,
    pub problems: Vec<Problem>,
}

/// Translates a reading with 09's rules.
pub fn translate(reading: &Reading) -> Translation {
    translate_with(reading, UNSUPPORTED)
}

fn translate_with(reading: &Reading, unsupported: &[(&str, &str)]) -> Translation {
    let mut r = Reader {
        reading,
        problems: Vec::new(),
    };

    let workspace_count =
        r.int_clamped(MARCO_GENERAL, "num-workspaces", 4, 1, MAX_WORKSPACES as i32) as usize;
    // Marco master's key, off where the schema has no key for it (151). With it on the count
    // is Caminito's, so every name is passed on.
    let dynamic_workspaces = r.bool(MARCO_GENERAL, "dynamic-workspaces", false);
    let named = if dynamic_workspaces {
        MAX_WORKSPACES
    } else {
        workspace_count
    };
    let workspace_names = (1..=named)
        .map(|n| {
            let name = r.string(MARCO_WORKSPACE_NAMES, &format!("name-{n}"), "");
            // Marco treats an empty name as unset.
            if name.is_empty() {
                default_workspace_name(n)
            } else {
                name
            }
        })
        .collect();
    // A grid has no more rows or columns than there can be workspaces.
    let most = MAX_WORKSPACES as i32;
    let workspace_layout = WorkspaceLayout {
        orientation: r.nick(
            CAMINITO,
            "workspace-layout-orientation",
            Orientation::Horizontal,
        ),
        rows: r.int_clamped(CAMINITO, "workspace-layout-rows", 1, 0, most) as usize,
        columns: r.int_clamped(CAMINITO, "workspace-layout-columns", 0, 0, most) as usize,
    };
    // Frame drags and tiling's zones both go by it (158).
    let drag_threshold = r.int_at_least(MOUSE, "drag-threshold", 8, 0);
    let policy = Config {
        workspace_count,
        workspace_names,
        dynamic_workspaces,
        workspace_layout,
        wrap_style: r.nick(MARCO_GENERAL, "wrap-style", WrapStyle::NoWrap),
        alt_tab_raise_windows: r.bool(MARCO_GENERAL, "alt-tab-raise-windows", false),
        // Marco master's key, with its default where the schema has no key for it (163).
        alt_tab_minimized_placement: r.nick(
            MARCO_GENERAL,
            "alt-tab-minimized-placement",
            MinimizedPlacement::End,
        ),
        focus_mode: r.nick(MARCO_GENERAL, "focus-mode", FocusMode::Click),
        raise_on_click: r.bool(MARCO_GENERAL, "raise-on-click", true),
        auto_raise: r.bool(MARCO_GENERAL, "auto-raise", false),
        // Marco's bounds on the delay (prefs.c:543-549).
        auto_raise_delay: Duration::from_millis(r.int_clamped(
            MARCO_GENERAL,
            "auto-raise-delay",
            500,
            0,
            10_000,
        ) as u64),
        placement_mode: r.nick(MARCO_GENERAL, "placement-mode", PlacementMode::Automatic),
        center_new_windows: r.bool(MARCO_GENERAL, "center-new-windows", true),
        show_desktop_skip_list: ShowDesktopSkipList::parse(&r.string(
            MARCO_GENERAL,
            "show-desktop-skip-list",
            "",
        )),
        allow_tiling: r.bool(MARCO_GENERAL, "allow-tiling", true),
        allow_top_tiling: r.bool(MARCO_GENERAL, "allow-top-tiling", true),
        allow_tile_cycling: r.bool(MARCO_GENERAL, "allow-tile-cycling", true),
        drag_threshold,
        focus_new_windows: r.nick(MARCO_GENERAL, "focus-new-windows", FocusNewWindows::Smart),
        // Marco master's key, whose schema default is `start`. Marco 1.29.0 has none, and an
        // urgent window keeps its place there, as 108 has it (165).
        urgent_placement: r.nick(
            MARCO_GENERAL,
            "alt-tab-urgent-placement",
            UrgentPlacement::Mru,
        ),
    };
    let switcher = SwitcherSettings {
        max_columns: r.int_clamped(MARCO_GENERAL, "alt-tab-max-columns", 5, 1, 64) as usize,
        thumbnails: r.bool(MARCO_GENERAL, "compositing-manager", true)
            && !r.bool(MARCO_GENERAL, "compositing-fast-alt-tab", false),
        icon_size: r.int_clamped(MARCO_GENERAL, "icon-size", 48, 8, 256),
    };

    let font_name = r.non_empty(INTERFACE, "font-name", "Sans 10");
    let titlebar_font = r.non_empty(MARCO_GENERAL, "titlebar-font", "Sans Bold 10");
    let frames = Frames {
        theme: r.non_empty(MARCO_GENERAL, "theme", "Menta"),
        titlebar_font: if r.bool(MARCO_GENERAL, "titlebar-uses-system-font", false) {
            font_name.clone()
        } else {
            titlebar_font
        },
        button_layout: ButtonLayout::parse(&r.string(
            MARCO_GENERAL,
            "button-layout",
            "menu:minimize,maximize,close",
        )),
        double_click: r.nick(
            MARCO_GENERAL,
            "action-double-click-titlebar",
            TitlebarAction::ToggleMaximize,
        ),
        middle_click: r.nick(
            MARCO_GENERAL,
            "action-middle-click-titlebar",
            TitlebarAction::Lower,
        ),
        right_click: r.nick(
            MARCO_GENERAL,
            "action-right-click-titlebar",
            TitlebarAction::Menu,
        ),
        attach_modal_dialogs: r.bool(MARCO_GENERAL, "attach-modal-dialogs", false),
        shadows: r.bool(MARCO_GENERAL, "compositing-manager", true),
    };

    // Every key of both schemas, so every value is parsed (148).
    let keys = [MARCO_GLOBAL_KEYBINDINGS, MARCO_WINDOW_KEYBINDINGS]
        .into_iter()
        .flat_map(|schema| {
            reading
                .strings_of(schema)
                .map(move |(key, value)| (schema, key, value))
        });
    let (bindings, problems) = Bindings::from_keys(keys);
    r.problems.extend(problems);

    let commands = Commands {
        screenshot: r.string(
            MARCO_KEYBINDING_COMMANDS,
            "command-screenshot",
            "mate-screenshot",
        ),
        window_screenshot: r.string(
            MARCO_KEYBINDING_COMMANDS,
            "command-window-screenshot",
            "mate-screenshot --window",
        ),
        area_screenshot: r.string(
            MARCO_KEYBINDING_COMMANDS,
            "command-area-screenshot",
            "mate-screenshot --area",
        ),
        numbered: (1..=12)
            .map(|n| r.string(MARCO_KEYBINDING_COMMANDS, &format!("command-{n}"), ""))
            .collect(),
        terminal: r.string(TERMINAL, "exec", "mate-terminal"),
    };

    let settings = Settings {
        policy,
        bindings,
        commands,
        frames,
        switcher,
        interface: Interface {
            gtk_theme: r.non_empty(INTERFACE, "gtk-theme", "Menta"),
            font_name,
            icon_theme: r.non_empty(INTERFACE, "icon-theme", "menta"),
            gtk: GtkOptions {
                key_theme: r.string(INTERFACE, "gtk-key-theme", "Default"),
                toolbar_style: r.string(INTERFACE, "toolbar-style", "both-horiz"),
                toolbar_icons_size: r.string(INTERFACE, "toolbar-icons-size", "large-toolbar"),
                cursor_blink: r.bool(INTERFACE, "cursor-blink", true),
                cursor_blink_time: r.int(INTERFACE, "cursor-blink-time", 1200),
                color_scheme: r.string(INTERFACE, "gtk-color-scheme", ""),
                im_preedit_style: r.string(INTERFACE, "gtk-im-preedit-style", "callback"),
                im_status_style: r.string(INTERFACE, "gtk-im-status-style", "callback"),
                im_module: r.string(INTERFACE, "gtk-im-module", ""),
                file_chooser_backend: r.string(INTERFACE, "file-chooser-backend", "gio"),
                decoration_layout: r.string(
                    INTERFACE,
                    "gtk-decoration-layout",
                    "menu:minimize,maximize,close",
                ),
                shell_shows_app_menu: r.bool(INTERFACE, "gtk-shell-shows-app-menu", false),
                shell_shows_menubar: r.bool(INTERFACE, "gtk-shell-shows-menubar", false),
                menus_have_icons: r.bool(INTERFACE, "menus-have-icons", true),
                buttons_have_icons: r.bool(INTERFACE, "buttons-have-icons", true),
                menubar_accel: r.string(INTERFACE, "menubar-accel", "F10"),
                show_input_method_menu: r.bool(INTERFACE, "show-input-method-menu", true),
                show_unicode_menu: r.bool(INTERFACE, "show-unicode-menu", true),
                automatic_mnemonics: r.bool(INTERFACE, "automatic-mnemonics", true),
                enable_primary_paste: r.bool(INTERFACE, "gtk-enable-primary-paste", true),
                enable_animations: r.bool(INTERFACE, "gtk-enable-animations", true),
                dialogs_use_header: r.bool(INTERFACE, "gtk-dialogs-use-header", false),
            },
            window_scaling_factor: match r.int_clamped(INTERFACE, "window-scaling-factor", 0, 0, 2)
            {
                0 => WindowScalingFactor::Auto,
                factor => WindowScalingFactor::Forced(factor as u32),
            },
        },
        font_rendering: FontRendering {
            antialiasing: r.nick(FONT_RENDERING, "antialiasing", Antialiasing::Rgba),
            hinting: r.nick(FONT_RENDERING, "hinting", Hinting::Slight),
            rgba_order: r.nick(FONT_RENDERING, "rgba-order", RgbaOrder::Rgb),
            dpi: r.double(FONT_RENDERING, "dpi", 0.0),
        },
        mouse: Mouse {
            // Empty means the default cursor theme.
            cursor_theme: r.string(MOUSE, "cursor-theme", ""),
            cursor_size: r.int_at_least(MOUSE, "cursor-size", 24, 1) as u32,
            double_click_time: Duration::from_millis(
                r.int_at_least(MOUSE, "double-click", 400, 1) as u64
            ),
            drag_threshold: drag_threshold as u32,
        },
        sound: Sound {
            theme_name: r.string(SOUND, "theme-name", "freedesktop"),
            event_sounds: r.bool(SOUND, "event-sounds", false),
            input_feedback_sounds: r.bool(SOUND, "input-feedback-sounds", false),
        },
        bell: Bell {
            audible: r.bool(MARCO_GENERAL, "audible-bell", true),
            visual: r.bool(MARCO_GENERAL, "visual-bell", false),
            visual_type: r.nick(
                MARCO_GENERAL,
                "visual-bell-type",
                VisualBellType::Fullscreen,
            ),
        },
        devices: devices(&mut r),
        keyboard: keyboard(&mut r),
        accessx: accessx(&mut r),
        keymap: keymap(&mut r).or(&reading.system_keymap),
        locker: LockerSettings {
            command: r.strings_or(CAMINITO, "locker-command", &["caminito-locker"]),
            lock_disabled: r.bool(LOCKDOWN, "disable-lock-screen", false),
        },
        idle: IdleSettings {
            delay: minutes(r.int_at_least(SESSION, "idle-delay", 5, 0)),
            blank: r.bool(SCREENSAVER, "idle-activation-enabled", true),
            lock: r
                .bool(SCREENSAVER, "lock-enabled", true)
                .then(|| minutes(r.int_at_least(SCREENSAVER, "lock-delay", 0, 0))),
            sleep_display: SleepDisplay {
                ac: seconds(r.int_at_least(POWER_MANAGER, "sleep-display-ac", 1800, 0)),
                battery: seconds(r.int_at_least(POWER_MANAGER, "sleep-display-battery", 600, 0)),
            },
        },
        night_light: night_light(&mut r),
    };
    for schema in &reading.missing {
        r.problems.push(Problem::MissingSchema((*schema).into()));
    }
    for (schema, key) in unsupported {
        if let Some(entry) = reading.get(schema, key)
            && entry.value != entry.default
        {
            r.problems.push(Problem::Unsupported {
                schema: (*schema).into(),
                key: (*key).into(),
                value: entry.value.to_string(),
            });
        }
    }
    Translation {
        settings,
        problems: r.problems,
    }
}

/// The coldest and warmest temperatures Night Light's keys take, in kelvin. 6500 is
/// neutral.
const TEMPERATURES: (u32, u32) = (1000, 6500);

fn night_light(r: &mut Reader<'_>) -> NightLightSettings {
    let (min, max) = TEMPERATURES;
    let mut temperature = |key, builtin| r.uint_clamped(CAMINITO, key, builtin, min, max);
    let noon_temperature = temperature("night-light-noon-temperature", 5500);
    let night_temperature = temperature("night-light-night-temperature", 3400);
    let constant_temperature = temperature("night-light-constant-temperature", 4000);
    NightLightSettings {
        enabled: r.bool(CAMINITO, "night-light-enabled", false),
        schedule: r.nick(CAMINITO, "night-light-schedule", Schedule::Sun),
        noon_temperature,
        night_temperature,
        constant_temperature,
        paused_until: r.int64(CAMINITO, "night-light-paused-until", 0),
        last_location: r.maybe_pair(CAMINITO, "night-light-last-location"),
    }
}

/// A key's `minutes`, which `int_at_least` has kept from being negative.
fn minutes(minutes: i32) -> Duration {
    Duration::from_secs(60 * minutes as u64)
}

/// A key's `seconds`, which `int_at_least` has kept from being negative.
fn seconds(seconds: i32) -> Duration {
    Duration::from_secs(seconds as u64)
}

/// The pointers' settings, as msd's mouse plugin applies them to libinput devices
/// (`msd-mouse-manager.c`).
fn devices(r: &mut Reader<'_>) -> Devices {
    let mouse_left_handed = r.bool(MOUSE, "left-handed", false);
    let mouse = DeviceSettings {
        send_events: None,
        left_handed: mouse_left_handed,
        accel_speed: accel_speed(r.double(MOUSE, "motion-acceleration", -1.0)),
        accel_profile: r.nick(MOUSE, "accel-profile", AccelProfile::Default),
        // Newer than MATE 1.28's schema, so often missing.
        natural_scroll: r.bool(MOUSE, "natural-scroll", false),
        // Touchpads keep their own (`msd-mouse-manager.c:851-876`).
        middle_emulation: Some(r.bool(MOUSE, "middle-button-enabled", true)),
        tap: None,
        disable_while_typing: None,
        click_method: None,
        scroll_method: None,
        horizontal_scroll: true,
    };

    // The button numbers are ignored, as msd does (`msd-mouse-manager.c:1275-1287`).
    let clickfinger = r.int(TOUCHPAD, "two-finger-click", 0) != 0
        || r.int(TOUCHPAD, "three-finger-click", 0) != 0;
    // libinput has one scroll method, and msd prefers two fingers
    // (`msd-mouse-manager.c:1520-1561`).
    let scroll_method = if r.bool(TOUCHPAD, "vertical-two-finger-scrolling", true) {
        ScrollMethod::TwoFinger
    } else if r.bool(TOUCHPAD, "vertical-edge-scrolling", true) {
        ScrollMethod::Edge
    } else {
        ScrollMethod::NoScroll
    };
    let horizontal_scroll = match scroll_method {
        ScrollMethod::TwoFinger => r.bool(TOUCHPAD, "horizontal-two-finger-scrolling", false),
        ScrollMethod::Edge => r.bool(TOUCHPAD, "horizontal-edge-scrolling", false),
        // Nothing scrolls, so there's nothing to drop.
        ScrollMethod::NoScroll => true,
    };
    let touchpad = DeviceSettings {
        send_events: Some(r.bool(TOUCHPAD, "touchpad-enabled", true)),
        left_handed: match r.nick(TOUCHPAD, "left-handed", Handedness::Mouse) {
            Handedness::Right => false,
            Handedness::Left => true,
            Handedness::Mouse => mouse_left_handed,
        },
        accel_speed: accel_speed(r.double(TOUCHPAD, "motion-acceleration", -1.0)),
        accel_profile: r.nick(TOUCHPAD, "accel-profile", AccelProfile::Default),
        natural_scroll: r.bool(TOUCHPAD, "natural-scroll", false),
        middle_emulation: None,
        tap: Some(r.bool(TOUCHPAD, "tap-to-click", false)),
        disable_while_typing: Some(r.bool(TOUCHPAD, "disable-while-typing", false)),
        click_method: Some(if clickfinger {
            ClickMethod::Clickfinger
        } else {
            ClickMethod::ButtonAreas
        }),
        scroll_method: Some(scroll_method),
        horizontal_scroll,
    };
    Devices { mouse, touchpad }
}

/// `motion-acceleration`'s 1 to 10, from the Mouse capplet, as libinput's speed
/// (`msd-mouse-manager.c:751-754`). -1 is unset, which here means the device's default
/// rather than msd's 0 (09). msd doesn't clamp, but libinput refuses a speed outside
/// [−1, 1].
fn accel_speed(acceleration: f64) -> Option<f64> {
    (acceleration != -1.0).then(|| ((acceleration - 1.0) * 2.0 / 9.0 - 1.0).clamp(-1.0, 1.0))
}

/// The keyboard's repeat and NumLock, as msd's keyboard plugin applies them
/// (`msd-keyboard-manager.c:165-265`).
fn keyboard(r: &mut Reader<'_>) -> Keyboard {
    let repeat = r.bool(KEYBOARD, "repeat", true);
    let rate = r.int(KEYBOARD, "rate", 30);
    let delay = r.int(KEYBOARD, "delay", 500);
    Keyboard {
        repeat_rate: if repeat { rate.max(0) } else { 0 },
        repeat_delay: delay.max(1),
        remember_numlock: r.bool(KEYBOARD, "remember-numlock-state", true),
        numlock: match r.nick(KEYBOARD, "numlock-state", NumLockState::Unknown) {
            NumLockState::On => Some(true),
            NumLockState::Off => Some(false),
            NumLockState::Unknown => None,
        },
    }
}

/// AccessX's keys, as msd's a11y-keyboard plugin gives them to the X server
/// (`msd-a11y-keyboard-manager.c:313-405`).
fn accessx(r: &mut Reader<'_>) -> AccessX {
    let mut millis = |key: &str| {
        Duration::from_millis(
            r.int_at_least(A11Y_KEYBOARD, key, 300, 0)
                .unsigned_abs()
                .into(),
        )
    };
    let slow_delay = millis("slowkeys-delay");
    let bounce_delay = millis("bouncekeys-delay");
    AccessX {
        gestures: r.bool(A11Y_KEYBOARD, "enable", false),
        sticky_keys: r.bool(A11Y_KEYBOARD, "stickykeys-enable", false),
        sticky_latch_to_lock: r.bool(A11Y_KEYBOARD, "stickykeys-latch-to-lock", true),
        sticky_two_key_off: r.bool(A11Y_KEYBOARD, "stickykeys-two-key-off", false),
        sticky_modifier_beep: r.bool(A11Y_KEYBOARD, "stickykeys-modifier-beep", false),
        slow_keys: r.bool(A11Y_KEYBOARD, "slowkeys-enable", false),
        slow_delay,
        slow_beep_press: r.bool(A11Y_KEYBOARD, "slowkeys-beep-press", false),
        slow_beep_accept: r.bool(A11Y_KEYBOARD, "slowkeys-beep-accept", false),
        slow_beep_reject: r.bool(A11Y_KEYBOARD, "slowkeys-beep-reject", false),
        bounce_keys: r.bool(A11Y_KEYBOARD, "bouncekeys-enable", false),
        bounce_delay,
        bounce_beep_reject: r.bool(A11Y_KEYBOARD, "bouncekeys-beep-reject", false),
        toggle_keys: r.bool(A11Y_KEYBOARD, "togglekeys-enable", false),
    }
}

/// The keymap libmatekbd's strings name, which are `layout\tvariant` and `group\toption`
/// (09).
fn keymap(r: &mut Reader<'_>) -> Keymap {
    let layouts = r.strings(KEYBOARD_XKB, "layouts");
    let (layouts, variants): (Vec<&str>, Vec<&str>) = layouts
        .iter()
        .map(|layout| layout.split_once('\t').unwrap_or((layout, "")))
        .unzip();
    let options = r.strings(KEYBOARD_XKB, "options");
    let options: Vec<&str> = options
        .iter()
        .map(|option| option.split_once('\t').map_or(option.as_str(), |(_, o)| o))
        .collect();
    Keymap {
        model: r.string(KEYBOARD_XKB, "model", ""),
        layouts: layouts.join(","),
        variants: variants.join(","),
        options: options.join(","),
    }
}

/// Reads keys from a reading, falling back to the schema default Caminito knows when a key
/// is missing, and noting each problem.
struct Reader<'a> {
    reading: &'a Reading,
    problems: Vec<Problem>,
}

impl Reader<'_> {
    /// A key's value if `accept` takes it. Otherwise the schema default, or `builtin` when
    /// the key or its default is unusable, noting the value that wasn't taken.
    fn read<T>(
        &mut self,
        schema: &str,
        key: &str,
        builtin: T,
        accept: impl Fn(&Value) -> Option<T>,
        show: impl Fn(&T) -> Value,
    ) -> T {
        let Some(entry) = self.reading.get(schema, key) else {
            return builtin;
        };
        if let Some(value) = accept(&entry.value) {
            return value;
        }
        let default = accept(&entry.default).unwrap_or(builtin);
        self.problems.push(Problem::Invalid {
            schema: schema.into(),
            key: key.into(),
            value: entry.value.to_string(),
            used: show(&default).to_string(),
        });
        default
    }

    fn string(&mut self, schema: &str, key: &str, builtin: &str) -> String {
        let accept = |v: &Value| match v {
            Value::Str(s) => Some(s.clone()),
            _ => None,
        };
        self.read(schema, key, builtin.into(), accept, |s| {
            Value::Str(s.clone())
        })
    }

    /// A string that mustn't be empty, such as a theme or font name.
    fn non_empty(&mut self, schema: &str, key: &str, builtin: &str) -> String {
        let accept = |v: &Value| match v {
            Value::Str(s) if !s.is_empty() => Some(s.clone()),
            _ => None,
        };
        self.read(schema, key, builtin.into(), accept, |s| {
            Value::Str(s.clone())
        })
    }

    fn bool(&mut self, schema: &str, key: &str, builtin: bool) -> bool {
        let accept = |v: &Value| match v {
            Value::Bool(b) => Some(*b),
            _ => None,
        };
        self.read(schema, key, builtin, accept, |b| Value::Bool(*b))
    }

    fn int(&mut self, schema: &str, key: &str, builtin: i32) -> i32 {
        let accept = |v: &Value| match v {
            Value::Int(i) => Some(*i),
            _ => None,
        };
        self.read(schema, key, builtin, accept, |i| Value::Int(*i))
    }

    fn uint(&mut self, schema: &str, key: &str, builtin: u32) -> u32 {
        let accept = |v: &Value| match v {
            Value::Uint(u) => Some(*u),
            _ => None,
        };
        self.read(schema, key, builtin, accept, |u| Value::Uint(*u))
    }

    fn int64(&mut self, schema: &str, key: &str, builtin: i64) -> i64 {
        let accept = |v: &Value| match v {
            Value::Int64(i) => Some(*i),
            _ => None,
        };
        self.read(schema, key, builtin, accept, |i| Value::Int64(*i))
    }

    /// A maybe pair of doubles, nothing by default.
    fn maybe_pair(&mut self, schema: &str, key: &str) -> Option<(f64, f64)> {
        let accept = |v: &Value| match v {
            Value::MaybePair(pair) => Some(*pair),
            _ => None,
        };
        self.read(schema, key, None, accept, |pair| Value::MaybePair(*pair))
    }

    fn double(&mut self, schema: &str, key: &str, builtin: f64) -> f64 {
        let accept = |v: &Value| match v {
            Value::Double(d) => Some(*d),
            _ => None,
        };
        self.read(schema, key, builtin, accept, |d| Value::Double(*d))
    }

    /// A list of strings, empty by default.
    fn strings(&mut self, schema: &str, key: &str) -> Vec<String> {
        self.strings_or(schema, key, &[])
    }

    /// A list of strings, `builtin` by default.
    fn strings_or(&mut self, schema: &str, key: &str, builtin: &[&str]) -> Vec<String> {
        let accept = |v: &Value| match v {
            Value::StrList(list) => Some(list.clone()),
            _ => None,
        };
        let builtin = builtin.iter().map(|s| (*s).to_owned()).collect();
        self.read(schema, key, builtin, accept, |list| {
            Value::StrList(list.clone())
        })
    }

    /// An enum key, by its nick.
    fn nick<T: Nick>(&mut self, schema: &str, key: &str, builtin: T) -> T {
        let accept = |v: &Value| match v {
            Value::Str(nick) => T::from_nick(nick),
            _ => None,
        };
        self.read(schema, key, builtin, accept, |t| {
            Value::Str(t.nick().into())
        })
    }

    /// An integer that's at least `min`. The schemas give these keys no range, so there's
    /// no bound to clamp to, and a smaller value takes the default.
    fn int_at_least(&mut self, schema: &str, key: &str, builtin: i32, min: i32) -> i32 {
        let accept = |v: &Value| match v {
            Value::Int(i) if *i >= min => Some(*i),
            _ => None,
        };
        self.read(schema, key, builtin, accept, |i| Value::Int(*i))
    }

    /// An integer, clamped to `min..=max`.
    fn int_clamped(&mut self, schema: &str, key: &str, builtin: i32, min: i32, max: i32) -> i32 {
        let value = self.int(schema, key, builtin);
        let clamped = value.clamp(min, max);
        if clamped != value {
            self.problems.push(Problem::OutOfRange {
                schema: schema.into(),
                key: key.into(),
                value: value.to_string(),
                used: clamped.to_string(),
            });
        }
        clamped
    }

    /// An unsigned integer, clamped to `min..=max`.
    fn uint_clamped(&mut self, schema: &str, key: &str, builtin: u32, min: u32, max: u32) -> u32 {
        let value = self.uint(schema, key, builtin);
        let clamped = value.clamp(min, max);
        if clamped != value {
            self.problems.push(Problem::OutOfRange {
                schema: schema.into(),
                key: key.into(),
                value: value.to_string(),
                used: clamped.to_string(),
            });
        }
        clamped
    }
}

#[cfg(test)]
mod tests;
