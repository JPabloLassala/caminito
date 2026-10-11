# System Info's "Windowing System" row names Caminito

Epic: about-caminito
Type: implementation
Status: ready-for-agent
Spec: [About Caminito](../about-caminito/spec.md)

## Why

MATE's System Info (`mate-system-info`, Control Center > Hardware) shows "Wayland" under "Windowing System" in a Session. The developer wants it to say Caminito.

The value is hardcoded from the GDK backend in `get_windowing_system()` of `capplets/system-info/mate-system-info.c`: "X11", "Wayland" or "Unknown". Nothing outside the program can change it, so it takes a carried patch.

## What to build

A sixth carried patch in `packaging/arch/mate-control-center-caminito`, to `get_windowing_system()`:

- **Under Caminito the row reads `Caminito (Wayland)`,** or `Caminito (X11)` when System Info is itself an X11 client (`GDK_BACKEND=x11`).
- **The gate is `MATE_WM_NAME=Caminito` alone,** through the helper that patch `0001-wm-common-Know-Caminito-from-the-environment-off-X11.patch` added to `capplets/common/wm-common.c`. A nested run names Caminito too.
- **Anywhere else the function is stock.**

The name isn't translated. Whether the backend in brackets goes through gettext is the implementing agent's to settle: upstream leaves "X11" and "Wayland" untranslated.

## Acceptance criteria

- [ ] In a Session, System Info's "Windowing System" row reads `Caminito (Wayland)`.
- [ ] Started as `GDK_BACKEND=x11 mate-system-info` in a Session, it reads `Caminito (X11)`.
- [ ] Started with `MATE_WM_NAME` unset, it reads `Wayland` or `X11` as stock does.
- [ ] The patch is `0006-*.patch`, listed in the PKGBUILD's `source` with its checksum, and the `pkgrel` of `mate-control-center-caminito` is bumped.
- [ ] [checklist.md](../../docs/testing/checklist.md) has a by-eye entry for the row.

## For the implementing agent to settle

Recorded in the Done section:

- whether `mate-system-info` already links `capplets/common`, or the patch has to add it to that capplet's build;
- how the three readings were checked, since the value is only ever drawn in a label.
