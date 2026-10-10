# Above, below and sticky

Epic: caminito-v0.1
Type: implementation
Status: needs-triage
Milestone: M8
Blocked by: 148
Map: [Caminito v0.1](../caminito-v0.1/map.md)

## What to build

Keeping a window above or below the others, and sticky windows, which show on every workspace, as compositor state ([05](05-v01-marco-feature-cut.md), [17](17-milestone-slicing.md)). Today Policy records above, below and sticky as a client asks for them, and nothing acts on them.

- **Layering:** Policy's stacking honours above and below: an above window stays over the normal ones of its workspace, a below window under them, as Marco's layers do.
- **Sticky:** a sticky window shows on every workspace, keeps its place on each, and goes with no workspace switch. Today each workspace has its own stack, so a sticky window has no stack to be in: the model for it is this ticket's (below).
- **The ways in:**
  - the bindings `toggle-above` and `toggle-on-all-workspaces`;
  - the titlebar actions `toggle_above` and `toggle_stick` (today they log and do nothing), and the Frame's above and stick buttons where `button-layout` has them;
  - X11 `_NET_WM_STATE` above, below and sticky, and `_NET_WM_DESKTOP` `0xFFFFFFFF`, before map and as requests ([39](39-x-window-properties-and-requests.md), [11](11-xwayland-integration.md)), which reach Policy since M3 and now take effect;
  - the Xfce foreign-toplevel protocol's `sticky`, `above` and `below` states, sent to the window list and honoured as requests ([ADR 0013](../../docs/adr/0013-window-workspaces-through-xfce-foreign-toplevel-protocol.md)).
- **Written back:** `_NET_WM_STATE` and `_NET_WM_DESKTOP` on an XWayland window, and the foreign-toplevel states, follow every change, whichever way it came.
- **Struts:** a sticky dock's struts count on every workspace ([64](64-work-areas-from-struts.md) counts them on its own workspace until then).

## Acceptance criteria

- [ ] Policy tests, written before the code: above and below against normal windows, transients and fullscreen windows; sticky across a workspace switch, a `move-to-workspace-*`, the Window switcher's lists and Show Desktop; a sticky dock's struts in every workspace's work area.
- [ ] Headless tests through the fixture, on xdg and XWayland windows: the two bindings; the titlebar actions and buttons; X11 `_NET_WM_STATE` and `_NET_WM_DESKTOP` requests and their properties written back; the foreign-toplevel states sent and a request honoured.
- [ ] `toggle-above` and `toggle-on-all-workspaces`, and the `toggle_above` and `toggle_stick` values of the titlebar action keys, have rows in `docs/parity.md` that say honoured, with the tests above; any of them in `UNSUPPORTED` leaves it.
- [ ] The checklist has an item, tagged M8: a window kept above and one made sticky from the Frame, from the binding and from the window list, seen in the pager and on each workspace, and an X11 app (`xterm` with `wmctrl -r :ACTIVE: -b add,above`). Running it in a Session is the developer's, and goes to [173](173-packaging-and-closing-m8.md)'s closing run.

## Notes

- **The window menu's** *Always on Top* and *Always on Visible Workspace* items are [161](161-window-menu.md)'s, on top of this.

## Open questions

**For the developer**, to settle before building:

- **The data model for sticky with a stack per workspace** (ADR-worthy). Options: a sticky window is in every workspace's stack, kept in step; there's one stack for sticky windows, merged into each workspace's at its layer; or the stacks become one with a workspace filter. It touches stacking, focus, the Window switcher's lists and Show Desktop's per-workspace semantics ([94](94-show-desktop.md)), and isn't easily reversed.
- **Whether `mate-panel-caminito`'s window-list patch needs to read `above`**, so that its menu's *Always on Top* shows, or stays as it is with only `sticky` read.

**From Marco's source**, to be settled by the implementing agent and recorded in the Done section: where a transient of an above window goes; how above and below stack against docks and fullscreen windows; whether a sticky window's place is the same on every workspace.
