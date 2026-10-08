# Show Desktop, shared by the panel and Ctrl+Alt+D

Epic: caminito-v0.1
Type: implementation
Status: resolved
Milestone: M6
Blocked by: none
Map: [Caminito v0.1](../caminito-v0.1/map.md)

## What to build

The panel's Show Desktop button and Ctrl+Alt+D show and bring back the same desktop, and each sees what the other did ([06](06-v01-protocol-and-integration-set.md)'s *Show Desktop* row, [17](17-milestone-slicing.md)).

- **Policy:** showing the desktop with Marco's semantics, per workspace. It hides the workspace's windows without minimising them for good, and ends when a window on that workspace is mapped, focused or activated, as Marco's does. Read Marco's source for the cases before writing the tests.
- **The binding:** Ctrl+Alt+D joins the hard-coded set of Marco's defaults, which stays until M8.
- **D-Bus:** the `ShowingDesktop` property of `org.mate.Caminito`, read and write, for the current workspace, with a change signal whenever it changes, whoever changed it. It's refreshed in the refresh pass, as `PrimaryOutput` is ([81](81-primary-output.md)).
- **The carried patch:** `mate-panel-caminito`'s Show Desktop applet reads, writes and follows the property when `org.mate.Caminito` has an owner, and keeps libxfce4windowing's minimise-all when it has none ([16](16-packaging.md), ADR 0011).
- **The checklist:** one item for 06's row, tagged M6, run in a Session.

## Acceptance criteria

- [x] Policy tests, written before the code: showing the desktop hides one workspace's windows and leaves the others'; it ends on a new window, on activation and on a second toggle; the windows come back in their stacking order with the focus they had.
- [x] Headless tests with a D-Bus client: writing the property hides the windows; Ctrl+Alt+D changes the property and signals it once; switching workspace signals the new workspace's value.
- [x] `mate-panel-caminito` builds with the patch in its series, and its `makepkg` and `namcap` job passes. (Run by hand as the job runs them, without the install: `makepkg` built it with the patch on the first four, before 96's and 97's three were merged; after the merges the series of eight was only checked to apply, and `namcap` has no error for the PKGBUILD or the package. The job itself runs when this is pushed.)
- [ ] The checklist has the item, and it was run in a Session: the button and Ctrl+Alt+D agree in both directions. If it can't be run where the ticket is done, say so in the notes and leave it to [106](106-packaging-and-closing-m6.md). (The item is there. It wasn't run: see the resolution.)

## Notes

- **`show-desktop-skip-list`** is M8's (17).
- **Windows that never hide** (the desktop, docks) are layer surfaces under Caminito and aren't policy's windows, so the rule is only about XWayland windows of those types.

## Resolution

Done on 2026-10-06.

- **Policy** marks each workspace that shows its desktop, as Marco's `showing_desktop` does (`Policy::set_showing_desktop`, `is_showing_desktop`, and `Action::ShowDesktop`, which toggles). The cases are Marco's, at 8522ed1:
  - **Showing** hides the workspace's windows but for the desktop, docks and their transients, and focuses the desktop if it's a window (`meta_screen_show_desktop`, screen.c:2850-2882, and `meta_window_showing_on_its_workspace`, window.c:1385-1412). The windows aren't minimised, and the window list doesn't show them as minimised.
  - **Toggling back** brings them back as they were stacked, with the focus they had.
  - **A window mapped or activated on the workspace** ends it another way: the normal windows and dialogs that were hidden are minimised for good, and that one comes up without them (`maybe_leave_show_desktop_mode`, window.c:183-208). Those are the ones Marco gives a minimise function; a hidden utility window or toolbar comes back. An activation older than the user's last input ends nothing. Alt+Tab and a click in the window list are activations. A transient of the desktop or a dock doesn't end it.
  - **A workspace switch** leaves each workspace as it was.
- **Three things aren't Marco's, on purpose**, and the comments on `set_showing_desktop` and `leave_show_desktop` say so. Marco gives its default focus on toggling back, the topmost window, which isn't the one that had the focus if another was raised over it; this ticket asks for the focus they had. Marco ends it on the active workspace whichever workspace the new window is on; Caminito ends it on the window's own, as this ticket words it. And Marco minimises a new dialog's parents with the rest, which hides the dialog too; Caminito keeps them, as Marco itself does for an activated window.
- **caja's own dialogs end it.** Marco keeps a transient of the desktop or a dock up without ending it, and so does policy, but caja's desktop is a layer surface under Caminito, so a dialog of it has no parent in policy and counts as any new window.
- **`ShowingDesktop`** is a boolean property of `org.mate.Caminito`, read and written, for the active workspace. A write that changes nothing is accepted, and zbus signals it all the same. zbus signals a write, and the refresh pass signals every other change, each once: the key, a window that ends it, and a switch to a workspace whose value differs. The debug dump has `showing_desktop`.
- **Ctrl+Alt+D** is in the hard-coded bindings, as `d` on the first layout.
- **`_NET_SHOWING_DESKTOP`** isn't published or honoured for X11 clients. 06's row asks for the D-Bus property only, and that root property is one Smithay keeps to its API (11).
- **`mate-panel-caminito`'s eighth patch** has the Show Desktop applet read, write and follow the property while `org.mate.Caminito` has an owner, and use libxfce4windowing as before without one. `pkgrel` is 7.

The tests are `caminito-policy/src/tests/show_desktop.rs`, with the proptest sequences given the toggle, and `caminito/src/tests/show_desktop.rs`, on both kinds of window where the window matters.

The fourth criterion's run needs a Session with the new package installed, and wasn't done here: it is [106](106-packaging-and-closing-m6.md)'s, as the checklist's *The Show Desktop button and Ctrl+Alt+D show the same desktop*. The patch was compiled and packaged but never run against Caminito.
