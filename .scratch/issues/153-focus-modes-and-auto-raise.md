# Focus modes and auto-raise

Epic: caminito-v0.1
Type: implementation
Status: ready-for-agent
Milestone: M8
Blocked by: 147
Map: [Caminito v0.1](../caminito-v0.1/map.md)

## What to build

Marco's three focus modes and auto-raise, for toplevels ([05](05-v01-marco-feature-cut.md), [09](09-gsettings-mapping.md)). Today focus follows the click only.

- **`focus-mode`:**
  - `click`: as today;
  - `sloppy`: the pointer entering a window focuses it, and leaving it for the desktop keeps the focus where it was;
  - `mouse`: as `sloppy`, but leaving for the desktop takes the focus away.
- **`raise-on-click`:** off, a click focuses a window without raising it, except on its Frame, as in Marco.
- **`auto-raise` and `auto-raise-delay`:** with a pointer focus mode, a window focused by the pointer is raised after the delay, if the pointer is still in it.
- **The pointer's enter and leave** reach Policy as new events, for xdg and XWayland windows alike, from the same place the pointer's focus is decided now. Policy decides; the compositor crate only reports.
- **Layer surfaces** keep taking the keyboard by their own keyboard interactivity, whatever the mode ([09](09-gsettings-mapping.md)).
- **The Window switcher and the workspace popup** hold the pointer focus modes off while they're open.

## Acceptance criteria

- [ ] Policy tests, written before the code: each mode with an enter, a leave to another window and a leave to the desktop; `raise-on-click` off on the body and on the Frame; auto-raise before and after the delay, and cancelled by a leave; a mode change applying at once.
- [ ] Headless tests through the fixture, on xdg and XWayland windows: sloppy and mouse focus following the pointer; auto-raise after the delay with the fixture's driven clock; a layer surface with on-demand interactivity not focused by an enter; the switcher open, and no focus change under it.
- [ ] `focus-mode`, `raise-on-click`, `auto-raise` and `auto-raise-delay` leave `UNSUPPORTED`; their rows in `docs/parity.md` say honoured, with the tests above.
- [ ] The checklist has an item, tagged M8: each mode with a native and an X11 window and the panel, and auto-raise at its default delay. Running it in a Session is the developer's, and goes to [173](173-packaging-and-closing-m8.md)'s closing run.

## Open questions

**For the developer**, decided when the ticket is done and listed in its Done section for confirmation:

- **Who owns the auto-raise timer.** Policy is free of time but for the `now` its calls take (15). Proposed: Policy keeps the deadline and says when it next needs to run, and one loop timer runs it, as the `idle` module's `IdleState` does ([128](128-idle-and-blanking.md)).

**From Marco's source**, to be settled by the implementing agent and recorded in the Done section:

- **Enters that aren't the user's:** whether an enter caused by a window restacking, mapping or moving under a still pointer, or by a workspace switch, changes the focus in Marco.
- **The desktop window** (caja's, a Layer surface here) under `mouse`: what counts as leaving to the desktop.
- **`raise-on-click`** with a click on a transient or a modal dialog's parent.
