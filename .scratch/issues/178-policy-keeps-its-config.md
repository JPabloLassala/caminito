# Policy keeps its `Config`, and stops copying it field by field

Epic: caminito-v0.1
Type: implementation
Status: resolved
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

- [x] `settings_changed` in `policy.rs` and in `policy/pointer.rs` has no line of the form `self.x = config.x`.
- [x] `Policy` has no field that holds a `Config` value but `config` itself.
- [x] `cargo test -p caminito-policy` passes with no test changed.
- [x] The compositor's settings tests (`settings::`) and `tests::focus_modes` pass with no test changed.
- [x] `tools/check` passes, or the Done section lists what was run and what was not.

No new test: the change removes the place the bug could be in, and the tests there are show the behaviour is the same.

## For the implementing agent to settle

Recorded in the Done section: how `PointerFocus`'s methods get the config, as an argument from `Policy` or otherwise.

## Done (2026-10-11)

- **How `PointerFocus` gets the config: it doesn't hold or take it, but for one argument.** The code that reads `focus-mode`, `auto-raise` and `auto-raise-delay` is `pointer_entered`, which is in `impl Policy` already, so it reads `self.config`. `PointerFocus::settings_changed(&config)` is given the new config by `Policy`, and only drops a pending auto-raise that the new settings turn off. `PointerFocus::raise_on_click()` went: its three callers read `self.config.raise_on_click`.
- **`apply_config` is new and private.** `Policy::new` puts the config in its field, so it can't hand it to `settings_changed` too. Both call `apply_config`, which does the reactions: the count unless `dynamic-workspaces` is on, the names, `apply()`. `Policy::new` doesn't call `PointerFocus::settings_changed`, since a new `PointerFocus` has no pending auto-raise to drop.
- **Auto-raise is read at the crossing, not at the settings change.** The `Option<Duration>` that tied the delay to the flag went with the mirror fields. The config changes only in `settings_changed`, so the values are the same.
- **No test first, as the ticket says, and no test changed.** Run, in the OrbStack machine on aarch64: `cargo test -p caminito-policy` (484 passed), `cargo test -p caminito --lib -- settings:: tests::focus_modes` (209 passed), `cargo fmt --all --check` and `cargo clippy --workspace --all-targets -- -D warnings`.
- **Not run: the rest of `tools/check`.** That's cargo-deny, the other crates' tests, the palette helper, the helpers' tests, the rest of caminito's suite, caminito-locker against caminito, and the extension. The diff is in `caminito-policy/src/policy.rs` and `policy/` alone.
- **Reviewed on both axes (`/code-review`).** No defect in behaviour was found; two comments in `policy.rs` were corrected after it.
