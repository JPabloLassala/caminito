# The global menu extension serves one fixed menu and claims

Epic: global-menu
Type: implementation
Status: open
Milestone: none: after v0.1
Blocked by: [115](115-caminito-takes-a-menu-claim.md)

## What to build

The global menu extension's first slice, under "The global menu extension" in [the spec](../vscode-global-menu/spec.md): its names, build, manifest, gate, claim rule and dbusmenu server, with one menu of one item. In a Session with the applet on a panel, a VS Code window on native Wayland with its custom title bar shows that menu in the panel, and the item runs its command in the focused window.

- **The directory, the id and the build:** `caminito-vscode/`, `caminito.global-menu`, TypeScript bundled with esbuild into plain files, npm with a committed lockfile.
- **The manifest:** `extensionKind: ["ui"]`, `capabilities.untrustedWorkspaces.supported: true`, `activationEvents: ["onStartupFinished"]`, and an `engines.vscode` floor that this ticket looks up: the oldest release with every stable API the extension uses.
- **The test seam.** The core takes the `vscode` API as a value. Tests hand it a fake and run it on a private `dbus-daemon` with a stand-in for `org.mate.Caminito`, with node's own test runner. A job in `ci.yml` type-checks, runs the tests and builds the bundle.
- **The gate.** It serves and claims only on Linux, while `org.mate.Caminito` has an owner and `GlobalMenuOffered` is true, and it watches both.
- **The claim rule,** all four cases of the spec, and never on focus lost.
- **The server:** `@jellybrick/dbus-next`, pinned, with the workaround for `configureMembers()`; `GetLayout` with the whole tree, `Event` answered before the command runs, `AboutToShow` answered at once with false; properties in alphabetical order.
- **An editor older than the floor** ([ADR 0022](../../docs/adr/0022-vs-code-s-extension-is-a-built-in-linked-by-an-alpm-hook.md)): each API is looked for before it's used, one log line names both versions, and nothing throws into the extension host.
- **A way to run it by hand** in a nested run, as the prototype's `run.sh` and `session.sh` did, with a runtime directory of its own.

## Acceptance criteria

- [x] Through the seam: a client on the bus reads the menu, and an `Event` on the item runs its command after the reply is sent.
- [x] Through the seam: a claim is made at activation with `focused` true, on focus gained, when `GlobalMenuOffered` turns true or `org.mate.Caminito` gains an owner while focused, and on an editor or tab-groups change while focused; none on focus lost or while unfocused.
- [x] Through the seam: no name is served and no claim is made off Linux, with no owner of `org.mate.Caminito`, or with `GlobalMenuOffered` false; and it starts and stops as they change.
- [x] With a fake that lacks the tab-groups and notebook APIs, the extension still serves and claims, and logs one line.
- [x] A failure at activation is logged and doesn't reach the host.
- [x] The CI job passes. (The `extension` job passed in CI on `master` on 2026-10-08, run 37853654941, at `b886f7a`.)
- [ ] In a nested run with VS Code 1.140, the panel shows the menu, the item runs in the focused one of two windows, and the menu comes back after a reload with nothing refocused.

## Notes

- Report the `configureMembers()` bug to `JellyBrick/node-dbus-next`.
- The prototype's `extension.js` is older than its findings in two places; the spec has the rule.
- The oldest base among the five editors is Cursor's, 1.128.

## Comments

**2026-10-08, the first slice built.** `caminito-vscode/` has the extension: `src/global-menu.mts` is the core (the gate, the claim rule, the rule for an old editor), `src/dbusmenu.mts` the server, `src/editor.mts` the part of the `vscode` API it takes as a value, and `src/extension.mts` the entry that hands it the real one. `npm test` runs node's own test runner on `.mts` files with Node's type stripping, each test on a `dbus-daemon` of its own with a stand-in for `org.mate.Caminito` (`test/harness.mts`). `nested/run.sh` is the run by hand. The CI job is `extension` in `ci.yml`.

- **The floor is 1.68.0,** looked up in `@types/vscode`'s releases: `window.onDidChangeActiveNotebookEditor` came in 1.68 and `window.tabGroups` in 1.67, and everything else used (`window.state`, `onDidChangeWindowState`, `onDidChangeActiveTextEditor`, `createOutputChannel`, `executeCommand`, `version`) is older. The type-check runs against `@types/vscode` 1.68.0, so an API newer than the floor fails it, and the two late events are optional in `Editor`. A test holds the floor to the manifest's `engines.vscode`. The log is a plain output channel, since `LogOutputChannel` would raise the floor to 1.74, and it's opened at its first line so an editor outside a Session shows none.
- **The prototype's branch isn't on the remote,** so the slice was written from the spec and tickets 04 and 10. The menu is File > New Text File, as the prototype's, served at `/org/mate/Caminito/GlobalMenuExtension/MenuBar`.
- **`Event` runs the command on the next turn of the loop,** after the reply is written. The tests show the reply comes while a command that never finishes is still running, and that a command that throws is logged. Whether the command begins only after the importer has read the reply isn't shown: the client is in the test's own process.
- **A bus that can't be reached** leaves dbus-next's calls unanswered, so the start waits for `connect` or `error` and a failure ends it with one line.
- **Not run here:** the CI job (nothing here runs an Arch container; it ran locally on Node 22.22 with dbus 1.14) and the nested run with VS Code 1.140, which needs a Wayland desktop and the applet's package.

**The `configureMembers()` report, not yet sent** (the session's GitHub access doesn't reach `JellyBrick/node-dbus-next`). Title: *`Interface.configureMembers()` registers nothing: the constructor's own fields hide the prototype's*. Body: In 0.11.4, `Interface`'s constructor defines `$properties`, `$methods` and `$signals` as own properties set to `undefined` (`_defineProperty(this, "$methods", void 0)` in `dist/index.cjs`), while `configureMembers()` puts the members on `this.prototype`. The own fields shadow the prototype's, so an interface configured that way exports no method, property or signal: a call gets `org.freedesktop.DBus.Error.UnknownMethod`. Deleting the three own fields in the subclass's constructor, after `super()`, works around it. A fix would be to declare the fields without initialisers (`declare`) or to have `configureMembers()` not depend on them.
