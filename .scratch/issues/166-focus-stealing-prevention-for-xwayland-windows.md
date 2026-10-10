# Focus-stealing prevention for XWayland windows, and urgency in the window list

Epic: caminito-v0.1
Type: implementation
Status: ready-for-agent
Milestone: M8
Blocked by: 165, 155
Map: [Caminito v0.1](../caminito-v0.1/map.md)

## What to build

[11](11-xwayland-integration.md)'s focus-stealing rules for XWayland windows, on [165](165-focus-stealing-prevention-for-native-windows.md)'s *smart*, *strict* and urgency, and urgency sent to mate-panel's window list for both kinds of window.

- **XWayland windows carry no token** (X11 startup notification is later, 05), so they take the no-token path:
  - under *smart*, a new XWayland window gets the focus if it's its client's first window, or transient for the focused window; any other becomes urgent;
  - under *strict*, only a transient for the focused window gets it.
- **`_NET_ACTIVE_WINDOW`:** a request with source indication 2 (a pager) is honoured; one with source 1 (an application) is honoured only if that client already owns the focused window, and otherwise makes the window urgent.
- **X11 urgency:** `XUrgencyHint` and `_NET_WM_STATE_DEMANDS_ATTENTION` already make a window urgent, and the Window switcher marks it. What's new is Caminito making one urgent itself, which it writes back as `_NET_WM_STATE_DEMANDS_ATTENTION` alongside [155](155-above-below-and-sticky.md)'s states, and clearing it.
- **The window list:** the Xfce foreign-toplevel protocol's `demands_attention` state is sent for an urgent window of either kind ([ADR 0013](../../docs/adr/0013-window-workspaces-through-xfce-foreign-toplevel-protocol.md)), so that mate-panel's window list can flash it. [65](65-foreign-toplevel.md) kept the window's demand for attention for this.

## Acceptance criteria

- [ ] Policy tests, written before the code: each X11 rule under *smart* and *strict*; the two source indications; urgency Caminito sets, and its clearing.
- [ ] Headless tests through the fixture, on XWayland windows with the `x11rb` client: a first window focused; a second refused and urgent; a dialog transient for the focused window focused; `_NET_ACTIVE_WINDOW` with each source; `_NET_WM_STATE_DEMANDS_ATTENTION` written back. And through the foreign-toplevel test client, on both kinds: `demands_attention` sent and cleared.
- [ ] The `_NET_ACTIVE_WINDOW` and urgency rows in `docs/parity.md`'s keyless section, and `focus-new-windows`' row for XWayland windows, say honoured, with the tests above.
- [ ] The checklist has an item, tagged M8: an X11 app that opens a second window by itself under *smart*, its button in mate-panel's window list showing it wants attention, and the urgency clearing when it's clicked. Running it in a Session is the developer's, and goes to [173](173-packaging-and-closing-m8.md)'s closing run.

## Open questions

**For the developer**, decided when the ticket is done and listed in its Done section for confirmation:

- **Whether `mate-panel-caminito` needs another Carried patch** to show `demands_attention` from libxfce4windowing's state, or shows it already. Found here; a patch, if needed, is written here and packaged by [173](173-packaging-and-closing-m8.md).
- **When urgency clears:** proposed, when the window is focused, or when the client clears its hint, as in Marco.

**From Marco's source**, to be settled by the implementing agent and recorded in the Done section: how Marco treats `_NET_ACTIVE_WINDOW` with source 0 (old clients), and whether it clears `DEMANDS_ATTENTION` itself on focus.
