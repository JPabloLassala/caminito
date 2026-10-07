//! What image capture copies (103): a capture source's size and scale, and its pixels drawn
//! into a client's buffer by whichever renderer the backend has. The protocols' handlers and
//! the sessions are in `handlers::image_capture`, and what a capture shows is `scene`'s.
//!
//! Decided here, as 103 asked:
//!
//! - **A minimised window, or one on another workspace,** is captured as it was last drawn,
//!   with its frame: its surfaces keep their buffers, and a capture never depended on the
//!   window showing.
//! - **A shaded window** is captured as it's drawn, its frame's titlebar alone, and its
//!   sessions get constraints of that size. No session stops for either.
//! - **Buffers are shm ones.** Smithay's state would carry dmabuf constraints, but only the
//!   tty could draw into a dmabuf, and nothing in v0.1 asks for one.
//! - **A session's first frame is copied at once,** and each later one when its source has
//!   changed, with the damage since the frame before, as the protocol lets a compositor
//!   wait. A client that asks for frame after frame is paced by its source.
//! - **An output that's turned off is gone,** as an unplugged one is: its sessions stop,
//!   and a client opens a new one once it's lit again.
//! - **`paint_cursors` is about the pointer's cursor alone.** A drag's icon is in an
//!   output's capture either way, as it's on the output. A window's capture has the cursor
//!   while the window is showing and the pointer is within it, whether or not another
//!   window covers it there, as the capture itself ignores the cover.
//! - **Cursor sessions aren't offered,** since Smithay's state leaves their frames and the
//!   cursor's position to the compositor.

use std::fmt;

use caminito_policy::WindowId;
use smithay::backend::allocator::Fourcc;
use smithay::backend::renderer::damage::{Error as DamageError, OutputDamageTracker};
use smithay::backend::renderer::{Color32F, ExportMem, ImportAll, ImportMem, Offscreen, Renderer};
use smithay::output::WeakOutput;
use smithay::reexports::wayland_server::protocol::wl_buffer::WlBuffer;
use smithay::reexports::wayland_server::protocol::wl_shm::Format;
use smithay::utils::{Buffer, Physical, Rectangle, Size};
use smithay::wayland::shm::{with_buffer_contents, with_buffer_contents_mut};

use crate::scene::{self, CLEAR_COLOR, SceneElement};
use crate::state::Caminito;

/// The shm formats a capture is copied in. Their bytes are the same, and a client that has
/// no use for the alpha of a window's capture takes the second.
pub(crate) const SHM_FORMATS: [Format; 2] = [Format::Argb8888, Format::Xrgb8888];

/// What a capture source shows, which Caminito keeps in the source's user data.
#[derive(Debug, Clone, PartialEq)]
pub(crate) enum CaptureSource {
    /// What an output shows.
    Output(WeakOutput),
    /// A window with its frame.
    Toplevel(WindowId),
}

/// One frame to copy: what of, and how it compares with the session's last.
pub(crate) struct Shot<'a> {
    pub source: &'a CaptureSource,
    pub paint_cursors: bool,
    /// The size the session asks its buffers to be.
    pub size: Size<i32, Physical>,
    pub scale: f64,
    /// What the session's frames so far showed.
    pub tracker: &'a mut OutputDamageTracker,
}

/// Why a frame wasn't copied.
#[derive(Debug)]
pub(crate) enum Error {
    /// The client's buffer isn't an shm one of the size and a format the session asks for.
    Buffer,
    /// The renderer couldn't draw the frame or read it back.
    Render(String),
}

impl fmt::Display for Error {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Error::Buffer => write!(f, "the buffer isn't what the session asks for"),
            Error::Render(err) => write!(f, "can't draw the frame: {err}"),
        }
    }
}

impl std::error::Error for Error {}

impl Caminito {
    /// The size in pixels and the scale of a capture of `source`, or `None` if the source
    /// is gone: an output that's unplugged or turned off, or a window that isn't managed
    /// any more. An output's capture has its mode's size, and a window's has its frame
    /// rect's at the scale of the window's main output.
    pub(crate) fn capture_extent(
        &self,
        source: &CaptureSource,
    ) -> Option<(Size<i32, Physical>, f64)> {
        match source {
            CaptureSource::Output(output) => {
                let output = output.upgrade().filter(|o| self.outputs.contains(o))?;
                let scale = output.current_scale().fractional_scale();
                Some((output.current_mode()?.size, scale))
            }
            CaptureSource::Toplevel(id) => {
                let frame = self.policy.frame_rect(*id)?;
                let main = self.policy.window(*id)?.main_output();
                let scale = main.map_or(1.0, |main| main.current_scale().fractional_scale());
                Some((frame.size.to_physical_precise_round(scale), scale))
            }
        }
    }
}

/// What `shot` shows, front to back, and what shows where none of it is.
fn elements<R>(
    renderer: &mut R,
    caminito: &Caminito,
    shot: &Shot<'_>,
) -> (Vec<SceneElement<R>>, Color32F)
where
    R: Renderer + ImportAll + ImportMem,
    R::TextureId: Send + Clone + 'static,
{
    let cursors = shot.paint_cursors;
    match shot.source {
        CaptureSource::Output(output) => {
            let elements = output
                .upgrade()
                .map(|output| scene::output_capture_elements(renderer, caminito, &output, cursors));
            (elements.unwrap_or_default(), CLEAR_COLOR)
        }
        CaptureSource::Toplevel(id) => {
            let elements =
                scene::window_capture_elements(renderer, caminito, *id, shot.scale, cursors);
            (elements, Color32F::TRANSPARENT)
        }
    }
}

/// Copies what `shot` shows into `buffer`, if it changed since the session's last frame,
/// and returns what changed. Nothing is copied, and `None` returned, if nothing did.
///
/// The frame is drawn whole into memory of the renderer's own and copied from there, since
/// no renderer draws into a client's shm buffer. Written from Smithay's renderer traits:
/// niri's and cosmic-comp's screencopy are GPL-3, so they weren't used (03).
pub(crate) fn copy<R, T>(
    renderer: &mut R,
    caminito: &Caminito,
    shot: Shot<'_>,
    buffer: &WlBuffer,
) -> Result<Option<Vec<Rectangle<i32, Buffer>>>, Error>
where
    R: Renderer + ImportAll + ImportMem + Offscreen<T> + ExportMem,
    R::TextureId: Send + Clone + 'static,
{
    let size = shot.size;
    let bytes = i64::from(size.w) * i64::from(size.h) * 4;
    let bytes = usize::try_from(bytes).map_err(|_| Error::Buffer)?;
    let fits = with_buffer_contents(buffer, |_, len, data| {
        data.width == size.w
            && data.height == size.h
            && data.stride == size.w * 4
            && SHM_FORMATS.contains(&data.format)
            && usize::try_from(data.offset).is_ok_and(|offset| offset + bytes <= len)
    });
    if !fits.unwrap_or(false) {
        return Err(Error::Buffer);
    }

    let (elements, clear) = elements(renderer, caminito, &shot);
    let tracker = shot.tracker;
    // Against the session's last frame, which is one frame old. A session's first frame
    // has none before it, and is all damage.
    let (damage, _) = tracker.damage_output(1, &elements).map_err(render_error)?;
    let Some(damage) = damage.filter(|damage| !damage.is_empty()) else {
        return Ok(None);
    };
    let damage = damage
        .iter()
        .map(|rect| {
            Rectangle::new(
                (rect.loc.x, rect.loc.y).into(),
                (rect.size.w, rect.size.h).into(),
            )
        })
        .collect();

    let buffer_size = Size::from((size.w, size.h));
    let mut target = renderer
        .create_buffer(Fourcc::Argb8888, buffer_size)
        .map_err(render_error)?;
    let mapping = {
        let mut framebuffer = renderer.bind(&mut target).map_err(render_error)?;
        // With no age, so that all of it is drawn: the memory is new.
        tracker
            .render_output(renderer, &mut framebuffer, 0, &elements, clear)
            .map_err(|err| match err {
                DamageError::Rendering(err) => render_error(err),
                DamageError::OutputNoMode(err) => render_error(err),
            })?;
        let whole = Rectangle::from_size(buffer_size);
        renderer
            .copy_framebuffer(&framebuffer, whole, Fourcc::Argb8888)
            .map_err(render_error)?
    };
    let pixels = renderer.map_texture(&mapping).map_err(render_error)?;
    if pixels.len() != bytes {
        return Err(Error::Render(format!(
            "the renderer read back {} bytes, not {bytes}",
            pixels.len()
        )));
    }
    let copied = with_buffer_contents_mut(buffer, |pool, len, data| {
        let Some(offset) = usize::try_from(data.offset)
            .ok()
            .filter(|offset| offset + bytes <= len)
        else {
            return false;
        };
        // Safety: `pool` is valid for `len` bytes while this closure runs, the `bytes`
        // written start `offset` into it and end within it, and `pixels` is the renderer's
        // own memory, which the client's pool doesn't overlap. No reference to the pool is
        // made, since the client may write it meanwhile.
        unsafe { std::ptr::copy_nonoverlapping(pixels.as_ptr(), pool.add(offset), bytes) };
        true
    });
    match copied {
        Ok(true) => Ok(Some(damage)),
        _ => Err(Error::Buffer),
    }
}

fn render_error(err: impl fmt::Debug) -> Error {
    Error::Render(format!("{err:?}"))
}
