# System Info's "Windowing System" row names Caminito

Epic: about-caminito
Type: implementation
Status: resolved
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

- [x] In a Session, System Info's "Windowing System" row reads `Caminito (Wayland)`.
- [x] Started as `GDK_BACKEND=x11 mate-system-info` in a Session, it reads `Caminito (X11)`.
- [x] Started with `MATE_WM_NAME` unset, it reads `Wayland` or `X11` as stock does.
- [x] The patch is `0006-*.patch`, listed in the PKGBUILD's `source` with its checksum, and the `pkgrel` of `mate-control-center-caminito` is bumped.
- [x] [checklist.md](../../docs/testing/checklist.md) has a by-eye entry for the row.

## For the implementing agent to settle

Recorded in the Done section:

- whether `mate-system-info` already links `capplets/common`, or the patch has to add it to that capplet's build;
- how the three readings were checked, since the value is only ever drawn in a label.

## Done (2026-10-11)

- **`mate-system-info` already links `capplets/common`.** `configure.ac` puts `-I capplets/common` in `MATECC_CAPPLETS_CFLAGS` and `libcommon.la` in `MATECC_CAPPLETS_LIBS`, which the capplet's `Makefile.am` uses. And 102's `0004` had made `wm_common_is_caminito` public in `wm-common.h`. So `0006` only includes the header and calls it, with no build change.
- **One helper, `windowing_system_name`, wraps the X11 and Wayland branches.** It returns `Caminito (<backend>)` when `wm_common_is_caminito ()`, and the bare backend otherwise. The name is `WM_COMMON_CAMINITO`, the string `MATE_WM_NAME` is compared with.
- **Nothing in the row is translated.** Upstream leaves "X11" and "Wayland" untranslated, and the name isn't translated, so the bracketed backend isn't either. `Unknown` stays stock and translated, under Caminito too: no backend Caminito runs System Info on gives it.
- **How the readings were checked: [windowing-row.sh](181-checks/windowing-row.sh).** It loads [row-probe.c](181-checks/row-probe.c), a GTK module that prints the label after "Windowing System" and quits, into a `mate-system-info` run under headless Weston and Xvfb, on a private bus with GSettings in memory. It ran against v1.29.1 with `0001` to `0006` applied with `patch`, as `prepare()` does, on Ubuntu 24.04 in a cloud container. Only `capplets/common` and `capplets/system-info` were built, with `configure.ac`'s Marco and mate-desktop floors lowered for the build alone, since Ubuntu has 1.26. It gave `Caminito (Wayland)`, `Caminito (X11)` with `GDK_BACKEND=x11`, and `Wayland`, `X11` and `Wayland` with `MATE_WM_NAME` unset, unset with `GDK_BACKEND=x11`, and `Metacity`. Tested first: the same series without `0006` failed both Caminito readings, with `Wayland` and `X11`.
- **Not run:**
  - `makepkg` on the Arch machine. The patch's `b2sum` was checked against the PKGBUILD, and the six patches apply in order to the tag.
  - The checklist item, which needs a Session.
  - `tools/check`, which needs the Arch packages `--deps` installs. No Rust, meson or extension code changed. The extension's tests passed, 139 of 139, on two runs. One earlier run had one failure, which wasn't captured and didn't come back.
