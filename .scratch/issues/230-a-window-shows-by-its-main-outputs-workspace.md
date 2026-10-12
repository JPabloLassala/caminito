# A window shows by its Main output's workspace

Epic: per-output-workspaces
Type: implementation
Status: ready-for-agent
Blocked by: 221
Spec: [Per-output workspaces](../per-output-workspaces/spec.md)

## What to build

The second of three slices: "The key" and "The model" in [the spec](../per-output-workspaces/spec.md). After it the outputs' active workspaces can differ, and what is drawn and hit follows. Only a test, or the debug interface, can make them differ yet: the bindings are ticket 231.

- **The key** `workspaces-per-output` in `org.mate.Caminito` (`data/org.mate.Caminito.gschema.xml`), default `true`, translated into policy's config. Off, every switch sets every output, as ticket 221 left it.
- **A switch of one output's workspace** in policy, which leaves the others alone when the key is on. The existing switch paths call it for every output until ticket 231 gives them one.
- **A window is showing** by its Main output's active workspace. The scene, hit-testing, the surfaces' output enter and leave, Xwayland's stacking and the redraw trigger go by the windows showing on every output.
- **A window that straddles two outputs** is drawn on both while it's showing.
- **A window dragged across** takes the other output's active workspace when its Main output changes, so it stays in view.
- **A change of the key applies at once**, with no window's number changed: turned on, every output keeps the workspace that was active; turned off, every output takes one workspace. Until ticket 231 there is no Active output, so the Primary output's is the one taken.

Sticky windows, families, moves and activation keep their one-output reading (the window's own output) and are finished by ticket 223.

## Acceptance criteria

- [ ] Policy unit tests with two outputs and the key on: a switch of one output leaves the other's active workspace and its showing windows alone; a window's showing follows its Main output; a drag across joins the other output's active workspace when most of the window has crossed, and not before.
- [ ] The same with the key off: both outputs switch together, and ticket 221's invariant holds whenever the key is off.
- [ ] The property tests' other invariants hold with the key on.
- [ ] A headless test with two outputs on different workspaces: each output's scene has its own workspace's windows, a straddling window is drawn on both, and input reaches only windows that are showing.
- [ ] Turning the key off and on in a running compositor does what is said above.
- [ ] With one output, nothing differs between the key on and off.
- [ ] Every existing test passes with the key off.

## For the implementing agent to settle

Recorded in the Done section:

- how the debug interface switches one output, for the headless tests and for trying it by hand before ticket 231.
