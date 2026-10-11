# Packaging and closing M8

Epic: caminito-v0.1
Type: implementation
Status: open: the manifests, the parity table, the README's section, the dependencies and the wizard are in, and the closing run, `tools/check`, the packages' build and the closing commit are the developer's, on their machine
Milestone: M8
Blocked by: 147, 148, 149, 150, 151, 152, 153, 154, 155, 156, 157, 158, 159, 160, 161, 162, 163, 164, 165, 166, 167, 168, 169, 170, 171, 172
Map: [Caminito v0.1](../caminito-v0.1/map.md)

## What to build

M8's exit criterion holds on the developer's machine, and the closing commit says so ([17](17-milestone-slicing.md)): every `org.mate.Marco.*` key and all 103 actions are honoured, never implemented, or on the roadmap.

- **Packaging ([16](16-packaging.md)):** `caminito-git` depends on zenity ([167](167-zenity-dialogs.md)) and on whatever plays sound ([170](170-audible-bell-and-switch-sounds.md)). Any Carried patch M8's tickets wrote (mate-panel's, for urgency or above) is in its Patched package. `pkgver` and every `pkgrel` are bumped, and all the packages are installed on the developer's machine from the built files.
- **The keybinding manifests:** the `50-caminito-*.xml` files list every action Caminito now binds, checked by their test against every action, so that mate-control-center's *Keyboard Shortcuts* shows them all.
- **The README** says what Marco parity means for v0.1 and points at `docs/parity.md`.
- **The parity table:** no row is *to do*, and the *unsupported* rows are exactly 05's *later* list and 09's roadmap, with any other moved there only with a reason beside it.
- **The closing run:** every item tagged M8, and every earlier one still open, on the installed packages, in a Session on the developer's machine, with a wizard script beside this ticket as [73](73-packaging-and-closing-m5.md) had. It includes [108](108-window-switcher.md)'s and [144](144-drag-to-unmaximize.md)'s Session checks, [101](101-mate-control-center-caminito.md)'s item with a changed shortcut now applying at once, and the pass over the parity table's rows that no test covers.
- **The closing commit,** `Close M8 Marco parity`, with the results in its message. It moves the tag of any item that's deferred, and updates the README's status and the map.

## Acceptance criteria

- [ ] `tools/check` passes on the developer's machine.
- [x] `docs/parity.md` has no row left to do, and its unsupported rows are exactly 05's and 09's roadmap; its test passes.
- [x] The manifests' test checks every action Caminito binds.
- [ ] 17's exit, run by hand in a Session: the parity table's rows that no automated test covers, each seen.
- [ ] X11 MATE and MATE on Wayfire still work with all the Patched packages installed (ADR 0011).
- [ ] Every checklist item tagged M8, and every earlier one still open, was run, and the results are in the closing commit's message.

## Notes

- **M8's tickets don't wait on [136](136-packaging-and-closing-m7.md),** M7's closing run, as 108 and 144 didn't. M7's items still open when this runs are part of it.

## Open questions

**For the developer:**

- **Patches mate-panel may need,** for urgency in the window list ([166](166-focus-stealing-prevention-for-xwayland-windows.md)) or for *Always on Top* in its menu ([155](155-above-below-and-sticky.md)): whether they're carried in `mate-panel-caminito`, or left as known gaps in the parity table.
  - **Answered on 2026-10-11:** left as known gaps, with no new Carried patch. The parity table has them on `toggle-above`'s, *Urgency*'s and `switch-panels`' rows.

## Done (2026-10-11)

The code is in; the closing run isn't. Run `173-closing-checks.sh` for it, part by part.

- **The developer's answer** to the open question is above: mate-panel gets no new patch, and its three gaps are in the parity table. The window list shows an urgent window with a bold label and doesn't flash it, its menu's *Always on Top* doesn't show the state, and Ctrl+Alt+Tab doesn't reach the panels.
- **The manifests had Marco's own entries,** so *Keyboard Shortcuts* didn't show 21 actions Caminito binds: `toggle-above`, `switch-windows-all`, the seven `-backward` actions and `run-command-1` to `-12`. The test went in first, `the_manifests_list_every_action_caminito_binds_once`, and failed on those 21; it checks both ways, against `bindable()` through `bound_keys()`.
- **`run-command-area-screenshot` is in no manifest.** It's master's, the schema Arch ships hasn't its key, and the dialog aborts on a key its schema hasn't. The test names it as the one exception.
- **`run-command-1` to `-12` are labelled "Run command N",** where the schema's summary is "Run a defined command" for all twelve. Marco's catalogue has no translation for the new text, so they show in English. For the developer to confirm.
- **The parity table has four statuses again.** *To do* left the table and its test, *To do in M8* is *Built in M8*, and two tests are new: every row has one of the four statuses, and a row is *unsupported* only under *Later*, with a reason beside it. That test checks where the rows are, not what they are: by hand, the thirteen are 05's five, 09's four, and the four that 147 moved there (`locate-pointer` and three of AccessX's), each with its reason.
- **`caminito-git` depends on `libcanberra` and `zenity`.** No `pkgrel` is bumped: no Patched package changed, and `caminito-git`'s `pkgver` is the commit, which makepkg writes at the closing run. The ticket asks for every `pkgrel` bumped; that's left to the developer, since a bump with no change only rebuilds.
- **The README has a *Marco parity* section.** Its status line and the map still say M7 is next: they change with the closing commit, and 136, M7's closing, is open too.
- **The wizard reads the checklist** rather than copying it: `items M8` is one stage for each item tagged M8 (108's and 101's among them), then 144's check, which has no item, the decisions M8's tickets left to confirm, and the pass over the parity table. `items M6 M7` is the earlier ones still open, `x11` the gate, `packages` runs `tools/check` and builds the packages, and `message` prints the results for the closing commit. They're saved in `173-checks/closing-results.md`.
- **The wizard was run only with scripted answers,** in the OrbStack machine, which has no Session and none of the packages: its `items` and `x11` parts to their end, and `message`. Its `packages` part wasn't run.
- **The tests run,** there, filtered by name and not the whole suite: `tests::parity`, `keybinding_manifests` and `input::bindings`, with clippy `-D warnings` on `caminito` and rustfmt. `tools/check` wasn't run.
- **Unmet, and the developer's:** `tools/check` on their machine, 17's exit by hand, the X11 MATE and Wayfire gate, the checklist's run, and the `Close M8 Marco parity` commit. 108, 151 and 160 are open on decisions the wizard's *decisions* stage lists.

