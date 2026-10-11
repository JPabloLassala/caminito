# Policy keeps its `Config`, and stops copying it field by field

Epic: caminito-v0.1
Type: implementation
Status: ready-for-agent
Map: [Caminito v0.1](../caminito-v0.1/map.md)

## Why

`Policy` (`caminito-policy/src/policy.rs`) has 14 fields that are copies of `Config`'s values: `wrap_style`, `allow_tiling`, `drag_threshold`, `focus_new_windows` and the rest. `Policy::new` gives each a placeholder, and `settings_changed` then writes over all of them, one `self.x = config.x` line each. `PointerFocus` (`policy/pointer.rs`) copies three more in its own `settings_changed`.

So a new setting is five edits in `policy.rs`: the `Config` field, `Config::default`, the `Policy` field, its placeholder in `Policy::new`, and its line in `settings_changed`. A forgotten last line compiles, and gives a setting that's read and never applied. The placeholders read as a second set of defaults (`allow_top_tiling` is `false` there and `true` in `Config::default`), though none of them survives `Policy::new`.

This was decided in an architecture review on 2026-10-11.

## What to build

- **`Policy` holds one `config: Config`,** and its code reads the settings from it. The 14 mirror fields go, with their placeholders in `Policy::new`.
- **`settings_changed` stores the config and does only its reactions:** the workspace count unless `dynamic-workspaces` is on, the workspaces' names, and `apply()`.
- **What stays Policy's own:** `workspace_count`, which is clamped and which Policy owns under `dynamic-workspaces`, and `workspace_names`, which come from the configured names and the count. `configured_names` goes: it's `config.workspace_names`.
- **`PointerFocus` stops copying too.** Its `mode`, `raise_on_click` and `auto_raise` fields go, and its methods are given what they need of the config by `Policy`. Its `settings_changed` keeps only its reaction: a pending auto-raise is dropped when the new settings turn it off.
- **The interface doesn't change.** `Policy::new(config)` and `settings_changed(config)` are as they were, so no test and no line of the compositor changes.

Left alone: `Config::default`. It's the tests' config, since the compositor fills every field in `settings/translate.rs`, and it differs from Marco's defaults in two fields for the tests written before 154 and 158. Making it Marco's is another change.

[177](177-policy-owns-a-windows-functions.md) adds `attach-modal-dialogs` to `Config`. Neither ticket blocks the other; the one that merges second follows the first.

## Acceptance criteria

- [ ] `settings_changed` in `policy.rs` and in `policy/pointer.rs` has no line of the form `self.x = config.x`.
- [ ] `Policy` has no field that holds a `Config` value but `config` itself.
- [ ] `cargo test -p caminito-policy` passes with no test changed.
- [ ] The compositor's settings tests (`settings::`) and `tests::focus_modes` pass with no test changed.
- [ ] `tools/check` passes, or the Done section lists what was run and what was not.

No new test: the change removes the place the bug could be in, and the tests there are show the behaviour is the same.

## For the implementing agent to settle

Recorded in the Done section: how `PointerFocus`'s methods get the config, as an argument from `Policy` or otherwise.
