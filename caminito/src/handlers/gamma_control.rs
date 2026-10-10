//! wlr-gamma-control (141, ADR 0023): a gamma client's table goes to the backend at once,
//! with no fade, and Night Light steps back from that output alone until the client lets
//! go.

use smithay::output::Output;

use super::sees_privileged_globals;
use crate::protocols::gamma_control::{GammaControlHandler, GammaControlState};
use crate::state::{Caminito, State};

impl Caminito {
    /// Offers wlr-gamma-control, as a privileged global (06, 68). Only the tty does, whose
    /// outputs have gamma tables.
    pub(crate) fn offer_gamma_control(&mut self) {
        let dh = &self.display_handle;
        self.gamma_control_state
            .offer::<State, _>(dh, sees_privileged_globals);
    }
}

impl GammaControlHandler for State {
    fn gamma_control_state(&mut self) -> &mut GammaControlState {
        &mut self.caminito.gamma_control_state
    }

    fn gamma_size(&mut self, output: &Output) -> Option<u32> {
        // A disabled output's `wl_output` lingers in clients until they release it.
        if !self.caminito.outputs.contains(output) {
            return None;
        }
        self.backend.gamma_size(output)
    }

    fn set_gamma(&mut self, output: &Output, table: Vec<u16>) -> bool {
        if !self.backend.set_gamma(output, Some(table)) {
            return false;
        }
        self.caminito.night_light_step_back(output);
        true
    }

    fn gamma_released(&mut self, output: &Output) {
        self.backend.set_gamma(output, None);
        self.caminito.night_light_step_in(output);
    }
}

impl State {
    /// The refresh pass's part of wlr-gamma-control: the control of an output that was
    /// unplugged or disabled fails, and the backend forgets its table.
    pub(crate) fn refresh_gamma_control(&mut self) {
        let caminito = &mut self.caminito;
        for output in caminito.gamma_control_state.fail_gone(&caminito.outputs) {
            self.backend.set_gamma(&output, None);
        }
    }
}
