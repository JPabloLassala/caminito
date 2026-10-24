//! ext-image-capture-source and ext-image-copy-capture (103), from Smithay's states: a
//! screenshot tool makes a source of an output or of a window, opens a session on it, and
//! has frames copied into its buffers. A handler only notes what was asked. The refresh
//! pass stops the sessions whose sources are gone, tells the others of a new size, and
//! copies the frames that are due, after the outputs are drawn.

use smithay::backend::renderer::damage::OutputDamageTracker;
use smithay::output::Output;
use smithay::utils::{Physical, Size, Transform};
use smithay::wayland::foreign_toplevel_list::ForeignToplevelHandle;
use smithay::wayland::image_capture_source::{
    ImageCaptureSource, ImageCaptureSourceHandler, OutputCaptureSourceHandler,
    OutputCaptureSourceState, ToplevelCaptureSourceHandler, ToplevelCaptureSourceState,
};
use smithay::wayland::image_copy_capture::{
    BufferConstraints, CaptureFailureReason, Frame, FrameRef, ImageCopyCaptureHandler,
    ImageCopyCaptureState, Session, SessionRef,
};

use crate::capture::{self, CaptureSource, SHM_FORMATS, Shot};
use crate::state::{Caminito, State};

/// A capture session, and what Caminito keeps for it.
pub(crate) struct CaptureSession {
    /// Smithay's session, which is stopped as this is dropped. It's the first field, so
    /// that a frame that waits fails as stopped.
    session: Session,
    source: CaptureSource,
    /// What the session's frames so far showed, for a capture of the size and scale beside
    /// it. There's none before the first frame, and a new one once either changes.
    tracker: Option<(Size<i32, Physical>, f64, OutputDamageTracker)>,
    /// The frame the client asked for, until it's copied: a session's first at the next
    /// refresh, and a later one once the source has changed.
    waiting: Option<Frame>,
}

/// What a session asks of its buffers for a capture of `size`: shm ones only.
fn constraints(size: Size<i32, Physical>) -> BufferConstraints {
    BufferConstraints {
        size: (size.w, size.h).into(),
        shm: SHM_FORMATS.to_vec(),
        dma: None,
    }
}

impl ImageCaptureSourceHandler for State {}

impl OutputCaptureSourceHandler for State {
    fn output_capture_source_state(&mut self) -> &mut OutputCaptureSourceState {
        &mut self.caminito.output_capture_source_state
    }

    fn output_source_created(&mut self, source: ImageCaptureSource, output: &Output) {
        let data = source.user_data();
        data.insert_if_missing(|| CaptureSource::Output(output.downgrade()));
    }
}

impl ToplevelCaptureSourceHandler for State {
    fn toplevel_capture_source_state(&mut self) -> &mut ToplevelCaptureSourceState {
        &mut self.caminito.toplevel_capture_source_state
    }

    /// A handle of a window that's gone makes a source of nothing, whose sessions are
    /// stopped as they're made.
    fn toplevel_source_created(
        &mut self,
        source: ImageCaptureSource,
        toplevel: ForeignToplevelHandle,
    ) {
        let caminito = &self.caminito;
        if let Some(id) = caminito.window_for_toplevel_identifier(&toplevel.identifier()) {
            let data = source.user_data();
            data.insert_if_missing(|| CaptureSource::Toplevel(id));
        }
    }
}

impl ImageCopyCaptureHandler for State {
    fn image_copy_capture_state(&mut self) -> &mut ImageCopyCaptureState {
        &mut self.caminito.image_copy_capture_state
    }

    fn capture_constraints(&mut self, source: &ImageCaptureSource) -> Option<BufferConstraints> {
        let source = source.user_data().get::<CaptureSource>()?;
        let (size, _) = self.caminito.capture_extent(source)?;
        Some(constraints(size))
    }

    fn new_session(&mut self, session: Session) {
        let source = session.source();
        // Smithay made the session only since `capture_constraints` found the source.
        let Some(source) = source.user_data().get::<CaptureSource>().cloned() else {
            return;
        };
        self.caminito.captures.push(CaptureSession {
            session,
            source,
            tracker: None,
            waiting: None,
        });
    }

    /// A frame asked for while the session's last one still waits takes its place, and
    /// that one fails.
    fn frame(&mut self, session: &SessionRef, frame: Frame) {
        let captures = &mut self.caminito.captures;
        if let Some(capture) = captures.iter_mut().find(|c| c.session == *session) {
            capture.waiting = Some(frame);
        }
    }

    fn frame_aborted(&mut self, frame: FrameRef) {
        for capture in &mut self.caminito.captures {
            if capture
                .waiting
                .as_ref()
                .is_some_and(|waiting| *waiting == frame)
            {
                capture.waiting = None;
            }
        }
    }

    fn session_destroyed(&mut self, session: SessionRef) {
        self.caminito.captures.retain(|c| c.session != session);
    }
}

impl State {
    /// Brings the capture sessions up to date with their sources: a session whose source is
    /// gone is stopped, one whose source changed size is sent new constraints, and a frame
    /// that waits is copied if it's a session's first or its source changed. It runs after
    /// the outputs are drawn, so a capture shows what its output does.
    pub(crate) fn refresh_captures(&mut self) {
        self.caminito.image_copy_capture_state.cleanup();
        let mut captures = std::mem::take(&mut self.caminito.captures);
        captures.retain_mut(|capture| self.refresh_capture(capture));
        self.caminito.captures = captures;
    }

    /// Brings one session up to date, and returns whether it goes on.
    fn refresh_capture(&mut self, capture: &mut CaptureSession) -> bool {
        let caminito = &self.caminito;
        let Some((size, scale)) = caminito.capture_extent(&capture.source) else {
            return false;
        };
        let session = &capture.session;
        let asked = session.current_constraints().map(|c| c.size);
        if asked != Some((size.w, size.h).into()) {
            session.update_constraints(constraints(size));
        }
        let Some(frame) = capture.waiting.take() else {
            return true;
        };

        let tracked = capture
            .tracker
            .take()
            .filter(|(tracked_size, tracked_scale, _)| {
                (*tracked_size, *tracked_scale) == (size, scale)
            });
        let (_, _, tracker) = capture.tracker.insert(tracked.unwrap_or_else(|| {
            let tracker = OutputDamageTracker::new(size, scale, Transform::Normal);
            (size, scale, tracker)
        }));
        let shot = Shot {
            source: &capture.source,
            paint_cursors: session.draw_cursor(),
            size,
            scale,
            tracker,
        };
        match self.backend.copy_capture(caminito, shot, &frame.buffer()) {
            Ok(Some(damage)) => frame.success(Transform::Normal, damage, caminito.clock.now()),
            Ok(None) => capture.waiting = Some(frame),
            Err(capture::Error::Buffer) => frame.fail(CaptureFailureReason::BufferConstraints),
            Err(err) => {
                // Once for each frame a client asks for, so not a warning.
                tracing::debug!("can't capture a frame: {err}");
                frame.fail(CaptureFailureReason::Unknown);
                // The damage it counted reached no buffer, so the next frame is whole.
                capture.tracker = None;
            }
        }
        true
    }
}

impl Caminito {
    /// How many capture sessions are open on the window, for a test that a client's stop
    /// reached Caminito.
    #[cfg(test)]
    pub(crate) fn window_capture_sessions(&self, id: caminito_policy::WindowId) -> usize {
        let source = CaptureSource::Toplevel(id);
        self.captures.iter().filter(|c| c.source == source).count()
    }
}
