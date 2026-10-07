//! The protocols that shape how surfaces are drawn: viewporter, fractional-scale,
//! presentation-time, single-pixel-buffer, content-type, alpha-modifier, fifo and
//! commit-timing, and on the tty linux-dmabuf and drm-syncobj (57). The fractional scale
//! itself is sent in the refresh pass, and a commit's wait for its buffer is in
//! `compositor`.

use smithay::backend::allocator::dmabuf::Dmabuf;
use smithay::wayland::dmabuf::{DmabufGlobal, DmabufHandler, DmabufState, ImportNotifier};
use smithay::wayland::drm_syncobj::{DrmSyncobjHandler, DrmSyncobjState};
use smithay::wayland::fractional_scale::FractionalScaleHandler;

use crate::state::State;

impl FractionalScaleHandler for State {}

impl DmabufHandler for State {
    fn dmabuf_state(&mut self) -> &mut DmabufState {
        self.backend.dmabuf_state()
    }

    fn dmabuf_imported(
        &mut self,
        _global: &DmabufGlobal,
        dmabuf: Dmabuf,
        notifier: ImportNotifier,
    ) {
        if self.backend.import_dmabuf(&dmabuf) {
            // It fails only if the client is gone.
            let _ = notifier.successful::<State>();
        } else {
            notifier.failed();
        }
    }
}

impl DrmSyncobjHandler for State {
    fn drm_syncobj_state(&mut self) -> Option<&mut DrmSyncobjState> {
        self.backend.syncobj_state()
    }
}
