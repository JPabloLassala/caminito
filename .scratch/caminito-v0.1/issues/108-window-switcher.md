# The window switcher

Type: implementation
Status: open
Milestone: M8
Blocked by: none
Map: [Caminito v0.1](../map.md)

## What to build

The popup that Alt+Tab opens: a grid of thumbnails in the style of KDE Plasma's Thumbnail Grid, in place of Marco's popup ([ADR 0016](../../../docs/adr/0016-one-window-switcher-in-place-of-marcos-alt-tab-popup.md)). It was settled with the developer on 2026-10-07, ahead of M8, and is built there ([17](17-milestone-slicing.md)). Until then `switch-windows` activates the most recently used window with no popup.

- **The actions.** Every `switch-*` action opens the same switcher over a different set of windows, and its `-backward` twin opens it stepping the other way:
  - `switch-windows` (Alt+Tab): the active workspace's windows;
  - `switch-windows-all` (unbound): every workspace's;
  - `switch-group` (Alt+\`): the focused application's;
  - `switch-panels` (Ctrl+Alt+Tab): docks and the desktop.

  `cycle-windows`, `cycle-group` and `cycle-panels` open no popup, as in Marco.
- **The list.**
  - Most recently used first, with minimised windows after the others, as Marco orders them.
  - The selection starts on the second entry, so a quick Alt+Tab goes to the window used before this one.
  - Left out: windows that ask to skip the taskbar and, but for `switch-panels`, docks and the desktop.
  - A window with a modal dialog is listed as its dialog.
  - Windows from every output are listed.
  - An urgent window is marked and keeps its place.
- **Few windows.** With no window to list, nothing happens. With one, it's activated and no popup opens.
- **The popup.**
  - It's centred on the output the pointer is on.
  - It's a grid that wraps at `alt-tab-max-columns`, with thumbnails that shrink to fit when there are many windows.
  - The selected window's title is under the grid.
  - It appears about 90 ms after the key, so a quick Alt+Tab switches with nothing shown.
  - Its colours and font are the GTK theme's, from the palette the helper gives frames (ADR 0003).
- **A thumbnail** is the window's surface, drawn live, with the application's icon in a corner. A minimised window shows its last content. With `compositing-manager` off or `compositing-fast-alt-tab` on, the grid shows icons with no thumbnails.
- **Keys.** While the modifier is held:
  - Tab and Shift+Tab step forward and back, or the action's own key when it isn't Tab;
  - the arrow keys move in the grid;
  - releasing the modifier, or Enter, commits: the selected window is activated, and a minimised one is restored;
  - Escape cancels and leaves focus and stacking as they were.
- **The pointer.** A click on a thumbnail commits that window. Hovering does nothing: the popup often opens under the pointer, and hover would take the selection.
- **The selected window** is raised while the switcher is open when `alt-tab-raise-windows` is set, and goes back if the switcher is cancelled. Otherwise nothing shows on the window itself: no border and no fading of the others.
- **Windows that change meanwhile.** One that closes leaves the grid, and the selection moves to the next if it was selected. One that opens isn't added and isn't focused until the switcher closes, when its focus is settled as for any new window.
- **When it doesn't open.** During a move or resize, and while the screen is locked, the actions do nothing. A client's open menu is dismissed first.
- **Where it lives.** The switcher's state is policy's: the windows listed, their order, the selection, committing and cancelling (ADR 0010). The compositor crate lays the grid out, draws it in the scene and tells policy of keys and clicks.

## Acceptance criteria

- [x] Policy tests, written before the code: each action's list and order, minimised windows last, the modal dialog for its parent, the skip-taskbar windows left out, the starting selection, stepping and wrapping both ways, commit, cancel, a window closing while selected, a window opening meanwhile, none and one window.
- [x] Headless tests: a quick Alt+Tab switches with no popup drawn; holding Alt draws it on the pointer's output; arrows, Enter, Escape and a click each do as above; a minimised window is restored on commit; nothing opens during a move or while locked. All but "while locked": there's no lock until M7 (see *What was decided*). The tests of the popup, the pointer and the thumbnails were written after their code, not before it.
- [x] A drawing test for the grid: its wrapping at `alt-tab-max-columns`, thumbnails shrinking, the icons-only grid, and the urgent mark. They read the popup's rectangles from the debug dump and the output's pixels: Marco's popup has no goldens to compare against (ADR 0016).
- [x] `docs/parity.md`'s rows for the `switch-*` actions and the keys below, each with its status and ADR 0016 where it's *never*. The file is new, with these rows alone until M8's audit.
- [ ] Checklist: live thumbnails of a playing video and of a minimised window, in a session. The item is in `docs/testing/checklist.md`, tagged M8, and wasn't run: the run in a Session is the developer's.

## Notes

- **The keys.** Honoured: `alt-tab-max-columns`, `alt-tab-raise-windows`, `compositing-manager`, `compositing-fast-alt-tab`. Never: `show-tab-border`, as nothing is drawn round the selected window.
- **Not settled on 2026-10-07,** and to be put to the developer when the ticket starts:
  - `icon-size`: proposed as the size of the icons in the icons-only grid, in logical pixels (09), with the corner icon a fixed fraction of its thumbnail.
  - `alt-tab-expand-to-fit-title`: proposed as never, with a title longer than the popup ellipsised, since the grid's width comes from its thumbnails.
  - How an urgent window is marked, and how the selected thumbnail is.
- **Urgency** itself comes with focus-stealing prevention, also M8's (17).
- **Drawing a window away from its place** is new to the scene, which draws each window once. Capture ([103](103-image-capture.md)) is the nearest thing to it.
- **Not taken from KDE:** fading or hiding the other windows, closing a window from the switcher, and hover to select.

## What was decided

The ticket left these to the developer, and they were decided when it was done, without them. Each is the developer's to confirm or change.

- **`icon-size`,** as proposed: the size of the icons in the icons-only grid, in logical pixels. A thumbnail's corner icon is a quarter of the thumbnail box's height, kept within 16 to 48.
- **`alt-tab-expand-to-fit-title`,** as proposed: never, with a title wider than the popup ellipsised at its end.
- **The selected thumbnail** has its cell filled with the palette's `Selected` background. Where that's the background of everything else, as in the neutral palette, the cell is tinted with a quarter of the text colour, so that the selection always shows.
- **An urgent window** has a dot, 8 logical pixels across, in its cell's top right corner, in the GTK theme's `warning_color`, or the text colour where the theme has none.
- **A thumbnail shows the window with its Frame,** as a capture of it does (103).
- **The lock** is M7's. While locked no binding will reach the bindings stage of the keyboard path, so the switcher has no code for it, and the test that nothing opens while locked is M7's to write.
- **Windows that never had the focus** are listed after those that had, topmost first. Marco's list has every window; policy's has those that were focused.
- **The starting selection** is the second entry when the first is the focused window, or the modal dialog that stands for it, and the first entry otherwise, as when `switch-panels` opens from a normal window. Backward starts on the last.
- **A press of a pointer button outside the cells cancels,** as a click ends Marco's grab. No press reaches a client while the switcher is open.
- **The workspace changing under an open switcher,** by a pager or another client's activation, closes it and activates nothing.
- **`switch-group`'s application** is the windows of the focused window's process where both name one, else those with its app id, else the focused window alone.
- **A modifier pressed while it's open reaches the client,** though no other key does: a client is told of the modifiers with the keys it gets, and would otherwise keep the ones it was last told of.

## What was done

- **The list and the state** are policy's (`caminito-policy/src/policy/switcher.rs`): `SwitchList`, `Policy::switch_list`, and `Switcher` with `switcher_step`, `switcher_select`, `switcher_commit` and `switcher_cancel`. `Action::Switch { list, backward }` stands in for `SwitchWindows`. `PolicyWindow` has one new fact, `skips_taskbar`. A window mapped while it's open gets its focus when it closes. `alt-tab-raise-windows` is `Config::alt_tab_raise_windows`, and the stacking the switcher opened with is put back on a cancel and before a commit. The property tests step, select, commit and cancel it.
- **The settings** (`settings/translate.rs`) are `Settings::switcher`: the columns, the icon size, and whether cells show thumbnails.
- **The keys** (`caminito/src/switcher/mod.rs`): Alt+Tab, Alt+\` and Ctrl+Alt+Tab, each with Shift for backward, join the hard-coded bindings. `switch-windows-all` is unbound. The open switcher has the keyboard before an inhibitor, Caminito's other bindings and the accelerator grabs. It commits on the release that lets go of a modifier of its binding, whichever key is still down. A client's xdg popups are dismissed before it opens, with an Xwayland keyboard grab.
- **The grid** (`switcher/layout.rs`) is pure geometry: the cells, the cell at a point, where an arrow key goes, and a window fitted in a cell. Thumbnails are at most 256 × 160 and shrink by one factor until the popup is within nine tenths of the output.
- **The popup** appears from a 90 ms timer on the output the pointer is on, and stays there. If that output goes it moves to the pointer's or the primary one, and the switcher is cancelled with none left.
- **The drawing** is a layer of the scene over everything a client draws (`SceneLayer::Switcher`). `switcher/draw.rs` draws the chrome with cairo, behind and in front of the thumbnails, again only when something it's drawn from changed. A thumbnail is the window's own render elements, rescaled, moved and cropped with Smithay's element wrappers, so it's live with no copy. A window that isn't showing is drawn from the buffers its surfaces still hold.
- **The debug dump** has a `switcher` member: the list, the windows, the selection, and the popup's output, style, columns and rectangles once it shows.

## What's left

- **The checklist run** in a Session, with a playing video.
- **A minimised window's thumbnail isn't live,** as the ticket says: it gets no frame callbacks, so its client stops drawing.
- **`cycle-windows`, `cycle-group` and `cycle-panels`** aren't bound: they're M8's, with the bindings read from gsettings (17).
- **An urgent xdg toplevel** is never marked, since no protocol Caminito has tells it of one (65). M8's focus-stealing prevention makes windows urgent itself (17).
- **A held Tab doesn't repeat.** Each press steps once: Caminito swallows the key, and nothing repeats it as an X server did under Marco.
- **An X client's open menu stays on screen.** Its keyboard grab through Xwayland is dropped, so the switcher has the keys, but nothing closes the override-redirect window: the X client does at the next click. An xdg popup is dismissed.
- **At a fractional scale** the chrome, drawn at the next whole scale and scaled down, and a thumbnail, rounded to physical pixels, can differ by a pixel at a thumbnail's edge. Not seen in a test, which samples the cells' middles.
- **How it was built:** the list, the state and the grid by subagents from a plan (`108-plan.md`), each reviewed, and the rest in one session. A review of the whole branch against this ticket and the coding standards then found three faults, fixed with a test each: a thumbnail on another output took its window's main output, and so its scale; the pointer reached the windows under the popup; and a click could land on a cell laid out before a window closed.
