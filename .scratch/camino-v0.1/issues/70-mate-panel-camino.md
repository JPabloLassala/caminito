# mate-panel-camino

Type: implementation
Status: open
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
- [ ] With `WAYLAND_DEBUG=1`, the panel sends `set_rectangle` for a window's button.
- [ ] In a nested run with mate-session, *Shut Down* from the panel's menu shows mate-session's confirm dialog. (Cancel it.)
- [ ] Under X11 MATE, the patched panel behaves as stock: window list, pager, Log Out and Shut Down.

## Notes

- **The rest of mate-panel's series** (`org.mate.Panel`, Show Desktop, the pager's coordinates, layout keys, workspace count and names) is M6 (17).
- **The nested criteria need the protocol tickets,** which this ticket isn't blocked by, since the package and its patches are written against the panel's own source. They're ticked when those are in; [72](72-session-file-and-first-login.md) runs them again in a real session.
- **Risk (17):** master must stay buildable at the pinned commit against Arch's current libxfce4windowing. If it doesn't, pin the nearest commit that does and record why.
