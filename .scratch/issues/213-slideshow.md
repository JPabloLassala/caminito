# Slideshow

Epic: wallpaper
Type: implementation
Status: resolved
Blocked by: 210, 212
Spec: [Wallpaper](../wallpaper/spec.md)

## What to build

The Slideshow kind, under "The kinds" in [the spec](../wallpaper/spec.md).

- **The drawer:** shows the pictures of a folder one after another. The interval is one of 5 minutes, 15 minutes, 1 hour and 1 day; the order is random or by name. Each output draws independently, so two outputs on one folder show different pictures. It picks up pictures added to or removed from the folder, and survives an empty or missing folder by showing the output's colour.
- **The dialog:** the folder, the interval, "In order, not at random", the number of pictures in the folder, and Positioning.

## Acceptance criteria

- [x] Unit tests with a clock that is passed in: the next change's time for each interval, a random order that shows every picture before any repeats, two outputs differing, and an empty folder.
- [x] [checklist.md](../../docs/testing/checklist.md) has an item: a folder at 5 minutes changes with a cross-fade, and differs on two outputs.

## Done (2026-10-14)

- **The tests came first, at the pure module.** `slideshow/tests.rs` drives `slideshow.rs` with the local time passed in as seconds, as `dynamic.rs` takes it: the next change's time for each interval, a change at the interval and none between, by name and round again, every picture once a round at random over twenty rounds for three ranks, the same order for the same folder, two ranks never on the same picture, one picture that never changes, and an empty or missing folder with no picture and no change. The page's tests (choosing a folder, interval and order, a Slideshow with no folder shown and not written, every output's ranks with the switch on) and a settings test of the Slideshow's drawn picture followed. `caminito/src/tests/caminito_wallpaperd.rs` has the real drawer put two pictures of one folder on two outputs, and it was seen to fail with every rank forced to 0.
- **What shows follows from the clock alone, with no state.** The time is cut into slots of the interval, counted in local wall-clock seconds from 1970 as a timed XML's are (216), so *1 day* changes at local midnight and *1 hour* on the hour, and a restarted drawer, and the dialog's sidebar, show what the drawer showed. The cost: a folder chosen at 10:14 with *15 minutes* changes at 10:15.
- **At random, rounds as long as the folder,** each a Fisher–Yates shuffle seeded by the folder's path (FNV-1a) and the round (SplitMix64), written out rather than taken from a crate since the order need only look random and be the same in both programs. Every picture shows once a round. A round doesn't know the one before it, so where one ends and the next begins a picture can come twice in a row, about one time in as many as the folder has pictures; ruling it out for every output at once would make each round depend on the last, which the clock-only order can't. The checklist item says so.
- **Two outputs differ by rank.** Each output on a folder takes its rank among the outputs on that folder by its monitor's identity, and shows the picture that many places on in the same order, so no two show the same one while the folder has as many pictures as outputs. With "Same on all outputs" off, two outputs on one folder at different intervals or orders are ranked apart but can still meet on a picture; that wasn't worth a rule.
- **The settings.** An output's set gains `slideshow`, the folder, `slideshow-interval`, an enum `org.mate.Caminito.WallpaperInterval` of `5-minutes`, `15-minutes` (the default), `hour` and `day`, and `slideshow-in-order`, off. The folder's own pictures are shown, not its folders', as an added folder's are in the Picture grid; by name is by path.
- **The drawer** reads the folder again at each refresh, which is at least every minute (`CLOCK_TICK`, the old `DYNAMIC_TICK`), and wakes at the next whole interval when that's sooner, so a change is on time and a picture added or taken out is picked up by the next change. A change is a Picture to the drawer, so it cross-fades as any other does; an empty or missing folder is the output's colour, logged at debug.
- **The dialog's Slideshow pane:** the folder on a button that chooses it, how many pictures it has (or that it has none, or isn't there), *Change every*, *In order, not at random* and *Positioning*. Choosing *Slideshow* with no folder yet writes nothing, as *Picture* with no picture. The sidebar shows each output's picture, ranked as the drawer ranks it, and the window renders again every minute so the sidebar, a Dynamic's too, and the count follow the folder and the clock.
- **Seen working under Xvfb** on a keyfile GSettings with `cosmos` as the folder: the pane and the sidebar's picture, and clicks by `xdotool` on *In order* and *Change every* written to the store. The folder chooser wasn't driven, nor was any of it run on Wayland or in a Session with two monitors: the checklist item is the developer's, or 220's.
- **Run in an Ubuntu 24.04 cloud container,** with the build's dependencies from apt but `libgtk-session-lock-dev`, which it hasn't. Under `dbus-run-session` the workspace's tests pass (caminito's 2085, `caminito-policy`'s 751, caminito-wallpaper's 87) but `caminito-theme`'s `goldens`, which fail the same way on the base here, as 212 found. The ignored drawer tests pass, the new one among them. `cargo clippy -p caminito-wallpaper --all-targets -D warnings` is clean and the schema compiles with `--strict`. `tools/check` wasn't run whole, and no package was built.
- **A proptest found a policy bug that isn't this ticket's.** One run of `random_operations_keep_the_invariants_with_workspaces_per_output` failed, shrinking to an output of 20 by 30, a window mapped, `Tile(Left)`, and a reserve of 1 at the bottom, which panics in Smithay's geometry. It fails the same on the base with that seed. The seed proptest saved wasn't committed; it's `cc 32ff43367f6299e8d7dea2314fb6d97408a1c80e797b429ba9ecf949c7bd14ad`, for a ticket of its own.
