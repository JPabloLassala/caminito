# The parity table, with every key and action

Epic: caminito-v0.1
Type: implementation
Status: resolved
Milestone: M8
Blocked by: none
Map: [Caminito v0.1](../caminito-v0.1/map.md)

## What to build

M8's audit ([17](17-milestone-slicing.md)): `docs/parity.md` gets a row for every key and every action of Marco's five `org.mate.Marco.*` schemas, and each row says what Caminito does with it and what covers it. Today the file has only the Window switcher's rows ([108](108-window-switcher.md)). Every other M8 ticket fills in its own rows, and M8 closes when none is left to do ([173](173-packaging-and-closing-m8.md)).

- **Where the list comes from.** [01](01-marco-feature-inventory.md)'s full inventory is on a branch that isn't in the repo, so the list is taken from the schemas themselves: Marco 1.29.0's, vendored for the tests, which Arch ships, plus the five keys Marco's master adds to it:
  - `dynamic-workspaces` in `general`;
  - `alt-tab-minimized-placement` (default `end`) and `alt-tab-urgent-placement` (default `start`) in `general`;
  - `run-command-area-screenshot` in `global-keybindings`;
  - `command-area-screenshot` in `keybinding-commands`.

  With them that's 40 keys in `general`, 36 in `workspace-names`, 15 in `keybinding-commands`, and 103 actions: 51 in `global-keybindings` and 52 in `window-keybindings`. The settings translation already reads a master key where the installed schema has it, as it does `command-area-screenshot` ([104](104-screenshots.md)), and the same rule covers the other four.
- **One row for each.** The numbered families (`switch-to-workspace-1` to `-12`, `move-to-workspace-1` to `-12`, `run-command-1` to `-12`, `command-1` to `-12`, `name-1` to `-36`) may share a row, as long as the row names the range and the test below counts every member.
- **Rows already settled** are filled in with their status and covering test, from the tickets that settled them:
  - the `switch-*` actions and the switcher's keys ([108](108-window-switcher.md), already in);
  - `show-desktop` ([94](94-show-desktop.md)), `panel-main-menu` and `panel-run-dialog` ([95](95-alt-f1-and-alt-f2.md)), the three screenshot commands and their bindings ([104](104-screenshots.md));
  - `num-workspaces` and the workspace names from M2 and [97](97-pager-count-and-names.md), and the workspace grid's left and right ([96](96-workspace-grid.md));
  - `close` and `toggle-maximized`, M1's;
  - `theme`, `titlebar-font`, `titlebar-uses-system-font`, `button-layout` and the three click actions on the titlebar, M2's;
  - the bell keys as [105](105-one-bell-event.md) left them: read, and not yet acted on;
  - the grabs that mate-settings-daemon's keybindings take through Caminito ([98](98-accelerator-grabs.md)) where a row needs them.
- **The rest say "to do"** and name the ticket, 148 to 172, that settles them. A row names its ticket even where that ticket may decide on *never*.
- **The never rows** with a key are 05's ([05](05-v01-marco-feature-cut.md)) and 108's, each with its reason: `application-based` and `disable-workarounds`, and `alt-tab-expand-to-fit-title` and `show-tab-border` (already in, ADR 0016).
- **A second section, with no keys.** Most of 05's and 09's roadmap has no key of Marco's, so it gets its own section, listing what Caminito won't do or does later and why:
  - never: `run-command-13` to `-32` and `set-spew-mark`, which aren't in the schema, wireframe moves and resizes, the empty close and unminimise effect hooks, the command-line options, marco-message and the `_MARCO_*` messages, the `_NET_DESKTOP_GEOMETRY` and `_NET_DESKTOP_VIEWPORT` stubs, the theme viewer and test programs, `org.mate.interface accessibility` and `window-scaling-factor-qt-sync` ([09](09-gsettings-mapping.md));
  - later: XWayland's `_NET_WM_WINDOW_OPACITY`, size increments and the resize popup, `_NET_WM_FULLSCREEN_MONITORS`, X11 startup-notification, session restore, `org.mate.peripherals-keyboard-xkb` `group-per-window` and `default-group`, `org.mate.accessibility-keyboard` `mousekeys-*`, and `org.mate.typing-break enabled`.
- **`UNSUPPORTED`** gains every M8 key whose default Caminito doesn't match yet, so that a user who has changed it from its default sees the log line until its ticket honours it ([09](09-gsettings-mapping.md)'s rule). A key whose default Caminito already matches (an effect that is off by default, a binding nothing reaches yet) isn't added. Each ticket from 148 on takes its keys out again.
- **A test** reads the vendored schema, adds master's five keys, and fails if a key or action has no row in the table. A second checks that every key in `UNSUPPORTED` has an *unsupported* or *to do* row.

## Acceptance criteria

- [x] The two tests above, written before the table is filled, fail on today's file and pass on the new one.
- [x] `docs/parity.md` has a row for every key and action of the five schemas, master's five included, each with a status and, for every row that's settled, the test or checklist item that covers it.
- [x] Every *to do* row names one of 148 to 172, and every one of 148 to 172 is named by at least one row, or its notes say why none.
- [x] The keyless section lists 05's *never* and *later* items and 09's unsupported keys, each with its reason.
- [x] `UNSUPPORTED` has the M8 keys whose defaults Caminito doesn't match yet, each with a comment naming its ticket, and the settings translation's tests still pass.
- [x] The checklist has an item, tagged M8: a pass over the table's rows that no automated test covers ([17](17-milestone-slicing.md)'s M8 checklist). It's run by [173](173-packaging-and-closing-m8.md), as its last step.

## Notes

- **The keys of other schemas Marco read** (`org.mate.interface enable-animations`, `org.mate.sound`'s three, `org.mate.peripherals-mouse`'s `double-click` and `drag-threshold`) have rows too, in a section of their own: 09 lists them under *Foreign keys Marco read*.
- **`org.mate.accessibility-keyboard`'s keys** are 13's and [134](134-accessx.md)'s, not Marco's, and aren't in the table unless the developer puts them in M8 (below).

## Open questions

**For the developer:**

- **Which schema.** Re-vendor Marco master's schema, so the tests see the five keys as Marco does, or keep 1.29.0 as Arch ships it and read master's keys where the installed schema has them. The proposal is the second: the tests are against what the developer's machine installs, and the table's test adds the five by name.
- **Whether *to do* is a status.** 17 has four statuses. The proposal is that *to do* is allowed until [173](173-packaging-and-closing-m8.md), which checks none is left, so that the table is whole from this ticket on.
- **`locate-pointer`** (Caminito draws the ripple itself, [53](53-input-settings-translation.md) sent it to this audit), and AccessX's `timeout-enable`, `capslock-beep-enable` and `feature-state-change-beep` ([134](134-accessx.md) left them unsupported): M8's, with a ticket of their own after 172, or later, with a row in the keyless section. They're in `UNSUPPORTED` today.

## Comments

**Triage (2026-10-10), decided by the shipping agent without the developer, for the developer to confirm:**

- **Which schema:** the proposal. 1.29.0 stays vendored as Arch ships it, and the table's test adds master's five keys by name.
- **Whether *to do* is a status:** the proposal. *To do* is allowed until 173, which checks none is left.
- **`locate-pointer`, `timeout-enable`, `capslock-beep-enable` and `feature-state-change-beep`:** later, each with a row in the keyless section and its reason, and still in `UNSUPPORTED`, so they're logged. No question marked a recommendation; this one writes no new ticket and leaves M8's scope as its tickets have it. The developer can bring any of them into M8 with a ticket after 172.
- **Which keys `UNSUPPORTED` gains:** by the purpose the ticket gives, so that a user who has changed a key from its default sees the log line. The ticket's "whose default Caminito doesn't match yet" reads the other way round from that purpose and from the entries already there (`visual-bell`, `locate-pointer`, `mousekeys-enable`, all off by default and matched off), and from 150, 165 and 169, which expect `wrap-style`, `focus-new-windows`, `alt-tab-urgent-placement` and `visual-bell` there. So a key is added when a value a user can set is one Caminito doesn't do, and left out when the value away from the default is what Caminito already does (`audible-bell`, `center-new-windows`, the three tiling keys, `reduced-resources`) or when it only matters with another key set.

## Done (2026-10-10)

- **Tests first:** `caminito/src/tests/parity.rs`, beside the keybinding manifests' test, whose reader of the vendored schema it shares. Both tests failed on the old table (keys with no row; `locate-pointer` unsupported with no row) and pass on the new one. The first also fails on a duplicate row or a key the schema doesn't have, and on a *to do* row that names none of 148 to 172.
- **The table's shape:** one section per Marco schema, the Window switcher's rows folded into `general` and `global-keybindings`; a section for the foreign keys (`org.mate.interface enable-animations`, `org.mate.sound`'s three, `double-click` and `drag-threshold`); and *What has no key of Marco's*, with *Never*, *Later* and *To do in M8* tables. A row's schema is its heading's, or written as `` `schema` `key` `` in the keyless tables; a family is `` `name-1` to `-36` ``, a wildcard `` `mousekeys-*` ``.
- **Honoured actions** keep Marco's default binding, hard-coded until 148, as 108's rows said; the rows say so and name 148. `compositing-manager` stays honoured: off already turns off everything Caminito has, and 171 and 172 must keep it so.
- **Tickets with no key of their own** (152, 157, 168, 171) are named by rows of *To do in M8*; 166 by the X11 startup-notification row. `alt-tab-minimized-placement` was named by no M8 ticket, so it went to 163 (the lists `switch-*` and `cycle-*` share), with a note added to 163.
- **`UNSUPPORTED`** gained 14 `general` keys, by the triage above: `focus-mode`, `raise-on-click`, `auto-raise`, `focus-new-windows`, `alt-tab-urgent-placement`, `placement-mode`, `wrap-style`, `dynamic-workspaces`, `mouse-button-modifier`, `resize-with-right-button`, both `action-scroll-*-titlebar`, `alt-tab-minimized-placement` and `show-desktop-skip-list`. The bindings' schemas aren't read, so no action could be there.
- **Not run here:** the cloud container has Ubuntu's Xwayland 23.2, which doesn't start under the fixture, so the 238 X11-tier tests failed at "Xwayland starts", which nothing here touches; every other test of the workspace passed, the translation's included. Clippy on this toolchain fails on three `manual_range_contains` lints in caminito-theme, untouched here; `caminito` is clean with that lint allowed. `tools/check` on the developer's machine, and the checklist item, are 173's.
