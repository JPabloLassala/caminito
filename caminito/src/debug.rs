//! The debug toggles' state and the JSON state dump (14). [`crate::dbus::debug`] only
//! exposes them.

use std::cell::RefCell;
use std::collections::HashMap;

use caminito_policy::{
    MenuEntry, MenuItem, MenuMark, PolicyWindow, SwitchEntry, SwitchList, Tile, WindowId,
};
use caminito_theme::cairo::{self, Context, Format, ImageSurface};
use serde::Serialize;
use smithay::backend::renderer::element::memory::MemoryRenderBuffer;
use smithay::desktop::LayerSurface;
use smithay::utils::{FrameExtents, Logical, Point, Rectangle};
use smithay::wayland::shell::wlr_layer::{ExclusiveZone, KeyboardInteractivity, Layer};

use crate::decoration::memory_buffer;
use crate::output::{output_geometry, output_id};
use crate::scene;
use crate::state::{Caminito, SEAT};
use crate::switcher::layout::CellStyle;
use crate::window::{Strut, with_x_facts};
use crate::window_menu::{Pane, Text};

/// The debug toggles, and what they draw with.
#[derive(Default)]
pub struct DebugToggles {
    /// Outlines each frame's strips, invisible resize border and title rect, and labels the
    /// scale its strips were rendered at.
    pub frame_debug: bool,
    /// Frame debug's label for each scale, drawn once.
    scale_labels: RefCell<HashMap<i32, MemoryRenderBuffer>>,
}

impl DebugToggles {
    /// Frame debug's label for strips rendered at `scale`, as a buffer at that scale.
    pub fn scale_label(&self, scale: i32) -> Option<MemoryRenderBuffer> {
        let mut labels = self.scale_labels.borrow_mut();
        if let Some(label) = labels.get(&scale) {
            return Some(label.clone());
        }
        let label = draw_label(&format!("×{scale}"), scale)
            .inspect_err(|err| tracing::warn!("can't draw a frame debug label: {err}"))
            .ok()
            .and_then(|surface| memory_buffer(surface, scale))?;
        labels.insert(scale, label.clone());
        Some(label)
    }
}

/// `text` in white on black, 14 logical pixels high, at `scale`.
fn draw_label(text: &str, scale: i32) -> Result<ImageSurface, cairo::Error> {
    const HEIGHT: i32 = 14;
    let width = 8 * text.chars().count() as i32 + 4;
    let surface = ImageSurface::create(Format::ARgb32, width * scale, HEIGHT * scale)?;
    let cr = Context::new(&surface)?;
    cr.scale(f64::from(scale), f64::from(scale));
    cr.set_source_rgb(0.0, 0.0, 0.0);
    cr.paint()?;
    cr.set_source_rgb(1.0, 1.0, 1.0);
    cr.select_font_face(
        "monospace",
        cairo::FontSlant::Normal,
        cairo::FontWeight::Bold,
    );
    cr.set_font_size(12.0);
    cr.move_to(2.0, 11.0);
    cr.show_text(text)?;
    drop(cr);
    Ok(surface)
}

#[derive(Serialize)]
struct StateDump {
    /// By id.
    windows: Vec<WindowDump>,
    workspaces: Vec<WorkspaceDump>,
    /// The Active output's active workspace.
    active_workspace: usize,
    /// Whether every output's active workspace shows its desktop (94).
    showing_desktop: bool,
    outputs: Vec<OutputDump>,
    /// The primary output's name (81): the user's choice, or its stand-in.
    primary_output: Option<String>,
    /// The name of the seat's Active output (ADR 0032), which its workspace bindings
    /// switch.
    active_output: Option<String>,
    /// The one scale of every XWayland window (41).
    x11_scale: u32,
    focus: Option<u64>,
    /// The lock state (12): `unlocked`, `pending` while Caminito waits for the locker,
    /// `locked`, or `locker-lost` while the session is locked and its locker is gone.
    lock: &'static str,
    /// The locker Caminito spawns (126).
    locker: LockerDump,
    /// Idle, the blank, the power-off and what holds them off (128, 129).
    idle: IdleDump,
    /// The keybindings in force (148), in the order they're tried.
    bindings: Vec<BindingDump>,
    /// The accelerators grabbed over `org.mate.Caminito` (98), by action id.
    accelerator_grabs: Vec<AcceleratorGrabDump>,
    /// The Window switcher (108), while it's open.
    switcher: Option<SwitcherDump>,
    /// The Workspace popup (152), while it shows.
    workspace_popup: Option<WorkspacePopupDump>,
    /// The window menu (161), while it's open.
    window_menu: Option<WindowMenuDump>,
    /// The Overview (241), while it's open.
    overview: Option<OverviewDump>,
    /// The Overview's transition (243), while it's drawn on its way open or closed: past
    /// policy's closing, while `overview` is `null`, too.
    #[serde(skip_serializing_if = "Option::is_none")]
    overview_transition: Option<OverviewTransitionDump>,
    /// Whether the apps are told to give up their own menubars (110): `on` while a Global
    /// menu applet is on the bus, `leaving` for the grace period after it left (114), and
    /// `off`.
    global_menu_switch: &'static str,
    /// The toplevels the menu claims tied to a menu (115), by window id.
    menu_claims: Vec<MenuClaimDump>,
    /// The bells rung (105).
    bell: BellDump,
    /// Night Light (137).
    night_light: NightLightDump,
    /// The sun's position, as `org.mate.Caminito`'s `Sun` (215).
    sun: SunDump,
    /// Where letting go of the move in progress would tile or maximise its window, frame
    /// and all, while the tile preview shows it (158).
    #[serde(skip_serializing_if = "Option::is_none")]
    tile_preview: Option<RectDump>,
    /// The Workspace slides that run (233), in the outputs' order.
    #[serde(skip_serializing_if = "Vec::is_empty")]
    workspace_slides: Vec<WorkspaceSlideDump>,
}

#[derive(Serialize)]
struct WorkspaceSlideDump {
    /// The name of the output that switched.
    output: String,
    /// The workspace the last switch left, and the one whose windows arrive.
    from: usize,
    to: usize,
    /// How far the arriving windows are from their own places now. Those of a workspace
    /// the slide left are that far from where their workspace ends up, off the output.
    offset: OffsetDump,
    /// Each window the slide moves, by id, with where its frame is drawn now: those of a
    /// retargeted slide's earlier workspaces too (235). A window of a workspace the slide
    /// draws at more than one place is here one time for each (239).
    windows: Vec<SlidingWindowDump>,
    /// The windows on the output that the slide leaves in their places, by id (234).
    still: Vec<u64>,
}

#[derive(Serialize)]
struct OffsetDump {
    x: i32,
    y: i32,
}

#[derive(Serialize)]
struct SlidingWindowDump {
    id: u64,
    rect: RectDump,
}

#[derive(Serialize)]
struct NightLightDump {
    /// `night-light-enabled`.
    enabled: bool,
    /// `sun` or `constant`.
    schedule: &'static str,
    /// `night-light-paused-until`: the Unix time a pause ends, or 0.
    paused_until: i64,
    /// The sun schedule's latitude and longitude, while it has one.
    location: Option<(f64, f64)>,
    /// Where the location comes from: `stored`, `time-zone`, or `none` for the fixed times.
    location_source: &'static str,
    /// The sun's elevation now, in degrees, while there's a location.
    sun_elevation: Option<f64>,
    /// The day's highest elevation, in degrees, while there's a location.
    sun_highest: Option<f64>,
    /// The Unix time the sun next climbs past −6°, as `NightLight`'s `next-sunrise`.
    next_sunrise: i64,
    /// The temperature the outputs are to have, in kelvin.
    target_temperature: u32,
    /// Each output's temperature now, partway through a fade or not. Only the tty writes it
    /// to a gamma table.
    outputs: Vec<NightLightOutputDump>,
}

#[derive(Serialize)]
struct SunDump {
    /// In degrees above the horizon: the real sun's, or the fixed times' with no location.
    elevation: f64,
    /// In degrees clockwise from north.
    azimuth: f64,
    /// As Night Light's.
    location_source: &'static str,
}

#[derive(Serialize)]
struct NightLightOutputDump {
    output: String,
    /// Night Light's temperature on it, or `None` while a gamma client's table is on it.
    temperature: Option<u32>,
    /// Whether a gamma client holds it (141).
    gamma_client: bool,
}

#[derive(Serialize)]
struct LockerDump {
    /// `idle` while Caminito keeps no locker running, `starting` before one is spawned or
    /// spawned again, `running`, or `gave-up` once it failed to start too often.
    state: &'static str,
    /// The line drawn on the lock once Caminito gave up on the locker.
    message: Option<String>,
}

#[derive(Serialize)]
struct IdleDump {
    /// Whether the outputs are blanked.
    blanked: bool,
    /// Whether they're powered off (129).
    powered_off: bool,
    /// Whether UPower says the machine runs on its battery, which picks the `sleep-display`
    /// key.
    on_battery: bool,
    /// Whether inhibitors hold idle off now. None do while the session is locked (12).
    inhibited: bool,
    /// What asks to hold idle off: each idle-inhibit surface, mate-session while it says
    /// one of its own does, and each of the Screensaver shim's inhibitors (127).
    inhibitors: Vec<InhibitorDump>,
    /// How many ext-idle-notify notifications clients have.
    notifications: usize,
}

#[derive(Serialize)]
struct InhibitorDump {
    /// `idle-inhibit`, `mate-session` or `screensaver`.
    source: &'static str,
    /// The application a Screensaver shim's inhibitor is for, by the name it gave.
    #[serde(skip_serializing_if = "Option::is_none")]
    application: Option<String>,
    /// The window an idle-inhibit surface is part of, if it's a window's.
    #[serde(skip_serializing_if = "Option::is_none")]
    window: Option<u64>,
    /// Whether an idle-inhibit surface is on screen, which it counts only while it is.
    #[serde(skip_serializing_if = "Option::is_none")]
    shown: Option<bool>,
}

#[derive(Serialize)]
struct BellDump {
    /// How many bells have rung (105).
    count: u64,
    /// The last one.
    last: Option<LastBellDump>,
}

#[derive(Serialize)]
struct LastBellDump {
    /// `gtk_shell1`, `xdg_system_bell`, `xkb` or `accessx`.
    source: &'static str,
    /// The window it was for, if any.
    window: Option<u64>,
}

#[derive(Serialize)]
struct SwitcherDump {
    /// The action's list: `windows`, `windows-all`, `group` or `panels`.
    list: &'static str,
    /// Whether it's a `cycle-*` action's (163), which has no popup.
    cycle: bool,
    /// The windows listed, in order.
    windows: Vec<u64>,
    /// The Layer surfaces listed among the panels (163), in order, by their namespaces.
    layers: Vec<String>,
    /// The selected window, if it's one.
    selected: Option<u64>,
    /// The selected Layer surface's namespace, if it's one.
    selected_layer: Option<String>,
    /// Whether its popup is drawn.
    visible: bool,
    /// The popup, once it has appeared.
    #[serde(flatten)]
    popup: Option<SwitcherPopupDump>,
}

#[derive(Serialize)]
struct SwitcherPopupDump {
    /// The name of the output the popup is on.
    output: String,
    /// What a cell shows: `thumbnails` or `icons`.
    style: &'static str,
    columns: usize,
    /// The popup and each window's cell, from the output's top left corner.
    popup: RectDump,
    cells: Vec<RectDump>,
}

#[derive(Serialize)]
struct OverviewDump {
    /// The name of the output it's on.
    output: String,
    /// The windows listed, bottom to top.
    windows: Vec<OverviewWindowDump>,
    /// The selected window (242), if one is.
    selected: Option<u64>,
    /// The workspace it lists the windows of: its output's active one (244).
    workspace: usize,
    /// Its bar's cells (244), one for each workspace from the first, once the refresh pass
    /// has laid it out.
    bar: Vec<OverviewCellDump>,
    /// The area beneath the bar that the windows are laid out in.
    area: Option<RectDump>,
    /// While a Workspace slide runs on the output (244), each window of a layout that it
    /// draws away from its place, with where it's drawn, in the order of the windows' ids:
    /// one time for each place, as `workspace_slides` has it.
    sliding: Vec<OverviewSlidingDump>,
    /// How much of black is over the wallpaper.
    dim: f32,
    /// The namespaces of the output's layer surfaces that aren't drawn while it's open, in
    /// the order they were made: the panel's and caja's desktop's.
    hidden_layer_surfaces: Vec<String>,
}

#[derive(Serialize)]
struct OverviewCellDump {
    workspace: usize,
    /// From the output's top left corner.
    rect: RectDump,
    /// Whether it's the active workspace's, which is marked.
    active: bool,
}

#[derive(Serialize)]
struct OverviewSlidingDump {
    /// The workspace whose layout it's in.
    workspace: usize,
    window: u64,
    /// Where it's drawn now, from the output's top left corner.
    rect: RectDump,
}

#[derive(Serialize)]
struct OverviewTransitionDump {
    /// The name of the output it's on.
    output: String,
    /// `opening` or `closing`.
    direction: &'static str,
    /// How open it's drawn, from 0, closed, to 1, open, on the ease-out.
    part: f64,
    /// How much of black is over the wallpaper now.
    dim: f32,
    /// How opaque the panel and caja's desktop are drawn now.
    layer_surfaces: f32,
    /// How opaque the labels are drawn now.
    labels: f32,
    /// Each window drawn, bottom to top, with where, from the output's top left corner, and
    /// how opaque.
    windows: Vec<OverviewDrawnDump>,
}

#[derive(Serialize)]
struct OverviewDrawnDump {
    window: u64,
    rect: RectDump,
    alpha: f32,
}

#[derive(Serialize)]
struct OverviewWindowDump {
    window: u64,
    /// Where the window is drawn in the layout, from the output's top left corner, once
    /// the refresh pass has laid it out.
    rect: Option<RectDump>,
    /// Where its icon and its title are drawn.
    label: Option<RectDump>,
}

#[derive(Serialize)]
struct WindowMenuDump {
    /// The window it's for.
    window: u64,
    /// The name of the output it's on, once it's laid out.
    output: Option<String>,
    /// The menu, from the output's top left corner.
    rect: Option<RectDump>,
    /// The selected item's place among `items`.
    selected: Option<usize>,
    items: Vec<MenuItemDump>,
    /// The submenu of workspaces, while it's open.
    submenu: Option<SubmenuDump>,
}

#[derive(Serialize)]
struct SubmenuDump {
    rect: Option<RectDump>,
    selected: Option<usize>,
    items: Vec<MenuItemDump>,
}

#[derive(Serialize)]
struct MenuItemDump {
    /// `item`, `submenu` for the item that opens it, or `separator`.
    kind: &'static str,
    /// The label as drawn, in the user's language.
    label: Option<String>,
    /// The letter that chooses it.
    mnemonic: Option<char>,
    /// Its keybinding, as drawn.
    accelerator: Option<String>,
    /// Whether it can't be chosen.
    greyed: bool,
    /// `checked` or `unchecked` for a check box, `set` or `unset` for a radio button.
    mark: Option<&'static str>,
    /// Its row, from the output's top left corner.
    rect: Option<RectDump>,
}

#[derive(Serialize)]
struct WorkspacePopupDump {
    /// The name of the output the popup is on.
    output: String,
    /// What a further direction does: `switch`, or `move` for the focused window too.
    mode: &'static str,
    /// The Workspace grid's shape.
    columns: usize,
    rows: usize,
    /// The workspace marked, which is the active one.
    selected: usize,
    /// The popup, and for each workspace, from the first, its cell, the preview in it and
    /// the band its name is in, from the output's top left corner.
    popup: RectDump,
    cells: Vec<RectDump>,
    previews: Vec<RectDump>,
    names: Vec<RectDump>,
}

#[derive(Serialize)]
struct BindingDump {
    /// The key of `org.mate.Marco.global-keybindings` or `window-keybindings` it came from.
    key: &'static str,
    /// The combination, in GTK's syntax.
    combination: String,
}

#[derive(Serialize)]
struct AcceleratorGrabDump {
    /// The id `AcceleratorActivated` carries.
    action: u32,
    /// As the grabber wrote it.
    accelerator: String,
    /// The grabber's unique name on the bus.
    owner: String,
}

#[derive(Serialize)]
struct MenuClaimDump {
    /// The toplevel's id.
    window: u64,
    /// The caller's unique name on the bus, which serves the menu.
    caller: String,
    /// The dbusmenu's object path.
    path: String,
}

#[derive(Serialize)]
struct WindowDump {
    id: u64,
    app_id: Option<String>,
    title: String,
    workspace: Option<usize>,
    geometry: Option<RectDump>,
    decorations: Decorations,
    /// The visible frame around the geometry, while it shows one.
    frame_extents: Option<ExtentsDump>,
    /// The scale the frame's strips were rendered at, while it shows one.
    strip_scale: Option<i32>,
    /// Whether it's an XWayland window.
    xwayland: bool,
    /// For an XWayland window, its X window's id and Marco's type for it.
    #[serde(skip_serializing_if = "Option::is_none")]
    x11: Option<X11Dump>,
    /// Whether it asks for the user's attention.
    urgent: bool,
    skip_taskbar: bool,
    skip_pager: bool,
    /// The half or quarter of its work area it's tiled to (158): `left`, `right`,
    /// `top-left`, `top-right`, `bottom-left` or `bottom-right`.
    #[serde(skip_serializing_if = "Option::is_none")]
    tile: Option<&'static str>,
}

/// The dump's name for `tile`.
fn tile_name(tile: Tile) -> &'static str {
    match tile {
        Tile::Left => "left",
        Tile::Right => "right",
        Tile::TopLeft => "top-left",
        Tile::TopRight => "top-right",
        Tile::BottomLeft => "bottom-left",
        Tile::BottomRight => "bottom-right",
    }
}

#[derive(Serialize)]
struct X11Dump {
    window: u32,
    window_type: String,
    /// `_NET_FRAME_EXTENTS` as Caminito's X connection last published it (39).
    frame_extents: Option<ExtentsDump>,
    /// The struts it reserves (39).
    struts: Vec<StrutDump>,
}

#[derive(Serialize)]
struct StrutDump {
    side: String,
    width: i32,
    /// The first and last pixel along the edge, or none for the whole edge.
    span: Option<(i32, i32)>,
}

impl From<Strut> for StrutDump {
    fn from(strut: Strut) -> Self {
        Self {
            side: format!("{:?}", strut.side).to_lowercase(),
            width: strut.width,
            span: strut.span,
        }
    }
}

#[derive(Serialize)]
#[serde(rename_all = "lowercase")]
enum Decorations {
    Ssd,
    Csd,
}

#[derive(Serialize)]
struct WorkspaceDump {
    name: String,
    /// Window ids, bottom to top.
    stacking: Vec<u64>,
}

#[derive(Serialize)]
struct OutputDump {
    id: u64,
    name: String,
    geometry: RectDump,
    scale: f64,
    /// Its active workspace, which is every output's with Spanning workspaces (ADR 0032).
    active_workspace: usize,
    /// The output less what layer surfaces' exclusive zones and XWayland windows' struts
    /// reserve along its edges (64).
    work_area: RectDump,
    /// In the order they were made.
    layer_surfaces: Vec<LayerSurfaceDump>,
    /// Whether the locker that holds the lock has a lock surface on it (12).
    lock_surface: bool,
}

#[derive(Serialize)]
struct LayerSurfaceDump {
    /// `background`, `bottom`, `top` or `overlay`.
    layer: &'static str,
    namespace: String,
    geometry: RectDump,
    /// As the protocol has it: what it reserves on its edge, 0 to stay out of what others
    /// reserve, or -1 to ignore that.
    exclusive_zone: i32,
    /// `none`, `exclusive` or `on_demand`.
    keyboard_interactivity: &'static str,
}

impl LayerSurfaceDump {
    fn new(layer: &LayerSurface, geometry: Rectangle<i32, Logical>) -> Self {
        let state = layer.cached_state();
        Self {
            layer: match state.layer {
                Layer::Background => "background",
                Layer::Bottom => "bottom",
                Layer::Top => "top",
                Layer::Overlay => "overlay",
            },
            namespace: layer.namespace().to_owned(),
            geometry: geometry.into(),
            exclusive_zone: match state.exclusive_zone {
                ExclusiveZone::Exclusive(zone) => zone as i32,
                ExclusiveZone::Neutral => 0,
                ExclusiveZone::DontCare => -1,
            },
            keyboard_interactivity: match state.keyboard_interactivity {
                KeyboardInteractivity::None => "none",
                KeyboardInteractivity::Exclusive => "exclusive",
                KeyboardInteractivity::OnDemand => "on_demand",
            },
        }
    }
}

#[derive(Serialize)]
struct RectDump {
    x: i32,
    y: i32,
    width: i32,
    height: i32,
}

impl From<Rectangle<i32, Logical>> for RectDump {
    fn from(rect: Rectangle<i32, Logical>) -> Self {
        Self {
            x: rect.loc.x,
            y: rect.loc.y,
            width: rect.size.w,
            height: rect.size.h,
        }
    }
}

#[derive(Serialize)]
struct ExtentsDump {
    left: i32,
    right: i32,
    top: i32,
    bottom: i32,
}

impl From<FrameExtents<i32, Logical>> for ExtentsDump {
    fn from(extents: FrameExtents<i32, Logical>) -> Self {
        Self {
            left: extents.left,
            right: extents.right,
            top: extents.top,
            bottom: extents.bottom,
        }
    }
}

impl Caminito {
    /// Windows, with the X window, Marco's type, the published frame extents and the
    /// struts for XWayland windows, workspaces with their names and stacking, whether the
    /// active one shows its desktop, outputs with their scales, work areas and layer
    /// surfaces, the X11 scale, focus, the lock state and the locker, idle, the keybindings
    /// with their keys, the accelerator grabs with their owners, the Window switcher, the
    /// Workspace popup, the Overview with each listed window's place (241) and the
    /// selection (242), the bells rung with the last one, Night Light, each window's tile
    /// with the tile preview (158), and the Workspace slides that run (233), as
    /// pretty-printed JSON.
    pub fn state_dump(&self) -> String {
        let policy = &self.policy;
        let windows = policy
            .windows()
            .map(|mapped| {
                let id = mapped.id();
                let ssd = mapped.is_ssd();
                let skips = mapped.skips();
                WindowDump {
                    id: id.0,
                    app_id: mapped.app_id(),
                    title: mapped.title(),
                    workspace: policy.workspace_of(id),
                    geometry: policy.geometry(id).map(RectDump::from),
                    decorations: if ssd {
                        Decorations::Ssd
                    } else {
                        Decorations::Csd
                    },
                    frame_extents: mapped
                        .shows_frame()
                        .then(|| policy.frame_extents(id).into()),
                    strip_scale: mapped.frame().drawn_scale(),
                    xwayland: mapped.window().is_x11(),
                    x11: mapped.window().x11_surface().map(|surface| {
                        let window = surface.window_id();
                        X11Dump {
                            window,
                            window_type: format!("{:?}", mapped.window_type()),
                            frame_extents: self.published_frame_extents(window).map(Into::into),
                            struts: with_x_facts(surface, |facts| facts.struts.clone())
                                .into_iter()
                                .map(Into::into)
                                .collect(),
                        }
                    }),
                    urgent: policy.is_urgent(id),
                    skip_taskbar: skips.taskbar,
                    skip_pager: skips.pager,
                    tile: policy.tile(id).map(tile_name),
                }
            })
            .collect();
        let workspaces = policy
            .workspace_names()
            .iter()
            .enumerate()
            .map(|(workspace, name)| WorkspaceDump {
                name: name.clone(),
                stacking: policy.stacking(workspace).iter().map(|id| id.0).collect(),
            })
            .collect();
        let work_area = |output| {
            let id = output_id(output);
            let mut areas = policy.outputs().iter().zip(policy.work_areas());
            let (_, area) = areas
                .find(|(output, _)| output.id == id)
                .expect("policy has every output");
            area
        };
        let outputs = self
            .outputs
            .iter()
            .map(|output| OutputDump {
                id: output_id(output).0,
                name: output.name(),
                geometry: output_geometry(output).into(),
                scale: output.current_scale().fractional_scale(),
                active_workspace: policy.active_workspace_on(output_id(output)),
                work_area: work_area(output).into(),
                layer_surfaces: self
                    .layer_surfaces(output)
                    .iter()
                    .map(|(layer, geometry)| LayerSurfaceDump::new(layer, *geometry))
                    .collect(),
                lock_surface: self.lock.surface_on(output).is_some(),
            })
            .collect();
        let bindings = self
            .settings
            .bindings
            .iter()
            .map(|(key, combination)| BindingDump { key, combination })
            .collect();
        let accelerator_grabs = self
            .accelerator_grabs
            .iter()
            .map(|(action, grab)| AcceleratorGrabDump {
                action,
                accelerator: grab.accelerator.clone(),
                owner: grab.owner.clone(),
            })
            .collect();
        let menu_claims = self
            .menu_claims
            .iter()
            .map(|(window, tie)| MenuClaimDump {
                window: window.0,
                caller: tie.caller.clone(),
                path: tie.path.to_string(),
            })
            .collect();
        let namespace = |entry: &SwitchEntry| self.layer_namespace(*entry);
        let switcher = policy.switcher().map(|switcher| SwitcherDump {
            list: match switcher.list {
                SwitchList::Windows => "windows",
                SwitchList::WindowsAll => "windows-all",
                SwitchList::Group => "group",
                SwitchList::Panels => "panels",
            },
            cycle: !switcher.popup,
            windows: switcher
                .entries
                .iter()
                .filter_map(|entry| Some(entry.window()?.0))
                .collect(),
            layers: switcher.entries.iter().filter_map(namespace).collect(),
            selected: switcher.entries[switcher.selected].window().map(|id| id.0),
            selected_layer: namespace(&switcher.entries[switcher.selected]),
            visible: self.visible_switcher().is_some(),
            popup: self.visible_switcher().map(|popup| SwitcherPopupDump {
                output: popup.output.name(),
                style: match popup.grid.style {
                    CellStyle::Thumbnails => "thumbnails",
                    CellStyle::Icons { .. } => "icons",
                },
                columns: popup.grid.columns,
                popup: popup.grid.popup.into(),
                cells: popup.grid.cells.iter().map(|cell| (*cell).into()).collect(),
            }),
        });
        let overview = policy.overview().and_then(|open| {
            let mut outputs = self.outputs.iter();
            let output = outputs.find(|output| output_id(output) == open.output)?;
            let laid_out = self.overview_on(output);
            let slot = |id: &WindowId| {
                let laid = &laid_out?.laid;
                let place = laid.windows.iter().position(|listed| listed == id)?;
                laid.layout.slots.get(place)
            };
            let bar = laid_out.map_or_else(Vec::new, |laid_out| {
                let cells = laid_out.bar.cells.iter().enumerate();
                cells
                    .map(|(workspace, cell)| OverviewCellDump {
                        workspace,
                        rect: (*cell).into(),
                        active: workspace == laid_out.active,
                    })
                    .collect()
            });
            let size = output_geometry(output).size;
            let mut sliding: Vec<OverviewSlidingDump> = laid_out
                .map(|laid_out| {
                    laid_out
                        .layouts()
                        .flat_map(|laid| {
                            let shifts = self.overview_shifts(laid_out, laid);
                            let places = laid.windows.iter().zip(&laid.layout.slots);
                            places.flat_map(move |(id, slot)| {
                                let shifts = shifts.clone().into_iter();
                                shifts.filter(|shift| *shift != Point::default()).map(
                                    move |shift| OverviewSlidingDump {
                                        workspace: laid.workspace,
                                        window: id.0,
                                        rect: Rectangle::new(
                                            slot.window.loc + shift,
                                            slot.window.size,
                                        )
                                        .into(),
                                    },
                                )
                            })
                        })
                        .collect()
                })
                .unwrap_or_default();
            sliding.sort_by_key(|dump| (dump.window, dump.rect.x, dump.rect.y));
            Some(OverviewDump {
                output: output.name(),
                windows: open
                    .windows
                    .iter()
                    .map(|id| OverviewWindowDump {
                        window: id.0,
                        rect: slot(id).map(|slot| slot.window.into()),
                        label: slot(id).map(|slot| slot.label.into()),
                    })
                    .collect(),
                selected: open.selected.map(|id| id.0),
                workspace: open.workspace,
                bar,
                area: laid_out.map(|laid_out| laid_out.bar.area_beneath(size).into()),
                sliding,
                dim: crate::overview::DIM,
                hidden_layer_surfaces: self
                    .layer_surfaces(output)
                    .iter()
                    .filter(|(layer, _)| {
                        let namespace = layer.namespace();
                        scene::overview_hides(self, output, layer.layer(), namespace)
                    })
                    .map(|(layer, _)| layer.namespace().to_owned())
                    .collect(),
            })
        });
        let workspace_popup = self.workspace_popup.as_ref().map(|popup| {
            let cells = &popup.layout.cells;
            WorkspacePopupDump {
                output: popup.output.name(),
                mode: match policy.workspace_popup() {
                    Some(open) if open.moves => "move",
                    _ => "switch",
                },
                columns: popup.layout.columns,
                rows: popup.layout.rows,
                selected: policy.active_workspace_on(output_id(&popup.output)),
                popup: popup.layout.popup.into(),
                cells: cells.iter().map(|cell| cell.rect.into()).collect(),
                previews: cells.iter().map(|cell| cell.preview.into()).collect(),
                names: cells.iter().map(|cell| cell.name.into()).collect(),
            }
        });
        let window_menu = policy.window_menu().map(|menu| {
            let shown = self
                .shown_window_menu()
                .filter(|shown| shown.texts.top.len() == menu.entries.len());
            let item = |item: Option<&MenuItem>, text: Option<&Text>, rect: Option<RectDump>| {
                MenuItemDump {
                    kind: match (item, text) {
                        (Some(_), _) => "item",
                        (None, Some(_)) => "submenu",
                        (None, None) => "separator",
                    },
                    label: text.map(|text| text.label.text.clone()),
                    mnemonic: text.and_then(|text| text.label.mnemonic.map(|(_, c)| c)),
                    accelerator: text.and_then(|text| text.accelerator.clone()),
                    greyed: item.is_some_and(|item| !item.sensitive),
                    mark: item.and_then(|item| match item.mark {
                        MenuMark::None => None,
                        MenuMark::Check(true) => Some("checked"),
                        MenuMark::Check(false) => Some("unchecked"),
                        MenuMark::Radio(true) => Some("set"),
                        MenuMark::Radio(false) => Some("unset"),
                    }),
                    rect,
                }
            };
            let row = |pane: Option<&Pane>, index: usize| {
                pane.and_then(|pane| pane.layout.rows.get(index).copied())
                    .map(RectDump::from)
            };
            let main = shown.map(|shown| &shown.main);
            let sub = shown.and_then(|shown| shown.submenu.as_ref());
            let items = menu.entries.iter().enumerate().map(|(index, entry)| {
                let text = shown.and_then(|shown| shown.texts.top[index].as_ref());
                let entry = match entry {
                    MenuEntry::Item(entry) => Some(entry),
                    MenuEntry::Separator | MenuEntry::Workspaces(_) => None,
                };
                item(entry, text, row(main, index))
            });
            WindowMenuDump {
                window: menu.window.0,
                output: shown.map(|shown| shown.output.name()),
                rect: main.map(|pane| pane.layout.rect.into()),
                selected: menu.selected,
                items: items.collect(),
                submenu: menu.submenu().filter(|_| menu.submenu_open).map(|items| {
                    let items = items.iter().enumerate().map(|(index, entry)| {
                        let text = shown.and_then(|shown| shown.texts.sub.get(index));
                        item(Some(entry), text, row(sub, index))
                    });
                    SubmenuDump {
                        rect: sub.map(|pane| pane.layout.rect.into()),
                        selected: menu.submenu_selected,
                        items: items.collect(),
                    }
                }),
            }
        });
        let now = self.now();
        let overview_transition = self.overview.as_ref().and_then(|open| {
            let transition = open.transition.filter(|t| !t.is_over(now))?;
            Some(OverviewTransitionDump {
                output: open.output.name(),
                direction: transition.heading.name(),
                part: open.part(now),
                dim: open.dim(now),
                layer_surfaces: open.faded_alpha(now),
                labels: open.labels_alpha(now),
                windows: scene::overview_drawn_windows(
                    self,
                    open,
                    &open.laid,
                    Point::default(),
                    now,
                )
                .into_iter()
                .map(|drawn| OverviewDrawnDump {
                    window: drawn.id.0,
                    rect: drawn.rect.into(),
                    alpha: drawn.alpha,
                })
                .collect(),
            })
        });
        let workspace_slides = self
            .outputs
            .iter()
            .filter_map(|output| {
                let slide = self.workspace_slides.on(output_id(output), now)?;
                let offset = slide.offset(now);
                Some(WorkspaceSlideDump {
                    output: output.name(),
                    from: slide.from,
                    to: slide.to,
                    offset: OffsetDump {
                        x: offset.x,
                        y: offset.y,
                    },
                    windows: self
                        .sliding_windows(slide, now)
                        .into_iter()
                        .map(|(id, rect)| SlidingWindowDump {
                            id: id.0,
                            rect: rect.into(),
                        })
                        .collect(),
                    still: self
                        .still_windows(slide)
                        .into_iter()
                        .map(|id| id.0)
                        .collect(),
                })
            })
            .collect();
        let active_output = policy.active_output(SEAT);
        let active_output = self
            .outputs
            .iter()
            .find(|output| Some(output_id(output)) == active_output)
            .map(|output| output.name());
        let dump = StateDump {
            windows,
            workspaces,
            active_workspace: policy.active_output_workspace(SEAT),
            showing_desktop: policy.is_showing_desktop(),
            outputs,
            primary_output: self.primary_output().map(|output| output.name()),
            active_output,
            x11_scale: self.x11_scale(),
            focus: policy.focus(SEAT).map(|id| id.0),
            lock: self.lock.state().name(),
            locker: LockerDump {
                state: self.locker.state_name(),
                message: self.locker.gave_up().then(|| self.locker.message()),
            },
            idle: self.idle_dump(),
            bindings,
            accelerator_grabs,
            switcher,
            workspace_popup,
            window_menu,
            overview,
            overview_transition,
            global_menu_switch: self.global_menu_switch.name(),
            menu_claims,
            bell: BellDump {
                count: self.bells.count,
                last: self.bells.last.map(|bell| LastBellDump {
                    source: bell.source.name(),
                    window: bell.window.map(|id| id.0),
                }),
            },
            night_light: self.night_light_dump(),
            sun: self.sun_dump(),
            tile_preview: policy.tile_preview(SEAT).map(RectDump::from),
            workspace_slides,
        };
        serde_json::to_string_pretty(&dump).expect("the dump serialises")
    }

    fn night_light_dump(&self) -> NightLightDump {
        let night_light = &self.night_light;
        let settings = night_light.settings();
        let sun = night_light.sun();
        let now = self.now();
        NightLightDump {
            enabled: settings.enabled,
            schedule: settings.schedule.name(),
            paused_until: settings.paused_until,
            location: sun
                .location
                .map(|location| (location.latitude, location.longitude)),
            location_source: sun.source.name(),
            sun_elevation: sun.elevation,
            sun_highest: sun.highest,
            next_sunrise: sun.next_sunrise,
            target_temperature: night_light.target(),
            outputs: self
                .outputs
                .iter()
                .map(|output| NightLightOutputDump {
                    output: output.name(),
                    temperature: night_light.applied(output, now),
                    gamma_client: self.gamma_control_state.holds(output),
                })
                .collect(),
        }
    }

    fn sun_dump(&self) -> SunDump {
        let sun = self.night_light.published_sun();
        SunDump {
            elevation: sun.position.elevation,
            azimuth: sun.position.azimuth,
            location_source: sun.location_source,
        }
    }

    fn idle_dump(&self) -> IdleDump {
        let idle = &self.idle;
        let surfaces = idle.inhibitor_surfaces().map(|surface| InhibitorDump {
            source: "idle-inhibit",
            application: None,
            window: self.window_of_surface(surface).map(|id| id.0),
            shown: Some(self.shows_surface(surface)),
        });
        let session = idle.session_inhibited().then_some(InhibitorDump {
            source: "mate-session",
            application: None,
            window: None,
            shown: None,
        });
        let screensaver = self
            .screensaver
            .inhibitor_applications()
            .map(|application| InhibitorDump {
                source: "screensaver",
                application: Some(application.to_owned()),
                window: None,
                shown: None,
            });
        IdleDump {
            blanked: idle.is_blanked(),
            powered_off: idle.is_powered_off(),
            on_battery: idle.on_battery(),
            inhibited: idle.is_inhibited(),
            inhibitors: surfaces.chain(session).chain(screensaver).collect(),
            notifications: idle.notification_count(),
        }
    }

    /// Switches the output named `name` to `workspace`, which with `workspaces-per-output`
    /// on leaves the other outputs alone (230), whichever is the Active output. Returns
    /// whether there's such an output and such a workspace.
    pub fn switch_output_workspace(&mut self, name: &str, workspace: usize) -> bool {
        let Some(output) = self.outputs.iter().find(|output| output.name() == name) else {
            return false;
        };
        if workspace >= self.policy.workspace_count() {
            return false;
        }
        let output = output_id(output);
        tracing::debug!("switching {name} to workspace {}", workspace + 1);
        self.policy.switch_output_workspace(output, workspace);
        true
    }

    /// Turns frame debug on or off, and returns whether it's now on.
    pub fn toggle_frame_debug(&mut self) -> bool {
        self.debug.frame_debug = !self.debug.frame_debug;
        tracing::info!(
            "frame debug {}",
            if self.debug.frame_debug { "on" } else { "off" }
        );
        self.queue_redraw_all();
        self.debug.frame_debug
    }
}
