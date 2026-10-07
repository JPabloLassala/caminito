# Settings thread

Type: implementation
Status: resolved
Milestone: M2
Blocked by:
Map: [Caminito v0.1](../map.md)

## What to build

The `settings` module from [15](15-crate-and-module-architecture.md), with the keys M2 honours ([09](09-gsettings-mapping.md)). It replaces M1's fixed workspace count (17).

- **The GLib thread:** one thread runs a GLib main loop and owns every `GSettings` object. `State::new` waits for its first full snapshot, so the config is complete before anything maps. After that it sends typed changes to the main loop over a `calloop::channel`.
- **Pure translation:** from GSettings values to a typed config, unit-tested in the module, with no GLib in the tests.
- **Rules for every key (09):**
  - Marco's schemas missing is a start-up error with a clear message.
  - Out-of-range values are clamped, or fall back to the schema default, and logged once.
  - *Unsupported* keys are logged once per run, and only when set away from their default. The mechanism is built and tested here; its real entries arrive with the milestones that read those keys.
- **Keys read in M2:**
  - `org.mate.Marco.general`: `theme`, `titlebar-font`, `titlebar-uses-system-font`, `button-layout`, `action-double-click-titlebar`, `action-middle-click-titlebar`, `action-right-click-titlebar`, `attach-modal-dialogs`, `num-workspaces`;
  - `org.mate.Marco.workspace-names`: `name-1` to `name-36`;
  - `org.mate.interface`: `gtk-theme`, `font-name`, `icon-theme`;
  - `org.mate.font-rendering`: `antialiasing`, `hinting`, `rgba-order`;
  - `org.mate.peripherals-mouse`: `cursor-theme`, `cursor-size`, `double-click`, `drag-threshold`.
- **Workspaces in policy:** a settings-changed event carries `num-workspaces` (clamped to 1–36) and the names. When the count shrinks, windows on removed workspaces move to the new last one and the active workspace follows, as in Marco. The names are held for ext-workspace (M5) and the debug dump ([32](32-debug-interface.md)).
- **Test schemas (14):** the schemas above vendored under `tests/schemas/` with their upstream commits recorded, compiled at test time, and used with `GSETTINGS_BACKEND=memory`.

## Acceptance criteria

- [x] Translation unit tests for every key above, including clamping and the unsupported-key rule, written before the code.
- [x] Headless tests: a window maps only after the first snapshot; changing `num-workspaces` live adds and removes workspaces with windows moved as Marco does.
- [x] The proptest operations include changing the workspace count, and the four invariants still hold.
- [x] Nested, `gsettings set org.mate.Marco.general num-workspaces 6` gives six workspaces to switch through.
