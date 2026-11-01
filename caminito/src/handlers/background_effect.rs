//! ext-background-effect-v1 (CMN-238): a client asks for Esmeril. Its region is double-buffered
//! state Smithay keeps, and the scene reads it, redrawn as any commit is.

use smithay::wayland::background_effect::{Capability, ExtBackgroundEffectHandler};

use crate::protocols::background_effect::{BackgroundEffectHandler, BackgroundEffectState};
use crate::state::{Caminito, State};

impl Caminito {
    /// What clients are told, which follows the settings alone: Pixman places Esmeril as GLES
    /// does (ADR 0048).
    pub(crate) fn blur_capabilities(esmeril: Option<u32>) -> Capability {
        match esmeril {
            Some(_) => Capability::Blur,
            None => Capability::empty(),
        }
    }
}

impl ExtBackgroundEffectHandler for State {
    fn capabilities(&self) -> Capability {
        Caminito::blur_capabilities(self.caminito.settings.esmeril)
    }
}

impl BackgroundEffectHandler for State {
    fn background_effect_state(&mut self) -> &mut BackgroundEffectState {
        &mut self.caminito.background_effect_state
    }
}
