# Development and testing loop

Type: grilling
Status: open
Blocked by:
Map: [Camino v0.1](../map.md)

## Question

How is Camino developed and tested?

- **Running it:** nested under the winit backend vs. on a TTY. 08 made bare `camino`, without `--session`, the entry point with no session. How does a developer run a full nested MATE session?
- **Tests:**
  - headless integration tests (a headless backend plus real Wayland and X11 test clients);
  - golden tests for the theme engine against Marco (07);
  - unit-test seams for window-management policy (focus, placement, tiling, workspaces) kept separate from Smithay types.
- **CI and tooling:** CI (which forge, and which MATE and Smithay versions), and logging and debug tooling (`RUST_LOG` or tracing, a debug overlay).
- **Keeping up:** how to track Smithay master bumps (03 expects breaking changes on every bump).
