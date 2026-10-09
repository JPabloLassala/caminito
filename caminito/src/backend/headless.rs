//! The headless backend: virtual outputs of any size and scale, rendered with pixman into
//! memory, for tests (14, 15).

use smithay::backend::allocator::Fourcc;
use smithay::backend::renderer::damage::OutputDamageTracker;
use smithay::backend::renderer::element::RenderElementStates;
use smithay::backend::renderer::pixman::PixmanRenderer;
use smithay::backend::renderer::{Bind, Color32F, ExportMem, Offscreen};
use smithay::output::{Mode, Output};
use smithay::reexports::pixman::Image;
use smithay::utils::{Buffer, Physical, Point, Raw, Rectangle, Size, Transform};

use crate::output::{DEFAULT_REFRESH, virtual_output};
use crate::scene::{self, CLEAR_COLOR};
use crate::state::Caminito;

/// The pixman renderer and every virtual output.
pub struct Headless {
    renderer: PixmanRenderer,
    targets: Vec<Target>,
    /// How many outputs were ever plugged in, which names the next one.
    plugged: usize,
    /// The modes monitors list but can't show, by connector, for tests of a configuration
    /// that fails.
    refused: Vec<(String, Mode)>,
    /// Whether the monitors are powered off, as the tty's DPMS would have them (129). As on
    /// the tty, no frame is drawn meanwhile.
    powered_off: bool,
}

/// A virtual output and the memory it renders into.
struct Target {
    output: Output,
    buffer: Image<'static, 'static>,
    damage_tracker: OutputDamageTracker,
    rendered: bool,
    /// Whether it was found and is yet to be lit, as a monitor on the tty is until the
    /// saved layout had its say (87).
    unlit: bool,
    /// The modes it was lit in and switched to, in order.
    mode_sets: Vec<Mode>,
}

impl Default for Headless {
    fn default() -> Self {
        Self {
            renderer: PixmanRenderer::new().expect("pixman renderer"),
            targets: Vec::new(),
            plugged: 0,
            refused: Vec::new(),
            powered_off: false,
        }
    }
}

impl Headless {
    pub fn renderer(&mut self) -> &mut PixmanRenderer {
        &mut self.renderer
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

        let buffer = self
            .renderer
            .create_buffer(Fourcc::Argb8888, (mode.w, mode.h).into())
            .expect("pixman buffer");
        self.targets.push(Target {
            damage_tracker: OutputDamageTracker::from_output(&output),
            output: output.clone(),
            buffer,
            rendered: false,
            unlit: true,
            mode_sets: Vec::new(),
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
        let target = target_mut(&mut self.targets, output).expect("a headless output");
        target.buffer = self
            .renderer
            .create_buffer(Fourcc::Argb8888, (mode.size.w, mode.size.h).into())
            .expect("pixman buffer");
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

    /// Unplugs a virtual monitor.
    pub fn unplug(&mut self, caminito: &mut Caminito, output: &Output) {
        self.targets.retain(|target| target.output != *output);
        caminito.remove_output(output);
    }

    /// Renders `output`, and returns how each element was drawn.
    pub fn render(&mut self, caminito: &mut Caminito, output: &Output) -> RenderElementStates {
        let target = target_mut(&mut self.targets, output).expect("a headless output");
        let mut framebuffer = self.renderer.bind(&mut target.buffer).expect("bind");
        let elements = scene::output_elements(&mut self.renderer, caminito, output);
        let states = target
            .damage_tracker
            .render_output(
                &mut self.renderer,
                &mut framebuffer,
                0,
                &elements,
                CLEAR_COLOR,
            )
            .expect("render")
            .states;
        target.rendered = true;
        states
    }

    /// The output's pixels as ARGB8888, row by row, or `None` before it's first rendered.
    pub fn pixels(&mut self, output: &Output) -> Option<Vec<u32>> {
        let target = target_mut(&mut self.targets, output)?;
        let size = (target.buffer.width() as i32, target.buffer.height() as i32);
        self.read(output, Rectangle::from_size(size.into()))
    }

    /// The output's pixel at `at` as ARGB8888, or `None` before it's first rendered.
    pub fn pixel(&mut self, output: &Output, at: Point<i32, Physical>) -> Option<u32> {
        self.read(output, Rectangle::new((at.x, at.y).into(), (1, 1).into()))?
            .first()
            .copied()
    }

    /// The output's pixels in `rect` as ARGB8888, row by row.
    fn read(&mut self, output: &Output, rect: Rectangle<i32, Buffer>) -> Option<Vec<u32>> {
        let target = target_mut(&mut self.targets, output)?;
        if !target.rendered {
            return None;
        }
        let framebuffer = self.renderer.bind(&mut target.buffer).ok()?;
        let mapping = self
            .renderer
            .copy_framebuffer(&framebuffer, rect, Fourcc::Argb8888)
            .ok()?;
        let bytes = self.renderer.map_texture(&mapping).ok()?;
        Some(
            bytes
                .as_chunks::<4>()
                .0
                .iter()
                .map(|p| u32::from_le_bytes(*p))
                .collect(),
        )
    }
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
