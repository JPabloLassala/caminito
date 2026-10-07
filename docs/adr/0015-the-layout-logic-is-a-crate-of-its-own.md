# 15. The layout logic is a crate of its own, with no dependencies

Date: 2026-10-05
Status: Accepted

## Context

ADR 0014 makes every layout contiguous, and has two programs keep it so: Caminito, which heals a layout on hot-plug and when an output changes size, and caminito-displays, which docks an output as the user drags it. Both need the same rules, or the page shows one layout and Caminito applies another.

ADR 0010 has four crates, and a crate only where it enforces a dependency rule. Ticket 82 proposed `caminito-policy` for this logic, as it is already free of protocols. But `caminito-policy` depends on Smithay for its geometry types, pinned by git revision and moved weekly by the canary, and Smithay brings calloop, drm-fourcc and some seventy other crates even with no features. caminito-displays is a GTK4 program that would use none of it.

## Decision

- **A fifth crate, `caminito-layout`,** holds docking, snapping, healing and what a change of size does to the neighbours. It has no dependencies, and its rule is that it gets none: no Smithay, no protocol, no toolkit. It is `#![forbid(unsafe_code)]`.
- **An output there is a rectangle of logical pixels,** in a type of the crate's own, and a layout is a slice of them. The caller works out an output's logical size, with Caminito's rounding (51).
- `caminito` and caminito-displays depend on it. `caminito-policy` doesn't: policy is told where the outputs are, and doesn't place them.

## Consequences

- caminito-displays builds without Smithay.
- `caminito` converts between Smithay's rectangles and the crate's, in `output.rs` alone.
- ADR 0010's "four Rust crates" is five, and six with caminito-displays (84).
- Rejected:
  - a module of `caminito-policy` (Smithay in a settings program, and a git pin shared with a program that has no use for it);
  - a module of `caminito`, with caminito-displays depending on the compositor (the same, and far more);
  - a copy of the logic in caminito-displays (two sets of rules to keep alike).
