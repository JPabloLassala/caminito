# Marco's zenity dialogs: a failed command, and `rename-workspace`

Epic: caminito-v0.1
Type: implementation
Status: resolved: the checklist item's run in a Session is 173's closing run
Milestone: M8
Blocked by: 149
Map: [Caminito v0.1](../caminito-v0.1/map.md)

## What to build

The helper dialogs Marco shows with zenity ([05](05-v01-marco-feature-cut.md)), with the same arguments, so that they look and read as Marco's.

- **A failed command:** a `run-command-*` binding whose command is empty, or fails to start, shows Marco's error dialog with its message, in place of today's log line alone ([149](149-window-and-workspace-actions.md)).
- **`rename-workspace`:** shows Marco's zenity entry dialog with the active workspace's name, and writes the answer to `org.mate.Marco.workspace-names name-N`, which the pager then shows ([97](97-pager-count-and-names.md)). Cancelling leaves the name.
- **zenity** becomes a dependency of `caminito-git`, packaged by [173](173-packaging-and-closing-m8.md).
- **The dialogs are clients like any other:** a zenity window takes the focus as a new window does, and nothing waits for it.

## Acceptance criteria

- [x] Headless tests through the fixture, written before the code, with a stand-in for zenity on `PATH` that records its arguments and answers: an empty command and a missing program each run it with Marco's arguments; `rename-workspace` writes the answer to the right `name-N`, and nothing on cancel.
- [x] `rename-workspace`'s row in `docs/parity.md` says honoured, and the keyless section's command error dialog too, with the tests above.
- [x] The checklist has an item, tagged M8: `run-command-1` bound with no command, and `rename-workspace` bound and used, with the pager showing the new name. Running it in a Session is the developer's, and goes to [173](173-packaging-and-closing-m8.md)'s closing run.

## Open questions

**From Marco's source**, to be settled by the implementing agent and recorded in the Done section: zenity's exact arguments for each dialog (title, text, icon, the entry's initial text), the messages and their translations, and which failures Marco reports.

**For the developer**, decided when the ticket is done and listed in its Done section for confirmation:

- **zenity missing:** proposed, the failure is logged once and nothing shows, as Marco does if it does the same.

## Work in progress (2026-10-10)

- **Stopped before landing, at the developer's request.** An agent had started it, uncommitted: a new `caminito/src/dialogs.rs` and `caminito/src/tests/dialogs.rs`, edits to `commands.rs`, `input/bindings.rs`, `input/mod.rs`, `lib.rs`, `state.rs`, the commands tests, `docs/parity.md` and the checklist. It was on the docs edits when stopped; nothing was tested or reviewed, and no triage was recorded.
- **The patch:** `167-wip.patch`, removed when the ticket landed, was made with `git diff --cached --binary` against 26a54f7. It applies cleanly to the branch this note lands on.

## Done (2026-10-11)

- **The start was the patch above.** It had the tests and the code together, so no run shows the tests red before the code. As a check, the guard against a second dialog was removed and its test failed.
- **zenity's arguments, from Marco at `8522ed1`** (`core/util.c:535-717`). The error: `zenity --error --title Marco --text <text>`. The entry: `zenity --entry --title Marco --text "New Workspace Name:" --entry-text <the name> --ok-label OK --cancel-label Cancel`. Marco sets no icon. Marco also passes `--display <screen>` and `--class marco-dialog`. zenity 4.2.2 refuses the two (`This option is not available`), so Caminito does not pass them. The display is in the environment Caminito gives to what it spawns.
- **The messages** (`core/keybindings.c:2398-2445`). A command that is not empty: `There was an error running <tt>COMMAND</tt>:` then an empty line and GLib's message. An empty command: GLib's message alone, `Text was empty (or contained only whitespace)`. Marco's default of a space is not empty, so its dialog names it.
- **Which failures Marco reports.** A command line that GLib cannot parse and a program that does not start, for the twelve numbered commands, the three screenshot commands and the terminal's. Marco's `No command N has been defined.` is for a command that is NULL, which a GSettings string never is, so Caminito does not have it.
- **The text is escaped for zenity's markup, which Marco's is not.** A command with `<` or `&` in it made a dialog with bad markup. For the developer to confirm.
- **The messages are Marco's English.** Marco translates them with its `marco` catalogue. Caminito sets no locale, and "Workspace N" is in English too, so no translation is in. For the developer to confirm; a translation needs `setlocale`, which is a change for the full process.
- **`rename-workspace`** (`core/keybindings.c:3647-3789`, `core/prefs.c:2271-2306`). The answer is written without its newline to the `name-N` of the workspace that was active at the press. A cancel, an empty answer, an answer that is not UTF-8 and the name it has now write nothing. A second press while the dialog shows does nothing, as in Marco.
- **No new thread.** The patch waited for zenity on a thread. The review cited `CODING_STANDARDS.md` ("A new thread is justified only when a library needs its own loop"), so zenity's stdout is now a source on the event loop, as Marco's watch is.
- **zenity missing: for the developer to confirm.** Each press logs a warning (`can't show Marco's error dialog: …`, `can't show Marco's dialog for rename-workspace: zenity: …`) and nothing shows. Marco does the same: it logs on each press, not one time only.
- **Two faults in the tests were found and corrected.** (1) The stand-in script sometimes failed to start with `Text file busy`, about 1 run in 100: a different test's thread forked while the script was open for writing. `install` now writes the stand-ins (`write_program` in `tests/commands.rs`). (2) `a_run_command_whose_command_is_empty_runs_nothing_and_is_logged` lost its log line when a `tests::dialogs` thread was the first to log that line: tracing caches the line as off. The test now presses one time first and rebuilds the cache.
- **What was run.** In the OrbStack machine: `tests::dialogs`, `tests::commands` and `tests::parity` (21 tests), then the `bindings`, `debug`, `window_actions` and `pager` tests (74 tests), and `cargo clippy -p caminito --all-targets`. The dialogs and commands group ran more than 400 times after the two corrections, with no failure.
- **What was not run.** The full test suite, at the developer's request. The dialogs with the real zenity in a Session: that is the checklist's item *A failed command's dialog, and `rename-workspace`*, for 173's closing run.
- **The `zenity` dependency of `caminito-git` is not in the PKGBUILD yet.** That is 173's, as its Packaging bullet says.
