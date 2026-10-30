//! Caminito's frames (07, 15): the Marco theme's frame drawn around each SSD window as four
//! strips, and the colours they're drawn with.
//!
//! [`FrameContext`] holds what every frame is drawn with: the theme, the title font and how
//! it's rendered, the button layout and the icon theme. Each window's [`WindowFrame`] derives
//! its frame from the window's [`WindowFacts`], and a [`FrameState`] and the window's
//! functions as Policy has them (ADR 0012, 177): whether the window is SSD, its extents in
//! every state, and its strips as `MemoryRenderBuffer`s, which it draws again only when
//! something they're drawn from changes. Its shadow (171) is drawn
//! with them, and isn't part of the frame.

pub mod control;
pub mod palette;
pub mod shadow;
pub mod style;
#[cfg(test)]
mod tests;

use std::sync::Arc;

use caminito_policy::{FrameState, Functions, WindowType};
use caminito_theme::cairo::{self, Context, Format, ImageSurface};
use caminito_theme::{
    ButtonFunction, ButtonLayout, ButtonStates, Frame, FrameFlags, FrameGeometry, FrameType,
    PaletteColors, Rect, Strip, Theme, ThemeDirs, Title, pango, title_context,
};
use smithay::backend::allocator::Fourcc;
use smithay::backend::renderer::element::memory::MemoryRenderBuffer;
use smithay::reexports::wayland_server::protocol::wl_buffer::WlBuffer;
use smithay::utils::{Buffer, FrameExtents, Logical, Point, Rectangle, Size, Transform};
use smithay::wayland::shm::with_buffer_contents;

use crate::decoration::control::FrameControl;
use crate::decoration::shadow::{ShadowInputs, ShadowType};
use crate::decoration::style::{FrameFacts, frame_style};
use crate::esmeril::Mask;
use crate::icons::IconLookup;
use crate::settings::Settings;
use crate::settings::translate::{Antialiasing, FontRendering, Hinting, RgbaOrder};

/// The resolution titles are laid out at: GTK's, with no DPI setting.
const TITLE_DPI: f64 = 96.0;

/// Marco's mini and full icon sizes, in logical pixels.
const MINI_ICON_SIZE: i32 = 16;
const ICON_SIZE: i32 = 48;

/// What every frame is drawn with.
pub struct FrameContext {
    theme_dirs: ThemeDirs,
    /// The theme `theme` names, which may have failed to load.
    theme_name: String,
    theme: Theme,
    font: pango::FontDescription,
    font_rendering: FontRendering,
    title_context: pango::Context,
    button_layout: ButtonLayout,
    /// Whether frames have shadows: `compositing-manager` (171).
    shadows: bool,
    icons: IconLookup,
    /// Counts changes to any of the above or to the palette, so frames know to draw again.
    generation: u64,
}

impl FrameContext {
    /// Loads the theme `settings` name from `theme_dirs`, or the built-in one if it fails.
    pub fn new(settings: &Settings, theme_dirs: ThemeDirs) -> Self {
        let font_rendering = settings.font_rendering;
        FrameContext {
            theme: load_theme(&settings.frames.theme, &theme_dirs),
            theme_name: settings.frames.theme.clone(),
            theme_dirs,
            font: pango::FontDescription::from_string(&settings.frames.titlebar_font),
            font_rendering,
            title_context: new_title_context(&font_rendering),
            button_layout: settings.frames.button_layout.clone(),
            shadows: settings.frames.shadows,
            icons: IconLookup::from_env(&settings.interface.icon_theme),
            generation: 0,
        }
    }

    /// Takes up new settings. Returns whether anything frames are drawn with changed.
    pub fn settings_changed(&mut self, settings: &Settings) -> bool {
        let frames = &settings.frames;
        let mut changed = false;
        if frames.theme != self.theme_name {
            self.theme = load_theme(&frames.theme, &self.theme_dirs);
            self.theme_name = frames.theme.clone();
            changed = true;
        }
        let font = pango::FontDescription::from_string(&frames.titlebar_font);
        if font != self.font {
            self.font = font;
            changed = true;
        }
        if settings.font_rendering != self.font_rendering {
            self.font_rendering = settings.font_rendering;
            self.title_context = new_title_context(&self.font_rendering);
            changed = true;
        }
        if frames.button_layout != self.button_layout {
            self.button_layout = frames.button_layout.clone();
            changed = true;
        }
        if frames.shadows != self.shadows {
            self.shadows = frames.shadows;
            changed = true;
        }
        if settings.interface.icon_theme != self.icons.theme() {
            self.icons.set_theme(&settings.interface.icon_theme);
            changed = true;
        }
        if changed {
            self.generation += 1;
        }
        changed
    }

    /// The palette in use changed, so every frame is drawn again.
    pub fn palette_changed(&mut self) {
        self.generation += 1;
    }

    pub fn theme(&self) -> &Theme {
        &self.theme
    }

    /// The extents of the frame a window of `facts` has in `state`, which policy places and
    /// maximises by: none if it has no frame. A pending window's are worked out here too,
    /// before it has a [`WindowFrame`].
    pub fn extents(&self, facts: &WindowFacts, state: FrameState) -> FrameExtents<i32, Logical> {
        style(facts, state, extent_functions(state))
            .map(|(frame_type, flags)| self.visible_borders(frame_type, &flags))
            .unwrap_or_default()
    }

    /// The frame's visible borders around the window geometry. The invisible resize border
    /// lies outside them.
    fn visible_borders(
        &self,
        frame_type: FrameType,
        flags: &FrameFlags,
    ) -> FrameExtents<i32, Logical> {
        let text_height = self.title(frame_type, flags, "").text_height;
        self.visible_borders_at(frame_type, flags, text_height)
    }

    /// [`Self::visible_borders`], for a title of `text_height`.
    fn visible_borders_at(
        &self,
        frame_type: FrameType,
        flags: &FrameFlags,
        text_height: i32,
    ) -> FrameExtents<i32, Logical> {
        let visible = self
            .theme
            .frame_borders(frame_type, text_height, flags)
            .visible;
        FrameExtents::new(visible.left, visible.right, visible.top, visible.bottom)
    }

    pub fn button_layout(&self) -> &ButtonLayout {
        &self.button_layout
    }

    /// Counts the changes to anything frames are drawn with, the palette too.
    pub(crate) fn generation(&self) -> u64 {
        self.generation
    }

    /// The pango context text is laid out in, with `org.mate.font-rendering`'s options.
    pub(crate) fn text_context(&self) -> &pango::Context {
        &self.title_context
    }

    /// A window's icon at `size` logical pixels times `scale`, as frames find it (07).
    pub(crate) fn window_icon(
        &mut self,
        source: &IconSource,
        size: i32,
        scale: i32,
    ) -> Option<ImageSurface> {
        window_icon(&mut self.icons, source, size, scale)
    }

    /// The desktop-file ID of the desktop entry for `app_id`, as frames find its icon by.
    pub(crate) fn desktop_id(&mut self, app_id: &str) -> Option<String> {
        self.icons.desktop_id(app_id)
    }

    /// A window's title laid out in the title font, as the frame's style scales it.
    pub fn title(&self, frame_type: FrameType, flags: &FrameFlags, text: &str) -> Title {
        let scale = self.theme.title_scale(frame_type, flags);
        Title::new(&self.title_context, &self.font, scale, text)
    }
}

/// The frame type and flags Marco draws a window of `facts` with in `state`, with
/// `functions`, or `None` if it has no frame. A fullscreen window shows no frame, so it's
/// never asked for.
fn style(
    facts: &WindowFacts,
    state: FrameState,
    functions: Functions,
) -> Option<(FrameType, FrameFlags)> {
    if !facts.wants_frame {
        return None;
    }
    frame_style(&FrameFacts {
        window_type: facts.window_type,
        functions,
        activated: state.focused,
        maximized: state.maximized,
        fullscreen: false,
        shaded: state.shaded,
        tiled_left: state.tiled_left,
        tiled_right: state.tiled_right,
    })
}

/// The functions a frame's extents can depend on, which Policy asks for them with in the
/// [`FrameState`] (177): the others only add or take a button.
fn extent_functions(state: FrameState) -> Functions {
    Functions {
        border_only: state.border_only,
        resize_horizontally: state.resizes_horizontally,
        resize_vertically: state.resizes_vertically,
        ..Functions::default()
    }
}

/// The theme named `name`, or Caminito's built-in one if it doesn't load (07).
fn load_theme(name: &str, dirs: &ThemeDirs) -> Theme {
    match Theme::load(name, dirs) {
        Ok(theme) => theme,
        Err(err) => {
            tracing::warn!("can't load the Marco theme {name}: {err}; using the built-in theme");
            Theme::builtin()
        }
    }
}

/// A title context with `org.mate.font-rendering`'s options.
fn new_title_context(rendering: &FontRendering) -> pango::Context {
    let mut options = cairo::FontOptions::new().expect("cairo makes font options");
    options.set_antialias(match rendering.antialiasing {
        Antialiasing::None => cairo::Antialias::None,
        Antialiasing::Grayscale => cairo::Antialias::Gray,
        Antialiasing::Rgba => cairo::Antialias::Subpixel,
    });
    options.set_hint_style(match rendering.hinting {
        Hinting::None => cairo::HintStyle::None,
        Hinting::Slight => cairo::HintStyle::Slight,
        Hinting::Medium => cairo::HintStyle::Medium,
        Hinting::Full => cairo::HintStyle::Full,
    });
    options.set_subpixel_order(match rendering.rgba_order {
        RgbaOrder::Rgb => cairo::SubpixelOrder::Rgb,
        RgbaOrder::Bgr => cairo::SubpixelOrder::Bgr,
        RgbaOrder::Vrgb => cairo::SubpixelOrder::Vrgb,
        RgbaOrder::Vbgr => cairo::SubpixelOrder::Vbgr,
    });
    title_context(&options, TITLE_DPI, &pango::Language::default())
}

/// Where a window's icon comes from (07).
#[derive(Debug, Clone, PartialEq)]
pub enum IconSource {
    /// xdg-toplevel-icon's: a name in the icon theme, else its buffers with their scales,
    /// else the app id's as for `AppId`.
    Toplevel {
        name: Option<String>,
        buffers: Vec<(WlBuffer, i32)>,
        app_id: Option<String>,
    },
    /// An XWayland window's `_NET_WM_ICON`, else the app id's as for `AppId` (07, 39).
    X11 {
        icon: NetWmIcon,
        app_id: Option<String>,
    },
    /// The desktop entry's icon for the app id, else `application-x-executable`.
    AppId(Option<String>),
}

/// `_NET_WM_ICON`'s images. Two are the same icon only if they were read together, so
/// comparing them is cheap.
#[derive(Debug, Clone)]
pub struct NetWmIcon(Arc<[IconImage]>);

/// One of `_NET_WM_ICON`'s images: non-premultiplied ARGB, one word a pixel, row by row.
#[derive(Debug)]
struct IconImage {
    width: i32,
    height: i32,
    argb: Vec<u32>,
}

impl PartialEq for NetWmIcon {
    fn eq(&self, other: &Self) -> bool {
        Arc::ptr_eq(&self.0, &other.0)
    }
}

impl NetWmIcon {
    /// Parses `_NET_WM_ICON`: width, height, then that many pixels, for each image. A
    /// truncated last image is left out. `None` if there's no whole image.
    pub fn parse(value: &[u32]) -> Option<Self> {
        let mut images = Vec::new();
        let mut rest = value;
        while let [width, height, pixels @ ..] = rest {
            let (width, height) = (*width as usize, *height as usize);
            let len = width.checked_mul(height)?;
            if width == 0 || len > pixels.len() || width > i32::MAX as usize {
                break;
            }
            images.push(IconImage {
                width: width as i32,
                height: height as i32,
                argb: pixels[..len].to_vec(),
            });
            rest = &pixels[len..];
        }
        (!images.is_empty()).then(|| NetWmIcon(images.into()))
    }
}

/// What a window's frame is derived from besides its [`FrameState`] and its functions,
/// which are Policy's (177): facts that change only when the window commits or its
/// decoration mode changes.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct WindowFacts {
    pub window_type: WindowType,
    /// It wants Caminito's frame.
    pub wants_frame: bool,
}

/// What a window's strips show of it.
#[derive(Debug, Clone, PartialEq)]
pub struct FrameContent {
    /// The window geometry's size.
    pub size: Size<i32, Logical>,
    pub title: String,
    pub icon: IconSource,
    /// The integer scale the strips are drawn at (13).
    pub scale: i32,
    /// Whether the visual bell flashes the frame (169).
    pub flashing: bool,
    /// Whether the window is kept above, and sticky, which show in the buttons the frame has
    /// (155).
    pub above: bool,
    pub stuck: bool,
}

/// What a window's strips are drawn from, besides [`FrameContext`].
#[derive(Debug, Clone, PartialEq)]
struct FrameInputs {
    pub frame_type: FrameType,
    pub flags: FrameFlags,
    /// The window geometry's size.
    pub size: Size<i32, Logical>,
    pub title: String,
    pub icon: IconSource,
    /// The integer scale the strips are drawn at (13).
    pub scale: i32,
}

/// One strip, and where it goes from the window geometry's origin.
#[derive(Debug)]
struct StripBuffer {
    rect: Rectangle<i32, Logical>,
    buffer: MemoryRenderBuffer,
    /// Where Esmeril goes behind it, if it has see-through pixels (ADR 0048).
    esmeril: Option<Mask>,
}

/// The window's icons at mini and full size, and what they were loaded for.
#[derive(Debug)]
struct WindowIcons {
    source: IconSource,
    scale: i32,
    icon_theme: String,
    mini: Option<ImageSurface>,
    full: Option<ImageSurface>,
}

/// One window's frame: whether it has one, its extents in every state, its strips, and what
/// they were last drawn from.
#[derive(Debug, Default)]
pub struct WindowFrame {
    facts: Option<WindowFacts>,
    ssd: bool,
    /// The extents in each [`FrameState`], by [`state_index`].
    extents: Vec<FrameExtents<i32, Logical>>,
    drawn: Option<(FrameInputs, ButtonStates, u64)>,
    /// Where everything was when the strips were last drawn, which the pointer finds the
    /// frame's controls by.
    geometry: Option<FrameGeometry>,
    strips: Vec<StripBuffer>,
    /// The shadow's bands, from the visible frame's top left corner, and what they were
    /// drawn from: kept while that's the same, whatever else of the frame changes (171).
    shadow: Option<(ShadowInputs, Vec<StripBuffer>)>,
    icons: Option<WindowIcons>,
    button_states: ButtonStates,
    renders: u64,
    shadow_renders: u64,
}

/// Where a state's extents are in [`WindowFrame::extents`].
fn state_index(state: FrameState) -> usize {
    let tiled = if state.tiled_left {
        1
    } else if state.tiled_right {
        2
    } else {
        0
    };
    tiled << 6
        | usize::from(state.border_only) << 5
        | usize::from(state.resizes_horizontally) << 4
        | usize::from(state.resizes_vertically) << 3
        | usize::from(state.maximized) << 2
        | usize::from(state.focused) << 1
        | usize::from(state.shaded)
}

/// How many [`FrameState`]s there are.
const FRAME_STATES: usize = 3 << 6;

impl WindowFrame {
    /// Takes up the window's facts, or the theme they're framed with: works out again
    /// whether the window is SSD, and its extents in every state.
    pub fn set_facts(&mut self, facts: WindowFacts, frame_context: &FrameContext) {
        let default = FrameState::default();
        self.ssd = style(&facts, default, extent_functions(default)).is_some();
        // A title is laid out once for each scale the theme's styles give it, not for each
        // state.
        let mut text_heights: Vec<(f64, i32)> = Vec::new();
        self.extents = (0..FRAME_STATES)
            .map(|index| {
                let state = FrameState {
                    maximized: index & 4 != 0,
                    focused: index & 2 != 0,
                    shaded: index & 1 != 0,
                    resizes_vertically: index & 8 != 0,
                    resizes_horizontally: index & 16 != 0,
                    border_only: index & 32 != 0,
                    tiled_left: index >> 6 == 1,
                    tiled_right: index >> 6 == 2,
                };
                debug_assert_eq!(state_index(state), index);
                let Some((frame_type, flags)) = style(&facts, state, extent_functions(state))
                else {
                    return FrameExtents::default();
                };
                let scale = frame_context.theme.title_scale(frame_type, &flags);
                let known = text_heights.iter().find(|(known, _)| *known == scale);
                let text_height = match known {
                    Some((_, text_height)) => *text_height,
                    None => {
                        let text_height = frame_context.title(frame_type, &flags, "").text_height;
                        text_heights.push((scale, text_height));
                        text_height
                    }
                };
                frame_context.visible_borders_at(frame_type, &flags, text_height)
            })
            .collect();
        self.facts = Some(facts);
    }

    /// Whether Caminito draws a frame for the window.
    pub fn is_ssd(&self) -> bool {
        self.ssd
    }

    /// The frame's extents in `state`: none for a CSD window, or before the facts are set.
    pub fn extents(&self, state: FrameState) -> FrameExtents<i32, Logical> {
        self.extents
            .get(state_index(state))
            .copied()
            .unwrap_or_default()
    }

    /// Draws the strips for `content` in `state` with `functions` again if anything they're
    /// drawn from changed since they were last drawn, or drops them if the window has no
    /// frame. Returns whether the strips changed.
    pub fn refresh(
        &mut self,
        frame_context: &mut FrameContext,
        colors: &PaletteColors,
        state: FrameState,
        functions: Functions,
        content: FrameContent,
    ) -> bool {
        let style = self.facts.and_then(|facts| style(&facts, state, functions));
        let Some((frame_type, mut flags)) = style else {
            return self.clear();
        };
        let FrameContent {
            size,
            title,
            icon,
            scale,
            flashing,
            above,
            stuck,
        } = content;
        flags.is_flashing = flashing;
        flags.above = above;
        flags.stuck = stuck;
        let inputs = FrameInputs {
            frame_type,
            flags,
            size,
            title,
            icon,
            scale,
        };
        self.update(frame_context, colors, inputs)
    }

    /// Draws the strips again if anything they're drawn from changed since they were last
    /// drawn. Returns whether it did.
    fn update(
        &mut self,
        frame_context: &mut FrameContext,
        colors: &PaletteColors,
        inputs: FrameInputs,
    ) -> bool {
        let key = (inputs, self.button_states, frame_context.generation);
        if self.drawn.as_ref() == Some(&key) {
            return false;
        }
        let (inputs, _, _) = &key;
        self.load_icons(&mut frame_context.icons, &inputs.icon, inputs.scale);
        let title = frame_context.title(inputs.frame_type, &inputs.flags, &inputs.title);
        let geometry = frame_context.theme.frame_geometry(
            inputs.frame_type,
            title.text_height,
            &inputs.flags,
            inputs.size.w,
            inputs.size.h,
            &frame_context.button_layout,
        );
        self.strips = self.draw(frame_context, colors, inputs, &title, &geometry);
        self.update_shadow(frame_context.shadows, inputs, &geometry);
        self.geometry = Some(geometry);
        self.drawn = Some(key);
        self.renders += 1;
        true
    }

    /// Draws the shadow of the frame `inputs` and `geometry` describe, unless the one it has
    /// was drawn from the same, or drops it where the frame has none: `shadows` is off, or
    /// Marco draws none for it.
    fn update_shadow(&mut self, shadows: bool, inputs: &FrameInputs, geometry: &FrameGeometry) {
        let Some(shadow_type) = ShadowType::of(&inputs.flags).filter(|_| shadows) else {
            self.shadow = None;
            return;
        };
        let scale = inputs.scale;
        let visible = geometry.visible_rect();
        let cutouts = geometry
            .corner_cutouts(scale)
            .into_iter()
            .map(|cutout| {
                Rectangle::<i32, Buffer>::new(
                    (cutout.x - visible.x * scale, cutout.y - visible.y * scale).into(),
                    (cutout.width, cutout.height).into(),
                )
            })
            .collect();
        let shadow = ShadowInputs {
            shadow_type,
            size: (visible.width, visible.height).into(),
            scale,
            cutouts,
        };
        if self
            .shadow
            .as_ref()
            .is_some_and(|(drawn, _)| *drawn == shadow)
        {
            return;
        }
        let bands = shadow::bands(&shadow)
            .into_iter()
            .map(|band| StripBuffer {
                rect: band.rect,
                buffer: MemoryRenderBuffer::from_slice(
                    &band.pixels,
                    Fourcc::Argb8888,
                    (band.size.w, band.size.h),
                    scale,
                    Transform::Normal,
                    None,
                ),
                esmeril: None,
            })
            .collect();
        self.shadow = Some((shadow, bands));
        self.shadow_renders += 1;
    }

    /// Drops the strips, for a window that shows no frame. Returns whether it had any.
    pub fn clear(&mut self) -> bool {
        self.drawn = None;
        self.geometry = None;
        self.shadow = None;
        !std::mem::take(&mut self.strips).is_empty()
    }

    /// The state each button is drawn in, from the next time the strips are drawn.
    pub fn set_button_states(&mut self, states: ButtonStates) {
        self.button_states = states;
    }

    pub fn button_states(&self) -> ButtonStates {
        self.button_states
    }

    /// The style set the frame was last drawn in, if it's showing.
    pub fn frame_type(&self) -> Option<FrameType> {
        self.drawn.as_ref().map(|(inputs, _, _)| inputs.frame_type)
    }

    /// The flags the frame was last drawn with, if it's showing.
    pub fn flags(&self) -> Option<FrameFlags> {
        self.drawn.as_ref().map(|(inputs, _, _)| inputs.flags)
    }

    /// The control at `point` from the window geometry's origin, or `None` where the frame
    /// takes no input.
    pub fn control_at(&self, point: Point<f64, Logical>) -> Option<FrameControl> {
        let geometry = self.geometry.as_ref()?;
        let flags = self.flags()?;
        let total = geometry.borders.total;
        let point = point.to_i32_floor::<i32>() + Point::from((total.left, total.top));
        control::in_input_region(geometry, point.x, point.y)
            .then(|| control::control_at(geometry, &flags, point.x, point.y))
    }

    /// Where a button is drawn, from the window geometry's origin, if the frame shows it.
    pub fn button_rect(&self, function: ButtonFunction) -> Option<Rectangle<i32, Logical>> {
        let geometry = self.geometry.as_ref()?;
        Some(from_frame_origin(
            geometry,
            geometry.button(function)?.visible,
        ))
    }

    /// The whole frame, its invisible resize border included, from the window geometry's
    /// origin, if it's showing.
    pub fn bounds(&self) -> Option<Rectangle<i32, Logical>> {
        let geometry = self.geometry.as_ref()?;
        let rect = Rect {
            x: 0,
            y: 0,
            width: geometry.width,
            height: geometry.height,
        };
        Some(from_frame_origin(geometry, rect))
    }

    /// Where the title goes, from the window geometry's origin, if the frame is showing and
    /// the title fits.
    pub fn title_rect(&self) -> Option<Rectangle<i32, Logical>> {
        let geometry = self.geometry.as_ref()?;
        let rect = geometry.title_rect;
        (rect.width > 0 && rect.height > 0).then(|| from_frame_origin(geometry, rect))
    }

    /// Each strip's buffer, and where it goes from the window geometry's origin.
    pub fn strips(&self) -> impl Iterator<Item = (Rectangle<i32, Logical>, &MemoryRenderBuffer)> {
        self.strips.iter().map(|strip| (strip.rect, &strip.buffer))
    }

    /// The mask of each strip with see-through pixels (ADR 0048), and where the strip goes
    /// from the window geometry's origin.
    pub(crate) fn esmeril(&self) -> impl Iterator<Item = (Rectangle<i32, Logical>, &Mask)> {
        self.strips
            .iter()
            .filter_map(|strip| Some((strip.rect, strip.esmeril.as_ref()?)))
    }

    /// The frame's visible rectangle, from the window geometry's origin, if it's showing.
    pub fn visible(&self) -> Option<Rectangle<i32, Logical>> {
        let geometry = self.geometry.as_ref()?;
        Some(from_frame_origin(geometry, geometry.visible_rect()))
    }

    /// Each band of the frame's shadow, and where it goes from the window geometry's
    /// origin: none for a frame with no shadow.
    pub fn shadow(&self) -> impl Iterator<Item = (Rectangle<i32, Logical>, &MemoryRenderBuffer)> {
        let origin = self.visible().map(|visible| visible.loc);
        let bands = self.shadow.iter().flat_map(|(_, bands)| bands);
        bands.filter_map(move |band| {
            Some((
                Rectangle::new(band.rect.loc + origin?, band.rect.size),
                &band.buffer,
            ))
        })
    }

    /// The shadow's rectangle, from the window geometry's origin, if the frame has one.
    pub fn shadow_rect(&self) -> Option<Rectangle<i32, Logical>> {
        let (shadow, _) = self.shadow.as_ref()?;
        Some(shadow.shadow_type.rect(self.visible()?))
    }

    /// The scale the strips were drawn at, while the frame is showing.
    pub fn drawn_scale(&self) -> Option<i32> {
        self.drawn.as_ref().map(|(inputs, _, _)| inputs.scale)
    }

    /// The scale the strips were last drawn at.
    pub fn scale(&self) -> i32 {
        self.drawn.as_ref().map_or(1, |(inputs, _, _)| inputs.scale)
    }

    /// The colour at the middle of the mini icon the frame last loaded, as cairo's ARGB32.
    #[cfg(test)]
    pub fn mini_icon_center(&self) -> Option<u32> {
        let icon = self.icons.as_ref()?.mini.as_ref()?;
        let (x, y) = (icon.width() / 2, icon.height() / 2);
        let offset = (y * icon.stride() + x * 4) as usize;
        let mut pixel = None;
        icon.with_data(|data| {
            pixel = Some(u32::from_ne_bytes(
                data[offset..offset + 4].try_into().unwrap(),
            ))
        })
        .ok()?;
        pixel
    }

    /// How many times the strips have been drawn.
    pub fn renders(&self) -> u64 {
        self.renders
    }

    /// How many times the shadow has been drawn.
    pub fn shadow_renders(&self) -> u64 {
        self.shadow_renders
    }

    fn draw(
        &self,
        frame_context: &FrameContext,
        colors: &PaletteColors,
        inputs: &FrameInputs,
        title: &Title,
        geometry: &FrameGeometry,
    ) -> Vec<StripBuffer> {
        let theme = &frame_context.theme;
        let icons = self.icons.as_ref();
        let frame = Frame {
            frame_type: inputs.frame_type,
            flags: inputs.flags,
            client_width: inputs.size.w,
            client_height: inputs.size.h,
            title,
            button_layout: &frame_context.button_layout,
            button_states: self.button_states,
            mini_icon: icons.and_then(|icons| icons.mini.as_ref()),
            icon: icons.and_then(|icons| icons.full.as_ref()),
            colors,
        };
        Strip::ALL
            .into_iter()
            .filter_map(|strip| {
                let rect = geometry.strip_rect(strip);
                if rect.width <= 0 || rect.height <= 0 {
                    return None;
                }
                let mut surface = theme
                    .render_strip(&frame, strip, inputs.scale)
                    .inspect_err(|err| {
                        tracing::warn!("can't draw a frame's {strip:?} strip: {err}")
                    })
                    .ok()?;
                let esmeril = Mask::of(&mut surface);
                Some(StripBuffer {
                    rect: from_frame_origin(geometry, rect),
                    buffer: memory_buffer(surface, inputs.scale)?,
                    esmeril,
                })
            })
            .collect()
    }

    /// Loads the icons, unless they're already loaded from `source` at `scale`.
    fn load_icons(&mut self, lookup: &mut IconLookup, source: &IconSource, scale: i32) {
        let loaded = self.icons.as_ref().is_some_and(|icons| {
            icons.source == *source && icons.scale == scale && icons.icon_theme == lookup.theme()
        });
        if loaded {
            return;
        }
        let mut load = |size: i32| window_icon(lookup, source, size, scale);
        let mini = load(MINI_ICON_SIZE);
        let full = load(ICON_SIZE);
        self.icons = Some(WindowIcons {
            source: source.clone(),
            scale,
            icon_theme: lookup.theme().to_owned(),
            mini,
            full,
        });
    }
}

/// `rect`, from the frame's top left corner, from the window geometry's origin instead.
fn from_frame_origin(geometry: &FrameGeometry, rect: Rect) -> Rectangle<i32, Logical> {
    let total = geometry.borders.total;
    Rectangle::new(
        (rect.x - total.left, rect.y - total.top).into(),
        (rect.width, rect.height).into(),
    )
}

/// A window's icon at `size` logical pixels times `scale`, in the order 07 gives.
fn window_icon(
    lookup: &mut IconLookup,
    source: &IconSource,
    size: i32,
    scale: i32,
) -> Option<ImageSurface> {
    let pixels = size * scale;
    let from_file = |lookup: &mut IconLookup, app_id: Option<&str>| {
        let path = lookup.window_icon(app_id, size, scale)?;
        lookup.load(&path, pixels)
    };
    match source {
        IconSource::Toplevel {
            name,
            buffers,
            app_id,
        } => name
            .as_deref()
            .and_then(|name| lookup.find_icon(name, size, scale))
            .and_then(|path| lookup.load(&path, pixels))
            .or_else(|| icon_from_buffers(buffers, pixels))
            .or_else(|| from_file(lookup, app_id.as_deref())),
        IconSource::X11 { icon, app_id } => {
            icon_from_net_wm_icon(icon, pixels).or_else(|| from_file(lookup, app_id.as_deref()))
        }
        IconSource::AppId(app_id) => from_file(lookup, app_id.as_deref()),
    }
}

/// The `_NET_WM_ICON` image nearest `pixels` wide, scaled to `pixels` square.
fn icon_from_net_wm_icon(icon: &NetWmIcon, pixels: i32) -> Option<ImageSurface> {
    // The smallest at least as big, else the biggest.
    let images = icon.0.iter();
    let nearest = images
        .clone()
        .filter(|image| image.width >= pixels)
        .min_by_key(|image| image.width)
        .or_else(|| images.max_by_key(|image| image.width))?;
    let surface = premultiplied(nearest)?;
    scaled(&surface, pixels)
        .inspect_err(|err| tracing::warn!("can't scale a window icon: {err}"))
        .ok()
}

/// An `_NET_WM_ICON` image as cairo's ARGB32, whose colours are premultiplied by alpha.
fn premultiplied(image: &IconImage) -> Option<ImageSurface> {
    let mut surface = ImageSurface::create(Format::ARgb32, image.width, image.height).ok()?;
    let stride = surface.stride() as usize;
    {
        let mut data = surface.data().ok()?;
        for (y, row) in image.argb.chunks(image.width as usize).enumerate() {
            for (x, argb) in row.iter().enumerate() {
                let alpha = argb >> 24;
                let channel = |shift: u32| ((argb >> shift & 0xff) * alpha + 127) / 255;
                let pixel = alpha << 24 | channel(16) << 16 | channel(8) << 8 | channel(0);
                let offset = y * stride + x * 4;
                data[offset..offset + 4].copy_from_slice(&pixel.to_ne_bytes());
            }
        }
    }
    surface.mark_dirty();
    Some(surface)
}

/// The xdg-toplevel-icon buffer nearest `pixels` square, scaled to it.
fn icon_from_buffers(buffers: &[(WlBuffer, i32)], pixels: i32) -> Option<ImageSurface> {
    let surfaces: Vec<ImageSurface> = buffers
        .iter()
        .filter_map(|(buffer, _)| surface_from_shm(buffer))
        .collect();
    // The smallest at least as big, else the biggest.
    let nearest = surfaces
        .iter()
        .filter(|s| s.width() >= pixels)
        .min_by_key(|s| s.width())
        .or_else(|| surfaces.iter().max_by_key(|s| s.width()))?;
    scaled(nearest, pixels)
        .inspect_err(|err| tracing::warn!("can't scale a window icon: {err}"))
        .ok()
}

/// A copy of an ARGB8888 or XRGB8888 shm buffer, which are cairo's ARGB32 and RGB24.
fn surface_from_shm(buffer: &WlBuffer) -> Option<ImageSurface> {
    use smithay::reexports::wayland_server::protocol::wl_shm::Format as ShmFormat;
    with_buffer_contents(buffer, |ptr, len, data| {
        let format = match data.format {
            ShmFormat::Argb8888 => Format::ARgb32,
            ShmFormat::Xrgb8888 => Format::Rgb24,
            _ => return None,
        };
        let (width, height, stride) = (data.width, data.height, data.stride);
        let end = data.offset as usize + stride as usize * height as usize;
        if width <= 0 || height <= 0 || stride < width * 4 || end > len {
            return None;
        }
        // Safety: the pool is mapped for `len` bytes while this runs.
        let pool = unsafe { std::slice::from_raw_parts(ptr, len) };
        let pixels = &pool[data.offset as usize..end];
        let mut surface = ImageSurface::create(format, width, height).ok()?;
        let surface_stride = surface.stride() as usize;
        {
            let mut target = surface.data().ok()?;
            let row = width as usize * 4;
            for y in 0..height as usize {
                target[y * surface_stride..][..row]
                    .copy_from_slice(&pixels[y * stride as usize..][..row]);
            }
        }
        surface.mark_dirty();
        Some(surface)
    })
    .ok()
    .flatten()
}

/// `surface` scaled to `pixels` square.
fn scaled(surface: &ImageSurface, pixels: i32) -> Result<ImageSurface, cairo::Error> {
    let target = ImageSurface::create(Format::ARgb32, pixels, pixels)?;
    let cr = Context::new(&target)?;
    cr.scale(
        f64::from(pixels) / f64::from(surface.width()),
        f64::from(pixels) / f64::from(surface.height()),
    );
    cr.set_source_surface(surface, 0.0, 0.0)?;
    cr.paint()?;
    drop(cr);
    Ok(target)
}

/// A cairo surface's pixels, such as a strip's, as a buffer at `scale`. Cairo's ARGB32 is a native-endian word, which
/// is `Argb8888` on the little-endian machines Caminito runs on.
pub(crate) fn memory_buffer(mut surface: ImageSurface, scale: i32) -> Option<MemoryRenderBuffer> {
    let (width, height) = (surface.width(), surface.height());
    let stride = surface.stride() as usize;
    let data = surface.data().ok()?;
    let row = width as usize * 4;
    let mut pixels = Vec::with_capacity(row * height as usize);
    for line in data.chunks(stride) {
        pixels.extend_from_slice(&line[..row]);
    }
    Some(MemoryRenderBuffer::from_slice(
        &pixels,
        Fourcc::Argb8888,
        (width, height),
        scale,
        Transform::Normal,
        None,
    ))
}
