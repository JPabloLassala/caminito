# The Overview opens and closes

Epic: overview
Type: implementation
Status: ready-for-agent
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

- [ ] Unit tests of the layout's pure function: one window, several of different sizes, more than one row, none enlarged, none over another, the order by real place.
- [ ] Policy's tests, with no protocol objects: the windows listed (minimised, skip-taskbar, sticky), the shortcut with another Interaction open, another Interaction asked for while the Overview is open, a click on a window, a click on the empty area, Esc.
- [ ] Headless tests that open it through the fixture and read the dump: the listed windows and their rectangles, the panel and caja's desktop not drawn, the wallpaper dimmed, and all as it was after it closes.
- [ ] A click on a window activates it and closes the Overview, and a minimised one is unminimised.
- [ ] A window in the layout gets frame callbacks, and what it draws shows in the layout's next frame.
- [ ] The session lock ends the Overview, and the shortcut during a Workspace slide cuts the slide to its end.
- [ ] A key press while it is open doesn't reach the focused window, Alt+Tab opens no Window switcher, and a media key works.
- [ ] The spec says which shortcut is the default and the key's name.
