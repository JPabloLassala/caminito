//! The Dock applet's keys (CMN-245, ADR 0050): Caminito tells one Dock applet, through
//! `caminito_dock_activation_v1`, which Dock item to activate. With a Dock panel on more than
//! one output, the applet told is the one on the pointer's output, else the one on the
//! focused window's, else the oldest that has a panel on an output, else the oldest.

use caminito_policy::OutputId;

use crate::output::output_id;
use crate::state::{Caminito, SEAT};

impl Caminito {
    /// Whether Caminito can tell a Dock applet which item to activate. The keys that
    /// activate a Dock item are bound only while it can.
    #[cfg_attr(
        not(test),
        expect(dead_code, reason = "the binding (CMN-246) will call it")
    )]
    pub(crate) fn has_dock_applet(&self) -> bool {
        self.dock_activation_state.applets().next().is_some()
    }

    /// Tells one Dock applet to activate its Dock item at `index`, counted from 0, and
    /// returns whether one was told.
    #[cfg_attr(
        not(test),
        expect(dead_code, reason = "the binding (CMN-246) will call it")
    )]
    pub(crate) fn activate_dock_item(&mut self, index: u32) -> bool {
        let applets: Vec<_> = self
            .dock_activation_state
            .applets()
            .map(|applet| {
                let output = applet
                    .surface()
                    .and_then(|surface| self.layer_for_surface(&surface))
                    .map(|(output, _)| output_id(&output));
                (applet, output)
            })
            .collect();
        let focused_output = self
            .policy
            .focus(SEAT)
            .and_then(|id| self.policy.window(id))
            .and_then(|mapped| mapped.main_output())
            .map(output_id);
        let applet_on = |output: Option<OutputId>| {
            output.and_then(|output| applets.iter().find(|(_, on)| *on == Some(output)))
        };
        let chosen = applet_on(self.pointer_output())
            .or_else(|| applet_on(focused_output))
            .or_else(|| applets.iter().find(|(_, on)| on.is_some()))
            .or_else(|| applets.first());
        let Some((applet, _)) = chosen else {
            return false;
        };
        applet.activate(index);
        true
    }
}
