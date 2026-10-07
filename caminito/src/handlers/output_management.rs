//! wlr-output-management (58): a client's configuration is checked whole against the
//! outputs and the backend, then applied whole, or not at all. The heads themselves are
//! sent in the refresh pass.

use smithay::output::{Mode, Output};

use crate::output::OutputChange;
use crate::protocols::output_management::{
    HeadRequest, OutputManagementHandler, OutputManagementState,
};
use crate::state::State;

/// A step of a configuration the backend has taken, which is taken back if a later one
/// fails.
enum Step {
    Enabled(Output),
    /// The output, and the mode it showed before.
    ModeChanged(Output, Mode),
}

impl OutputManagementHandler for State {
    fn output_management_state(&mut self) -> &mut OutputManagementState {
        &mut self.caminito.output_management_state
    }

    fn test_output_configuration(&mut self, configuration: &[HeadRequest]) -> bool {
        self.plan_output_changes(configuration).is_some()
    }

    fn apply_output_configuration(&mut self, configuration: &[HeadRequest]) -> bool {
        let Some(changes) = self.plan_output_changes(configuration) else {
            return false;
        };
        // A monitor turned on again brings the saved layout back (90).
        self.apply_with_saved_layout(configuration, &changes) || self.apply_output_changes(&changes)
    }
}

impl State {
    /// What a configuration would change, if the outputs and the backend can show it.
    fn plan_output_changes(&self, configuration: &[HeadRequest]) -> Option<Vec<OutputChange>> {
        let changes = self.caminito.plan_output_changes(configuration)?;
        self.backend.can_change_outputs(&changes).then_some(changes)
    }

    /// Changes the outputs. The steps that can fail, lighting a monitor and switching its
    /// mode, come first, so that a failure leaves only those to take back.
    pub(crate) fn apply_output_changes(&mut self, changes: &[OutputChange]) -> bool {
        let mut taken = Vec::new();
        for change in changes {
            let Some(settings) = change.to else {
                continue;
            };
            let output = &change.output;
            let step = if !self.caminito.outputs.contains(output) {
                self.backend
                    .enable_output(&mut self.caminito, output, settings.mode)
                    .then(|| Step::Enabled(output.clone()))
            } else {
                match output.current_mode() {
                    Some(old) if old != settings.mode => self
                        .backend
                        .set_output_mode(&mut self.caminito, output, settings.mode)
                        .then(|| Step::ModeChanged(output.clone(), old)),
                    _ => continue,
                }
            };
            let Some(step) = step else {
                self.take_back(taken);
                return false;
            };
            taken.push(step);
        }
        for change in changes {
            if change.to.is_none() && self.caminito.outputs.contains(&change.output) {
                self.backend
                    .disable_output(&mut self.caminito, &change.output);
            }
        }
        self.caminito.finish_output_changes(changes);
        true
    }

    /// Takes back the steps of a configuration that failed, newest first.
    fn take_back(&mut self, steps: Vec<Step>) {
        for step in steps.into_iter().rev() {
            match step {
                Step::Enabled(output) => self.backend.disable_output(&mut self.caminito, &output),
                Step::ModeChanged(output, old) => {
                    if !self
                        .backend
                        .set_output_mode(&mut self.caminito, &output, old)
                    {
                        tracing::warn!("can't switch {} back to its mode", output.name());
                    }
                }
            }
        }
    }
}
