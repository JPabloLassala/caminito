//! Esmeril (ADR 0048): a blur of what's beneath, with raised saturation and contrast, drawn
//! behind the see-through parts of a frame, of a popup of Caminito's and of the windows of the
//! listed applications. It's Plasma's Blur effect and its Background
//! contrast effect, whose behaviour and numbers are taken and whose code isn't: the passes
//! and offsets of each strength step from KWin's `src/plugins/blur/blur.cpp`, and the colour
//! transform from `src/plugins/backgroundcontrast/contrast.cpp`, with libplasma's default
//! saturation (`src/plasma/theme.cpp`) and a contrast of Caminito's own. The
//! shaders are written from the dual Kawase algorithm, as Marius Bjørge published it
//! ("Bandwidth-Efficient Rendering", SIGGRAPH 2015).
//!
//! An [`Esmeril`] is a framebuffer effect of Smithay's: its geometry is the strip widened by
//! as much as the blur reads, so that the damage tracker redraws all of that whenever any of
//! it changes, and captures it again. What's captured is blurred once, into textures the
//! tracker keeps for the element, and only the strip's own rectangle is drawn.
//!
//! A popup that slides (ADR 0037) carries its Esmeril with it, cut off at its place and faded
//! as the popup is. That's done here and not by Smithay's `CropRenderElement`, which would cut
//! the element's geometry, and so what the damage tracker redraws of what the blur reads.
//!
//! The alpha of a strip, or of a listed window's surface, says where it's drawn: wherever that
//! has any alpha at all. The strip or the surface is drawn over it, so an opaque pixel hides it
//! and a part-clear one shows it as much as it's see-through, and a wholly clear pixel shows
//! what's beneath with no Esmeril. A surface's alpha is read from its texture as it's drawn, so
//! it follows the client's commits, whose damage redraws the element where they changed it.
//!
//! A window's Esmeril goes with it where an animation draws it (CMN-236): made as if the window
//! were at rest with its frame's corner at the origin, then scaled and moved as the window's
//! elements are, so it matches them. A closing window's picture keeps its frame's masks, and is
//! drawn over them so.
//!
//! A layer surface and its menus (CMN-232) get it by the same road, once their buffer is known
//! to have see-through pixels, and fade and slide as they do. Its tooltips don't: a menu is told
//! from a tooltip by the grab it takes, which a tooltip never does.
//!
//! A client that asks through `ext-background-effect-v1` (CMN-238) gets it in its region, under
//! every pixel there, a wholly clear one too: the region alone says where, and the surface is
//! drawn over it as usual. That region replaces a listed window's or a layer surface's whole-surface
//! Esmeril, on that surface only. On a lock surface it goes right under the surface that asks, so
//! the Locker's window blurs only its own subsurface (CMN-301).
//!
//! Pixman draws nothing for it (ADR 0048), though its damage is the same.

use std::cell::RefCell;
use std::sync::{Arc, Mutex};

use caminito_theme::cairo::ImageSurface;
use smithay::backend::allocator::{Buffer as _, Fourcc, format::has_alpha};
use smithay::backend::renderer::element::{Element, Id, RenderElement};
use smithay::backend::renderer::gles::{
    GlesError, GlesFrame, GlesPixelProgram, GlesRenderer, GlesTexProgram, GlesTexture, Uniform,
    UniformName, UniformType, UniformValue, ffi, link_program,
};
#[cfg(test)]
use smithay::backend::renderer::pixman::PixmanRenderer;
use smithay::backend::renderer::utils::{
    CommitCounter, RendererSurfaceStateUserData, with_renderer_surface_state,
};
use smithay::backend::renderer::{Frame, FrameContext, ImportMem, Offscreen, Renderer, Texture};
use smithay::reexports::wayland_server::protocol::wl_buffer::WlBuffer;
use smithay::reexports::wayland_server::protocol::wl_shm::Format as ShmFormat;
use smithay::reexports::wayland_server::protocol::wl_surface::WlSurface;
use smithay::utils::user_data::UserDataMap;
use smithay::utils::{Buffer, Logical, Physical, Point, Rectangle, Scale, Size, Transform};
use smithay::wayland::background_effect::BackgroundEffectSurfaceCachedState;
use smithay::wayland::compositor::{RectangleKind, RegionAttributes, SurfaceData, with_states};
use smithay::wayland::dmabuf::get_dmabuf;
use smithay::wayland::shm::with_buffer_contents;

/// The default of `esmeril-strength`: KWin's `blur.kcfg`.
pub(crate) const DEFAULT_STRENGTH: u32 = 15;

/// The strongest of `esmeril-strength`, and the number of steps.
pub(crate) const MAX_STRENGTH: u32 = 15;

/// The saturation: the one Plasma's themes give the Background contrast effect by default
/// (libplasma's `Theme::backgroundSaturation`).
const SATURATION: &str = "1.7";

/// The contrast, in the Background contrast effect's transform. It's Caminito's own: Plasma's
/// default contrasts, 0.45 and 0.3, are below 1 and go with an intensity that makes up for
/// them, and ADR 0048 raises the contrast and has no intensity.
const CONTRAST: &str = "1.2";

/// How much alpha a strip's pixel needs to get all of Esmeril. From none at alpha 0 to all
/// here, so that a rounded corner's edge doesn't step from a clear pixel to a blurred one.
const WHOLE_FROM_ALPHA: &str = "0.125";

/// One step of `esmeril-strength`.
#[derive(Debug, Clone, Copy, PartialEq)]
struct Step {
    /// How many times it's halved, and how many times doubled again.
    iterations: usize,
    /// How far from each pixel the passes sample, in half pixels.
    offset: f32,
    /// How far beyond its own rectangle it reads, in physical pixels: KWin's `expandSize`.
    reach: i32,
}

/// What each iteration count allows, as KWin's `initBlurStrengthValues` has it: the smallest
/// offset, the largest, the reach, and how many of the steps it has, which in KWin's
/// arithmetic is its share of the offsets' ranges, rounded up, with what's over taken from
/// the last.
const ITERATIONS: [(f32, f32, i32, usize); 4] = [
    (1.0, 2.0, 10, 2),
    (2.0, 3.0, 20, 2),
    (2.0, 5.0, 50, 5),
    (3.0, 8.0, 150, 6),
];

/// The step of a `strength` from 1 to [`MAX_STRENGTH`].
fn step(strength: u32) -> Step {
    let mut left = strength.clamp(1, MAX_STRENGTH) as usize;
    for (index, &(min, max, reach, steps)) in ITERATIONS.iter().enumerate() {
        if left <= steps {
            return Step {
                iterations: index + 1,
                offset: min + (max - min) / steps as f32 * left as f32,
                reach,
            };
        }
        left -= steps;
    }
    unreachable!("the steps of the iterations add up to the strongest strength")
}

/// Whether a window with `app_id`, a native window's app id or an Xwayland window's `WM_CLASS`
/// class, is one of `esmeril-applications`, without regard to ASCII case: mate-terminal's class
/// is `Mate-terminal`.
pub(crate) fn is_listed(applications: &[String], app_id: Option<&str>) -> bool {
    app_id.is_some_and(|app_id| {
        applications
            .iter()
            .any(|listed| listed.eq_ignore_ascii_case(app_id))
    })
}

/// Whether a pixel whose alpha is its last byte is neither opaque nor wholly clear.
fn is_part_clear(pixel: &[u8; 4]) -> bool {
    pixel[3] != 0 && pixel[3] != 0xff
}

/// Whether a surface's `buffer` can have see-through pixels: every pixel of an shm one is read,
/// and a dmabuf's aren't reachable, so its format says.
fn buffer_is_see_through(buffer: &WlBuffer) -> bool {
    if let Ok(dmabuf) = get_dmabuf(buffer) {
        return has_alpha(dmabuf.format().code);
    }
    with_buffer_contents(buffer, |ptr, len, data| {
        match data.format {
            // Alpha is each pixel's last byte in both.
            ShmFormat::Argb8888 | ShmFormat::Abgr8888 => {}
            ShmFormat::Xrgb8888 | ShmFormat::Xbgr8888 => return false,
            _ => return true,
        }
        let (width, height, stride) = (data.width as usize, data.height as usize, data.stride);
        let (stride, offset) = (stride as usize, data.offset as usize);
        if width == 0 || stride < width * 4 || offset + stride * height > len {
            return false;
        }
        // Safety: the pool is mapped for `len` bytes while this runs.
        let pool = unsafe { std::slice::from_raw_parts(ptr, len) };
        pool[offset..offset + stride * height]
            .chunks(stride)
            .flat_map(|line| line[..width * 4].as_chunks::<4>().0)
            .any(is_part_clear)
    })
    // A buffer of neither kind is a single pixel's, which has its own alpha.
    .unwrap_or(true)
}

/// Whether a surface's last commit has see-through pixels, its buffer being read once for each
/// commit: a layer surface that hasn't any gets no Esmeril (CMN-232).
pub(crate) fn has_see_through_pixels(states: &SurfaceData) -> bool {
    let Some(data) = states.data_map.get::<RendererSurfaceStateUserData>() else {
        return false;
    };
    let state = data.lock().unwrap();
    let Some(buffer) = state.buffer() else {
        return false;
    };
    let commit = state.current_commit();
    states
        .data_map
        .insert_if_missing_threadsafe(|| Mutex::new(None::<(CommitCounter, bool)>));
    let kept = states
        .data_map
        .get::<Mutex<Option<(CommitCounter, bool)>>>();
    let mut kept = kept.expect("it was just inserted").lock().unwrap();
    match *kept {
        Some((seen, see_through)) if seen == commit => see_through,
        _ => {
            let see_through = buffer_is_see_through(buffer);
            *kept = Some((commit, see_through));
            see_through
        }
    }
}

/// A strip's alpha, which says where Esmeril goes on it, for a strip that has see-through
/// pixels. One strip's mask keeps its [`Id`], and so its blur, for as long as the strip is
/// the same.
#[derive(Debug, Clone)]
pub(crate) struct Mask {
    id: Id,
    /// ARGB8888, top row first, with no gaps between the rows.
    pixels: Arc<Vec<u8>>,
    size: Size<i32, Buffer>,
}

impl Mask {
    /// The mask of a strip drawn on `surface`, or `None` if it has no see-through pixels:
    /// every pixel is opaque or wholly clear (ADR 0048).
    pub(crate) fn of(surface: &mut ImageSurface) -> Option<Self> {
        let (width, height) = (surface.width(), surface.height());
        let stride = surface.stride() as usize;
        let data = surface.data().ok()?;
        let row = width as usize * 4;
        let mut pixels = Vec::with_capacity(row * height as usize);
        for line in data.chunks(stride) {
            pixels.extend_from_slice(&line[..row]);
        }
        // Cairo's ARGB32 is native-endian, as ARGB8888 is little-endian, on what Caminito
        // runs on: alpha is each pixel's last byte.
        let see_through = pixels.as_chunks::<4>().0.iter().any(is_part_clear);
        see_through.then(|| Self {
            id: Id::new(),
            pixels: Arc::new(pixels),
            size: (width, height).into(),
        })
    }
}

/// The blur region a client asked for on a surface (CMN-238). One region keeps its [`Id`], and so
/// its blur, for as long as it's the same; a new one is damaged whole, even in the same bounds.
#[derive(Debug, Clone)]
pub(crate) struct AskedRegion {
    id: Id,
    /// Apart from one another, in the surface's coordinates.
    rects: Arc<Vec<Rectangle<i32, Logical>>>,
}

/// `region`'s rectangles, added and subtracted in their order, as ones apart from one another.
fn flattened(region: &RegionAttributes) -> Vec<Rectangle<i32, Logical>> {
    let mut rects = Vec::new();
    for &(kind, rect) in &region.rects {
        rects = Rectangle::subtract_rects_many_in_place(rects, [rect]);
        if let RectangleKind::Add = kind {
            rects.push(rect);
        }
    }
    rects
}

/// That a popup took a grab, as a menu does and a tooltip never does.
struct Grabbed;

/// Records that `popup` took a grab, so it's a menu.
pub(crate) fn mark_grabbed(popup: &WlSurface) {
    with_states(popup, |states| {
        states.data_map.insert_if_missing(|| Grabbed)
    });
}

/// Whether `popup` took a grab: a menu, and not a tooltip, which never does.
pub(crate) fn took_a_grab(popup: &WlSurface) -> bool {
    with_states(popup, |states| states.data_map.get::<Grabbed>().is_some())
}

/// The region the client has committed on this surface, if it has: an empty one asks for none.
pub(crate) fn asked_region(states: &SurfaceData) -> Option<AskedRegion> {
    let rects = {
        let mut cached = states
            .cached_state
            .get::<BackgroundEffectSurfaceCachedState>();
        flattened(cached.current().blur_region.as_ref()?)
    };
    states
        .data_map
        .insert_if_missing_threadsafe(|| Mutex::new(None::<AskedRegion>));
    let kept = states.data_map.get::<Mutex<Option<AskedRegion>>>();
    let mut kept = kept.expect("it was just inserted").lock().unwrap();
    match &*kept {
        Some(region) if *region.rects == rects => {}
        _ => {
            *kept = Some(AskedRegion {
                id: Id::new(),
                rects: Arc::new(rects),
            });
        }
    }
    kept.clone()
}

/// What says where an [`Esmeril`] goes.
#[derive(Debug, Clone)]
enum Behind {
    Strip(Mask),
    /// A surface of a listed window, by whose texture's alpha it goes.
    Surface(WlSurface),
    /// A client's region, under every pixel of it, from the corner of the surface.
    Region(Vec<Rectangle<i32, Physical>>),
}

/// What's beneath the Esmeril over the wallpaper (ADR 0048): each surface of the wallpaper with
/// its place and its commit. While it's the same, the blur of it is kept and not made again,
/// as the Overview fades the Esmeril in and out at every frame.
#[derive(Debug, Clone, PartialEq)]
pub(crate) struct Beneath(pub(crate) Vec<(WlSurface, Point<i32, Physical>, CommitCounter)>);

/// The id of the Esmeril behind one surface, which keeps its blur for as long as the surface
/// lives. The surface's own element has the id of its resource.
struct SurfaceId(Id);

/// Where and how a surface's Esmeril is drawn, for one frame of an output.
#[derive(Debug, Clone, Copy)]
pub(crate) struct SurfacePlace {
    /// The output's logical size.
    pub output: Size<i32, Logical>,
    pub scale: f64,
    pub strength: u32,
    pub alpha: f32,
    /// Where it's cut off, if it is.
    pub cut: Option<Rectangle<i32, Physical>>,
}

/// Esmeril behind one strip of a frame, one popup of Caminito's or one surface, of a listed
/// window or a layer surface, for one frame of an output.
#[derive(Debug, Clone)]
pub struct Esmeril {
    id: Id,
    behind: Behind,
    strength: u32,
    /// The rectangle of the strip or the surface.
    strip: Rectangle<i32, Physical>,
    /// The part of the strip that's drawn: all of it, but for a popup that slides, Caminito's
    /// or a layer surface's, which is cut off at its place.
    shown: Rectangle<i32, Physical>,
    /// How much of it shows, as a popup that slides or a panel in the Overview's way fades.
    alpha: f32,
    /// The rectangle the blur reads: the part that's shown, widened by its reach, on the
    /// output.
    read: Rectangle<i32, Physical>,
    /// The output's size, in the coordinates of the two rectangles.
    output: Size<i32, Physical>,
    /// What's beneath it, where it's blurred once and kept while that's the same.
    beneath: Option<Beneath>,
}

impl Esmeril {
    /// Esmeril behind a strip at `strip` on an output of the logical size `output`, at `scale`,
    /// or `None` if it's off the output.
    pub(crate) fn new(
        mask: &Mask,
        strip: Rectangle<i32, Logical>,
        output: Size<i32, Logical>,
        scale: f64,
        strength: u32,
    ) -> Option<Self> {
        Self::sliding(mask, strip, strip, 1.0, output, scale, strength)
    }

    /// As [`Esmeril::new`], for a popup that slides (ADR 0037): the strip is moved off its
    /// `place`, cut off at it, and shows `alpha` of itself. `None` too if none of it shows.
    pub(crate) fn sliding(
        mask: &Mask,
        strip: Rectangle<i32, Logical>,
        place: Rectangle<i32, Logical>,
        alpha: f32,
        output: Size<i32, Logical>,
        scale: f64,
        strength: u32,
    ) -> Option<Self> {
        let strip = strip.to_f64().to_physical_precise_round(scale);
        let place: Rectangle<i32, Physical> = place.to_f64().to_physical_precise_round(scale);
        let shown = strip.intersection(place)?;
        let output = output.to_f64().to_physical_precise_round(scale);
        let behind = Behind::Strip(mask.clone());
        Self::at(
            mask.id.clone(),
            behind,
            strip,
            shown,
            alpha,
            output,
            strength,
        )
    }

    /// Esmeril behind `surface`, with its origin at `location`, as `place` has it, or `None` if
    /// it has no view yet or none of it shows.
    pub(crate) fn behind_surface(
        surface: &WlSurface,
        states: &SurfaceData,
        location: Point<f64, Physical>,
        place: &SurfacePlace,
    ) -> Option<Self> {
        let (strip, on_output) = Self::surface_strip(states, location, place)?;
        states
            .data_map
            .insert_if_missing_threadsafe(|| SurfaceId(Id::new()));
        let id = states.data_map.get::<SurfaceId>()?.0.clone();
        let behind = Behind::Surface(surface.clone());
        let shown = match place.cut {
            Some(cut) => strip.intersection(cut)?,
            None => strip,
        };
        Self::at(
            id,
            behind,
            strip,
            shown,
            place.alpha,
            on_output,
            place.strength,
        )
    }

    /// Esmeril in a client's `region` of a surface, with its arguments as in
    /// [`Esmeril::behind_surface`], or `None` too if the region has nothing on the surface.
    pub(crate) fn in_region(
        region: &AskedRegion,
        states: &SurfaceData,
        location: Point<f64, Physical>,
        place: &SurfacePlace,
    ) -> Option<Self> {
        let (strip, on_output) = Self::surface_strip(states, location, place)?;
        // By their corners, so that rectangles apart from one another stay apart.
        let on_strip: Vec<_> = region
            .rects
            .iter()
            .filter_map(|rect| {
                let rect = rect.to_f64().to_physical(place.scale);
                let corner = (location + rect.loc).to_i32_round();
                let far = (location + rect.loc + rect.size.to_point()).to_i32_round();
                Rectangle::new(corner, (far - corner).to_size()).intersection(strip)
            })
            .collect();
        let bounds = on_strip.iter().copied().reduce(Rectangle::merge)?;
        let shown = match place.cut {
            Some(cut) => bounds.intersection(cut)?,
            None => bounds,
        };
        let rects = on_strip
            .into_iter()
            .map(|rect| Rectangle::new(rect.loc - strip.loc, rect.size))
            .collect();
        let behind = Behind::Region(rects);
        let id = region.id.clone();
        Self::at(
            id,
            behind,
            strip,
            shown,
            place.alpha,
            on_output,
            place.strength,
        )
    }

    /// The rectangle of a surface with its origin at `location`, and the output's size, both
    /// physical, or `None` if it has no view yet or is off the output.
    fn surface_strip(
        states: &SurfaceData,
        location: Point<f64, Physical>,
        &SurfacePlace { output, scale, .. }: &SurfacePlace,
    ) -> Option<(Rectangle<i32, Physical>, Size<i32, Physical>)> {
        let data = states.data_map.get::<RendererSurfaceStateUserData>()?;
        let view = data.lock().unwrap().view()?;
        // As its surface's element rounds it, so that the two line up.
        let corner = location.to_i32_round();
        let far = (view.dst.to_f64().to_physical(scale).to_point() + location).to_i32_round();
        let strip = Rectangle::new(corner, (far - corner).to_size());
        let on_output: Size<i32, Physical> = output.to_f64().to_physical_precise_round(scale);
        strip
            .overlaps(Rectangle::from_size(on_output))
            .then_some((strip, on_output))
    }

    fn at(
        id: Id,
        behind: Behind,
        strip: Rectangle<i32, Physical>,
        shown: Rectangle<i32, Physical>,
        alpha: f32,
        output: Size<i32, Physical>,
        strength: u32,
    ) -> Option<Self> {
        let reach = step(strength).reach;
        let mut read = shown;
        read.loc -= (reach, reach).into();
        read.size += (2 * reach, 2 * reach).into();
        let read = read.intersection(Rectangle::from_size(output))?;
        Some(Self {
            id,
            behind,
            strength,
            strip,
            shown,
            alpha,
            read,
            output,
            beneath: None,
        })
    }

    /// Esmeril over all of an output of the logical size `output`, at `scale`, over the wallpaper
    /// whose surfaces are `beneath`, while the Overview is drawn there, showing `alpha` of itself.
    pub(crate) fn over_wallpaper(
        id: Id,
        beneath: Beneath,
        output: Size<i32, Logical>,
        scale: f64,
        strength: u32,
        alpha: f32,
    ) -> Option<Self> {
        let output = output.to_f64().to_physical_precise_round(scale);
        let strip = Rectangle::from_size(output);
        let behind = Behind::Region(vec![strip]);
        let mut esmeril = Self::at(id, behind, strip, strip, alpha, output, strength)?;
        esmeril.beneath = Some(beneath);
        Some(esmeril)
    }

    /// This Esmeril behind what it's behind once that's scaled by `factor` from the origin and
    /// moved by `by`, as Smithay's `RescaleRenderElement` and `RelocateRenderElement` place it,
    /// cut off at `crop`, and at `alpha` of itself, on an output of `output`'s physical size: as
    /// an animation draws a window (CMN-236). `None` if none of it shows. The blur is read
    /// again around where it's drawn, so it can be made where nothing bounds it.
    pub(crate) fn zoomed(
        &self,
        factor: Scale<f64>,
        by: Point<i32, Physical>,
        crop: Rectangle<i32, Physical>,
        alpha: f32,
        output: Size<i32, Physical>,
    ) -> Option<Self> {
        let scaled = |rect: Rectangle<i32, Physical>| rect.to_f64().upscale(factor).to_i32_round();
        let placed = |rect| {
            let mut rect = scaled(rect);
            rect.loc += by;
            rect
        };
        let behind = match &self.behind {
            // From the strip's corner, which is scaled with them.
            Behind::Region(rects) => Behind::Region(rects.iter().copied().map(scaled).collect()),
            behind => behind.clone(),
        };
        Self::at(
            self.id.clone(),
            behind,
            placed(self.strip),
            placed(self.shown).intersection(crop)?,
            self.alpha * alpha,
            output,
            self.strength,
        )
    }

    /// The rectangle of the strip or the surface, on the output.
    #[cfg(test)]
    pub(crate) fn strip(&self) -> Rectangle<i32, Physical> {
        self.strip
    }

    /// The part of the strip that's drawn, on the output.
    #[cfg(test)]
    pub(crate) fn shown(&self) -> Rectangle<i32, Physical> {
        self.shown
    }
}

impl Element for Esmeril {
    fn id(&self) -> &Id {
        &self.id
    }

    /// Changes with the strength, which then damages all of it, and it's blurred again.
    fn current_commit(&self) -> CommitCounter {
        CommitCounter::from(self.strength as usize)
    }

    fn src(&self) -> Rectangle<f64, Buffer> {
        Rectangle::from_size((f64::from(self.read.size.w), f64::from(self.read.size.h)).into())
    }

    fn geometry(&self, _scale: Scale<f64>) -> Rectangle<i32, Physical> {
        self.read
    }

    /// Changes with a popup's fade, which then damages it.
    fn alpha(&self) -> f32 {
        self.alpha
    }

    fn is_framebuffer_effect(&self) -> bool {
        true
    }
}

/// A renderer the scene can draw Esmeril with.
pub trait DrawsEsmeril: Renderer {
    /// What's beneath `esmeril` changed: takes it from `frame`, and blurs it.
    fn capture(
        frame: &mut Self::Frame<'_, '_>,
        esmeril: &Esmeril,
        cache: &UserDataMap,
    ) -> Result<(), Self::Error>;

    /// Draws what was last blurred, in `damage`, which is from the corner of what it reads.
    fn draw(
        frame: &mut Self::Frame<'_, '_>,
        esmeril: &Esmeril,
        damage: &[Rectangle<i32, Physical>],
        cache: Option<&UserDataMap>,
    ) -> Result<(), Self::Error>;
}

impl<R: DrawsEsmeril> RenderElement<R> for Esmeril {
    fn draw(
        &self,
        frame: &mut R::Frame<'_, '_>,
        _src: Rectangle<f64, Buffer>,
        _dst: Rectangle<i32, Physical>,
        damage: &[Rectangle<i32, Physical>],
        _opaque_regions: &[Rectangle<i32, Physical>],
        cache: Option<&UserDataMap>,
    ) -> Result<(), R::Error> {
        R::draw(frame, self, damage, cache)
    }

    fn capture_framebuffer(
        &self,
        frame: &mut R::Frame<'_, '_>,
        _src: Rectangle<f64, Buffer>,
        _dst: Rectangle<i32, Physical>,
        cache: &UserDataMap,
    ) -> Result<(), R::Error> {
        R::capture(frame, self, cache)
    }
}

#[cfg(test)]
impl DrawsEsmeril for PixmanRenderer {
    fn capture(
        _frame: &mut Self::Frame<'_, '_>,
        _esmeril: &Esmeril,
        _cache: &UserDataMap,
    ) -> Result<(), Self::Error> {
        Ok(())
    }

    fn draw(
        _frame: &mut Self::Frame<'_, '_>,
        _esmeril: &Esmeril,
        _damage: &[Rectangle<i32, Physical>],
        _cache: Option<&UserDataMap>,
    ) -> Result<(), Self::Error> {
        Ok(())
    }
}

const PASS_VERTEX: &str = "#version 100
attribute vec2 vert;
varying vec2 uv;
void main() {
    uv = vert;
    gl_Position = vec4(vert * 2.0 - 1.0, 0.0, 1.0);
}
";

const DOWNSAMPLE: &str = "#version 100
precision highp float;
uniform sampler2D tex;
uniform vec2 half_pixel;
uniform float offset;
varying vec2 uv;
void main() {
    vec2 d = half_pixel * offset;
    vec4 sum = texture2D(tex, uv) * 4.0;
    sum += texture2D(tex, uv - d);
    sum += texture2D(tex, uv + d);
    sum += texture2D(tex, uv + vec2(d.x, -d.y));
    sum += texture2D(tex, uv - vec2(d.x, -d.y));
    gl_FragColor = sum / 8.0;
}
";

const UPSAMPLE: &str = "#version 100
precision highp float;
uniform sampler2D tex;
uniform vec2 half_pixel;
uniform float offset;
varying vec2 uv;
void main() {
    vec2 d = half_pixel * offset;
    vec4 sum = texture2D(tex, uv + vec2(-d.x * 2.0, 0.0));
    sum += texture2D(tex, uv + vec2(-d.x, d.y)) * 2.0;
    sum += texture2D(tex, uv + vec2(0.0, d.y * 2.0));
    sum += texture2D(tex, uv + vec2(d.x, d.y)) * 2.0;
    sum += texture2D(tex, uv + vec2(d.x * 2.0, 0.0));
    sum += texture2D(tex, uv + vec2(d.x, -d.y)) * 2.0;
    sum += texture2D(tex, uv + vec2(0.0, -d.y * 2.0));
    sum += texture2D(tex, uv + vec2(-d.x, -d.y)) * 2.0;
    gl_FragColor = sum / 12.0;
}
";

/// The last pass's `main`, which draws on the frame `blurred` by where the fragment is on the
/// framebuffer, so whatever the output's transform, weighted by `mask_alpha`.
fn composite_main(mask_alpha: &str) -> String {
    format!(
        "uniform sampler2D blurred;
uniform vec4 region;
uniform float alpha;
varying vec2 v_coords;
void main() {{
    vec3 color = texture2D(blurred, (gl_FragCoord.xy - region.xy) / region.zw).rgb;
    float luma = dot(color, vec3(0.2126, 0.7152, 0.0722));
    color = mix(vec3(luma), color, {SATURATION});
    color = clamp(color * {CONTRAST} + (1.0 - {CONTRAST}) / 2.0, 0.0, 1.0);
    float weight = smoothstep(0.0, {WHOLE_FROM_ALPHA}, {mask_alpha}) * alpha;
    gl_FragColor = vec4(color * weight, weight);
}}
"
    )
}

/// The last pass behind a strip, with `mask` by where the fragment is on the strip.
fn composite_shader() -> String {
    format!(
        "precision highp float;
uniform sampler2D mask;
{}",
        composite_main("texture2D(mask, v_coords).a")
    )
}

/// The last pass in a client's region, under every pixel of it.
fn region_composite_shader() -> String {
    format!("precision highp float;\n{}", composite_main("1.0"))
}

/// The last pass behind a surface: a texture shader of Smithay's, drawn as the surface's element
/// draws its texture, so that the kind of its buffer, its transform and its viewport are
/// Smithay's to take care of, and `tex` is the surface's own.
fn surface_composite_shader() -> String {
    format!(
        "#version 100
//_DEFINES_
#if defined(EXTERNAL)
#extension GL_OES_EGL_image_external : require
#endif
precision highp float;
#if defined(EXTERNAL)
uniform samplerExternalOES tex;
#else
uniform sampler2D tex;
#endif
#if defined(NO_ALPHA)
#define MASK_ALPHA 1.0
#else
#define MASK_ALPHA texture2D(tex, v_coords).a
#endif
{}",
        composite_main("MASK_ALPHA")
    )
}

/// A downsample's or an upsample's program.
#[derive(Debug)]
struct Pass {
    program: ffi::types::GLuint,
    tex: ffi::types::GLint,
    half_pixel: ffi::types::GLint,
    offset: ffi::types::GLint,
    vert: ffi::types::GLuint,
}

impl Pass {
    fn link(gl: &ffi::Gles2, fragment: &str) -> Result<Self, GlesError> {
        // Safety: the renderer's context is current in `with_context`.
        unsafe {
            let program = link_program(gl, PASS_VERTEX, fragment)?;
            let location = |name: &std::ffi::CStr| gl.GetUniformLocation(program, name.as_ptr());
            Ok(Self {
                program,
                tex: location(c"tex"),
                half_pixel: location(c"half_pixel"),
                offset: location(c"offset"),
                vert: gl.GetAttribLocation(program, c"vert".as_ptr()) as ffi::types::GLuint,
            })
        }
    }
}

/// What's compiled once for each GL context, and lives as long as it does.
#[derive(Debug)]
struct Programs {
    down: Pass,
    up: Pass,
    composite: GlesPixelProgram,
    region_composite: GlesPixelProgram,
    surface_composite: GlesTexProgram,
    framebuffer: ffi::types::GLuint,
}

/// The programs of `frame`'s context, or `None` if they don't compile, which is said once.
fn programs(frame: &mut GlesFrame<'_, '_>) -> Option<Arc<Programs>> {
    let context = frame.egl_context().user_data();
    if let Some(programs) = context.get::<Option<Arc<Programs>>>() {
        return programs.clone();
    }
    let programs = compile(frame)
        .inspect_err(|err| tracing::warn!("can't compile Esmeril's shaders: {err}"))
        .ok()
        .map(Arc::new);
    let context = frame.egl_context().user_data();
    context.insert_if_missing(|| programs.clone());
    programs
}

fn compile(frame: &mut GlesFrame<'_, '_>) -> Result<Programs, GlesError> {
    let (down, up, framebuffer) = frame.with_context(|gl| {
        let down = Pass::link(gl, DOWNSAMPLE)?;
        let up = Pass::link(gl, UPSAMPLE)?;
        let mut framebuffer = 0;
        // Safety: the renderer's context is current in `with_context`.
        unsafe { gl.GenFramebuffers(1, &mut framebuffer) };
        Ok::<_, GlesError>((down, up, framebuffer))
    })??;
    let mut renderer = frame.renderer();
    let composite = renderer.as_mut().compile_custom_pixel_shader(
        composite_shader(),
        &[
            UniformName::new("blurred", UniformType::_1i),
            UniformName::new("mask", UniformType::_1i),
            UniformName::new("region", UniformType::_4f),
        ],
    )?;
    let region_composite = renderer.as_mut().compile_custom_pixel_shader(
        region_composite_shader(),
        &[
            UniformName::new("blurred", UniformType::_1i),
            UniformName::new("region", UniformType::_4f),
        ],
    )?;
    let surface_composite = renderer.as_mut().compile_custom_texture_shader(
        surface_composite_shader(),
        &[
            UniformName::new("blurred", UniformType::_1i),
            UniformName::new("region", UniformType::_4f),
        ],
    )?;
    Ok(Programs {
        down,
        up,
        composite,
        region_composite,
        surface_composite,
        framebuffer,
    })
}

/// What's kept of one element between its captures.
#[derive(Debug)]
struct Blur {
    /// What it read from the framebuffer, as it was.
    source: GlesTexture,
    /// The result, at the size of `source`, then each iteration's, each half the last.
    levels: Vec<GlesTexture>,
    /// Where it read from, on the framebuffer.
    region: Rectangle<i32, Physical>,
    /// A strip's mask, once it's first drawn.
    mask: Option<GlesTexture>,
    /// What was beneath when it was blurred, at which strength and on an output turned which
    /// way, where it's kept while that's the same.
    beneath: Option<(Beneath, u32, Transform)>,
}

impl Blur {
    fn new(
        frame: &mut GlesFrame<'_, '_>,
        region: Rectangle<i32, Physical>,
        iterations: usize,
    ) -> Result<Self, GlesError> {
        let mut renderer = frame.renderer();
        let renderer = renderer.as_mut();
        let size = |level: usize| -> Size<i32, Buffer> {
            (
                (region.size.w >> level).max(1),
                (region.size.h >> level).max(1),
            )
                .into()
        };
        let mut levels = Vec::with_capacity(iterations + 1);
        for level in 0..=iterations {
            levels.push(Offscreen::<GlesTexture>::create_buffer(
                renderer,
                Fourcc::Abgr8888,
                size(level),
            )?);
        }
        let source = Offscreen::<GlesTexture>::create_buffer(renderer, Fourcc::Abgr8888, size(0))?;
        Ok(Self {
            source,
            levels,
            region,
            mask: None,
            beneath: None,
        })
    }
}

/// Runs `pass` from `from` into `to`, which is a quarter or four times its size.
///
/// # Safety
///
/// The renderer's context is current, and `framebuffer` is bound.
unsafe fn run_pass(
    gl: &ffi::Gles2,
    pass: &Pass,
    from: &GlesTexture,
    to: &GlesTexture,
    offset: f32,
    instancing: bool,
) {
    const QUAD: [f32; 8] = [0.0, 0.0, 1.0, 0.0, 0.0, 1.0, 1.0, 1.0];
    let size = to.size();
    // Safety: as this function's caller promises; `QUAD` outlives the draw, and the attribute
    // is left disabled for what's drawn next.
    unsafe {
        gl.FramebufferTexture2D(
            ffi::FRAMEBUFFER,
            ffi::COLOR_ATTACHMENT0,
            ffi::TEXTURE_2D,
            to.tex_id(),
            0,
        );
        gl.Viewport(0, 0, size.w, size.h);
        gl.UseProgram(pass.program);
        bind_texture(gl, 0, from);
        gl.Uniform1i(pass.tex, 0);
        gl.Uniform2f(pass.half_pixel, 0.5 / size.w as f32, 0.5 / size.h as f32);
        gl.Uniform1f(pass.offset, offset);
        gl.BindBuffer(ffi::ARRAY_BUFFER, 0);
        gl.EnableVertexAttribArray(pass.vert);
        if instancing {
            // Smithay's draws leave a divisor on the attributes they use.
            gl.VertexAttribDivisor(pass.vert, 0);
        }
        gl.VertexAttribPointer(
            pass.vert,
            2,
            ffi::FLOAT,
            ffi::FALSE,
            0,
            QUAD.as_ptr().cast(),
        );
        gl.DrawArrays(ffi::TRIANGLE_STRIP, 0, 4);
        gl.DisableVertexAttribArray(pass.vert);
    }
}

/// Binds `texture` to a unit, sampled smoothly and stopping at its edge.
///
/// # Safety
///
/// The renderer's context is current.
unsafe fn bind_texture(gl: &ffi::Gles2, unit: u32, texture: &GlesTexture) {
    // Safety: as this function's caller promises.
    unsafe {
        gl.ActiveTexture(ffi::TEXTURE0 + unit);
        gl.BindTexture(ffi::TEXTURE_2D, texture.tex_id());
        for (parameter, value) in [
            (ffi::TEXTURE_MIN_FILTER, ffi::LINEAR),
            (ffi::TEXTURE_MAG_FILTER, ffi::LINEAR),
            (ffi::TEXTURE_WRAP_S, ffi::CLAMP_TO_EDGE),
            (ffi::TEXTURE_WRAP_T, ffi::CLAMP_TO_EDGE),
        ] {
            gl.TexParameteri(ffi::TEXTURE_2D, parameter, value as i32);
        }
    }
}

#[cfg(test)]
thread_local! {
    static BLURS: std::cell::Cell<usize> = const { std::cell::Cell::new(0) };
}

/// How many blurs GLES has made on this thread.
#[cfg(test)]
pub(crate) fn blurs() -> usize {
    BLURS.with(std::cell::Cell::get)
}

impl DrawsEsmeril for GlesRenderer {
    fn capture(
        frame: &mut GlesFrame<'_, '_>,
        esmeril: &Esmeril,
        cache: &UserDataMap,
    ) -> Result<(), GlesError> {
        let Some(programs) = programs(frame) else {
            return Ok(());
        };
        let step = step(esmeril.strength);
        let region = frame
            .transformation()
            .transform_rect_in(esmeril.read, &esmeril.output);

        cache.insert_if_missing(|| RefCell::new(None::<Blur>));
        let cell = cache
            .get::<RefCell<Option<Blur>>>()
            .expect("it was just inserted");
        let mut blur = cell.borrow_mut();
        let kept = blur.as_ref().is_some_and(|blur| {
            blur.region.size == region.size && blur.levels.len() == step.iterations + 1
        });
        if !kept {
            *blur = Some(Blur::new(frame, region, step.iterations)?);
        }
        let blur = blur.as_mut().expect("it was just made");
        let beneath = esmeril
            .beneath
            .clone()
            .map(|beneath| (beneath, esmeril.strength, frame.transformation()));
        // Only an Esmeril that names what's beneath it is kept: what's beneath any other changes
        // with no commit of its own, as a window moves.
        if kept && blur.region == region && beneath.is_some() && blur.beneath == beneath {
            return Ok(());
        }
        blur.region = region;
        blur.beneath = beneath;
        #[cfg(test)]
        BLURS.with(|blurs| blurs.set(blurs.get() + 1));

        let instancing = frame
            .capabilities()
            .contains(&smithay::backend::renderer::gles::Capability::Instancing);
        frame.with_context(|gl| {
            // Safety: the renderer's context is current in `with_context`. The target, the
            // viewport, the scissor and the blending are put back, as Smithay sets them only
            // at a frame's start; it sets the rest again at each of its draws.
            unsafe {
                let mut target = 0;
                gl.GetIntegerv(ffi::FRAMEBUFFER_BINDING, &mut target);
                let mut viewport = [0; 4];
                gl.GetIntegerv(ffi::VIEWPORT, viewport.as_mut_ptr());
                let scissor = gl.IsEnabled(ffi::SCISSOR_TEST) == ffi::TRUE;

                gl.Disable(ffi::SCISSOR_TEST);
                gl.Disable(ffi::BLEND);
                gl.ActiveTexture(ffi::TEXTURE0);
                gl.BindTexture(ffi::TEXTURE_2D, blur.source.tex_id());
                // RGB, as a framebuffer with no alpha can't be copied to anything with it.
                gl.CopyTexImage2D(
                    ffi::TEXTURE_2D,
                    0,
                    ffi::RGB,
                    region.loc.x,
                    region.loc.y,
                    region.size.w,
                    region.size.h,
                    0,
                );

                gl.BindFramebuffer(ffi::FRAMEBUFFER, programs.framebuffer);
                let mut from = &blur.source;
                for to in &blur.levels[1..] {
                    run_pass(gl, &programs.down, from, to, step.offset, instancing);
                    from = to;
                }
                for to in blur.levels[..step.iterations].iter().rev() {
                    run_pass(gl, &programs.up, from, to, step.offset, instancing);
                    from = to;
                }

                gl.BindFramebuffer(ffi::FRAMEBUFFER, target as ffi::types::GLuint);
                gl.Viewport(viewport[0], viewport[1], viewport[2], viewport[3]);
                if scissor {
                    gl.Enable(ffi::SCISSOR_TEST);
                }
                gl.Enable(ffi::BLEND);
                gl.BlendFunc(ffi::ONE, ffi::ONE_MINUS_SRC_ALPHA);
            }
        })
    }

    fn draw(
        frame: &mut GlesFrame<'_, '_>,
        esmeril: &Esmeril,
        damage: &[Rectangle<i32, Physical>],
        cache: Option<&UserDataMap>,
    ) -> Result<(), GlesError> {
        let Some(programs) = programs(frame) else {
            return Ok(());
        };
        let Some(cell) = cache.and_then(|cache| cache.get::<RefCell<Option<Blur>>>()) else {
            return Ok(());
        };
        let mut blur = cell.borrow_mut();
        let Some(blur) = blur.as_mut() else {
            return Ok(());
        };
        // A strip's mask, imported once, or a surface's texture, as it is now: a surface with
        // none, as one of a single-pixel buffer, gets nothing.
        let surface_texture = match &esmeril.behind {
            Behind::Strip(mask) => {
                if blur.mask.is_none() {
                    let mut renderer = frame.renderer();
                    blur.mask = Some(renderer.as_mut().import_memory(
                        &mask.pixels,
                        Fourcc::Argb8888,
                        mask.size,
                        false,
                    )?);
                }
                None
            }
            Behind::Surface(surface) => {
                let context = frame.context_id();
                let drawn = with_renderer_surface_state(surface, |state| {
                    let texture = state.texture::<GlesTexture>(context)?.clone();
                    let src = state.view()?.src.to_buffer(
                        f64::from(state.buffer_scale()),
                        state.buffer_transform(),
                        &state.buffer_size()?.to_f64(),
                    );
                    Some((texture, src, state.buffer_transform()))
                });
                let Some(drawn) = drawn.flatten() else {
                    return Ok(());
                };
                Some(drawn)
            }
            Behind::Region(_) => None,
        };

        // From the corner of what it reads to the corner of the strip, of which only what's
        // shown is drawn.
        let strip = esmeril.strip;
        let to_strip = esmeril.read.loc - strip.loc;
        let shown = Rectangle::new(esmeril.shown.loc - strip.loc, esmeril.shown.size);
        let damage: Vec<_> = damage
            .iter()
            .filter_map(|rect| {
                let mut rect = *rect;
                rect.loc += to_strip;
                rect.intersection(shown)
            })
            .collect();
        let damage: Vec<_> = match &esmeril.behind {
            Behind::Region(rects) => damage
                .iter()
                .flat_map(|damaged| rects.iter().filter_map(|rect| rect.intersection(*damaged)))
                .collect(),
            _ => damage,
        };
        if damage.is_empty() {
            return Ok(());
        }

        let blurred = &blur.levels[0];
        let region = blur.region;
        let region = Uniform::new(
            "region",
            UniformValue::_4f(
                region.loc.x as f32,
                region.loc.y as f32,
                region.size.w as f32,
                region.size.h as f32,
            ),
        );
        let strip_mask = blur.mask.as_ref();
        frame.with_context(|gl| {
            // Safety: the renderer's context is current in `with_context`.
            unsafe {
                bind_texture(gl, 1, blurred);
                if let Some(mask) = strip_mask {
                    bind_texture(gl, 2, mask);
                }
                gl.ActiveTexture(ffi::TEXTURE0);
            }
        })?;
        if let Some((texture, src, transform)) = surface_texture {
            return frame.render_texture_from_to(
                &texture,
                src,
                strip,
                &damage,
                &[],
                transform,
                esmeril.alpha,
                Some(&programs.surface_composite),
                &[Uniform::new("blurred", UniformValue::_1i(1)), region],
            );
        }
        if let Behind::Region(_) = esmeril.behind {
            return frame.render_pixel_shader_to(
                &programs.region_composite,
                Rectangle::from_size((f64::from(strip.size.w), f64::from(strip.size.h)).into()),
                strip,
                (strip.size.w, strip.size.h).into(),
                Some(&damage),
                esmeril.alpha,
                &[Uniform::new("blurred", UniformValue::_1i(1)), region],
            );
        }
        frame.render_pixel_shader_to(
            &programs.composite,
            Rectangle::from_size((f64::from(strip.size.w), f64::from(strip.size.h)).into()),
            strip,
            (strip.size.w, strip.size.h).into(),
            Some(&damage),
            esmeril.alpha,
            &[
                Uniform::new("blurred", UniformValue::_1i(1)),
                Uniform::new("mask", UniformValue::_1i(2)),
                region,
            ],
        )
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn the_steps_are_kwins() {
        let steps: Vec<(usize, f32)> = (1..=MAX_STRENGTH)
            .map(|strength| {
                let step = step(strength);
                (step.iterations, step.offset)
            })
            .collect();
        let expected = [
            (1, 1.5),
            (1, 2.0),
            (2, 2.5),
            (2, 3.0),
            (3, 2.6),
            (3, 3.2),
            (3, 3.8),
            (3, 4.4),
            (3, 5.0),
            (4, 3.0 + 5.0 / 6.0),
            (4, 3.0 + 5.0 / 3.0),
            (4, 5.5),
            (4, 3.0 + 10.0 / 3.0),
            (4, 3.0 + 25.0 / 6.0),
            (4, 8.0),
        ];
        for (strength, (got, want)) in steps.iter().zip(expected).enumerate() {
            assert_eq!(got.0, want.0, "the iterations of step {}", strength + 1);
            assert!(
                (got.1 - want.1).abs() < 1e-4,
                "the offset of step {}: {} and not {}",
                strength + 1,
                got.1,
                want.1,
            );
        }
    }

    #[test]
    fn a_region_is_added_and_subtracted_in_its_order() {
        let rect = |x, y, w, h| Rectangle::<i32, Logical>::new((x, y).into(), (w, h).into());
        let region = RegionAttributes {
            rects: vec![
                (RectangleKind::Add, rect(0, 0, 10, 10)),
                (RectangleKind::Add, rect(5, 0, 10, 10)),
                (RectangleKind::Subtract, rect(0, 0, 15, 5)),
            ],
        };
        let rects = flattened(&region);
        let area: i32 = rects.iter().map(|rect| rect.size.w * rect.size.h).sum();
        assert_eq!(area, 15 * 5, "{rects:?}");
        assert!(
            rects
                .iter()
                .all(|rect| rect.loc.y >= 5 && rect.loc.x + rect.size.w <= 15)
        );
    }

    #[test]
    fn the_default_is_kwins_with_its_reach() {
        assert_eq!(step(DEFAULT_STRENGTH).reach, 150);
    }
}
