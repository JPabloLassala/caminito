# Contributing to Caminito

Thank you for your help with Caminito. This file tells you how to send a contribution and where
to start.

## Bugs and ideas

Report bugs and give ideas in [GitHub issues](https://github.com/JPabloLassala/caminito/issues).
For a bug, give the version (`caminito --version`), the distribution, and the steps that cause the
bug.

## Licence: inbound = outbound

Caminito is GPL-3.0-or-later ([ADR 0051](docs/adr/0051-caminito-is-gpl-3-0-or-later.md)). When
you send a contribution, you give it under the same licence: GPL-3.0-or-later. There is no
contributor licence agreement (CLA).

## Sign off each commit

Each commit must have a `Signed-off-by:` line with your name and email. With this line, you agree
to the [Developer Certificate of Origin](https://developercertificate.org) (DCO): you have the
right to give the change under the project's licence.

To add the line, use `git commit -s`. To add it to the commits of a branch that you made before,
use `git rebase --signoff master`.

## Where to start

- [README.md](README.md): what Caminito is, how to build it, how to run it and how to test it.
- [CONTEXT.md](CONTEXT.md): the project's vocabulary. Use its words in the code, the commits and
  the issues.
- [CODING_STANDARDS.md](CODING_STANDARDS.md): how the code is written. `tools/check` runs the
  checks that a change must pass.
- [docs/adr/](docs/adr/): the architecture decision records. Read the ADRs about the part that you
  change before you change it.

## Reference code

Caminito follows the behaviour of other projects, and some of its code comes from them. The rules
are in [CODING_STANDARDS.md](CODING_STANDARDS.md) and in
[ADR 0051](docs/adr/0051-caminito-is-gpl-3-0-or-later.md). In summary:

- Copy or adapt code only from a project whose licence allows GPL-3.0-or-later, for example niri,
  anvil, Marco and mate-screensaver. Keep the notice of the project, and add the SPDX lines that
  CODING_STANDARDS.md tells.
- From cosmic-comp, take only the behaviour, never the code. cosmic-comp is GPL-3.0-only, and its
  code would make the `caminito` binary GPL-3.0-only.
