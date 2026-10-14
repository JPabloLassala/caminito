# The Overview opens and closes

Epic: overview
Type: implementation
Status: resolved
Blocked by: None (can start immediately)
Spec: [Overview](../overview/spec.md)

## What to build

The first slice, and the one a user can try: the **Overview** (CONTEXT.md) on one output, with no transition and no bar, under "Opening and closing", "Which windows", "The layout", "The rest of the output" and "Where its state lives" in [the spec](../overview/spec.md). The arrow keys, the middle click and windows that come and go are ticket 242, the transition is 243, the bar is 244, the drag is 245 and two outputs are 246.

- **A System shortcut opens it**, kept in `org.mate.Caminito`, in both copies of the schema, and listed in System Shortcuts. First check if Super+W is free in Marco's and mate-settings-daemon's defaults: if it is, it is the default, and if it isn't, Super+Tab is. Record which in the spec's "Not decided" and name the key there under "Settings". With the shortcut removed, nothing opens the Overview.
- **It is an Interaction**, with its state in policy: whether it is open and the windows listed. The shortcut does nothing while the seat has another Interaction, and none opens while the Overview is open. The session lock ends it.
- **Pressed during a Workspace slide**, it opens at once and the slide is cut to its end.
- **The windows** are those the Window switcher lists for the output's active workspace: minimised ones are there, skip-taskbar ones aren't, and a sticky window is in every workspace of its output.
- **The layout** is a pure function of the windows' rectangles and the area, as the Window switcher's is: rows of windows, each keeping its aspect and its size relative to the others, none enlarged, none over another, in the order of their real places. Each has its title and its icon beneath it, drawn with cairo in the GTK theme's colours and font (ADR 0003).
- **The windows are live**: clients keep drawing and keep getting frame callbacks. The Window switcher's thumbnails are the nearest thing the scene has to a window drawn at another size and place.
- **The panel and caja's desktop aren't drawn** while it is open, and the wallpaper stays, dimmed, with no blur. With no transition yet, both change in one frame.
- **A click on a window** activates it and closes the Overview. A minimised window is unminimised. **A click on the empty area**, Esc or the shortcut again closes it and changes nothing.
- **Keys:** media keys work. Every other key and System shortcut is swallowed, Alt+Tab among them, until 242 and 244 add theirs.
- It opens on a workspace with no windows, and on one with a focused fullscreen window, which is laid out like any other.
- The debug dump shows the Overview: its output, the windows listed and each one's rectangle in the layout.

## Acceptance criteria

- [x] Unit tests of the layout's pure function: one window, several of different sizes, more than one row, none enlarged, none over another, the order by real place.
- [x] Policy's tests, with no protocol objects: the windows listed (minimised, skip-taskbar, sticky), the shortcut with another Interaction open, another Interaction asked for while the Overview is open, a click on a window, a click on the empty area, Esc.
- [x] Headless tests that open it through the fixture and read the dump: the listed windows and their rectangles, the panel and caja's desktop not drawn, the wallpaper dimmed, and all as it was after it closes.
- [x] A click on a window activates it and closes the Overview, and a minimised one is unminimised.
- [x] A window in the layout gets frame callbacks, and what it draws shows in the layout's next frame.
- [x] The session lock ends the Overview, and the shortcut during a Workspace slide cuts the slide to its end.
- [x] A key press while it is open doesn't reach the focused window, Alt+Tab opens no Window switcher, and a media key works.
- [x] The spec says which shortcut is the default and the key's name.

## Done (2026-10-13)

- **Super+W is the default**, as `overview` in `org.mate.Caminito` (both schema copies, since `tests/schemas/` links to `data/`). It was checked first, in the OrbStack machine: no default in marco 1.29.0's schemas or mate-settings-daemon-camino 1.29.1's has `<Super>` or `<Mod4>`. The spec's "Not decided" and "Settings" say so. The binding is after Marco's in the table, so a Marco key set to Super+W wins. System Shortcuts lists it from a fourth manifest, `50-caminito-overview-key.xml`, whose schema is `org.mate.Caminito`; marco's catalogue has no translation for its name.
- **A media key is an accelerator grab of a key with an `XF86` keysym.** `GrabAccelerators` doesn't say what a grab is for, so this is the one rule Caminito can use. It's my decision, recorded in the spec's "The keys"; the developer may want another. Modifiers go on to the client, as with the Window switcher open.
- **Policy keeps the list as the windows are now**, at every `apply`: a window that goes leaves it, and one that maps on the workspace joins it. That is part of 242's "windows that come and go", taken here because a stale list is a crash risk; 242 still has the layout's selection and the tests of a map while open. The Overview closes when its output goes (246 changes that).
- **The list is the Window switcher's candidates** for the output's active workspace, of windows whose Main output is the Overview's (`output_showing`), bottom to top, with a modal dialog in place of its parent, as the switcher has it.
- **The layout** splits the windows into rows by their centres from the top, each row by x, and takes the row count that leaves them largest; one factor ≤ 1 for all. With no room for the spacing it lays them out with none.
- **What isn't drawn on its output:** the windows as they are, a window shrinking into its button, the top and bottom layers, and every background surface but the wallpaper (`mate-background`). The overlay layer (msd's OSD) and X override-redirect windows stay above. The dimming is black at 0.5.
- **Clicks:** the left button on a window or its label chooses it; another button on a window does nothing until 242; any press on the empty area, on any output, closes it.
- **A switch under the Overview is a cut** on its output, and opening it ends that output's running slide (`refresh_workspace_slides`). 244 changes this.
- **The tests were written before the code** for policy and the layout; the headless tests came after the first build. To check that the headless tests can fail, the scene's replacement of the windows and the key swallowing were broken on purpose, and six tests failed.
- **What ran:** `caminito-policy`'s whole suite (698), and `caminito`'s tests of the modules the change touches (`overview`, debug, bindings, button bindings, accelerators, workspace slide and popup, switcher, window menu, lock, layer shell, input, manifests, parity, settings, override-redirect, drag icons, minimise animation, per-output workspaces, show desktop, panel, commands, window actions, move-to: 805), with fmt and clippy. Not the full suite, by the standing rule. Not run in a real Session: that's 247's.
- **From the review:** a minimise animation under way no longer shows over the Overview, and the test module says why some tests map xdg windows alone. Left as they are: the parallel vectors of windows, slots and labels in the compositor's `Overview`, and the use of `policy.frame_rect` in one test, which other test files do too.
