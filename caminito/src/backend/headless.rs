//! The headless backend: virtual outputs of any size and scale, rendered into memory, for
//! tests (14, 15). It draws with pixman, or, for a test of a shader's pixels, with GLES on
//! surfaceless EGL, Mesa's llvmpipe where there's no GPU (CMN-230).

use smithay::backend::allocator::Fourcc;
use smithay::backend::egl::native::EGLSurfacelessDisplay;
use smithay::backend::egl::{self, EGLContext, EGLDisplay};
use smithay::backend::renderer::damage::OutputDamageTracker;
use smithay::backend::renderer::element::RenderElementStates;
use smithay::backend::renderer::gles::{GlesError, GlesRenderer, GlesTexture};
use smithay::backend::renderer::pixman::PixmanRenderer;
use smithay::backend::renderer::{
    Bind, Color32F, ExportMem, ImportAll, ImportMem, Offscreen, Renderer,
};
use smithay::output::{Mode, Output};
use smithay::reexports::pixman::Image;
use smithay::utils::{Buffer, Physical, Point, Raw, Rectangle, Size, Transform};

use crate::output::{DEFAULT_REFRESH, virtual_output};
use crate::scene::{self, CLEAR_COLOR};
use crate::state::Caminito;

/// Why the GLES renderer can't be had.
#[derive(Debug)]
pub enum Error {
    /// There's no surfaceless EGL display, or no context on it.
    Egl(egl::Error),
    /// The context can't run Smithay's GLES renderer.
    Gles(GlesError),
}

impl std::fmt::Display for Error {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Error::Egl(err) => write!(f, "can't create surfaceless EGL: {err}"),
            Error::Gles(err) => write!(f, "can't start the GLES renderer: {err}"),
        }
    }
}

impl std::error::Error for Error {}

/// What the headless backend draws with, and the memory of each output.
pub enum Drawing {
    /// Pixman, which every test draws with unless it chooses GLES.
    Pixman(PixmanRenderer, Vec<Image<'static, 'static>>),
    /// GLES, which runs the scene's shaders.
    Gles(Box<GlesRenderer>, Vec<GlesTexture>),
}

/// Runs `$body` with `$renderer` as the drawing's renderer and `$texture` as its texture
/// type, and `$memory` as its outputs' memory, whichever it draws with.
macro_rules! drawing {
    ($drawing:expr, $renderer:ident, $memory:ident, $texture:ident => $body:expr) => {
        match $drawing {
            $crate::backend::headless::Drawing::Pixman($renderer, $memory) => {
                type $texture = smithay::reexports::pixman::Image<'static, 'static>;
                $body
            }
            $crate::backend::headless::Drawing::Gles($renderer, $memory) => {
                let $renderer = &mut **$renderer;
                type $texture = smithay::backend::renderer::gles::GlesTexture;
                $body
            }
        }
    };
}
pub(crate) use drawing;

/// The renderer and every virtual output.
pub struct Headless {
    drawing: Drawing,
    targets: Vec<Target>,
    /// How many outputs were ever plugged in, which names the next one.
    plugged: usize,
    /// The modes monitors list but can't show, by connector, for tests of a configuration
    /// that fails.
    refused: Vec<(String, Mode)>,
    /// Whether the monitors are powered off, as the tty's DPMS would have them (129). As on
    /// the tty, no frame is drawn meanwhile.
    powered_off: bool,
    /// The gamma size of the monitors plugged in from now on, standing in for the tty's
    /// CRTCs in tests of wlr-gamma-control (141). `None`, as at first: no gamma table.
    gamma_size: Option<u32>,
}

/// A virtual output. The memory it renders into is the drawing's, at its index.
struct Target {
    output: Output,
    size: Size<i32, Buffer>,
    damage_tracker: OutputDamageTracker,
    rendered: bool,
    /// Whether it was found and is yet to be lit, as a monitor on the tty is until the
    /// saved layout had its say (87).
    unlit: bool,
    /// The modes it was lit in and switched to, in order.
    mode_sets: Vec<Mode>,
    /// The number of entries in each of its gamma ramps, if it has a table.
    gamma_size: Option<u32>,
    /// The gamma client's table on it.
    gamma: Option<Vec<u16>>,
}

impl Default for Headless {
    /// Drawing with pixman.
    fn default() -> Self {
        let renderer = PixmanRenderer::new().expect("pixman renderer");
        Self::drawing(Drawing::Pixman(renderer, Vec::new()))
    }
}

impl Headless {
    /// Drawing with GLES on surfaceless EGL, or the reason it can't.
    pub fn gles() -> Result<Self, Error> {
        // Safety: no other code in the tests creates EGL displays.
        let display = unsafe { EGLDisplay::new(EGLSurfacelessDisplay) }.map_err(Error::Egl)?;
        let context = EGLContext::new(&display).map_err(Error::Egl)?;
        // Safety: the context is new, and only this renderer makes it current.
        let renderer = unsafe { GlesRenderer::new(context) }.map_err(Error::Gles)?;
        Ok(Self::drawing(Drawing::Gles(Box::new(renderer), Vec::new())))
    }

    fn drawing(drawing: Drawing) -> Self {
        Self {
            drawing,
            targets: Vec::new(),
            plugged: 0,
            refused: Vec::new(),
            powered_off: false,
            gamma_size: None,
        }
    }

    /// What it draws with, for the code generic over a renderer.
    pub(crate) fn renderer(&mut self) -> &mut Drawing {
        &mut self.drawing
    }

    /// Plugs in a virtual monitor whose EDID says it's `physical_size` millimetres, with
    /// modes of `modes`, the first of which it prefers. An internal one is named as a
    /// laptop's panel is (50).
    pub fn plug(
        &mut self,
        caminito: &mut Caminito,
        physical_size: Size<i32, Raw>,
        modes: &[Size<i32, Physical>],
        internal: bool,
    ) -> Output {
        self.plugged += 1;
        let connector = if internal { "eDP" } else { "headless" };
        let name = format!("{connector}-{}", self.plugged);
        self.plug_at(caminito, name, physical_size, modes)
    }

    /// Plugs a virtual monitor in at the connector `name`, as `plug` does at a new one: an
    /// unplugged monitor plugged in again where it was. It's lit by `light`.
    pub fn plug_at(
        &mut self,
        caminito: &mut Caminito,
        name: String,
        physical_size: Size<i32, Raw>,
        modes: &[Size<i32, Physical>],
    ) -> Output {
        let mode = modes[0];
        let output = virtual_output(name, "Headless", physical_size, mode, Transform::Normal);
        for &size in &modes[1..] {
            output.add_mode(Mode {
                size,
                refresh: DEFAULT_REFRESH,
            });
        }

        let size = (mode.w, mode.h).into();
        drawing!(&mut self.drawing, renderer, memory, _T => memory.push(create(renderer, size)));
        self.targets.push(Target {
            damage_tracker: OutputDamageTracker::from_output(&output),
            output: output.clone(),
            size,
            rendered: false,
            unlit: true,
            mode_sets: Vec::new(),
            gamma_size: self.gamma_size,
            gamma: None,
        });
        caminito.add_output(output.clone());
        output
    }

    /// Switches a virtual monitor to a mode of `size`.
    pub fn change_mode(
        &mut self,
        caminito: &mut Caminito,
        output: &Output,
        size: Size<i32, Physical>,
    ) {
        let mode = Mode {
            size,
            refresh: DEFAULT_REFRESH,
        };
        assert!(self.set_mode(caminito, output, mode), "the mode is shown");
    }

    /// Makes a virtual monitor fail to show its mode of `size`, as a monitor does whose
    /// mode the GPU can't drive, and the monitor plugged in again at its connector too.
    pub fn refuse_mode(&mut self, output: &Output, size: Size<i32, Physical>) {
        let mode = Mode {
            size,
            refresh: DEFAULT_REFRESH,
        };
        self.refused.push((output.name(), mode));
    }

    /// Switches a virtual monitor to `mode`, and returns whether it did. One that isn't
    /// lit yet only takes note of the mode, as on the tty.
    pub fn set_mode(&mut self, caminito: &mut Caminito, output: &Output, mode: Mode) -> bool {
        let unlit = target_mut(&mut self.targets, output).is_some_and(|target| target.unlit);
        if !unlit && !self.resize(output, mode) {
            return false;
        }
        output.change_current_state(Some(mode), None, None, None);
        caminito.output_mode_changed(output);
        true
    }

    /// The virtual monitors plugged in and not lit yet.
    pub(crate) fn unlit_outputs(&self) -> Vec<Output> {
        let unlit = self.targets.iter().filter(|target| target.unlit);
        unlit.map(|target| target.output.clone()).collect()
    }

    /// Lights a virtual monitor that was plugged in, in the mode its output has, and
    /// returns whether it did.
    pub(crate) fn light(&mut self, output: &Output) -> bool {
        let Some(mode) = output.current_mode() else {
            return false;
        };
        if !self.resize(output, mode) {
            return false;
        }
        let target = target_mut(&mut self.targets, output).expect("a headless output");
        target.unlit = false;
        true
    }

    /// Turns a virtual monitor off. One that wasn't lit yet won't be.
    pub(crate) fn disable(&mut self, caminito: &mut Caminito, output: &Output) {
        let target = target_mut(&mut self.targets, output).expect("a headless output");
        target.unlit = false;
        caminito.disable_output(output);
    }

    /// Lights a disabled virtual monitor again in `mode`, and returns whether it did.
    pub fn enable(&mut self, caminito: &mut Caminito, output: &Output, mode: Mode) -> bool {
        if !self.resize(output, mode) {
            return false;
        }
        caminito.enable_output(output, mode);
        true
    }

    /// Gives a virtual monitor the memory for `mode`, which is a mode set, unless it
    /// refuses the mode.
    fn resize(&mut self, output: &Output, mode: Mode) -> bool {
        if self.refused.contains(&(output.name(), mode)) {
            return false;
        }
        let index = target_index(&self.targets, output).expect("a headless output");
        let size = (mode.size.w, mode.size.h).into();
        drawing!(&mut self.drawing, renderer, memory, _T => {
            memory[index] = create(renderer, size);
        });
        let target = &mut self.targets[index];
        target.size = size;
        target.rendered = false;
        target.mode_sets.push(mode);
        true
    }

    /// The modes a virtual monitor was lit in and switched to since it was plugged in, in
    /// order.
    pub(crate) fn mode_sets(&mut self, output: &Output) -> Vec<Mode> {
        target_mut(&mut self.targets, output).map_or_else(Vec::new, |t| t.mode_sets.clone())
    }

    /// Powers every virtual monitor off or on, keeping its mode (129).
    pub(crate) fn set_powered(&mut self, on: bool) {
        self.powered_off = !on;
    }

    /// Whether the monitors are powered off.
    pub(crate) fn is_powered_off(&self) -> bool {
        self.powered_off
    }

    /// Gives the virtual monitors plugged in from now on gamma tables of `size` entries a
    /// ramp, or none.
    pub(crate) fn give_gamma(&mut self, size: Option<u32>) {
        self.gamma_size = size;
    }

    /// The number of entries in each of `output`'s gamma ramps, if it has a table.
    pub(crate) fn gamma_size(&self, output: &Output) -> Option<u32> {
        let target = self.targets.iter().find(|t| t.output == *output)?;
        target.gamma_size
    }

    /// Puts a gamma client's table on `output`, or takes it off. Night Light's tables
    /// aren't kept: the dump shows them.
    pub(crate) fn set_gamma(&mut self, output: &Output, table: Option<Vec<u16>>) -> bool {
        let Some(target) = target_mut(&mut self.targets, output) else {
            return false;
        };
        let size = target.gamma_size.map(|size| size as usize * 3);
        if table
            .as_ref()
            .is_some_and(|table| Some(table.len()) != size)
        {
            return false;
        }
        target.gamma = table;
        true
    }

    /// The gamma client's table on `output`.
    pub(crate) fn client_gamma(&self, output: &Output) -> Option<Vec<u16>> {
        let target = self.targets.iter().find(|t| t.output == *output)?;
        target.gamma.clone()
    }

    /// Unplugs a virtual monitor.
    pub fn unplug(&mut self, caminito: &mut Caminito, output: &Output) {
        if let Some(index) = target_index(&self.targets, output) {
            self.targets.remove(index);
            drawing!(&mut self.drawing, _renderer, memory, _T => {
                memory.remove(index);
            });
        }
        caminito.remove_output(output);
    }

    /// Renders `output`, and returns how each element was drawn.
    pub fn render(&mut self, caminito: &mut Caminito, output: &Output) -> RenderElementStates {
        let index = target_index(&self.targets, output).expect("a headless output");
        let target = &mut self.targets[index];
        let states = drawing!(&mut self.drawing, renderer, memory, _T => {
            render(renderer, &mut memory[index], &mut target.damage_tracker, caminito, output)
        });
        target.rendered = true;
        states
    }

    /// The output's pixels as ARGB8888, row by row, or `None` before it's first rendered.
    pub fn pixels(&mut self, output: &Output) -> Option<Vec<u32>> {
        let size = self.targets.iter().find(|t| t.output == *output)?.size;
        self.read(output, Rectangle::from_size(size))
    }

    /// The output's pixel at `at` as ARGB8888, or `None` before it's first rendered.
    pub fn pixel(&mut self, output: &Output, at: Point<i32, Physical>) -> Option<u32> {
        self.read(output, Rectangle::new((at.x, at.y).into(), (1, 1).into()))?
            .first()
            .copied()
    }

    /// The output's pixels in `rect` as ARGB8888, row by row.
    fn read(&mut self, output: &Output, rect: Rectangle<i32, Buffer>) -> Option<Vec<u32>> {
        let index = target_index(&self.targets, output)?;
        if !self.targets[index].rendered {
            return None;
        }
        drawing!(&mut self.drawing, renderer, memory, _T => {
            read(renderer, &mut memory[index], rect)
        })
    }
}

/// New memory of `size` for an output.
fn create<R, T>(renderer: &mut R, size: Size<i32, Buffer>) -> T
where
    R: Offscreen<T>,
{
    renderer
        .create_buffer(Fourcc::Argb8888, size)
        .expect("the headless renderer makes memory for an output")
}

/// Renders `output` into `memory`, and returns how each element was drawn.
fn render<R, T>(
    renderer: &mut R,
    memory: &mut T,
    damage_tracker: &mut OutputDamageTracker,
    caminito: &Caminito,
    output: &Output,
) -> RenderElementStates
where
    R: Renderer + ImportAll + ImportMem + Bind<T>,
    R::TextureId: Send + Clone + 'static,
{
    let mut framebuffer = renderer.bind(memory).expect("bind");
    let elements = scene::output_elements(renderer, caminito, output);
    damage_tracker
        .render_output(renderer, &mut framebuffer, 0, &elements, CLEAR_COLOR)
        .expect("render")
        .states
}

/// The pixels of `memory` in `rect` as ARGB8888, row by row from the top. GLES's mapping
/// says it's flipped, but Smithay draws into a texture top row first, so it isn't here.
fn read<R, T>(renderer: &mut R, memory: &mut T, rect: Rectangle<i32, Buffer>) -> Option<Vec<u32>>
where
    R: Bind<T> + ExportMem,
{
    let framebuffer = renderer.bind(memory).ok()?;
    let mapping = renderer
        .copy_framebuffer(&framebuffer, rect, Fourcc::Argb8888)
        .ok()?;
    let bytes = renderer.map_texture(&mapping).ok()?;
    Some(
        bytes
            .as_chunks::<4>()
            .0
            .iter()
            .map(|p| u32::from_le_bytes(*p))
            .collect(),
    )
}

fn target_index(targets: &[Target], output: &Output) -> Option<usize> {
    targets.iter().position(|t| t.output == *output)
}

fn target_mut<'a>(targets: &'a mut [Target], output: &Output) -> Option<&'a mut Target> {
    targets.iter_mut().find(|t| t.output == *output)
}

/// `color` as an ARGB8888 pixel.
pub fn argb8888(color: Color32F) -> u32 {
    color
        .components()
        .map(|c| (c * 255.).round() as u32)
        .into_iter()
        .zip([16, 8, 0, 24])
        .fold(0, |pixel, (c, shift)| pixel | c << shift)
}
