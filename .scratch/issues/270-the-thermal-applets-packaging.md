# The Thermal applet's packaging

Epic: thermal-applet
Type: implementation
Status: ready-for-agent
Blocked by: 267, 268, 269
Spec: [Thermal applet](../thermal-applet/spec.md)

## What to build

The packaging and docs in [the spec](../thermal-applet/spec.md). The applet isn't in the default panel: it's in Add to Panel only.

- **Arch:** `caminito-git` builds and installs the applet and `helpers/caminito-applet-common/`, and depends on `lm_sensors`.
- **Debian:** `packaging/debian/caminito` does the same, with `libsensors-dev` to build and `libsensors5` to run.
- **README:** the Thermal applet, that it only watches, and that SATA drives need the `drivetemp` module.
- **[checklist.md](../../docs/testing/checklist.md):**
  - the readings match `sensors`
  - `stress-ng --cpu 0` turns the panel orange and spins the fan up
  - a 24 px panel shows one line

## Acceptance criteria

- [ ] `makepkg` and the Debian build install the applet, and it's listed in Add to Panel as "Temperatures and Fans".
- [ ] A new session's default panel doesn't have it.
- [ ] The README and the checklist are updated.
