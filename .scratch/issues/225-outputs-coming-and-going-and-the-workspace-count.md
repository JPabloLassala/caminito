# Outputs coming and going, and the workspace count

Epic: per-output-workspaces
Type: implementation
Status: resolved
Blocked by: 231
Spec: [Per-output workspaces](../per-output-workspaces/spec.md)

## What to build

"Outputs coming and going" in [the spec](../per-output-workspaces/spec.md).

- **An output goes**: each of its windows goes to the Primary output's workspace of the same number (`rehome_windows` moves them to the Primary output's work area today, with no thought for workspaces). They don't return when the output does. If the output was the Active output, the Primary output is.
- **An output appears**, new or returning, disabled then enabled: it starts on workspace 1.
- **A smaller `num-workspaces`** clamps every output's active and previous workspace, and every window.
- **Dynamic workspaces** keep one count: a workspace is in use when any output has a window on that number, an empty one in the middle goes only when it's empty on every output, and the renumbering applies to every output together. `num-workspaces` is written back as now.

## Acceptance criteria

- [x] Policy unit tests: an output with windows on workspaces 1 to 3 is removed, and each window is on the Primary output's workspace of its number, with the Primary output's active workspace and showing windows as they were apart from the newcomers of that number.
- [x] The output is added again: it shows workspace 1 and no window has moved back.
- [x] A shrink of the count with two outputs on different workspaces clamps each.
- [x] Dynamic workspaces with windows on 1 to 3 of one output and 1 of the other: the count is the same for both; emptying workspace 2 on one output keeps it while the other has a window there, and removes it when neither has, with both outputs' active workspaces renumbered.
- [x] The property tests' invariants hold through output changes with the key on.
- [x] With the key off, each behaves as today.
- [x] [checklist.md](../../docs/testing/checklist.md) has an item: unplug the second monitor with windows on several of its workspaces, find them on the same numbers of the first, and plug it back in.

## Done (2026-10-13)

- **Much of it was true already.** Tickets 221 and 230 made a lower `num-workspaces` clamp every output and every window, and made `dynamic-workspaces` count and renumber by number for every output together. A window of an output that went kept its number, because an output change notes each window's Main output with no window following it. Their tests here (`caminito-policy/src/tests/outputs_coming_and_going.rs`) passed at once. The clamp's were then seen to fail with the clamp broken on purpose.
- **Tested first, and red:** an output added took the workspace that stood for the outputs, not workspace 1; a seat with no focus kept none when its Active output went; and a workspace that `dynamic-workspaces` removed under one output gave every seat the top window of any output.
- **An output that joins others starts on workspace 1**, with no previous workspace, with the key on. With the key off it takes the one workspace, as before.
- **Where no output stays, the primary output's workspace is kept, and this differs from the ADR's words.** ADR 0032 says "A new or returning output starts on workspace 1". With the key on, when the last output goes, or every output is replaced in one change, the outputs that come take the active and previous workspace the primary output had. If not, a user with one monitor is back on workspace 1 each time the monitor is unplugged or drops its link, with the windows left on the other workspaces, and story 16 says that nothing changes with one monitor. Ticket 224 keeps the shown desktops the same way. It is the developer's decision to keep this or not: the rule is in `ActiveWorkspaces::set_outputs`.
- **The windows go to the primary output, also where another output takes the gone one's place.** Caminito lays the outputs out again before it tells policy, so with three outputs the windows of the one in the middle were on the output that moved into its place. With the key on, each window whose Main output goes is moved onto the primary output's work area, and a maximised, tiled or fullscreen one is laid out there. The code review found this. With the key off such a window stays where it is, as before. A window that was mostly on another output and only partly on the one that went stays that other output's.
- **The Active output.** A seat whose Active output goes has the primary output, as ticket 231 made it. New here: such a seat with no focus, as after a switch to an empty workspace, takes the primary output's default focus. A seat whose focused window is put away on the primary output took it already.
- **Dynamic workspaces and the focus.** When a workspace an output showed is removed, only the seats whose Active output showed it take the default focus, from that output's windows. The other seats keep theirs. With the key off every seat takes it, as before. The workspace that stands for the outputs no longer counts as removed while policy has an output.
- **The previous workspace is forgotten, not clamped**, where the lower count removes it, as the one output's was (150) and as the spec has it ("as it clamps the one now"). The ticket's words say "clamps".
- **Not changed:** the switch sound of a lower count and of the dynamic renumbering is one, from the Active output's workspace (231). `num-workspaces` is written back by the same code as before.
- **The property tests** have two more invariants: with the key on, an output that joins others starts on the first workspace; and no window changes its workspace's number when outputs come, go, move or change. They ran with 3000 cases once, and 1500 after the review's fix.
- **A headless test** unplugs and replugs the second output on both kinds of window (`caminito/src/tests/per_output_workspaces.rs`). An output disabled and enabled through output management has no test of its own: it goes through the same `lay_out_outputs`.
- **Not changed, from the review:** the headless test reads policy through the fixture's state, as the rest of its file does, where the debug dump has the same facts.
- **The checklist** has "Unplugging a monitor keeps its windows on their workspaces". It was not run: it needs two monitors and the panel, and is ticket 229's.
- **Run in the OrbStack machine**, not the full suite: fmt and clippy clean for `caminito-policy` and `caminito`; policy's 677 tests; and caminito's headless tests filtered by `per_output_workspaces`, `dynamic_workspaces`, `output_lifecycle`, `primary_output`, `workspace`, `show_desktop`, `switcher`, `tests::debug`, `sounds`, `focus`, `placement`, `tiling`, `maximi`, `fullscreen` and `layer_shell` (627 passed).
