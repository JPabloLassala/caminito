//! Esmeril (ADR 0048): a blur of what's beneath, with raised saturation and contrast, drawn
//! behind the see-through parts of a frame. It's Plasma's Blur effect and its Background
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
//! The strip's alpha says where it's drawn: wherever the strip has any alpha at all. The
//! strip is drawn over it, so an opaque pixel hides it and a part-clear one shows it as much
//! as it's see-through, and a wholly clear pixel shows what's beneath with no Esmeril.
//!
//! A layer surface or its popup (CMN-232) is a strip whose alpha is its client's: its mask is
//! the surface's own texture at the frame it's drawn, whichever kind of buffer that came from.
//!
//! Pixman draws nothing for it (ADR 0048), though its damage is the same.

use std::cell::RefCell;
use std::sync::Arc;

use caminito_theme::cairo::ImageSurface;
use smithay::backend::allocator::{Buffer as _, Fourcc, format::has_alpha};
use smithay::backend::renderer::element::{Element, Id, RenderElement};
use smithay::backend::renderer::gles::{
    GlesError, GlesFrame, GlesPixelProgram, GlesRenderer, GlesTexture, Uniform, UniformName,
    UniformType, UniformValue, ffi, link_program,
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
use smithay::utils::{Buffer, Logical, Physical, Rectangle, Scale, Size, Transform};
use smithay::wayland::compositor::with_states;
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

/// A strip's or a surface's alpha, which says where Esmeril goes on it, for one that has
/// see-through pixels. One strip's mask keeps its [`Id`], and so its blur, for as long as the strip is
/// the same, and a surface's for as long as the surface lives.
#[derive(Debug, Clone)]
pub(crate) struct Mask {
    id: Id,
    source: Source,
}

/// Where a mask's alpha is.
#[derive(Debug, Clone)]
enum Source {
    /// A frame's strip.
    Pixels {
        /// ARGB8888, top row first, with no gaps between the rows.
        pixels: Arc<Vec<u8>>,
        size: Size<i32, Buffer>,
    },
    /// A client's surface, and how many of its commits have been seen.
    Surface { surface: WlSurface, commits: usize },
}

/// What a surface's mask keeps between frames, so that its buffer is read once for each commit.
#[derive(Debug)]
struct SurfaceMask {
    id: Id,
    commit: CommitCounter,
    commits: usize,
    see_through: bool,
}

/// Whether a pixel whose alpha is its last byte is neither opaque nor wholly clear.
fn is_part_clear(pixel: &[u8; 4]) -> bool {
    pixel[3] != 0 && pixel[3] != 0xff
}

/// Whether a surface's `buffer` can have see-through pixels: every pixel of an shm one is read,
/// and a dmabuf's aren't reachable, so its format says.
fn is_see_through(buffer: &WlBuffer) -> bool {
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
            source: Source::Pixels {
                pixels: Arc::new(pixels),
                size: (width, height).into(),
            },
        })
    }

    /// The mask of `surface` as last committed, with its rectangle from the surface's origin,
    /// or `None` if it has no see-through pixels, or it turns or crops its buffer, as the mask
    /// is all of the texture the right way round, and no client that gets Esmeril does either.
    pub(crate) fn of_surface(surface: &WlSurface) -> Option<(Self, Rectangle<i32, Logical>)> {
        with_states(surface, |states| {
            let state = states.data_map.get::<RendererSurfaceStateUserData>()?;
            let state = state.lock().unwrap();
            let view = state.view()?;
            let buffer = state.buffer()?;
            let whole = Rectangle::from_size(state.buffer_size()?.to_f64());
            if state.buffer_transform() != Transform::Normal || view.src != whole {
                return None;
            }
            let commit = state.current_commit();
            states
                .data_map
                .insert_if_missing(|| RefCell::new(None::<SurfaceMask>));
            let kept = states.data_map.get::<RefCell<Option<SurfaceMask>>>()?;
            let mut kept = kept.borrow_mut();
            let kept = match kept.as_mut() {
                Some(kept) if kept.commit == commit => kept,
                Some(kept) => {
                    kept.commit = commit;
                    kept.commits += 1;
                    kept.see_through = is_see_through(buffer);
                    kept
                }
                None => kept.insert(SurfaceMask {
                    id: Id::new(),
                    commit,
                    commits: 0,
                    see_through: is_see_through(buffer),
                }),
            };
            kept.see_through.then(|| {
                let mask = Self {
                    id: kept.id.clone(),
                    source: Source::Surface {
                        surface: surface.clone(),
                        commits: kept.commits,
                    },
                };
                (mask, Rectangle::new(view.offset, view.dst))
            })
        })
    }

    /// How many commits of its surface it has seen, which a frame's strip has none of.
    fn commits(&self) -> usize {
        match self.source {
            Source::Pixels { .. } => 0,
            Source::Surface { commits, .. } => commits,
        }
    }
}

/// Esmeril behind one strip of a frame, or one surface, for one frame of an output.
#[derive(Debug, Clone)]
pub struct Esmeril {
    mask: Mask,
    strength: u32,
    /// How much of it shows, as what's in front of it fades.
    alpha: f32,
    /// The strip's rectangle.
    strip: Rectangle<i32, Physical>,
    /// The part of the strip it's drawn on.
    shown: Rectangle<i32, Physical>,
    /// The rectangle the blur reads: the strip's, widened by its reach, on the output.
    read: Rectangle<i32, Physical>,
    /// The output's size, in the coordinates of the two rectangles.
    output: Size<i32, Physical>,
}

impl Esmeril {
    /// Esmeril behind a strip at `strip` on an output of the logical size `output`, at `scale`,
    /// and `alpha`, or `None` if it's off the output.
    pub(crate) fn new(
        mask: &Mask,
        strip: Rectangle<i32, Logical>,
        output: Size<i32, Logical>,
        scale: f64,
        strength: u32,
        alpha: f32,
    ) -> Option<Self> {
        let strip = strip.to_f64().to_physical_precise_round(scale);
        let output: Size<i32, Physical> = output.to_f64().to_physical_precise_round(scale);
        let reach = step(strength).reach;
        let mut read = strip;
        read.loc -= (reach, reach).into();
        read.size += (2 * reach, 2 * reach).into();
        let read = read.intersection(Rectangle::from_size(output))?;
        Some(Self {
            mask: mask.clone(),
            strength,
            alpha,
            strip,
            shown: strip,
            read,
            output,
        })
    }

    /// As it is, drawn only inside `place`, or `None` if nothing of it is.
    pub(crate) fn cut_to(mut self, place: Rectangle<i32, Physical>) -> Option<Self> {
        self.shown = self.strip.intersection(place)?;
        Some(self)
    }

    /// The strip's rectangle, on the output.
    #[cfg(test)]
    pub(crate) fn strip(&self) -> Rectangle<i32, Physical> {
        self.strip
    }
}

impl Element for Esmeril {
    fn id(&self) -> &Id {
        &self.mask.id
    }

    /// Changes with the strength and with a commit of its surface, which then damage all of
    /// it, and it's blurred again.
    fn current_commit(&self) -> CommitCounter {
        let steps = MAX_STRENGTH as usize + 1;
        CommitCounter::from(self.mask.commits() * steps + self.strength as usize)
    }

    fn alpha(&self) -> f32 {
        self.alpha
    }

    fn src(&self) -> Rectangle<f64, Buffer> {
        Rectangle::from_size((f64::from(self.read.size.w), f64::from(self.read.size.h)).into())
    }

    fn geometry(&self, _scale: Scale<f64>) -> Rectangle<i32, Physical> {
        self.read
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

/// The last pass, drawn on the frame: `blurred` by where the fragment is on the framebuffer,
/// so whatever the output's transform, and `mask` by where it is on the strip.
fn composite_shader() -> String {
    format!(
        "precision highp float;
uniform sampler2D blurred;
uniform sampler2D mask;
uniform float mask_flipped;
uniform vec4 region;
uniform float alpha;
varying vec2 v_coords;
void main() {{
    vec3 color = texture2D(blurred, (gl_FragCoord.xy - region.xy) / region.zw).rgb;
    float luma = dot(color, vec3(0.2126, 0.7152, 0.0722));
    color = mix(vec3(luma), color, {SATURATION});
    color = clamp(color * {CONTRAST} + (1.0 - {CONTRAST}) / 2.0, 0.0, 1.0);
    vec2 on_mask = vec2(v_coords.x, mix(v_coords.y, 1.0 - v_coords.y, mask_flipped));
    float weight = smoothstep(0.0, {WHOLE_FROM_ALPHA}, texture2D(mask, on_mask).a) * alpha;
    gl_FragColor = vec4(color * weight, weight);
}}
"
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
            UniformName::new("mask_flipped", UniformType::_1f),
            UniformName::new("region", UniformType::_4f),
        ],
    )?;
    Ok(Programs {
        down,
        up,
        composite,
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
    /// The mask of a frame's strip, once it's first drawn.
    mask: Option<GlesTexture>,
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

/// The texture a surface was last drawn from, unless it's one a `sampler2D` can't read: a
/// dmabuf of a format the renderer can only import as an external texture.
fn surface_texture(frame: &GlesFrame<'_, '_>, surface: &WlSurface) -> Option<GlesTexture> {
    let context = frame.context_id();
    let formats = frame.egl_context().dmabuf_render_formats();
    with_renderer_surface_state(surface, |state| {
        let dmabuf = state.buffer().and_then(|buffer| get_dmabuf(buffer).ok());
        if dmabuf.is_some_and(|dmabuf| !formats.contains(&dmabuf.format())) {
            return None;
        }
        state.texture::<GlesTexture>(context).cloned()
    })
    .flatten()
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
        blur.region = region;

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
        let mask = match &esmeril.mask.source {
            Source::Pixels { pixels, size } => {
                if blur.mask.is_none() {
                    let mut renderer = frame.renderer();
                    blur.mask = Some(renderer.as_mut().import_memory(
                        pixels,
                        Fourcc::Argb8888,
                        *size,
                        false,
                    )?);
                }
                blur.mask.clone().expect("it was just imported")
            }
            Source::Surface { surface, .. } => {
                let Some(texture) = surface_texture(frame, surface) else {
                    return Ok(());
                };
                texture
            }
        };

        // From the corner of what it reads to the corner of the strip.
        let strip = esmeril.strip;
        let to_strip = esmeril.read.loc - strip.loc;
        let whole = Rectangle::new(esmeril.shown.loc - strip.loc, esmeril.shown.size);
        let damage: Vec<_> = damage
            .iter()
            .filter_map(|rect| {
                let mut rect = *rect;
                rect.loc += to_strip;
                rect.intersection(whole)
            })
            .collect();
        if damage.is_empty() {
            return Ok(());
        }

        let blurred = &blur.levels[0];
        let region = blur.region;
        frame.with_context(|gl| {
            // Safety: the renderer's context is current in `with_context`.
            unsafe {
                bind_texture(gl, 1, blurred);
                bind_texture(gl, 2, &mask);
                gl.ActiveTexture(ffi::TEXTURE0);
            }
        })?;
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
                Uniform::new(
                    "mask_flipped",
                    UniformValue::_1f(if mask.is_y_inverted() { 1.0 } else { 0.0 }),
                ),
                Uniform::new(
                    "region",
                    UniformValue::_4f(
                        region.loc.x as f32,
                        region.loc.y as f32,
                        region.size.w as f32,
                        region.size.h as f32,
                    ),
                ),
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
    fn the_default_is_kwins_with_its_reach() {
        assert_eq!(step(DEFAULT_STRENGTH).reach, 150);
    }
}
