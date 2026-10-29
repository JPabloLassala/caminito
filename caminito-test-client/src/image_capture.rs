//! An image-capture client, as a screenshot tool is one (103): it makes a source of an
//! output, or of a toplevel of the foreign toplevel list, opens a session on it through
//! `ext_image_copy_capture_v1`, and has frames copied into shm buffers of the size the
//! session asks for.

use std::fs::File;
use std::os::unix::fs::FileExt;

use wayland_client::{Connection, Dispatch, QueueHandle, WEnum};
use wayland_protocols::ext::image_capture_source::v1::client::ext_foreign_toplevel_image_capture_source_manager_v1::{
    self, ExtForeignToplevelImageCaptureSourceManagerV1,
};
use wayland_protocols::ext::image_capture_source::v1::client::ext_image_capture_source_v1::{
    self, ExtImageCaptureSourceV1,
};
use wayland_protocols::ext::image_capture_source::v1::client::ext_output_image_capture_source_manager_v1::{
    self, ExtOutputImageCaptureSourceManagerV1,
};
use wayland_protocols::ext::image_copy_capture::v1::client::ext_image_copy_capture_frame_v1::{
    self, ExtImageCopyCaptureFrameV1,
};
use wayland_protocols::ext::image_copy_capture::v1::client::ext_image_copy_capture_manager_v1::{
    self, ExtImageCopyCaptureManagerV1, Options,
};
use wayland_protocols::ext::image_copy_capture::v1::client::ext_image_copy_capture_session_v1::{
    self, ExtImageCopyCaptureSessionV1,
};

pub use wayland_client::protocol::wl_shm::Format;
pub use wayland_protocols::ext::image_copy_capture::v1::client::ext_image_copy_capture_frame_v1::FailureReason;

use crate::{Buffer, Client, State};

/// The capture globals the client bound, and the sessions it opened.
#[derive(Default)]
pub(crate) struct ImageCapture {
    output_sources: Option<ExtOutputImageCaptureSourceManagerV1>,
    toplevel_sources: Option<ExtForeignToplevelImageCaptureSourceManagerV1>,
    manager: Option<ExtImageCopyCaptureManagerV1>,
    sessions: Vec<Session>,
}

/// The index of one of a client's capture sessions.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct SessionId(usize);

/// A capture session, and the source it copies.
pub struct Session {
    proxy: ExtImageCopyCaptureSessionV1,
    _source: ExtImageCaptureSourceV1,
    /// The constraints since the last `done`.
    pending: Constraints,
    /// Every set of constraints the session has sent, oldest first.
    pub constraints: Vec<Constraints>,
    /// Whether the compositor stopped the session.
    pub stopped: bool,
    /// Every frame the client asked for, oldest first.
    pub frames: Vec<Frame>,
}

/// What a session asks of the buffers it copies into.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct Constraints {
    pub size: (i32, i32),
    pub shm_formats: Vec<Format>,
    /// Whether the session takes dmabufs too.
    pub dmabuf: bool,
}

/// A frame the client asked for, and the buffer it's copied into.
pub struct Frame {
    _proxy: ExtImageCopyCaptureFrameV1,
    _buffer: Buffer,
    /// The buffer's memory.
    file: File,
    size: (i32, i32),
    /// What the compositor said changed since the session's last frame, as x, y, width and
    /// height.
    pub damage: Vec<(i32, i32, i32, i32)>,
    /// How the frame ended, once it has.
    pub outcome: Option<Outcome>,
}

/// How a frame ended.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Outcome {
    /// The buffer has the source's pixels.
    Ready,
    Failed(FailureReason),
}

/// A captured frame's pixels as ARGB8888, row by row.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Image {
    pub size: (i32, i32),
    pub pixels: Vec<u32>,
}

impl Image {
    /// The pixel at `(x, y)`.
    pub fn at(&self, x: i32, y: i32) -> u32 {
        let (width, height) = self.size;
        assert!(
            (0..width).contains(&x) && (0..height).contains(&y),
            "({x}, {y}) is in a {width} × {height} image"
        );
        self.pixels[(y * width + x) as usize]
    }
}

impl Frame {
    /// The buffer's pixels, which are the source's once the frame is ready.
    pub fn image(&self) -> Image {
        let (width, height) = self.size;
        let mut bytes = vec![0; (width * height * 4) as usize];
        self.file
            .read_exact_at(&mut bytes, 0)
            .expect("a buffer of its size");
        let pixels = bytes.as_chunks::<4>().0;
        Image {
            size: self.size,
            pixels: pixels.iter().map(|p| u32::from_le_bytes(*p)).collect(),
        }
    }
}

impl Client {
    /// Binds the two source managers and the copy-capture manager.
    pub fn bind_image_capture(&mut self) {
        let (registry, globals, qh) = (&self.registry, &self.state.globals, &self.qh);
        self.state.image_capture.output_sources = Some(crate::bind(registry, globals, qh, 1));
        self.state.image_capture.toplevel_sources = Some(crate::bind(registry, globals, qh, 1));
        self.state.image_capture.manager = Some(crate::bind(registry, globals, qh, 1));
    }

    /// Opens a session on the bound output at `output`, with the cursor painted on its
    /// frames or not. Roundtrip before asking for a frame, for its constraints.
    pub fn capture_output(&mut self, output: usize, paint_cursors: bool) -> SessionId {
        let sources = self.state.image_capture.output_sources.as_ref();
        let source = sources.expect("bind_image_capture first").create_source(
            &self.state.outputs[output].proxy,
            &self.qh,
            (),
        );
        self.open_capture_session(source, paint_cursors)
    }

    /// Opens a session on the toplevel at `listed` in the foreign toplevel list, with the
    /// cursor painted on its frames or not. Roundtrip before asking for a frame, for its
    /// constraints.
    pub fn capture_toplevel(&mut self, listed: usize, paint_cursors: bool) -> SessionId {
        let sources = self.state.image_capture.toplevel_sources.as_ref();
        let source = sources.expect("bind_image_capture first").create_source(
            &self.state.foreign_toplevels.listed[listed].proxy,
            &self.qh,
            (),
        );
        self.open_capture_session(source, paint_cursors)
    }

    fn open_capture_session(
        &mut self,
        source: ExtImageCaptureSourceV1,
        paint_cursors: bool,
    ) -> SessionId {
        let capture = &mut self.state.image_capture;
        let id = SessionId(capture.sessions.len());
        let options = if paint_cursors {
            Options::PaintCursors
        } else {
            Options::empty()
        };
        let manager = capture.manager.as_ref().expect("bind_image_capture first");
        let proxy = manager.create_session(&source, options, &self.qh, id);
        capture.sessions.push(Session {
            proxy,
            _source: source,
            pending: Constraints::default(),
            constraints: Vec::new(),
            stopped: false,
            frames: Vec::new(),
        });
        id
    }

    /// Destroys the session, as a client does once it has its screenshot or ends its
    /// screencast.
    pub fn stop_capture(&mut self, id: SessionId) {
        self.state.image_capture.sessions[id.0].proxy.destroy();
    }

    /// Asks for a frame copied into a new buffer of the size the session last asked for.
    pub fn request_capture_frame(&mut self, id: SessionId) {
        let constraints = self.state.capture_session(id).constraints.last();
        let size = constraints.expect("constraints; roundtrip first").size;
        self.request_capture_frame_of_size(id, size);
    }

    /// Asks for a frame copied into a new buffer of `size`, all of it to be written, whatever
    /// size the session asks for.
    pub fn request_capture_frame_of_size(&mut self, id: SessionId, size: (i32, i32)) {
        let (file, pool, buffer) = self.shm_buffer_file(size, 0);
        let session = &mut self.state.image_capture.sessions[id.0];
        let proxy = session
            .proxy
            .create_frame(&self.qh, (id, session.frames.len()));
        proxy.attach_buffer(&buffer);
        proxy.damage_buffer(0, 0, size.0, size.1);
        proxy.capture();
        session.frames.push(Frame {
            _proxy: proxy,
            _buffer: Buffer { buffer, pool },
            file,
            size,
            damage: Vec::new(),
            outcome: None,
        });
    }
}

impl State {
    /// One of the client's capture sessions.
    pub fn capture_session(&self, id: SessionId) -> &Session {
        &self.image_capture.sessions[id.0]
    }
}

impl Session {
    /// The frame the client last asked for.
    pub fn last_frame(&self) -> &Frame {
        self.frames.last().expect("request_capture_frame first")
    }
}

impl Dispatch<ExtImageCopyCaptureSessionV1, SessionId> for State {
    fn event(
        state: &mut Self,
        _: &ExtImageCopyCaptureSessionV1,
        event: ext_image_copy_capture_session_v1::Event,
        id: &SessionId,
        _: &Connection,
        _: &QueueHandle<Self>,
    ) {
        use ext_image_copy_capture_session_v1::Event;

        let session = &mut state.image_capture.sessions[id.0];
        match event {
            Event::BufferSize { width, height } => {
                session.pending.size = (width as i32, height as i32);
            }
            Event::ShmFormat {
                format: WEnum::Value(format),
            } => session.pending.shm_formats.push(format),
            Event::DmabufDevice { .. } => session.pending.dmabuf = true,
            Event::Done => {
                let constraints = std::mem::take(&mut session.pending);
                session.constraints.push(constraints);
            }
            Event::Stopped => session.stopped = true,
            _ => {}
        }
    }
}

impl Dispatch<ExtImageCopyCaptureFrameV1, (SessionId, usize)> for State {
    fn event(
        state: &mut Self,
        _: &ExtImageCopyCaptureFrameV1,
        event: ext_image_copy_capture_frame_v1::Event,
        (session, frame): &(SessionId, usize),
        _: &Connection,
        _: &QueueHandle<Self>,
    ) {
        use ext_image_copy_capture_frame_v1::Event;

        let frame = &mut state.image_capture.sessions[session.0].frames[*frame];
        match event {
            Event::Damage {
                x,
                y,
                width,
                height,
            } => frame.damage.push((x, y, width, height)),
            Event::Ready => frame.outcome = Some(Outcome::Ready),
            Event::Failed {
                reason: WEnum::Value(reason),
            } => frame.outcome = Some(Outcome::Failed(reason)),
            _ => {}
        }
    }
}

/// Proxies that send no events.
macro_rules! no_events {
    ($($proxy:ty => $event:ty),* $(,)?) => {
        $(
            impl Dispatch<$proxy, ()> for State {
                fn event(
                    _: &mut Self,
                    _: &$proxy,
                    _: $event,
                    _: &(),
                    _: &Connection,
                    _: &QueueHandle<Self>,
                ) {
                }
            }
        )*
    };
}

no_events!(
    ExtOutputImageCaptureSourceManagerV1 => ext_output_image_capture_source_manager_v1::Event,
    ExtForeignToplevelImageCaptureSourceManagerV1 =>
        ext_foreign_toplevel_image_capture_source_manager_v1::Event,
    ExtImageCaptureSourceV1 => ext_image_capture_source_v1::Event,
    ExtImageCopyCaptureManagerV1 => ext_image_copy_capture_manager_v1::Event,
);
