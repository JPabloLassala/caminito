# 51. Caminito is GPL-3.0-or-later

Date: 2026-10-27
Status: Accepted

## Context

Caminito was declared GPL-2.0-or-later, with a GPLv2 `COPYING`, before it was made public. A licence audit found that the declaration wasn't what the shipped work can be distributed under:

- The `caminito` binary links Apache-2.0-only crates (`winit*` through Smithay, `gethostname` through x11rb). Apache-2.0 is compatible with GPLv3 and not with GPLv2, so the binary is distributable only under GPLv3. `deny.toml` already said so; the README, the About dialog and the packages didn't.
- `caminito-locker` links `gtk-session-lock`, which is GPL-3.0-or-later.
- `data/remap-templates/macos-pc.remaps` is converted from Toshy's config, which is GPL-3.0 (ADR 0038).
- `caminito/src/backend/tty/gamma.rs` follows niri's `set_gamma` step by step (niri is GPL-3.0-or-later).

A comparison with niri and cosmic-comp found no copied code beyond `gamma.rs`: the other files that cite them share their behaviour, not their text. The commits' only human author is the developer, so the developer can relicense alone. The code ported from mate-screensaver and Marco is GPL-2.0-or-later, which allows GPLv3.

A permissive licence (MIT, Apache-2.0) was considered, since much of the code is AI-written. It isn't possible for the parts that matter: the locker, the carried patches and the compositor are derived from GPL code. AI authorship argues for no licence over another: where the output has no copyright, no licence applies to it, and where it has one (the developer's selection and changes), the licence is the developer's choice.

## Decision

- **All of Caminito's own code is GPL-3.0-or-later,** copyright "2025-2026 Juan Pablo Lassala", one licence for the whole repo, including `caminito-vscode` and the logo.
- **Licence data follows REUSE:** each licence's text in `LICENSES/`, each path's licence and copyright in `REUSE.toml`, and an SPDX header only on a file from a third party (anvil, Marco, mate-screensaver, Toshy, the test themes, `gamma.rs`). `reuse lint` runs in the checks.
- **Reference code is copied or adapted only from a project whose licence allows GPL-3.0-or-later** (niri, anvil, Marco, mate-screensaver), with its notice kept. From cosmic-comp, which is GPL-3.0-only, only behaviour is taken, never code, since its code would make the `caminito` binary GPL-3.0-only.
- **Contributions are inbound = outbound:** a contribution comes under GPL-3.0-or-later, with a DCO `Signed-off-by:`, and no CLA.
- **No trademark policy** for the name or the logo, until a fork makes one necessary.

## Consequences

- One licence for every binary, so the About dialog and the packages say one thing, and the Apache crates need no exception.
- The Toshy template stays marked GPL-3.0-only, as Toshy's licence doesn't say "or later"; it's data installed beside the binaries, not linked.
- Rejected:
  - keeping GPL-2.0-or-later and documenting that two binaries are GPLv3 (two licences to explain, and `reuse lint` can't check it);
  - GPL for the derived parts and MIT for the rest (the parts that would be MIT are the least useful to take alone);
  - a CLA (only needed to relicense, and "or later" already allows a later GPL).
