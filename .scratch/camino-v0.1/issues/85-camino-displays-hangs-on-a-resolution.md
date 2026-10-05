# camino-displays hangs when a resolution is chosen

Type: implementation
Status: open
Milestone: M6 (proposed)
Blocked by: none
Map: [Camino v0.1](../map.md)

## What to build

In camino-displays ([84](84-camino-displays.md)), choosing a resolution from the list hangs the program. The developer saw it on 2026-10-05: the window disappears, the process stays, and it has to be killed and started again.

- **To find first:** how to see it again, and where the process is stuck. Whether it happens in `--demo` too, which would take Camino and output management out of it. A backtrace of the hung process (`gdb -p`, or `eu-stack -p`) says whether it spins or waits.
- **A suspect, not checked:** the list is a `gtk::DropDown`, and its `selected` handler in `camino-displays/src/window.rs` calls `change`, which calls `render`, which gives the same drop-down a new model (`set_model`) and a selection while its popover is still open and the notification is still being sent. `rendering` keeps the handler from running again, but not GTK from being handed a new model in the middle of its own selection. The refresh rate's list is filled the same way, in the same `render`.
- **What 84 tested** was each setting driven with pointer events under Xvfb in the demo, where this wasn't seen. So the theme, the scale, or the real monitors' list of modes may matter.
- **The fix follows the cause.** If it is the model replaced during the notification: fill the lists only when their entries change, or after the handler returns.

## Acceptance criteria

- [ ] The cause is written here, with how it was seen.
- [ ] A test, written before the fix, that fails with the hang, if the cause can be reached without a display. Otherwise how it was checked by hand.
- [ ] Choosing each resolution of a display, and then each refresh rate, leaves the window up and the page showing the choice.
- [ ] `docs/testing/checklist.md` has an item for it, if the checks of 84 there don't cover it.

## Notes

- **Not known yet:** whether the session was the installed `camino-git` or a build from the tree, whether it was one monitor or two, and whether it hangs on the first choice or only on some resolutions.
- **The refresh rate and the scale** go through the same `change`. Whether they hang as well is to look at with it.
