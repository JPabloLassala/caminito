# Spec: About Caminito

Written on 2026-10-11 from a grilling session with the developer. It holds the decisions; the two tickets say what to build.

Nothing here is built.

## Problem

Nothing in a Session says it is running Caminito. MATE's System Info shows "Wayland" under "Windowing System", which is as true of the Wayfire session, and there is no about box and no version to quote in a bug report: the workspace version is `0.1.0` in every build, and the only revision is in the package's name.

## Solution

- System Info's "Windowing System" row names Caminito, through a carried patch.
- A small program, `caminito-about`, shows what Caminito is and which build it is, with an "About Caminito" entry in the Control Center.
- One version string, the Cargo version with the build's revision, is shown by the dialog and printed by `caminito --version`.

## Decisions

### The System Info row

- The row is `mate-system-info`'s, in `mate-control-center`. It is hardcoded from the GDK backend ("X11", "Wayland" or "Unknown") in `get_windowing_system()` of `capplets/system-info/mate-system-info.c`, with no hook, so it takes a carried patch in `packaging/arch/mate-control-center-caminito`.
- Under Caminito it reads `Caminito (Wayland)`. The name comes first because that's what was asked for, and the protocol stays because the row is the protocol's under X11 MATE ("X11", never "Marco") and a bug report needs it.
- When System Info itself is an Xwayland client it reads `Caminito (X11)`.
- The gate is `MATE_WM_NAME=Caminito` alone, through the helper patch 0001 added to `capplets/common/wm-common.c`. So a nested run names Caminito too. Anywhere else the row is stock.

### The dialog

- A workspace crate of its own, `caminito-about`, in GTK4 like caminito-displays. Not a second binary of caminito-displays, which is the displays program, and not a flag of the compositor, which doesn't link GTK.
- The layout is variant C of the prototype (below): one window with no second page. The icon is on the left. Beside it are the name and the version on one line, the description under them, and one small dim line with "Built on Marco and Smithay", the licence and the repository link.
- The text:
  - name `Caminito`, icon `caminito`;
  - description `A Wayland compositor for the MATE desktop`;
  - `Marco` and `Smithay` link to their repositories;
  - `GPL-2.0-or-later` links to the GNU page of the licence's text;
  - the repository link is `https://github.com/JPabloLassala/caminito`.
- It has no live facts from the running compositor (renderer, Xwayland's version). Those are System Info's kind of thing and would need a new interface on `org.mate.Caminito`.
- Escape closes it. It has no Close button, since the titlebar has one. Starting it a second time raises the window that is open.
- It is in English only, as caminito-displays is.

### The Control Center entry

- `Name=About Caminito`, `Icon=caminito`, as "About Me" and "About MATE" are named.
- `Categories=GTK;Settings;HardwareSettings;` puts it in Hardware, beside System Info and Displays.
- It installs to `share/caminito/xdg/applications`, the directory only a Session has on its data path, beside caminito-displays's entry. Under X11 MATE it would describe a compositor that isn't running.

### The version

- The string is the Cargo version and then the revision in brackets: `0.1.0 (r672.989d558)`. The revision has the PKGBUILD's `pkgver` form, so the dialog and `pacman -Q caminito-git` agree.
- A build step takes the revision from an environment variable the PKGBUILD sets from `pkgver`, then from `git` in the checkout, and leaves it out when there is neither: the string is then `0.1.0`.
- It is the version of the build that is installed, not one asked of the running compositor. The two differ only between an upgrade and the next login.
- `caminito --version` prints the same string. The build step is in one place that both programs use, so they can't disagree.

### Not written down elsewhere

No ADR: none of this is hard to reverse. No glossary entry: caminito-displays has one because it replaces a MATE capplet, and an about box replaces nothing.

## Prototype

Three variants of the dialog were tried on 2026-10-11: A, the stock `GtkAboutDialog`; B, rows in System Info's manner; C, a compact card. The developer chose C. The prototype is `tools/prototype-about-caminito.py` on the branch `prototype/about-caminito`, and isn't on master. It is PyGObject and throwaway: `caminito-about` is written afresh in Rust.

## Tickets

- [181](../issues/181-system-info-names-caminito.md): System Info's row names Caminito.
- [182](../issues/182-about-caminito.md): `caminito-about` and the version string.

Neither blocks the other.
