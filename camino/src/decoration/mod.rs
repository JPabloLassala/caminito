//! Camino's frames (07, 15): the Marco theme's frame drawn around each SSD window as four
//! strips, and the colours they're drawn with.
//!
//! [`FrameContext`] holds what every frame is drawn with: the theme, the title font and how
//! it's rendered, the button layout and the icon theme. Each window's [`WindowFrame`] derives
//! its frame from the window's [`WindowFacts`] and a [`FrameState`] (ADR 0012): whether the
//! window is SSD, its extents in every state, and its strips as `MemoryRenderBuffer`s, which
//! it draws again only when something they're drawn from changes.

pub mod control;
pub mod palette;
pub mod style;
#[cfg(test)]
mod tests;

use camino_policy::{FrameState, SizeHints, WindowType};
use camino_theme::cairo::{self, Context, Format, ImageSurface};
use camino_theme::{
    ButtonFunction, ButtonLayout, ButtonStates, Frame, FrameFlags, FrameGeometry, FrameType,
    PaletteColors, Rect, Strip, Theme, ThemeDirs, Title, pango, title_context,
};
use smithay::backend::allocator::Fourcc;
use smithay::backend::renderer::element::memory::MemoryRenderBuffer;
use smithay::reexports::wayland_server::protocol::wl_buffer::WlBuffer;
use smithay::utils::{FrameExtents, Logical, Point, Rectangle, Size, Transform};
use smithay::wayland::shm::with_buffer_contents;

use crate::decoration::control::FrameControl;
use crate::decoration::style::{FrameFacts, frame_style};
use crate::icons::{IconLookup, load_icon};
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
    attach_modal_dialogs: bool,
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
            attach_modal_dialogs: settings.frames.attach_modal_dialogs,
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
        if frames.attach_modal_dialogs != self.attach_modal_dialogs {
            self.attach_modal_dialogs = frames.attach_modal_dialogs;
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

    pub fn attach_modal_dialogs(&self) -> bool {
        self.attach_modal_dialogs
    }

    /// The extents of the frame a window of `facts` has in `state`, which policy places and
    /// maximises by: none if it has no frame. A pending window's are worked out here too,
    /// before it has a [`WindowFrame`].
    pub fn extents(&self, facts: &WindowFacts, state: FrameState) -> FrameExtents<i32, Logical> {
        self.style(facts, state)
            .map(|(frame_type, flags)| self.visible_borders(frame_type, &flags))
            .unwrap_or_default()
    }

    /// The frame type and flags Marco draws a window of `facts` with in `state`, or `None`
    /// if it has no frame. A fullscreen window shows no frame, so it's never asked for.
    fn style(&self, facts: &WindowFacts, state: FrameState) -> Option<(FrameType, FrameFlags)> {
        if !facts.wants_frame {
            return None;
        }
        let facts = FrameFacts {
            window_type: facts.window_type,
            has_parent: facts.has_parent,
            hints: facts.hints,
            activated: state.focused,
            maximized: state.maximized,
            fullscreen: false,
            shaded: state.shaded,
        };
        frame_style(&facts, self.attach_modal_dialogs)
    }

    /// The frame's visible borders around the window geometry. The invisible resize border
    /// lies outside them.
    fn visible_borders(
        &self,
        frame_type: FrameType,
        flags: &FrameFlags,
    ) -> FrameExtents<i32, Logical> {
        let title = self.title(frame_type, flags, "");
        let visible = self
            .theme
            .frame_borders(frame_type, title.text_height, flags)
            .visible;
        FrameExtents::new(visible.left, visible.right, visible.top, visible.bottom)
    }

    pub fn button_layout(&self) -> &ButtonLayout {
        &self.button_layout
    }

    /// A window's title laid out in the title font, as the frame's style scales it.
    pub fn title(&self, frame_type: FrameType, flags: &FrameFlags, text: &str) -> Title {
        let scale = self.theme.title_scale(frame_type, flags);
        Title::new(&self.title_context, &self.font, scale, text)
    }
}

/// The theme named `name`, or Camino's built-in one if it doesn't load (07).
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
    /// The desktop entry's icon for the app id, else `application-x-executable`.
    AppId(Option<String>),
}

/// What a window's frame is derived from besides its [`FrameState`]: facts that change only
/// when the window commits or its decoration mode changes.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct WindowFacts {
    pub window_type: WindowType,
    /// It's transient for a managed window.
    pub has_parent: bool,
    pub hints: SizeHints,
    /// It wants Camino's frame.
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
    extents: [FrameExtents<i32, Logical>; 8],
    drawn: Option<(FrameInputs, ButtonStates, u64)>,
    /// Where everything was when the strips were last drawn, which the pointer finds the
    /// frame's controls by.
    geometry: Option<FrameGeometry>,
    strips: Vec<StripBuffer>,
    icons: Option<WindowIcons>,
    button_states: ButtonStates,
    renders: u64,
}

/// Where a state's extents are in [`WindowFrame::extents`].
fn state_index(state: FrameState) -> usize {
    usize::from(state.maximized) << 2 | usize::from(state.focused) << 1 | usize::from(state.shaded)
}

impl WindowFrame {
    /// Takes up the window's facts, or the theme they're framed with: works out again
    /// whether the window is SSD, and its extents in every state.
    pub fn set_facts(&mut self, facts: WindowFacts, frame_context: &FrameContext) {
        self.ssd = frame_context.style(&facts, FrameState::default()).is_some();
        for (index, extents) in self.extents.iter_mut().enumerate() {
            let state = FrameState {
                maximized: index & 4 != 0,
                focused: index & 2 != 0,
                shaded: index & 1 != 0,
            };
            debug_assert_eq!(state_index(state), index);
            *extents = frame_context.extents(&facts, state);
        }
        self.facts = Some(facts);
    }

    /// Whether Camino draws a frame for the window.
    pub fn is_ssd(&self) -> bool {
        self.ssd
    }

    /// The frame's extents in `state`: none for a CSD window.
    pub fn extents(&self, state: FrameState) -> FrameExtents<i32, Logical> {
        self.extents[state_index(state)]
    }

    /// Draws the strips for `content` in `state` again if anything they're drawn from changed
    /// since they were last drawn, or drops them if the window has no frame. Returns whether
    /// the strips changed.
    pub fn refresh(
        &mut self,
        frame_context: &mut FrameContext,
        colors: &PaletteColors,
        state: FrameState,
        content: FrameContent,
    ) -> bool {
        let style = self
            .facts
            .and_then(|facts| frame_context.style(&facts, state));
        let Some((frame_type, flags)) = style else {
            return self.clear();
        };
        let FrameContent {
            size,
            title,
            icon,
            scale,
        } = content;
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
        self.geometry = Some(geometry);
        self.drawn = Some(key);
        self.renders += 1;
        true
    }

    /// Drops the strips, for a window that shows no frame. Returns whether it had any.
    pub fn clear(&mut self) -> bool {
        self.drawn = None;
        self.geometry = None;
        !std::mem::take(&mut self.strips).is_empty()
    }

    /// The state each button is drawn in, from the next time the strips are drawn.
    pub fn set_button_states(&mut self, states: ButtonStates) {
        self.button_states = states;
    }

    pub fn button_states(&self) -> ButtonStates {
        self.button_states
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

    /// The scale the strips were drawn at, while the frame is showing.
    pub fn drawn_scale(&self) -> Option<i32> {
        self.drawn.as_ref().map(|(inputs, _, _)| inputs.scale)
    }

    /// The scale the strips were last drawn at.
    pub fn scale(&self) -> i32 {
        self.drawn.as_ref().map_or(1, |(inputs, _, _)| inputs.scale)
    }

    /// How many times the strips have been drawn.
    pub fn renders(&self) -> u64 {
        self.renders
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
                let surface = theme
                    .render_strip(&frame, strip, inputs.scale)
                    .inspect_err(|err| {
                        tracing::warn!("can't draw a frame's {strip:?} strip: {err}")
                    })
                    .ok()?;
                Some(StripBuffer {
                    rect: from_frame_origin(geometry, rect),
                    buffer: memory_buffer(surface, inputs.scale)?,
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
        load_icon(&path, pixels)
    };
    match source {
        IconSource::Toplevel {
            name,
            buffers,
            app_id,
        } => name
            .as_deref()
            .and_then(|name| lookup.find_icon(name, size, scale))
            .and_then(|path| load_icon(&path, pixels))
            .or_else(|| icon_from_buffers(buffers, pixels))
            .or_else(|| from_file(lookup, app_id.as_deref())),
        IconSource::AppId(app_id) => from_file(lookup, app_id.as_deref()),
    }
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
/// is `Argb8888` on the little-endian machines Camino runs on.
pub(crate) fn memory_buffer(mut surface: ImageSurface, scale: i32) -> Option<MemoryRenderBuffer> {
    let (width, height) = (surface.width(), surface.height());
    let stride = surface.stride() as usize;
    let data = surface.data().ok()?;
    let row = width as usize * 4;
    let pixels: Vec<u8> = data
        .chunks(stride)
        .flat_map(|line| &line[..row])
        .copied()
        .collect();
    Some(MemoryRenderBuffer::from_slice(
        &pixels,
        Fourcc::Argb8888,
        (width, height),
        scale,
        Transform::Normal,
        None,
    ))
}
