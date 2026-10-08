# The Window Switcher Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Build ticket 108: the Window switcher, a grid of live thumbnails that Alt+Tab and Marco's other `switch-*` actions open while the modifier is held.

**Architecture:** Policy (`caminito-policy`) owns the switcher's state: which windows are listed, their order, the selection, commit and cancel (ADR 0010, ADR 0016). The compositor crate (`caminito`) reads Marco's keys, feeds policy the keys and clicks, lays the grid out, and draws it in the scene: one cairo buffer for the popup's chrome, and each window's own surfaces drawn smaller for the thumbnails.

**Tech Stack:** Rust 2024, Smithay (pinned by `rev`), cairo and pango through `caminito-theme`'s re-exports, calloop timers, proptest, insta.

**Spec:** `.scratch/issues/108-window-switcher.md` (the ticket) and `docs/adr/0016-one-window-switcher-in-place-of-marcos-alt-tab-popup.md`. Read both. Where this plan and the ticket disagree, the ticket wins, except for the rulings below.

## Global Constraints

- Follow `CODING_STANDARDS.md` and use `CONTEXT.md`'s terms. The thing is the **Window switcher**; never "task switcher", "Alt+Tab popup", "tab popup" or "switcher" alone in prose and docs (identifiers may be short).
- `caminito-policy` names no protocol object, is `#![forbid(unsafe_code)]`, uses only Smithay's geometry types, and takes time as an argument (`now`). It never reads a clock.
- `scene` is the only code that knows the layer order.
- Handlers change policy state and mark it dirty. Outward views (the debug dump) are produced on demand, not updated inline.
- Tests go through the seam: `TestWindow` for policy; `Fixture`, the Wayland or X11 client and the debug dump (`Caminito::state_dump`) for the compositor. No sleeps to wait for a state: dispatch until a condition holds. A fixed wait is right only to show that nothing happens, with a comment that says why the time is enough.
- A test of behaviour both kinds of window share runs on both through `on_both_kinds!`. A test that maps one kind says why.
- Test names are sentences about behaviour in snake case.
- No new dependency.
- Exact values from the ticket: the popup appears **90 ms** after the key. The selection starts on the **second** entry. Honoured keys of `org.mate.Marco.general`: `alt-tab-max-columns` (1 to 64, default 5), `alt-tab-raise-windows` (default false), `compositing-manager` (default true), `compositing-fast-alt-tab` (default false), `icon-size` (8 to 256, default 48). Never honoured: `show-tab-border`, `alt-tab-expand-to-fit-title`.
- Default bindings (hard-coded until M8 reads `org.mate.Marco.*`): `switch-windows` Alt+Tab, `switch-group` Alt+grave, `switch-panels` Ctrl+Alt+Tab; `switch-windows-all` is unbound. Shift with any of them steps backward.
- CI must pass: `cargo fmt --all --check`, `cargo clippy --workspace --all-targets -- -D warnings`, and the tests.
- Commits: imperative subject that names the ticket, for example `List the windows a switch action shows (ticket 108)`. The body is prose: what changed and why, what is left, how it was tested. End each commit message with the line `Co-Authored-By: Claude Opus 5.5 (1M context) <noreply@anthropic.com>`. Do not push.

### How to build and test

Caminito does not build on macOS. Run every cargo command in the OrbStack machine `caminito`. The checkout is mounted at the same path.

```sh
orb -m caminito bash -c 'cd /Users/jpablo/personalrepos/caminito-ticket-108 && export CARGO_TARGET_DIR=$HOME/target-108 && ulimit -n $(ulimit -Hn) && <command>'
```

- Policy: `cargo test -p caminito-policy`
- Compositor, one test module: `dbus-run-session -- cargo test -p caminito switcher`
- Compositor, all: `dbus-run-session -- cargo test -p caminito` (several minutes)
- Lints: `cargo fmt --all --check` and `cargo clippy --workspace --all-targets -- -D warnings`
- Accept an insta snapshot: run the test with `INSTA_UPDATE=always`, then read the diff of the `.snap` file or the inline snapshot before you commit it.

Known noise, not yours to fix: `caminito-theme`'s goldens fail on aarch64, and `xwayland_crashing_again_and_again_as_it_starts…` fails now and then in the full `caminito` suite (it passes alone). Directly after an edit the machine can see a file at its old size, and cargo reports a truncated file: run the command again. Other sessions use the same machine: do not use `/tmp` or display `:99`, and do not `pkill`.

### Rulings made when the plan was written

The ticket leaves these open or cannot be met as written. Each is recorded in the ticket's notes by Task 7.

1. **`icon-size`** is the size of the icons in the icons-only grid, in logical pixels. The corner icon of a thumbnail is one quarter of the thumbnail box's height, kept within 16 to 48 logical pixels.
2. **`alt-tab-expand-to-fit-title`** is never honoured. A title wider than the popup is ellipsised at its end.
3. **The selected cell** is filled with the palette's `Selected` background. **An urgent window's cell** has a filled dot, 8 logical pixels across, in its top right corner, in the palette's named colour `warning_color`, or the `Selected` background when the theme has none.
4. **A thumbnail shows the window with its Frame,** as a capture of the window does (103).
5. **The lock** is M7's and does not exist. While locked no binding will reach stage 4 of the keyboard path (`lock_passes`), so the switcher needs no code for it. The headless test "nothing opens while locked" is left to M7.
6. **Windows that never had the focus** are listed after those that had, topmost first. Marco's list holds every window; policy's `mru` holds only those that were focused.
7. **The starting selection** is the second entry when the first entry is the seat's focused window (or the modal dialog that stands for it), and the first entry otherwise, as when `switch-panels` opens from a normal window. Backward starts on the last entry.
8. **A press of a pointer button outside the popup's cells cancels the switcher,** as it ends Marco's grab. No press reaches a client while the switcher is open.
9. **The workspace changing under an open switcher** (a pager, another client's activation) closes it without activating anything.
10. **`switch-group`'s application** is the windows with the focused window's pid when both have one, else those with its app id when both have one, else the focused window alone.

## Review Focus

Inputs the ticket implies and a user will meet. Each has its test in the task named.

1. **The modifier is released while another key is still down, or the host takes the keyboard from a nested Caminito** (`State::release_pressed_keys`): the switcher commits and does not stay open. Task 4.
2. **Every listed window closes while the switcher is open:** it closes, the focus is the default one, nothing panics. Task 2.
3. **The output the popup is on is unplugged while it is open:** the popup goes to an output that is left, or the switcher closes when none is; nothing panics. Task 5.
4. **Very many windows on a small output** (60 windows on 800 × 600): every cell keeps a positive size and lies inside the output. Task 3.
5. **A title that is empty, very long, or not Latin:** the popup draws, the title stays inside it. Task 5.

---

### Task 1: The list a switch action shows

Policy learns which windows each `switch-*` action lists, and in what order. No state yet.

**Files:**
- Modify: `caminito-policy/src/window.rs` (one new fact on `PolicyWindow`)
- Modify: `caminito-policy/src/test_window.rs` (fields and builders for the new facts)
- Modify: `caminito-policy/src/policy.rs` (declare `mod switcher;`, nothing else)
- Create: `caminito-policy/src/policy/switcher.rs` (a child module of `policy`, so it reads `Policy`'s private fields; the crate already uses this shape in `caminito/src/input/bindings.rs` with `bindings/accelerators.rs`)
- Modify: `caminito-policy/src/lib.rs` (export `SwitchList`)
- Modify: `caminito/src/window/mod.rs` (`impl PolicyWindow for Mapped`: the new fact)
- Test: `caminito-policy/src/tests/switcher.rs` (new; add `mod switcher;` to `tests/mod.rs`)

**Interfaces:**
- Consumes: `Policy`'s private fields `windows`, `stacks`, `active`, `focus`, `mru`, and its private `modal_transient(id) -> WindowId`.
- Produces:

```rust
// caminito-policy/src/window.rs, in `trait PolicyWindow`, among the facts:
/// Whether the window is left out of the taskbar, by its own asking or by Marco's rules
/// for its type. The Window switcher leaves it out too (108).
fn skips_taskbar(&self) -> bool;

// caminito-policy/src/policy/switcher.rs
/// The windows a `switch-*` action lists, as Marco's `MetaTabList` has them (108).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum SwitchList {
    /// `switch-windows`: the active workspace's windows.
    Windows,
    /// `switch-windows-all`: every workspace's.
    WindowsAll,
    /// `switch-group`: the focused application's, on the active workspace.
    Group,
    /// `switch-panels`: docks and the desktop.
    Panels,
}

impl<W: PolicyWindow> Policy<W> {
    /// The windows `list` shows `seat`, in the Window switcher's order.
    pub fn switch_list(&self, seat: SeatId, list: SwitchList) -> Vec<WindowId>;
}
```

`TestWindow` gets `pub skip_taskbar: bool`, `pub pid: Option<u32>`, `pub app_id: Option<String>` (all off or `None` in `new`), the builders `skipping_taskbar()`, `with_pid(u32)`, `with_app_id(&str)`, and its `pid()`, `app_id()` and `skips_taskbar()` return them.

`Mapped::skips_taskbar` returns `self.skips().taskbar`.

**The rules** (`switch_list`):

1. *Candidates.* `Windows` and `Panels`: the windows on the active workspace. `WindowsAll`: every workspace's. `Group`: the windows on the active workspace of the seat's focused window's application (ruling 10), found from the focused window's transient root; none when the seat focuses no window.
2. *Left out.* For `Panels`, keep only `WindowType::Dock` and `WindowType::Desktop`. For the others, drop those two types and drop every window whose `skips_taskbar()` is true.
3. *The modal dialog.* Each window kept is listed as `modal_transient(window)`: its last modal dialog that is not minimised, or itself. A dialog is never listed twice.
4. *Order.* Most recently used first: by the better of the window's and its dialog's place in `mru`. Windows in neither come after, in stacking order from the top, the active workspace's stack first and then the others from the first (ruling 6). Then, keeping that order, every window whose base window is minimised goes after all that are not.
5. Windows a workspace hides because it shows its desktop are listed like any other.

- [ ] **Step 1: Add the fact and the test window's fields.** Add `skips_taskbar` to the trait, the fields and builders to `TestWindow`, and the implementation on `Mapped`. Build the workspace (`cargo build --workspace --all-targets`) to find every implementer of the trait.

- [ ] **Step 2: Write the failing tests** in `caminito-policy/src/tests/switcher.rs`. Use the helpers of `tests/mod.rs` (`policy()`, `ms`, `id`, `SEAT`). One test per rule, named as a sentence. At least:

```rust
use super::*;
use crate::SwitchList;

#[test]
fn switch_windows_lists_the_active_workspace_most_recently_used_first() {
    let mut policy = policy();
    for w in 1..=3 {
        policy.map_window(TestWindow::new(w, 400, 300), ms(w));
    }
    policy.click(SEAT, id(1), ms(4));
    policy.map_window(TestWindow::new(4, 400, 300), ms(5));
    policy.move_to_workspace(id(4), 1);

    assert_eq!(policy.switch_list(SEAT, SwitchList::Windows), [id(1), id(3), id(2)]);
}

#[test]
fn minimised_windows_come_after_the_others() {
    let mut policy = policy();
    for w in 1..=3 {
        policy.map_window(TestWindow::new(w, 400, 300), ms(w));
    }
    policy.minimize(id(3));

    // 3 was the most recently used, and is last for being minimised.
    assert_eq!(policy.switch_list(SEAT, SwitchList::Windows), [id(2), id(1), id(3)]);
}

#[test]
fn a_window_with_a_modal_dialog_is_listed_as_its_dialog() {
    let mut policy = policy();
    policy.map_window(TestWindow::new(1, 400, 300), ms(1));
    policy.map_window(TestWindow::new(2, 400, 300), ms(2));
    policy.map_window(
        TestWindow::new(3, 200, 100)
            .with_type(WindowType::ModalDialog)
            .with_parent(1)
            .skipping_taskbar(),
        ms(3),
    );

    assert_eq!(policy.switch_list(SEAT, SwitchList::Windows), [id(3), id(2)]);
}
```

  Also write, in the same style: `windows_that_skip_the_taskbar_are_left_out`; `docks_and_the_desktop_are_left_out_of_all_but_switch_panels` (assert `Windows`, `WindowsAll` and `Group` leave them out and `Panels` lists only them); `switch_windows_all_lists_every_workspace`; `switch_group_lists_the_focused_applications_windows` (by pid, by app id when no pid, and the focused window alone with neither); `switch_group_lists_nothing_with_no_focused_window`; `windows_that_never_had_the_focus_come_after_the_others_topmost_first` (map with `without_input()` so they are never focused); `a_window_hidden_by_show_desktop_is_still_listed`; `a_minimised_modal_dialog_leaves_its_parent_listed_as_itself`.

- [ ] **Step 3: Run them and see them fail.** `cargo test -p caminito-policy switcher`. Expected: they do not compile, as `switch_list` and `SwitchList` do not exist. Record the output for the report.

- [ ] **Step 4: Implement `SwitchList` and `switch_list`** in `policy/switcher.rs`, with a `//!` module doc that says the module owns the Window switcher's list and state (108, ADR 0016). Cite Marco where a rule is Marco's, if you know the file; do not invent a line number.

- [ ] **Step 5: Run the tests.** `cargo test -p caminito-policy`. Expected: all pass, the old ones too.

- [ ] **Step 6: Lint and commit.** `cargo fmt --all --check`, `cargo clippy --workspace --all-targets -- -D warnings`, then commit.

---

### Task 2: The switcher's state in policy

Policy opens the Window switcher on a `switch-*` action, steps it, commits and cancels it, and keeps it right as windows come and go.

**Files:**
- Modify: `caminito-policy/src/action.rs` (replace `Action::SwitchWindows`)
- Modify: `caminito-policy/src/policy.rs` (the `switcher` field, `Config`, the hooks in `action`, `map_window`, `unmap_window`, `switch_workspace`, `begin_grab`)
- Modify: `caminito-policy/src/policy/switcher.rs` (the state and its methods)
- Modify: `caminito-policy/src/lib.rs` (export `Switcher`)
- Modify: `caminito-policy/src/tests/actions.rs`, `caminito-policy/src/tests/invariants.rs`
- Modify: `caminito/src/input/bindings.rs` and any other user of `Action::SwitchWindows` (keep Alt+Tab bound; see below)
- Test: `caminito-policy/src/tests/switcher.rs`

**Interfaces:**
- Consumes: Task 1's `SwitchList` and `Policy::switch_list(seat, list) -> Vec<WindowId>`.
- Produces:

```rust
// action.rs: replaces `SwitchWindows`.
/// A `switch-*` action (108): opens the Window switcher over `list`, or steps the one
/// that is open, towards the end of the list or `backward`.
Switch { list: SwitchList, backward: bool },

// policy.rs, in `Config`:
/// `alt-tab-raise-windows`: the Window switcher raises the selected window while it's
/// open (108).
pub alt_tab_raise_windows: bool,   // false in `Default`

// policy/switcher.rs
/// The open Window switcher (108): what it lists, and which entry is selected.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Switcher {
    pub seat: SeatId,
    pub list: SwitchList,
    /// The windows listed, in order. Never empty.
    pub windows: Vec<WindowId>,
    /// The selected entry's place in `windows`.
    pub selected: usize,
}

impl<W: PolicyWindow> Policy<W> {
    /// The Window switcher, while it's open.
    pub fn switcher(&self) -> Option<&Switcher>;
    /// Selects the next entry, or the one before with `backward`, wrapping at the ends.
    pub fn switcher_step(&mut self, backward: bool);
    /// Selects the entry at `index`. Does nothing if there's none.
    pub fn switcher_select(&mut self, index: usize);
    /// Closes the switcher and activates the selected window, as `activate` does.
    pub fn switcher_commit(&mut self, now: Duration);
    /// Closes the switcher and leaves focus and stacking as they were when it opened.
    pub fn switcher_cancel(&mut self, now: Duration);
}
```

The private state beside `Switcher` (stacks to restore, windows mapped meanwhile) is yours to shape; keep it in `policy/switcher.rs`.

**The rules:**

1. `action(seat, Action::Switch { list, backward }, now)`:
   - with the switcher open: `switcher_step(backward)`, whatever `list` is;
   - else, while `seat` has a grab (an interactive move or resize): nothing;
   - else with `switch_list(seat, list)` empty: nothing; with one window: `activate(seat, it, now)` and no switcher; with more: the switcher opens, with the starting selection of ruling 7.
2. While it's open, no seat's focus changes and no window is activated by the switcher. With `alt_tab_raise_windows` set, the selected window is raised (as `raise` does) when the switcher opens and at each change of selection, from the stacking the switcher opened with, so only one window is ever out of place.
3. `switcher_commit` closes it, puts the stacking back as it opened if it raised anything, then `activate`s the selected window for the switcher's seat. That restores a minimised window and switches workspace, as `activate` does.
4. `switcher_cancel` closes it and puts the stacking back. Focus is as it was.
5. A listed window that is unmapped leaves `windows`. If it was selected, the next one is (the first, if it was last). If none is left the switcher closes as if cancelled (Review Focus 2).
6. A window mapped while it's open isn't added, and isn't focused then. When the switcher closes, by commit or cancel, each such window that is still managed and on the active workspace is focused as `map_window` would have focused it, in the order they mapped, after the commit's activation.
7. `switch_workspace` to another workspace closes the switcher without activating, before it switches (ruling 9). `switcher_commit` must not trip over this when its own `activate` switches workspace.
8. `begin_move` and `begin_resize` return false while the switcher is open.
9. Policy's `apply` runs after every change, as everywhere else in `Policy`.

- [ ] **Step 1: Write the failing tests** in `tests/switcher.rs`. At least:

```rust
const SWITCH: Action = Action::Switch { list: SwitchList::Windows, backward: false };
const SWITCH_BACK: Action = Action::Switch { list: SwitchList::Windows, backward: true };

/// Three windows, 3 the most recently used and focused.
fn three() -> Policy<TestWindow> {
    let mut policy = policy();
    for w in 1..=3 {
        policy.map_window(TestWindow::new(w, 400, 300), ms(w));
    }
    policy
}

#[test]
fn the_switcher_opens_on_the_second_entry_and_changes_no_focus() {
    let mut policy = three();
    policy.action(SEAT, SWITCH, ms(10));

    let switcher = policy.switcher().expect("the switcher is open");
    assert_eq!(switcher.windows, [id(3), id(2), id(1)]);
    assert_eq!(switcher.selected, 1);
    assert_eq!(policy.focus(SEAT), Some(id(3)));
    assert_eq!(policy.stacking(0), [id(1), id(2), id(3)]);
}

#[test]
fn stepping_wraps_both_ways() {
    let mut policy = three();
    policy.action(SEAT, SWITCH, ms(10));
    policy.action(SEAT, SWITCH, ms(11));
    assert_eq!(policy.switcher().unwrap().selected, 2);
    policy.action(SEAT, SWITCH, ms(12));
    assert_eq!(policy.switcher().unwrap().selected, 0);
    policy.action(SEAT, SWITCH_BACK, ms(13));
    assert_eq!(policy.switcher().unwrap().selected, 2);
}

#[test]
fn commit_activates_the_selected_window_and_restores_a_minimised_one() {
    let mut policy = three();
    policy.minimize(id(2));
    policy.action(SEAT, SWITCH, ms(10)); // 3, 1, then 2 minimised
    policy.switcher_select(2);
    policy.switcher_commit(ms(11));

    assert!(policy.switcher().is_none());
    assert_eq!(policy.focus(SEAT), Some(id(2)));
    assert!(!policy.is_minimized(id(2)));
    assert_eq!(policy.stacking(0).last(), Some(&id(2)));
}

#[test]
fn cancel_leaves_focus_and_stacking_as_they_were() {
    let mut policy = three();
    let before = policy.stacking(0).to_vec();
    policy.action(SEAT, SWITCH, ms(10));
    policy.switcher_step(false);
    policy.switcher_cancel(ms(11));

    assert!(policy.switcher().is_none());
    assert_eq!(policy.focus(SEAT), Some(id(3)));
    assert_eq!(policy.stacking(0), before);
}
```

  Also write: `the_switcher_opens_backward_on_the_last_entry`; `with_no_window_to_list_nothing_happens`; `with_one_window_it_is_activated_and_no_switcher_opens` (one normal window that is minimised or unfocused; it ends focused, `switcher()` is `None`); `switch_panels_from_a_normal_window_starts_on_the_first_entry`; `a_selected_window_that_closes_gives_the_selection_to_the_next`; `an_unselected_window_that_closes_leaves_the_grid_and_keeps_the_selection_on_its_window` (the selected *window* stays selected when an earlier entry goes); `the_last_listed_window_closing_closes_the_switcher`; `a_window_that_opens_meanwhile_is_not_listed_nor_focused_until_the_switcher_closes` (after cancel it has the focus; after commit it has the focus too, mapped after the selected window was activated); `the_selected_window_is_raised_while_open_with_alt_tab_raise_windows` and `…_and_goes_back_on_cancel`; `without_alt_tab_raise_windows_the_stacking_does_not_change_while_open`; `nothing_opens_during_a_move` (after `begin_move`); `no_move_begins_while_the_switcher_is_open`; `switching_workspace_closes_the_switcher_without_activating`; `committing_a_window_on_another_workspace_switches_to_it` (with `SwitchList::WindowsAll`).

  Update `tests/actions.rs`: its two `switch_windows_…` tests exercised the action with no popup. Rewrite them on the new action plus `switcher_commit`, or delete the ones the new tests replace; do not leave a test of removed behaviour.

- [ ] **Step 2: Run them and see them fail** (`cargo test -p caminito-policy switcher`). Record the output.

- [ ] **Step 3: Implement.** Replace `Action::SwitchWindows`. In `caminito/src/input/bindings.rs`, bind Alt+Tab to `Action::Switch { list: SwitchList::Windows, backward: false }` and fix its unit test, so the workspace builds. The compositor commits nothing yet: after this task Alt+Tab in a running Caminito opens policy's switcher and leaves it open. That is expected and Task 4 completes it; say so in the commit body. To keep `caminito`'s suite green meanwhile, make the place in `caminito/src/input/mod.rs` that runs a `Bound::Action` call `policy.switcher_commit(now)` straight after a `Switch` action, with a comment that Task 4 of ticket 108's plan replaces it.

- [ ] **Step 4: Extend the invariants** in `tests/invariants.rs`. Add `Action::Switch` with each `SwitchList` and both directions to the action strategy in place of `SwitchWindows`, add the ops `SwitcherStep(bool)`, `SwitcherSelect(usize)`, `SwitcherCommit` and `SwitcherCancel`, and check after every op: when `switcher()` is `Some`, `windows` isn't empty, has no duplicates, every window in it is managed, and `selected < windows.len()`. Read how the file checks its other invariants first and follow it.

- [ ] **Step 5: Run everything.** `cargo test -p caminito-policy`, then `dbus-run-session -- cargo test -p caminito bindings` and `… cargo test -p caminito foreign_toplevel` and `… window_properties` (they use Alt+Tab). Expected: pass.

- [ ] **Step 6: Lint and commit.**

---

### Task 3: The grid's geometry

A pure module in the compositor crate: where the popup and its cells are, which cell a point is in, and where an arrow key goes. No Wayland, no cairo.

**Files:**
- Create: `caminito/src/switcher/mod.rs` (the `//!` doc: the Window switcher's side in the compositor (108, ADR 0016); for now it only declares `pub(crate) mod layout;`)
- Create: `caminito/src/switcher/layout.rs`
- Modify: `caminito/src/lib.rs` (declare `mod switcher;` as its neighbours are declared)
- Test: a `tests` module at the end of `layout.rs` (pure logic sits beside its code)

**Interfaces:**
- Consumes: nothing from earlier tasks.
- Produces:

```rust
use smithay::utils::{Logical, Point, Rectangle, Size};

/// What a cell shows.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum CellStyle {
    /// A thumbnail of the window, with its icon in a corner.
    Thumbnails,
    /// The window's icon alone, `icon_size` logical pixels square.
    Icons { icon_size: i32 },
}

/// A way an arrow key moves the selection.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum Direction { Left, Right, Up, Down }

/// The Window switcher's popup, laid out for one output. Every rectangle is in logical
/// pixels from the output's top left corner.
#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) struct Grid {
    /// The whole popup, centred on the output.
    pub popup: Rectangle<i32, Logical>,
    /// Each entry's cell, in the list's order, row by row.
    pub cells: Vec<Rectangle<i32, Logical>>,
    /// Where the selected window's title is drawn, under the cells.
    pub title: Rectangle<i32, Logical>,
    pub columns: usize,
    pub style: CellStyle,
}

impl Grid {
    /// Lays `count` entries out on an output of `output` logical pixels, in rows of at
    /// most `max_columns`. `count` is at least 1.
    pub(crate) fn new(count: usize, max_columns: usize, output: Size<i32, Logical>, style: CellStyle) -> Grid;
    /// The cell at `point`, if it's in one.
    pub(crate) fn cell_at(&self, point: Point<f64, Logical>) -> Option<usize>;
    /// The entry an arrow key moves the selection to from `from`.
    pub(crate) fn neighbour(&self, from: usize, direction: Direction) -> usize;
    /// The box inside cell `index` that its thumbnail or icon is fitted in.
    pub(crate) fn content(&self, index: usize) -> Rectangle<i32, Logical>;
    /// Where a window of `source` logical pixels is drawn in cell `index`: scaled to fit
    /// `content(index)` with its aspect kept, never enlarged, and centred.
    pub(crate) fn thumbnail(&self, index: usize, source: Size<i32, Logical>) -> Rectangle<i32, Logical>;
}
```

**The rules.** Name each number as a `const` with a `///` doc.

- `columns = count.min(max_columns).max(1)`; `rows = count.div_ceil(columns)`. A last row that isn't full starts at the left.
- The popup has a padding of 12 inside its edge, a gap of 8 between cells, and a title band 32 high under the cells with the same padding. A cell has a padding of 8 around its content.
- `Thumbnails`: a cell's content is at most 256 × 160. The popup is at most nine tenths of the output's width and of its height. When the cells don't fit at full size, every content box shrinks by one factor, the same both ways, until they fit, and is never smaller than 16 × 16 (Review Focus 4: with the minimum reached the popup may be as large as the output, and no larger; every cell still has a positive size and lies within the output).
- `Icons { icon_size }`: a cell's content is `icon_size` square, and shrinks the same way.
- `neighbour`: `Left` and `Right` go to the entry before and after, wrapping at the ends of the list, as Tab does. `Up` and `Down` keep the column and go a row up or down, wrapping; when the row arrived at has no entry in that column, it's that row's last entry.
- `cell_at` takes a point from the output's corner, as the rectangles are.

- [ ] **Step 1: Write the failing tests.** At least: `a_grid_wraps_at_the_maximum_columns` (7 entries, 5 columns: 2 rows, the sixth cell under the first); `fewer_entries_than_the_maximum_make_one_row`; `the_popup_is_centred_on_the_output`; `cells_do_not_overlap_and_lie_inside_the_popup`; `thumbnails_shrink_when_there_are_many_windows` (30 entries on 1920 × 1080 have smaller cells than 3 entries, and all fit in nine tenths); `sixty_windows_on_a_small_output_keep_positive_cells_inside_it` (800 × 600); `an_icons_grid_has_cells_for_the_icon_size`; `left_and_right_wrap_at_the_ends_of_the_list`; `up_and_down_keep_the_column_and_wrap`; `down_into_a_short_last_row_goes_to_its_last_entry`; `a_point_is_in_the_cell_it_is_over_and_in_none_between_cells`; `a_thumbnail_keeps_its_aspect_and_is_never_enlarged` (a 100 × 50 source stays 100 × 50, centred; a 1920 × 1080 source fits the content box with its aspect within a pixel); and one insta snapshot of a 7-entry grid's rectangles as text, so that a change to a number shows as a diff.

- [ ] **Step 2: Run them and see them fail.** `dbus-run-session -- cargo test -p caminito switcher::layout`.

- [ ] **Step 3: Implement.** `#[expect(dead_code, reason = "…")]` is right for what later tasks of this plan use first; say which task in the reason.

- [ ] **Step 4: Run the tests, lint, commit.**

---

### Task 4: Keys, settings and the debug dump

The compositor reads the switcher's keys of `org.mate.Marco.general`, binds the `switch-*` actions, and runs the keyboard while the switcher is open. After this task the switcher works with no popup drawn.

**Files:**
- Modify: `caminito/src/settings/translate.rs` and `caminito/src/settings/translate/tests.rs` (the keys)
- Modify: `caminito/src/input/bindings.rs` (the bindings)
- Modify: `caminito/src/input/mod.rs` (the keyboard path)
- Modify: `caminito/src/switcher/mod.rs` (the compositor's state for the open switcher)
- Modify: `caminito/src/state.rs` (the field on `Caminito`; the refresh pass; passing new settings to policy already happens through `settings.policy`)
- Modify: `caminito/src/debug.rs` (the dump)
- Modify: `caminito/src/tests/fixture.rs` (key constants that are missing: grave is 41, Enter 28, Escape 1, Up 103, Down 108; follow the constants that are there)
- Test: `caminito/src/tests/switcher.rs` (new; add to `tests/mod.rs`)
- Modify: `caminito/src/tests/bindings.rs` and other tests that need the new behaviour

**Interfaces:**
- Consumes: Task 2's `Action::Switch { list, backward }`, `Config::alt_tab_raise_windows`, `Policy::switcher() -> Option<&Switcher>` (`seat`, `list`, `windows`, `selected`), `switcher_step(bool)`, `switcher_select(usize)`, `switcher_commit(now)`, `switcher_cancel(now)`. Task 3's `Grid::new(count, max_columns, output_size, style)`, `Grid::neighbour(from, Direction)`, `CellStyle`.
- Produces:

```rust
// settings/translate.rs, a new field `pub switcher: SwitcherSettings` on `Settings`:
/// What the Window switcher is drawn with (108), from `org.mate.Marco.general`.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct SwitcherSettings {
    /// `alt-tab-max-columns`, 1 to 64.
    pub max_columns: usize,
    /// Whether cells show thumbnails: `compositing-manager` on and
    /// `compositing-fast-alt-tab` off. Otherwise they show icons alone.
    pub thumbnails: bool,
    /// `icon-size`, 8 to 256: the icons of the icons-only grid, in logical pixels.
    pub icon_size: i32,
}
// and `settings.policy.alt_tab_raise_windows` from `alt-tab-raise-windows`.

// switcher/mod.rs
/// The compositor's side of the open Window switcher.
pub(crate) struct OpenSwitcher { /* the modifiers that hold it open, the action's key,
    and from Task 5 the popup's timer, output and buffers */ }
// state.rs: `pub(crate) switcher: Option<OpenSwitcher>` on `Caminito`.
impl Caminito {
    /// The grid the open switcher is laid out in, on the output it's shown on or would
    /// be: the one the pointer is on, else the primary output.
    pub(crate) fn switcher_grid(&self) -> Option<Grid>;
}
```

The debug dump gets a `switcher` member, `null` while it's closed:

```json
"switcher": { "list": "windows", "windows": [3, 2, 1], "selected": 2, "visible": false }
```

`list` is `windows`, `windows-all`, `group` or `panels`; `windows` are ids in order; `selected` is the selected window's id; `visible` is false until Task 5.

**The rules:**

1. *Settings.* Translate the four keys with the helpers the file already has for a ranged integer and a boolean (see how `num-workspaces` and its neighbours are read and reported out of range). Tests in `translate/tests.rs` in the style there: defaults, each key set, out-of-range values clamped and reported.
2. *Bindings.* Alt+Tab `Switch { Windows, backward: false }`, Alt+grave `Switch { Group, … }`, Ctrl+Alt+Tab `Switch { Panels, … }`, and each with Shift held as well with `backward: true`. `switch-windows-all` is unbound. Check which keysym the keyboard path looks a Shift+Tab up by (`raw_latin_sym_or_raw_current_sym`) with a test, not by assumption.
3. *Opening.* When a `Switch` binding fires with the switcher closed: if Caminito holds the pointer for a move or resize (`window_grab`), do nothing. Otherwise first dismiss a client's open menu: an xdg popup grab on the keyboard and pointer, and an Xwayland keyboard grab (see `handlers/xdg_shell.rs` near `PopupKeyboardGrab`, `handlers/input.rs::dismiss_popup`, and `Caminito::xwayland_keyboard_grab`). Then call `policy.action`. If policy's switcher is now open, remember in `Caminito::switcher` the modifiers the binding names without Shift (Alt, or Ctrl+Alt) and its key. Remove Task 2's stand-in `switcher_commit` call.
4. *While it's open,* every key event is looked at before the shortcuts-inhibitor check, Caminito's other bindings and the accelerator grabs, and after VT switching:
   - a press of the action's key, or of Tab: step, backward with Shift held;
   - a press of an arrow key: `policy.switcher_select(grid.neighbour(selected, direction))`;
   - a press of Return or KP_Enter: commit; of Escape: cancel;
   - any other press does nothing;
   - every one of those presses is swallowed with its release (`suppressed_keys`), so no client sees it;
   - a release after which the modifiers that hold it open aren't all down: commit, and the release goes on to the client, which saw the press.
5. *Staying in step with policy.* Policy can close its switcher itself (rulings 5 and 9 of Task 2, a window closing). In the refresh pass, drop `Caminito::switcher` when `policy.switcher()` is `None`. After every change, `queue_redraw_all`.
6. *The dump* is built from policy's switcher and `Caminito::switcher` on demand.

- [ ] **Step 1: Write the failing settings tests, see them fail, implement the settings, see them pass.**

- [ ] **Step 2: Write the failing headless tests** in `caminito/src/tests/switcher.rs`. Read the switcher through the dump (parse `state_dump()` as `tests/debug.rs` or `tests/xwayland_windows.rs` do) and the windows through the fixture's queries. A start:

```rust
//! The Window switcher (108): the keys that open, step, commit and cancel it, the pointer
//! in it, and what's drawn.

use super::fixture::{Fixture, KEY_LEFTALT, KEY_TAB, Kind, WindowSpec, on_both_kinds};
use crate::state::SEAT;

on_both_kinds! {
    fn a_quick_alt_tab_switches_to_the_window_used_before(kind) {
        let (mut f, id) = Fixture::connected(kind);
        let a = f.map(id, WindowSpec::plain((400, 300)));
        let b = f.map(id, WindowSpec::plain((400, 300)));
        assert_eq!(f.state.caminito.policy.focus(SEAT), f.policy_id(b));

        f.chord(&[KEY_LEFTALT, KEY_TAB]);

        assert_eq!(f.state.caminito.policy.focus(SEAT), f.policy_id(a));
        assert!(f.state.caminito.policy.switcher().is_none());
    }
}
```

  Check `on_both_kinds!`'s real shape and `Fixture::connected` in `tests/fixture.rs` before you copy this. Also write: `holding_alt_keeps_the_switcher_open_and_tab_steps_it` (press Alt, press and release Tab twice: the dump's `selected` moves, the focus doesn't; release Alt: the selected window has the focus); `shift_tab_steps_backward`; `the_arrow_keys_move_in_the_grid` (7 windows and `alt-tab-max-columns` 3 through `Fixture::with_settings`: Down from the second entry selects the fifth); `enter_commits` and `escape_cancels_and_leaves_focus_and_stacking_as_they_were`; `a_minimised_window_is_restored_on_commit`; `alt_grave_lists_the_focused_applications_windows` (two clients, or two processes with `connect_as_process`); `ctrl_alt_tab_lists_docks_and_the_desktop` (X11 only, and say why: only X11 has those window types as managed windows); `nothing_opens_during_a_move` (begin an Alt+left-button drag, then Tab: the dump's `switcher` is `null`); `a_clients_open_menu_is_dismissed_first` (xdg only: a popup with a grab; see `tests/popups.rs` and `tests/grabs.rs`); `keys_pressed_while_it_is_open_reach_no_client_and_alt_s_release_does` (xdg only, an insta snapshot of `format_input`, as `tests/bindings.rs` does); `releasing_alt_while_another_key_is_down_commits` and `the_host_taking_the_keyboard_commits` (`State::release_pressed_keys`) for Review Focus 1; `an_inhibitor_does_not_trap_the_keyboard_in_an_open_switcher` (the switcher never opens under an active inhibitor, as bindings pass to the client).

- [ ] **Step 3: Run them and see them fail.** `dbus-run-session -- cargo test -p caminito switcher`.

- [ ] **Step 4: Implement** the bindings, `OpenSwitcher`, the keyboard path, the refresh hook and the dump. Keep the switcher's key handling in `switcher/mod.rs` as methods the keyboard path calls; `input/mod.rs` is 737 lines already and should gain only the calls.

- [ ] **Step 5: Fix the tests the change touches.** `tests/bindings.rs::alt_tab_focuses_the_previously_focused_window` still holds as a quick Alt+Tab; keep or fold it into the new module. Update the debug dump's snapshots.

- [ ] **Step 6: Run the whole `caminito` suite, lint, commit.**

---

### Task 5: The popup: when and where it shows, the pointer, and its chrome

The popup becomes visible 90 ms after the key, on the pointer's output. Caminito draws its background, the selected cell, the icons, the urgent mark and the title. A click on a cell commits that window. Thumbnails come in Task 6: in thumbnails mode this task leaves each content box empty but for its corner icon.

**Files:**
- Modify: `caminito/src/switcher/mod.rs` (the timer, the output, the pointer)
- Create: `caminito/src/switcher/draw.rs` (cairo and pango: the chrome's buffers)
- Modify: `caminito/src/scene.rs` (a `Switcher` layer)
- Modify: `caminito/src/input/mod.rs` (`on_pointer_button`, and `Caminito::under` for the new layer)
- Modify: `caminito/src/decoration/mod.rs` (make the icon loader reachable: `window_icon` and `IconSource` are what frames load icons with)
- Modify: `caminito/src/window/mod.rs` (a way to get a window's icon at a size, if `Mapped` needs one)
- Modify: `caminito/src/debug.rs` (more of the dump)
- Modify: `caminito/src/output.rs` or where an output is removed (Review Focus 3)
- Test: `caminito/src/tests/switcher.rs`, a `tests` module in `draw.rs` if pure pieces need one

**Interfaces:**
- Consumes: Task 3's `Grid` (`popup`, `cells`, `title`, `columns`, `style`, `cell_at`, `content`), `CellStyle`. Task 4's `OpenSwitcher`, `Caminito::switcher`, `Caminito::switcher_grid()`, `SwitcherSettings { max_columns, thumbnails, icon_size }`. Policy's `switcher()`, `switcher_select`, `switcher_commit`, `switcher_cancel`.
- Produces:

```rust
// switcher/mod.rs
/// How long after the key the popup appears, so a quick Alt+Tab shows nothing (108).
pub(crate) const POPUP_DELAY: Duration = Duration::from_millis(90);

impl Caminito {
    /// The output the visible popup is on, and its grid; `None` while the switcher is
    /// closed or its popup hasn't appeared.
    pub(crate) fn visible_switcher(&self) -> Option<(&Output, &Grid)>;
}

// scene.rs: `SceneLayer::Switcher`, between `Lock` and `Overlay` in `LAYERS`.

// switcher/draw.rs
/// The popup's chrome for one output, as buffers at the output's scale: `under` is behind
/// the thumbnails (background, selected cell, urgent marks, title, and the icons of an
/// icons-only grid), `over` is in front of them (the thumbnails' corner icons).
pub(crate) struct Chrome { pub under: MemoryRenderBuffer, pub over: Option<MemoryRenderBuffer> }
```

The dump's `switcher` gains `visible` (now true once shown), `output` (the output's name, or `null`), `style` (`thumbnails` or `icons`), `columns`, `popup` and `cells` (rectangles from the output's corner, in the dump's rectangle shape).

**The rules:**

1. *The delay.* When the switcher opens, start a calloop `Timer` of `POPUP_DELAY` (see `input/repeat.rs` for how a timer is kept and removed). When it fires and the switcher is still open, the popup is visible on the output the pointer is on then (else the primary output), and that output is redrawn. Closing the switcher removes the timer. The output doesn't follow the pointer afterwards.
2. *The scene.* `SceneLayer::Switcher` draws on the popup's output only: `over`, then (Task 6) the thumbnails, then `under`, front to back. Everything that matches on `SceneLayer` handles the new layer; nothing in it takes the pointer for a client.
3. *The chrome* (`draw.rs`), in logical pixels scaled by the buffer's scale. Use the integer scale frames are drawn at for that output (see how `Mapped`'s `frame_scale` is chosen and how `scene` places a `MemoryRenderBuffer` at a fractional output scale). Colours from `caminito.palette.palette().variant(Variant::Default)`:
   - the popup: filled with `background(GtkState::Normal)`, with a border one logical pixel wide in `text(GtkState::Normal)` at 30 % alpha;
   - the selected cell: filled with `background(GtkState::Selected)`;
   - an urgent window's cell (`Mapped::is_urgent`): ruling 3's dot;
   - icons: in an icons-only grid, each window's icon fills its content box; in a thumbnails grid, the corner icon of ruling 1 at the content box's bottom right, in `over`. A window with no icon gets `icons::FALLBACK_ICON`, as frames do;
   - the title: the selected window's `Mapped::title()`, in `settings.interface.font_name`, with the font rendering frames use (`decoration::new_title_context` and `FrameContext` show how), centred in `grid.title`, on one line, ellipsised at its end, in `text(GtkState::Normal)` (Review Focus 5: an empty title draws nothing; no text leaves `grid.title`).
   - Draw again only when an input changed: the list, the selection, a title, an urgent flag, an icon, the grid, the scale, the palette or the font. Keep the buffers on `OpenSwitcher`.
4. *The pointer.* While policy's switcher is open, a button press never reaches a client, and its release is swallowed too. A press in a cell of a visible popup selects that entry and commits. Any other press cancels (ruling 8). Motion changes no selection.
5. *The output going away* (Review Focus 3). If the popup's output is removed or disabled while it shows, move the popup to the output the pointer is on, else the primary output; if there's none, cancel the switcher.
6. A change that alters the grid (a listed window closes, a setting changes) lays it out again and redraws.

- [ ] **Step 1: Write the failing headless tests.** Pixels come from `f.state.backend.headless_mut().pixel(&output, at)`, colours through `crate::backend::headless::argb8888` or the palette's own values; `tests/frames.rs` and `tests/decoration.rs` show both. Write: `a_quick_alt_tab_draws_no_popup` (after the chord the scene has no element in `SceneLayer::Switcher`; use `State::scene_layers`); `holding_alt_draws_the_popup_on_the_pointers_output` (two outputs, the pointer on the second; press Alt and Tab, then `dispatch_until` the dump says `visible`; its `output` is the second's name and only that output has `Switcher` elements); `the_popup_is_not_drawn_before_its_delay` (straight after the press it isn't visible; this is the one place a fixed short wait may be right, if you need one, with its comment); `a_click_on_a_cell_commits_that_window` (the cell's centre from the dump, plus the output's place); `a_click_outside_the_cells_cancels`; `hovering_a_cell_selects_nothing`; `no_click_reaches_a_client_while_it_is_open` (xdg only, the client's input log); and the drawing tests the ticket asks for: `the_grid_wraps_at_alt_tab_max_columns` (the dump's `columns` and `cells`), `thumbnails_shrink_with_many_windows` (cells smaller with 20 windows than with 3), `the_selected_cell_is_drawn_in_the_selection_colour` (a pixel just inside the selected cell's corner against one in another cell), `with_compositing_off_the_grid_shows_icons` and `with_fast_alt_tab_the_grid_shows_icons` (the dump's `style`, and the pixel at a cell's centre is the test icon's colour; `Fixture::frame_icon_color` shows where a known icon comes from), `an_urgent_window_is_marked` (X11 only: `_NET_WM_STATE_DEMANDS_ATTENTION`, as `tests/xwayland_windows.rs::a_window_demanding_attention_is_urgent`; the dot's pixel differs from the same place in a cell that isn't urgent), `a_very_long_title_stays_inside_the_popup` and `an_empty_or_non_latin_title_draws` (Review Focus 5: no pixel outside `popup` differs from the same scene with a short title; with an empty and with a non-Latin title the popup still draws), `unplugging_the_popups_output_moves_the_popup` and `…_with_no_output_left_cancels` (Review Focus 3).

- [ ] **Step 2: Run them and see them fail.**

- [ ] **Step 3: Implement** the timer and the output, then the pointer, then the scene layer and `draw.rs`.

- [ ] **Step 4: Run the whole `caminito` suite, lint, commit.** Two commits are fine if they stand alone: when and where it shows with the pointer, then the drawing.

---

### Task 6: Live thumbnails

Each cell of a thumbnails grid shows its window, drawn live from the window's own surfaces at a smaller size.

**Files:**
- Modify: `caminito/src/scene.rs` (the thumbnails' elements in the `Switcher` layer; `SceneElement`'s new variants)
- Modify: `caminito/src/handlers/compositor.rs` or where a window's commit queues a redraw (a listed window's commit redraws the popup's output)
- Modify: `caminito/src/backend/*.rs` only if a renderer's bounds need the new element types
- Test: `caminito/src/tests/switcher.rs`

**Interfaces:**
- Consumes: Task 5's `Caminito::visible_switcher() -> Option<(&Output, &Grid)>`, `SceneLayer::Switcher`, `Chrome { under, over }`; Task 3's `Grid::thumbnail(index, source)` and `Grid::content(index)`; `scene::one_window_elements` (private to `scene.rs`), `Policy::frame_rect(id)` and `Policy::geometry(id)`, as `scene::window_capture_elements` uses them.
- Produces: nothing later tasks use in code.

**The rules:**

1. For each entry of a visible thumbnails grid, the window is drawn as a capture of it is (ruling 4): its surfaces over its Frame's strips, from the top left of policy's frame rect, a shaded window as its titlebar alone. `window_capture_elements` is the model; do not paint a cursor.
2. It's scaled to `grid.thumbnail(index, frame_rect.size)`, placed there on the popup's output, and nothing of it shows outside that rectangle (a window's popups and subsurfaces can reach past its frame rect). Smithay has `RescaleRenderElement`, `RelocateRenderElement` and `CropRenderElement` in `smithay::backend::renderer::element::utils` for this; niri and cosmic-comp wrap elements the same way, but both are GPL-3: read Smithay's own docs and anvil, not their code (03).
3. A window that isn't showing (minimised, or on another workspace) is drawn the same way, from the buffers its surfaces still hold: that is its last content.
4. An icons-only grid (`SwitcherSettings::thumbnails` false) draws no thumbnail.
5. *Live.* While the popup is visible, a commit on any surface of a listed window queues a redraw of the popup's output, whether or not the window shows there. Check that a showing window on another output still gets its frame callbacks from its own main output, and that nothing sends a minimised window frame callbacks because of the popup.
6. The layer order within `SceneLayer::Switcher`, front to back: `over`, the thumbnails, `under`.

- [ ] **Step 1: Write the failing headless tests.** A test client's window is one colour (`WINDOW_COLOR` in `tests/fixture.rs`; `ack_and_commit_buffer` takes another). Write: `a_cell_shows_its_windows_content` (CSD windows so the content fills the thumbnail: the pixel at the centre of `grid.thumbnail` is the window's colour; run on both kinds); `a_thumbnail_follows_its_window_live` (xdg only, and say why: the test client commits a buffer of another colour while the popup shows, and after a dispatch the centre pixel is the new colour, with no key pressed between); `a_minimised_window_shows_its_last_content`; `a_framed_windows_thumbnail_has_its_frame` (an SSD window: a pixel near the thumbnail's top edge isn't the window's colour nor the cell's background); `nothing_of_a_thumbnail_is_drawn_outside_its_cell` (a window much wider than high: pixels of the content box above and below the thumbnail are the cell's colour); `thumbnails_are_right_on_a_scaled_output` (scale 2 and scale 1.5: the centre pixel, at physical coordinates); `the_icons_only_grid_draws_no_thumbnail` (extend Task 5's icon tests: no pixel of the cell is the window's colour).

- [ ] **Step 2: Run them and see them fail.**

- [ ] **Step 3: Implement.**

- [ ] **Step 4: Run the whole `caminito` suite, lint, commit.** If the renderer on the headless backend (pixman) can't draw a scaled element, stop and report BLOCKED with what you saw; do not swap the test for a weaker one.

---

### Task 7: The parity table, the checklist and the ticket

**Files:**
- Create: `docs/parity.md`
- Modify: `docs/testing/checklist.md` (one item, tagged M8)
- Modify: `.scratch/issues/108-window-switcher.md` (criteria, notes, what was done)
- Modify: `.scratch/caminito-v0.1/map.md` (M8's line)
- Modify: `caminito-policy/src/action.rs` and other comments that still say Alt+Tab has no popup until M8 (search the tree for `M8` near `Alt+Tab`, `switch-windows` and `popup`), `README.md` if it describes Alt+Tab
- Modify: `data/keybindings/50-caminito-global-key.xml` only if it's wrong now (it lists `switch-windows`, `switch-group`, `switch-panels` and the `cycle-*` actions already)

**Interfaces:**
- Consumes: what Tasks 1 to 6 built. Read their commits (`git log --stat master..HEAD`) before you write.
- Produces: no code.

**What to write:**

1. **`docs/parity.md`.** It doesn't exist; M8's audit fills it later (17). Start it with a short introduction (what the table is: each Marco feature and key with Caminito's status, per ticket 17's M8 exit; read `.scratch/issues/17-milestone-slicing.md` and 05 and 09 for the words they use for a status, and use the same ones), and the rows ticket 108 owns, each with its status and, where it's *never*, ADR 0016 as the reason:
   - the actions `switch-windows`, `switch-windows-backward`, `switch-windows-all`, `switch-windows-all-backward`, `switch-group`, `switch-group-backward`, `switch-panels`, `switch-panels-backward` (say which are bound by default, and that Shift steps backward);
   - the keys `alt-tab-max-columns`, `alt-tab-raise-windows`, `compositing-manager`, `compositing-fast-alt-tab`, `icon-size`, `alt-tab-expand-to-fit-title` (never), `show-tab-border` (never).
   Say only what the code does. `cycle-*` aren't this ticket's: leave them out, or mark them as not yet done if 17's words need a row.
2. **The checklist item,** in the shape the file's items have (Milestone, Steps, Expected, Debug tool), tagged M8: live thumbnails of a playing video and of a minimised window, in a Session, on the tty and from the display manager as the file's other Session items say. Include the keys to try (Tab, Shift+Tab, arrows, Enter, Escape, a click) and `org.mate.Caminito.Debug`'s state dump as the debug tool.
3. **The ticket.** Tick each acceptance criterion that's met, with the same honesty as 104's: the "while locked" half of the headless criterion isn't met and says why (ruling 5), and the checklist criterion's item is written but its run in a Session is the developer's. Add a `## What was done` section in the style of 104's, and record each of the ten rulings of this plan's *Rulings* section under the ticket's notes as decided when the ticket was done, for the developer to confirm or change: the three the ticket named as not settled first. Leave `Status: open`: the checklist run and the developer's word on the rulings are still to come.
4. **The map.** Bring M8's line up to date in the words the map uses for a ticket whose code is in and whose checks are the developer's (99's wording on M6's line is the model).

- [ ] **Step 1: Read** ADR 0016, tickets 05, 09, 17, 104 and 108, and the branch's commits.
- [ ] **Step 2: Write** the four documents and fix the stale comments.
- [ ] **Step 3: Check.** Every relative link in what you wrote resolves (`ls` the target). `cargo fmt --all --check` and `cargo clippy --workspace --all-targets -- -D warnings` pass if you touched a `.rs` file; `dbus-run-session -- cargo test -p caminito keybinding_manifests` passes if you touched a manifest.
- [ ] **Step 4: Commit.**
