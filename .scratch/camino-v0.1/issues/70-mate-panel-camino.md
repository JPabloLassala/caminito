# mate-panel-camino

Type: implementation
Status: resolved
Milestone: M5
Blocked by: 69
Map: [Camino v0.1](../map.md)

## What to build

mate-panel as a Patched package ([16](16-packaging.md)), on a pinned master commit with libxfce4windowing on, so its window list and workspace switcher work on Wayland ([06](06-v01-protocol-and-integration-set.md)), with the two Carried patches the session needs first ([17](17-milestone-slicing.md)).

- **`packaging/arch/mate-panel-camino/PKGBUILD`,** from Arch's mate-panel PKGBUILD with the smallest diff:
  - the source at a pinned master commit, starting from `58ac6c1`, which 06 and 08 read, or a newer one if it builds;
  - `pkgver` as `1.28.7.r<N>.g<hash>` (16);
  - `provides` and `conflicts` on `mate-panel`;
  - `libxfce4windowing` added to `depends`, and the configure option that turns it on (`configure.ac:219`), with the Wayland and layer-shell options Arch's build may leave off.
- **The patches,** both ungated because they're correct on any compositor (ADR 0011):
  - `0001` **the minimise rectangle:** the window list calls `xfw_window_set_button_geometry` for each button when it's allocated, which reaches Camino as wlr-foreign-toplevel `set_rectangle` ([65](65-foreign-toplevel.md));
  - `0002` **Shut Down:** the action button takes the session-manager path whenever `org.gnome.SessionManager` has an owner, instead of running `systemctl` because GDK is on Wayland (`panel-action-button.c:250-350`, `:423-436`, 08). About 10 to 15 lines.
- **CI:** a line each in the Packaging job and the weekly patch-on-head job ([69](69-mate-session-manager-camino.md)).

## Acceptance criteria

- [ ] `makepkg -si` installs it in place of `mate-panel`; `namcap` reports nothing unexplained; both CI jobs are green.
- [ ] Nested, with [63](63-layer-shell.md), [65](65-foreign-toplevel.md) and [66](66-ext-workspace.md) in: the panel is a layer surface on its edge; the window list shows a button per window with its icon, and a click focuses or minimises it; the workspace switcher shows the workspaces and switches on click.
- [x] With `WAYLAND_DEBUG=1`, the panel sends `set_rectangle` for a window's button.
- [ ] In a nested run with mate-session, *Shut Down* from the panel's menu shows mate-session's confirm dialog. (Cancel it.)
- [ ] Under X11 MATE, the patched panel behaves as stock: window list, pager, Log Out and Shut Down.

## Notes

- **The rest of mate-panel's series** (`org.mate.Panel`, Show Desktop, the pager's coordinates, layout keys, workspace count and names) is M6 (17).
- **The nested criteria need the protocol tickets,** which this ticket isn't blocked by, since the package and its patches are written against the panel's own source. They're ticked when those are in; [72](72-session-file-and-first-login.md) runs them again in a real session.
- **Risk (17):** master must stay buildable at the pinned commit against Arch's current libxfce4windowing. If it doesn't, pin the nearest commit that does and record why.
- **Resolved on 2026-10-04 with four criteria unticked:** the package, both patches, the two CI lines and the README are on `master`. What is left needs things this ticket doesn't have:
  - **The nested criteria** need [63](63-layer-shell.md), [65](65-foreign-toplevel.md), [66](66-ext-workspace.md) and [61](61-session-start-up-and-exit.md)'s `camino --session`, which aren't in. [72](72-session-file-and-first-login.md) runs them in a real session.
  - **The CI criterion, and namcap on the built package,** need a push, as in [69](69-mate-session-manager-camino.md).
  - **The X11 criterion** needs a login. What was checked without one is below.
- **Baseline:** `58ac6c1` (`58ac6c101bdf2e2975c0395723ad16e9c9764f5c`) is still master's head. It builds with autotools against Arch's libxfce4windowing 4.20.7.
- **`pkgver` isn't `git describe`'s:** master comes from `v1.28.1`, and the 1.28 branch's tags aren't its ancestors, so `git describe` gives `v1.28.1-111-g58ac6c1`, which is below Arch's 1.28.7. `pkgver()` puts the commit count since `v1.28.1` and the hash after `_basever=1.28.7`, which gives `1.28.7.r111.g58ac6c1`. `_basever` moves by hand when Arch's version does.
- **Three configure options more than Arch's:**
  - `--enable-wayland`, so a missing gtk-layer-shell or libxfce4windowing stops the build. Without it, configure turns Wayland off and says nothing.
  - `--with-in-process-applets=all`. Arch builds the applets out of process, and on Wayland those don't start ("Cannot open display"), so the panel had no window list at all. In process, they work on X11 too.
  - `--disable-eds`. Master's clock links evolution-data-server when it's installed, and that isn't a dependency.
- **The applet library's soname changed, and that breaks other packages' applets.** Master has `libmate-panel-applet-4.so.2` (upstream's `fe3ae97d`, for a new `activate` entry in the class). Arch's `mate-applets`, `mate-sensors-applet` and `mate-notification-daemon`'s applet are linked to `.so.1`, which leaves with the stock package, so they don't start under the patched panel, on X11 too. In the virtual machine that is `/usr/lib/mate-notification-daemon/mate-notification-applet` only. The README says so. What to do about it (build those packages again, or keep the old library beside the new one) isn't decided: it's for [73](73-packaging-and-closing-m5.md).
- **`0001`, the minimise rectangle,** sends each window's rectangle from an idle, not in the button's `size-allocate`: the buttons are first allocated before the panel has a `wl_surface`, and `set_rectangle` with no surface is a protocol error. The idle also sends one request for each window when several buttons move. A window in a group gets the group's button.
- **`0002`, Shut Down,** adds `panel_session_manager_is_present` (an owner of `org.gnome.SessionManager`) and uses it at both places that looked at GDK's backend. It's 27 lines, not 10 to 15.
- **`sha256sums`,** as in Arch's mate-panel PKGBUILD. The `git config submodule.mate-submodules.url` line is Arch's too. It names no submodule that mate-panel has (the name is `mate-panel/mate-submodules`), so the submodule comes from GitHub; it's left as Arch has it.
- **Checked in the virtual machine (aarch64, `makepkg -A`):**
  - The package builds and both patches apply with no offset. `pacman -U` replaces `mate-panel` 1.28.7-2, and `pacman -Q mate-panel` answers `mate-panel-camino 1.28.7.r111.g58ac6c1-1`. It stays installed there, with `libxfce4windowing` and `labwc`.
  - `namcap PKGBUILD` prints nothing. On the built package, the lines are namcap on Arch Linux ARM calling every linked library uninstalled, as in 69.
  - **On Wayland, under a headless labwc 0.20.2** ([70-panel-checks/check.sh](70-panel-checks/check.sh), with settings in a key file and a private bus): the panel makes two layer surfaces, binds ext-workspace, and with `WAYLAND_DEBUG=1` sends `zwlr_foreign_toplevel_handle_v1.set_rectangle(wl_surface, 34, 0, 180, 24)` for a window that was open before the panel, and one more for each window that opens. That is what the `set_rectangle` criterion is ticked on; it wasn't Camino.
  - **Shut Down on Wayland,** with a click on a Shut Down button from `wlrctl`: with a stand-in owner of `org.gnome.SessionManager` ([fake-sm.py](70-panel-checks/fake-sm.py)), the panel calls `CanShutdown` and then `Shutdown`, and opens no window. With no owner, it opens its own "System Shutdown" dialog, as stock.
  - **On X11, under Xvfb with Marco** ([x11.sh](70-panel-checks/x11.sh)): the panel starts with its applets in process, the window list sets `_NET_WM_ICON_GEOMETRY` on a window, and the panel calls `CanShutdown`. The pager, Log Out and Shut Down weren't clicked.
  - The weekly job's sed, run by hand on the PKGBUILD, takes `#commit=` out of the source.
