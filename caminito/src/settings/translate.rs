//! GSettings values into Caminito's typed settings, with 09's rules for every key. Nothing
//! here touches GLib, so it's tested with plain values.

use std::collections::{BTreeMap, HashSet};
use std::fmt;
use std::time::Duration;

use caminito_policy::{
    Config, MAX_WORKSPACES, Orientation, WorkspaceLayout, default_workspace_name,
};
use caminito_theme::ButtonLayout;

pub const MARCO_GENERAL: &str = "org.mate.Marco.general";
pub const MARCO_WORKSPACE_NAMES: &str = "org.mate.Marco.workspace-names";
pub const MARCO_KEYBINDING_COMMANDS: &str = "org.mate.Marco.keybinding-commands";
pub const INTERFACE: &str = "org.mate.interface";
pub const FONT_RENDERING: &str = "org.mate.font-rendering";
pub const MOUSE: &str = "org.mate.peripherals-mouse";
pub const TOUCHPAD: &str = "org.mate.peripherals-touchpad";
pub const KEYBOARD: &str = "org.mate.peripherals-keyboard";
pub const KEYBOARD_XKB: &str = "org.mate.peripherals-keyboard-xkb.kbd";
pub const SOUND: &str = "org.mate.sound";
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
    // Caminito will draw the ripple itself; M8's parity audit decides when (53).
    (MOUSE, "locate-pointer"),
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
    Double(f64),
    Str(String),
    StrList(Vec<String>),
    /// Any other type, printed as GVariant text.
    Other(String),
}

impl fmt::Display for Value {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Value::Bool(b) => write!(f, "{b}"),
            Value::Int(i) => write!(f, "{i}"),
            Value::Double(d) => write!(f, "{d}"),
            Value::Str(s) => write!(f, "'{s}'"),
            Value::StrList(list) => write!(f, "{list:?}"),
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
    pub keymap: Keymap,
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
/// `org.mate.Marco.keybinding-commands` (104). Only the screenshots' are read until M8
/// reads the bindings (17).
#[derive(Debug, Clone, PartialEq)]
pub struct Commands {
    /// `command-screenshot`.
    pub screenshot: String,
    /// `command-window-screenshot`.
    pub window_screenshot: String,
    /// `command-area-screenshot`. Marco 1.29.0's schema has no key for it, so it's often
    /// the default of the Marco that has (`org.mate.marco.gschema.xml:780-781` at `8522ed1`).
    pub area_screenshot: String,
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
/// and M8 acts on them (17).
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
    /// The whole screen, or the outputs the window is on.
    Fullscreen,
    /// The window's frame.
    FrameFlash,
}

impl Nick for VisualBellType {
    const NICKS: &'static [(&'static str, Self)] = &[
        ("fullscreen", Self::Fullscreen),
        ("frame-flash", Self::FrameFlash),
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
    let workspace_names = (1..=workspace_count)
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
    let policy = Config {
        workspace_count,
        workspace_names,
        workspace_layout,
        alt_tab_raise_windows: r.bool(MARCO_GENERAL, "alt-tab-raise-windows", false),
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
    };

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
    };

    let settings = Settings {
        policy,
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
            drag_threshold: r.int_at_least(MOUSE, "drag-threshold", 8, 0) as u32,
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
        keymap: keymap(&mut r).or(&reading.system_keymap),
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

    fn double(&mut self, schema: &str, key: &str, builtin: f64) -> f64 {
        let accept = |v: &Value| match v {
            Value::Double(d) => Some(*d),
            _ => None,
        };
        self.read(schema, key, builtin, accept, |d| Value::Double(*d))
    }

    /// A list of strings, empty by default.
    fn strings(&mut self, schema: &str, key: &str) -> Vec<String> {
        let accept = |v: &Value| match v {
            Value::StrList(list) => Some(list.clone()),
            _ => None,
        };
        self.read(schema, key, Vec::new(), accept, |list| {
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
}

#[cfg(test)]
mod tests;
