# A second switch retargets the slide

Epic: workspace-slide
Type: implementation
Status: resolved
Blocked by: 233
Spec: [Workspace slide](../workspace-slide/spec.md)

## What to build

"Timing" and "Policy and input" in [the spec](../workspace-slide/spec.md): a slide that is cut short or sent elsewhere.

- **A second switch during a slide retargets it** from where the windows are, so the movement is continuous. The workspaces passed over are drawn as they go by, each at its cell's place.
- **A switch back** to the workspace the slide left returns from where it is.
- **The session lock ends a slide at once**, as does the output being disabled.
- A change of `workspace-slide` or of the three other keys during a slide lets the running one finish.

## Acceptance criteria

- [x] A headless test: three switches right, each made part-way through the last, give offsets that never jump, and three workspaces' windows are drawn in turn.
- [x] A switch right and back part-way returns the first workspace's windows to their places without a jump.
- [x] Locking part-way leaves no slide in the debug dump, and the unlocked output shows the new workspace at rest.

## Done (2026-10-13)

- **How it's built:** a slide keeps each workspace it has left, with its windows and its place from the workspace that arrives (`Left` in `caminito/src/workspace_slide.rs`). A switch on an output whose slide still runs makes the next slide from it (`Slide::retargeted`): it starts as far from its end as the windows are, every workspace left before moves by the new switch's distance, and the workspace just left joins them. So each window is drawn in the same place at the switch, and the Scene needed no change.
- **A retargeted slide takes its own 250 ms** on the same curve, from the second switch. The place doesn't jump and the speed does: the windows are at their fastest again. A held binding so never comes to rest between its repeats. The feel is 237's to check in a Session.
- **For the developer, a decision I made: a workspace is drawn at one place.** With `wrap-style` on, a second switch can wrap back to a workspace the slide still draws on its way out (two workspaces, right and right again). That workspace is then drawn where it arrives alone, so its windows jump from one side to the other; every other workspace is continuous. Plasma draws such a workspace two times. To do that the Scene must draw one window at two places, which it can't today. The spec's "Timing" has this, and a unit test holds it. With `wrap-style` off, its default, it can't happen.
- **The lock ends every slide** in the refresh pass that follows the lock request, and a switch under the lock is a cut. A slide whose output is gone ends in the same pass. A change of the four keys alone ends nothing; with a key against it, a switch during a slide is a cut, as "A change applies to the next switch" has it. These three are in the spec now.
- **A switch with no slide between two that have one** (the slide's workspace isn't the one the next switch leaves) starts a new slide and retargets nothing. No test has it.
- **The tests stop Caminito's clock** (`Fixture::stop_clock`, new): it runs in real time in a test, so the frame before a switch and the one after couldn't be read at the same time. With it stopped, the tests compare each window's drawn rectangle before and after a switch for equality.
- **Tests:** written before the code and seen to fail on 233's: three switches right, a switch another way, a switch back, and the lock. Written with the code: a workspace passed over, read from the pixels 20 ms after each of two switches; the key turned off part-way; an unplugged output, which reads the slides' deadline since the dump has a slide by its output. 233's `a_second_switch_ends_the_running_slide_and_starts_its_own` is gone, since that's no longer what happens. The arithmetic has unit tests beside the code.
- **Not tested:** the three other keys changed part-way (they're one boolean with `workspace-slide`), a retarget on two outputs (236's), and a real Session (237's).
- **What was run** (OrbStack machine, aarch64): `cargo fmt --all --check`, `cargo clippy --workspace --all-targets -- -D warnings`, the slide's 35 tests, and 133 of `caminito`'s beside it: `tests::lock`, `debug`, `workspace_moves`, `workspace_popup`, `per_output_workspaces`, `minimise_animation`, `idle` and `deadlines`. All passed. **Not run:** the full suite, at the developer's standing word.
- **Code review:** two reviews, of the standards and of the spec, found no bug. Fixed from them: the names in `offset` and `shift`, `Slide::with_windows` in place of writes by index, a filter that couldn't match, and the test of the passed-over workspace's pixels. Left: `Slide::from` repeats the workspace of one of `left`, and the `old_alone` spot is worked out in three tests.
