# camino-displays hangs when a resolution is chosen

Type: implementation
Status: resolved
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

- [x] The cause is written here, with how it was seen.
- [x] A test, written before the fix, that fails with the hang, if the cause can be reached without a display. Otherwise how it was checked by hand.
- [x] Choosing each resolution of a display, and then each refresh rate, leaves the window up and the page showing the choice.
- [x] `docs/testing/checklist.md` has an item for it, if the checks of 84 there don't cover it.

## Resolution

**The cause is the suspect.** `render` gave the resolution's `gtk::DropDown` a new model from that drop-down's own `selected` handler, and GTK never returned from it. The main loop was gone from then on, so the window stopped drawing and answering.

- **How it was seen:** in `--demo`, so Camino and output management have no part in it. `target/debug/camino-displays --demo` under Xvfb with `GDK_BACKEND=x11`, GTK 4.22.5, driven with XTest pointer events: open Resolution on the built-in display, press its second entry. It hung on the first choice, every time, with the main thread at 100% of a core. It spins, it doesn't wait.
- **Where it was stuck**, from the process run under `gdb` and interrupted: a press on a row of the popover's list activates it, `gtk_single_selection_set_selected` notifies, the drop-down notifies `selected`, the handler in `window.rs` calls `change` and `render`, `render` calls `gtk_drop_down_set_model`, that calls `gtk_list_view_set_model` on the very list view whose row is being activated, and the list view makes list items without end (`g_object_new` under it in every sample).
- **The fix:** a list is given new entries only when they differ from the ones it has (`fill` in `window.rs`). Choosing a resolution leaves the resolutions as they were, and choosing a rate leaves the rates, so the list that is replaced is never the one being chosen from. A resolution still replaces the rates, which is another drop-down's model and was never the trouble.
- **The refresh rate hung the same way,** through the same `render`, and is fixed by the same. The scale didn't: it's a slider with no model.
- **No test before the fix:** the cause is GTK's, in the binary, and needs a display, which CI doesn't have. What the fix leans on in the library has a test, written after it: `choosing_from_a_list_leaves_its_entries_as_they_were` in `page/tests.rs`.
- **How it was checked by hand,** on the same set-up, before and after: before, the hang above. After, each resolution of each of the demo's three displays and then each of its refresh rates, 21 choices, and a press on the scale: the process is idle after each, and the page shows the choice, with the rates of the new resolution in the other list.
- **The checklist:** the second step of *The page with MATE's theme, at 100% and at a fractional scale* now chooses each resolution and each refresh rate of a display.

Not known, or not done:

- **Why 84's run under Xvfb didn't see it.** Its note says each setting was driven; the code of the lists is as 84 left it.
- **The window disappearing** wasn't seen under Xvfb, where the window froze with its popover up. In the Session it is taken to be what a client that stopped answering looks like there, and wasn't looked at on its own. The check on real monitors, with their longer lists of modes, is the developer's.
- **A hot-plug while a list is open** still gives that list a new model. It isn't from the list's own handler, and wasn't tried.
- **Of the first report:** the build was the installed `camino-git` (r191.78c06ad), with two monitors connected, going by the Session on the day. Whether it was the first choice there isn't known; in the demo it is.
- **For 86** there is a note in that ticket, of second starts that wait for a first one that hangs.
