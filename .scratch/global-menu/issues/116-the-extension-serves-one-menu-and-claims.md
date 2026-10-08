# The global menu extension serves one fixed menu and claims

Type: implementation
Status: open
Milestone: none: after v0.1
Blocked by: [115](115-caminito-takes-a-menu-claim.md)

## What to build

The global menu extension's first slice, under "The global menu extension" in [the spec](../../vscode-global-menu/spec.md): its names, build, manifest, gate, claim rule and dbusmenu server, with one menu of one item. In a Session with the applet on a panel, a VS Code window on native Wayland with its custom title bar shows that menu in the panel, and the item runs its command in the focused window.

- **The directory, the id and the build:** `caminito-vscode/`, `caminito.global-menu`, TypeScript bundled with esbuild into plain files, npm with a committed lockfile.
- **The manifest:** `extensionKind: ["ui"]`, `capabilities.untrustedWorkspaces.supported: true`, `activationEvents: ["onStartupFinished"]`, and an `engines.vscode` floor that this ticket looks up: the oldest release with every stable API the extension uses.
- **The test seam.** The core takes the `vscode` API as a value. Tests hand it a fake and run it on a private `dbus-daemon` with a stand-in for `org.mate.Caminito`, with node's own test runner. A job in `ci.yml` type-checks, runs the tests and builds the bundle.
- **The gate.** It serves and claims only on Linux, while `org.mate.Caminito` has an owner and `GlobalMenuOffered` is true, and it watches both.
- **The claim rule,** all four cases of the spec, and never on focus lost.
- **The server:** `@jellybrick/dbus-next`, pinned, with the workaround for `configureMembers()`; `GetLayout` with the whole tree, `Event` answered before the command runs, `AboutToShow` answered at once with false; properties in alphabetical order.
- **An editor older than the floor** ([ADR 0022](../../../docs/adr/0022-vs-code-s-extension-is-a-built-in-linked-by-an-alpm-hook.md)): each API is looked for before it's used, one log line names both versions, and nothing throws into the extension host.
- **A way to run it by hand** in a nested run, as the prototype's `run.sh` and `session.sh` did, with a runtime directory of its own.

## Acceptance criteria

- [ ] Through the seam: a client on the bus reads the menu, and an `Event` on the item runs its command after the reply is sent.
- [ ] Through the seam: a claim is made at activation with `focused` true, on focus gained, when `GlobalMenuOffered` turns true or `org.mate.Caminito` gains an owner while focused, and on an editor or tab-groups change while focused; none on focus lost or while unfocused.
- [ ] Through the seam: no name is served and no claim is made off Linux, with no owner of `org.mate.Caminito`, or with `GlobalMenuOffered` false; and it starts and stops as they change.
- [ ] With a fake that lacks the tab-groups and notebook APIs, the extension still serves and claims, and logs one line.
- [ ] A failure at activation is logged and doesn't reach the host.
- [ ] The CI job passes.
- [ ] In a nested run with VS Code 1.140, the panel shows the menu, the item runs in the focused one of two windows, and the menu comes back after a reload with nothing refocused.

## Notes

- Report the `configureMembers()` bug to `JellyBrick/node-dbus-next`.
- The prototype's `extension.js` is older than its findings in two places; the spec has the rule.
- The oldest base among the five editors is Cursor's, 1.128.
