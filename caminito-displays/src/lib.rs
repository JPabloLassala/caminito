//! caminito-displays without its toolkit (84, ADR 0014): the page's state, and the client of
//! output management that reads what Caminito shows and applies a configuration, and the
//! Night Light tab's state (142).
//!
//! The GTK window over them is the binary's. Caminito's headless tests drive these two
//! against Caminito itself, which is why they're a library.

#![forbid(unsafe_code)]

pub mod night_light;
pub mod outputs;
pub mod page;
