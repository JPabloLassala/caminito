//! GSettings values into Camino's typed settings, with 09's rules for every key. Nothing
//! here touches GLib, so it's tested with plain values.

use std::collections::{BTreeMap, HashSet};
use std::fmt;
use std::time::Duration;

use camino_policy::{Config, MAX_WORKSPACES, default_workspace_name};
use camino_theme::ButtonLayout;

pub const MARCO_GENERAL: &str = "org.mate.Marco.general";
pub const MARCO_WORKSPACE_NAMES: &str = "org.mate.Marco.workspace-names";
pub const INTERFACE: &str = "org.mate.interface";
pub const FONT_RENDERING: &str = "org.mate.font-rendering";
pub const MOUSE: &str = "org.mate.peripherals-mouse";
pub const SOUND: &str = "org.mate.sound";

/// A schema Camino reads.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Schema {
    pub id: &'static str,
    /// Marco's schemas must be installed (09). Without another, its keys are defaults.
    pub required: bool,
}

/// Every schema Camino reads. Each is read whole, so an unsupported key needs only its
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
];

/// Keys Camino doesn't support yet, whose defaults it doesn't match (09). Each is logged
/// once per run, and only when set away from its default. Entries arrive with the
/// milestones that read their neighbours.
pub const UNSUPPORTED: &[(&str, &str)] = &[];

/// Marco's schemas that aren't installed, which stop Camino starting (09).
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct MissingSchemas(pub Vec<&'static str>);

impl fmt::Display for MissingSchemas {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(
            f,
            "Marco's GSettings schemas {} aren't installed; Camino reads Marco's settings, \
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

/// A GSettings value, as far as Camino reads it. Enums are their nicks.
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

/// Every key of the schemas Camino found, as GSettings has them.
#[derive(Debug, Clone, Default, PartialEq)]
pub struct Reading {
    entries: BTreeMap<(String, String), Entry>,
    /// Optional schemas that aren't installed.
    missing: Vec<&'static str>,
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
    /// A value Camino can't use, replaced by the schema default.
    Invalid {
        schema: String,
        key: String,
        value: String,
        used: String,
    },
    /// A key Camino doesn't support, set away from its default.
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
                "{schema} {key} is {value}, but Camino doesn't support it yet; ignoring it"
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

/// Camino's settings, typed.
#[derive(Debug, Clone, PartialEq)]
pub struct Settings {
    /// Policy's settings: the workspace count and names.
    pub policy: Config,
    pub frames: Frames,
    pub interface: Interface,
    pub font_rendering: FontRendering,
    pub mouse: Mouse,
    pub sound: Sound,
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

/// `org.mate.interface`.
#[derive(Debug, Clone, PartialEq)]
pub struct Interface {
    pub gtk_theme: String,
    pub font_name: String,
    pub icon_theme: String,
    pub gtk: GtkOptions,
}

/// The `org.mate.interface` keys Camino only passes on to X11 GTK apps through XSETTINGS,
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

/// The `org.mate.peripherals-mouse` keys Camino itself uses (09).
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
    let policy = Config {
        workspace_count,
        workspace_names,
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

    let settings = Settings {
        policy,
        frames,
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

/// Reads keys from a reading, falling back to the schema default Camino knows when a key
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
