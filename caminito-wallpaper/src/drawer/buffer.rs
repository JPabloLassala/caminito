//! An output's buffers: shared memory the compositor reads, written from a cairo frame.
//! A buffer is reused once the compositor releases it, so a cross-fade draws into the
//! same two or three.

use std::fs::File;
use std::os::unix::fs::FileExt;
use std::sync::Arc;
use std::sync::atomic::{AtomicBool, Ordering};

use wayland_client::protocol::wl_buffer::{self, WlBuffer};
use wayland_client::protocol::wl_shm::{self, WlShm};
use wayland_client::{Connection, Dispatch, QueueHandle};

use super::App;

/// The most buffers an output has. A frame with all of them busy waits for a release.
const MOST: usize = 3;

/// What a buffer's release is told to.
pub struct BufferData {
    /// Whether the compositor still reads it.
    busy: AtomicBool,
    /// The registry name of the output it's of.
    pub output: u32,
}

/// One buffer.
pub struct Buffer {
    file: File,
    buffer: WlBuffer,
    size: (i32, i32),
    data: Arc<BufferData>,
}

impl Drop for Buffer {
    fn drop(&mut self) {
        self.buffer.destroy();
    }
}

/// Why a frame can't be shown.
#[derive(Debug)]
pub enum Error {
    /// The shared memory can't be made or written.
    Memory(std::io::Error),
    /// The frame's pixels are borrowed elsewhere.
    Borrowed(cairo::BorrowError),
}

impl std::fmt::Display for Error {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Error::Memory(err) => write!(f, "can't use shared memory: {err}"),
            Error::Borrowed(err) => write!(f, "can't read the frame: {err}"),
        }
    }
}

/// An output's buffers.
#[derive(Default)]
pub struct Buffers {
    buffers: Vec<Buffer>,
}

impl Buffers {
    /// A buffer the compositor doesn't read, with `frame` in it, made if need be, or `None`
    /// while every buffer is busy.
    pub fn write(
        &mut self,
        frame: &cairo::ImageSurface,
        shm: &WlShm,
        qh: &QueueHandle<App>,
        output: u32,
    ) -> Result<Option<&WlBuffer>, Error> {
        let size = (frame.width(), frame.height());
        self.buffers
            .retain(|buffer| buffer.size == size || buffer.is_busy());
        let free = self.buffers.iter().position(|buffer| !buffer.is_busy());
        let index = match free {
            Some(index) => index,
            None if self.buffers.len() < MOST => {
                let stride = frame.stride();
                self.buffers
                    .push(Buffer::new(size, stride, shm, qh, output)?);
                self.buffers.len() - 1
            }
            None => return Ok(None),
        };
        let buffer = &self.buffers[index];
        let mut written = Ok(());
        frame
            .with_data(|data| written = buffer.file.write_all_at(data, 0))
            .map_err(Error::Borrowed)?;
        written.map_err(Error::Memory)?;
        buffer.data.busy.store(true, Ordering::Relaxed);
        Ok(Some(&buffer.buffer))
    }
}

impl Buffer {
    fn new(
        size: (i32, i32),
        stride: i32,
        shm: &WlShm,
        qh: &QueueHandle<App>,
        output: u32,
    ) -> Result<Self, Error> {
        let length = stride * size.1;
        let fd = rustix::fs::memfd_create("caminito-wallpaper", rustix::fs::MemfdFlags::CLOEXEC)
            .map_err(|err| Error::Memory(err.into()))?;
        let file = File::from(fd);
        file.set_len(length as u64).map_err(Error::Memory)?;
        let pool = shm.create_pool(std::os::fd::AsFd::as_fd(&file), length, qh, ());
        let data = Arc::new(BufferData {
            busy: AtomicBool::new(false),
            output,
        });
        // cairo's RGB24 is wl_shm's XRGB8888: a native-endian word, its top byte unused.
        let buffer = pool.create_buffer(
            0,
            size.0,
            size.1,
            stride,
            wl_shm::Format::Xrgb8888,
            qh,
            data.clone(),
        );
        pool.destroy();
        Ok(Buffer {
            file,
            buffer,
            size,
            data,
        })
    }

    fn is_busy(&self) -> bool {
        self.data.busy.load(Ordering::Relaxed)
    }
}

impl Dispatch<WlBuffer, Arc<BufferData>> for App {
    fn event(
        app: &mut Self,
        _: &WlBuffer,
        event: wl_buffer::Event,
        data: &Arc<BufferData>,
        _: &Connection,
        _: &QueueHandle<Self>,
    ) {
        if let wl_buffer::Event::Release = event {
            data.busy.store(false, Ordering::Relaxed);
            app.buffer_released(data.output);
        }
    }
}
