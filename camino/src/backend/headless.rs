//! The headless backend: virtual outputs of any size and scale, rendered with pixman into
//! memory, for tests (14, 15).

use smithay::backend::allocator::Fourcc;
use smithay::backend::renderer::damage::OutputDamageTracker;
use smithay::backend::renderer::pixman::PixmanRenderer;
use smithay::backend::renderer::{Bind, Color32F, ExportMem, Offscreen};
use smithay::output::Output;
use smithay::reexports::pixman::Image;
use smithay::utils::{Physical, Rectangle, Size, Transform};

use crate::output::virtual_output;
use crate::scene::{self, CLEAR_COLOR};
use crate::state::Camino;

/// The pixman renderer and every virtual output.
pub struct Headless {
    renderer: PixmanRenderer,
    targets: Vec<Target>,
}

/// A virtual output and the memory it renders into.
struct Target {
    output: Output,
    buffer: Image<'static, 'static>,
    damage_tracker: OutputDamageTracker,
    rendered: bool,
}

impl Default for Headless {
    fn default() -> Self {
        Self {
            renderer: PixmanRenderer::new().expect("pixman renderer"),
            targets: Vec::new(),
        }
    }
}

impl Headless {
    pub fn renderer(&mut self) -> &mut PixmanRenderer {
        &mut self.renderer
    }

    /// Adds a virtual output of `size` physical pixels at `scale`.
    pub fn add_output(
        &mut self,
        camino: &mut Camino,
        size: Size<i32, Physical>,
        scale: f64,
    ) -> Output {
        let output = virtual_output(
            format!("headless-{}", self.targets.len() + 1),
            "Headless",
            size,
            scale,
            Transform::Normal,
        );

        let buffer = self
            .renderer
            .create_buffer(Fourcc::Argb8888, (size.w, size.h).into())
            .expect("pixman buffer");
        self.targets.push(Target {
            damage_tracker: OutputDamageTracker::from_output(&output),
            output: output.clone(),
            buffer,
            rendered: false,
        });
        camino.add_output(output.clone());
        output
    }

    pub fn render(&mut self, camino: &mut Camino, output: &Output) {
        let target = target_mut(&mut self.targets, output).expect("a headless output");
        let mut framebuffer = self.renderer.bind(&mut target.buffer).expect("bind");
        let elements = scene::output_elements(&mut self.renderer, camino, output);
        target
            .damage_tracker
            .render_output(
                &mut self.renderer,
                &mut framebuffer,
                0,
                &elements,
                CLEAR_COLOR,
            )
            .expect("render");
        target.rendered = true;
    }

    /// The output's pixels as ARGB8888, row by row, or `None` before it's first rendered.
    pub fn pixels(&mut self, output: &Output) -> Option<Vec<u32>> {
        let target = target_mut(&mut self.targets, output)?;
        if !target.rendered {
            return None;
        }
        let size = (target.buffer.width() as i32, target.buffer.height() as i32);
        let framebuffer = self.renderer.bind(&mut target.buffer).ok()?;
        let mapping = self
            .renderer
            .copy_framebuffer(
                &framebuffer,
                Rectangle::from_size(size.into()),
                Fourcc::Argb8888,
            )
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
