//! The protocols that shape how surfaces are drawn: viewporter, fractional-scale,
//! presentation-time, single-pixel-buffer, content-type, alpha-modifier, fifo and
//! commit-timing. Only fractional-scale needs a handler; the scale itself is sent in the
//! refresh pass.

use smithay::wayland::fractional_scale::FractionalScaleHandler;

use crate::state::State;

impl FractionalScaleHandler for State {}
