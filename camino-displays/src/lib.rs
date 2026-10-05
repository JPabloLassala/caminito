//! camino-displays without its toolkit (84, ADR 0014): the page's state, and the client of
//! output management that reads what Camino shows and applies a configuration.
//!
//! The GTK window over them is the binary's. Camino's headless tests drive these two
//! against Camino itself, which is why they're a library.

#![forbid(unsafe_code)]

pub mod outputs;
pub mod page;
