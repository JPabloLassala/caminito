# The minimise animation

Epic: caminito-v0.1
Type: implementation
Status: resolved
Milestone: M8
Blocked by: 147
Map: [Caminito v0.1](../caminito-v0.1/map.md)

## What to build

Marco's minimise animation, towards the window list's button ([05](05-v01-marco-feature-cut.md)). [65](65-foreign-toplevel.md) stores the button's rectangle that mate-panel sets for each window, and nothing reads it yet.

- **The animation:** a minimised window shrinks into its button's rectangle, as Marco's does, drawn in the Scene from the window's last content.
- **Off** with `compositing-manager` off, with `reduced-resources` on, or with `org.mate.interface enable-animations` off ([09](09-gsettings-mapping.md)).
- **Both kinds of window,** and whichever way they're minimised (a binding, the Frame's button, the window list, a client's request).

## Acceptance criteria

- [x] Headless tests through the fixture, written before the code, with the driven clock: a minimised window drawn between its place and the rectangle part-way through, and gone at the end; no animation with each of the three keys; no rectangle; the window restored during the animation.
- [x] `reduced-resources` and `enable-animations` leave `UNSUPPORTED` if 147 put them there; their rows, and `compositing-manager`'s, in `docs/parity.md` say what they turn off, with the tests above.
- [x] The checklist has an item, tagged M8: minimising a native and an X11 window from the Frame and the window list, then with `reduced-resources` on. Running it in a Session is the developer's, and goes to [173](173-packaging-and-closing-m8.md)'s closing run.

## Open questions

**From Marco's source**, to be settled by the implementing agent and recorded in the Done section: the animation's shape (a scaled window, or Marco's outline), its duration and easing.

**For the developer**, decided when the ticket is done and listed in its Done section for confirmation:

- **No rectangle** (no window list, or a window it doesn't show): proposed, no animation.
- **Whether Policy sees the window as minimised during it:** proposed, yes, at once, with only the drawing delayed, so that the focus moves on at once.

## Done (2026-10-11)

- **Tests first:** `caminito/src/tests/minimise_animation.rs`, through the fixture with the driven clock, on both kinds of window. Before the drawing was in the Scene, the eight tests that read the shrinking window failed and the "no animation" ones passed. They cover: the window part-way and gone at the end; each of the three keys, and a key turned off while Caminito runs; no rectangle, and a rectangle taken back; the window restored during the animation; the four ways to minimise (Alt+F9, the Frame's button, the window list, the client's request); a rectangle on a window's surface as well as a layer surface's. The translation's and the box's arithmetic have unit tests beside the code.
- **From Marco's source (1.28.1, `effects.c`, `window.c:1490-1530`):** the duration is Marco's 250 ms (`META_MINIMIZE_ANIMATION_LENGTH`), at a constant speed, with Marco's integer arithmetic for the box and a step every 15 ms. Marco's shape is a black outline 3 pixels thick, with the window hidden at once.
- **The developer's answers, asked before the build (2026-10-11):** *the shape* is the window's last content with its Frame, scaled into the box, not Marco's outline; *no rectangle* means no animation, where Marco's box goes to the screen's bottom right corner; *Policy has the window minimised at once*, as Marco does, so the focus moves on at once and only the drawing takes its time.
- **`compositing-manager` off turns it off,** as this ticket says. Marco's outline doesn't depend on it; the parity row says so.
- **How it's built:** nothing hooks the ways to minimise. The refresh pass keeps which windows were on screen, and starts an animation for one that is minimised at the next pass (`caminito/src/minimize_animation.rs`); a window that's restored or gone ends its animation there. The Scene draws it in a layer of its own, `Minimizing`, over the windows and the top layer, so the window goes into the panel's button, and under X's override-redirect windows. It reuses the Window switcher's thumbnail drawing. A loop timer redraws every 15 ms while one runs.
- **After master's 163 and 171 came in:** 163's thumbnails of Layer surfaces and the animation share the Scene's `scaled_elements`. The shrinking window has no shadow (171), as a thumbnail has none. Only the affected tests were run after these merges, as the developer asked: the animation's, the Window switcher's, the shadows', the translation's and the parity table's, with fmt and clippy.
- **`UNSUPPORTED`:** 147 put neither key there, so nothing left it. The three rows of `docs/parity.md` are *honoured*, with the tests.
- **The checklist** has *The minimise animation*, tagged M8, for 173's closing run. It wasn't run in a Session here.
- **Not run, and what failed:** in the OrbStack machine (aarch64), `cargo fmt --check` and clippy pass, and the workspace's tests pass but for master's known failures there (four `tests::keymap` tests, caminito-theme's goldens) and `lock::a_second_clients_lock_is_refused_once_the_locker_is_gone`, which failed once in the full run and passes alone. `/code-review` wasn't run: the developer stopped it. `tools/check` on the developer's machine is 173's.
- **Found on the way, not this ticket's:** caminito-policy's `random_operations_keep_the_invariants` found a case that panics (`Attempting to create a Size of negative size: (79, -30)`, with a 20 × 30 output). Policy isn't touched here; the seed wasn't committed, and the developer has it.
