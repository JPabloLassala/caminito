# The offer to hide the in-window menubar

Epic: global-menu
Type: implementation
Status: open: the code is in, and the four runs under "To run" are the developer's
Milestone: none: after v0.1
Blocked by: [116](116-the-extension-serves-one-menu-and-claims.md)

## What to build

[ADR 0021](../../docs/adr/0021-vs-code-s-in-window-menubar-is-the-user-s-to-hide.md), under "The offer" in [the spec](../vscode-global-menu/spec.md). A user whose menubar is now shown twice is asked once, for the machine, whether to hide the one in the window.

- **When the extension claims while `window.menuBarVisibility` is `classic` or `visible`,** it shows a notification: "The panel now shows this window's menus. Hide the menu bar in the window? Pressing Alt shows it again."
- **Hide Menu Bar** sets the setting to `toggle` in the user's settings. **Keep Menu Bar** changes nothing. The extension writes the setting at no other time.
- **`caminito.globalMenu.offerToHideMenuBar`:** a boolean, true by default, application-scoped, with `ignoreSync`. It's turned off when the notification is shown, whatever the answer, and there's no offer while it's off.
- **One host offers.** Before offering, a host asks for the bus name `org.mate.Caminito.GlobalMenuExtension.pid<pid>`, with its parent's pid, without queueing. Only the host that gets it offers, and it keeps the name while it lives.
- **No offer from the agents window,** known by its workspace file, `agent-sessions.code-workspace` in the user's settings directory. Its host serves and claims.

## Acceptance criteria

- [x] Through the seam: a claim with the setting at `classic` or `visible` shows the notification once and turns the extension's setting off; at `toggle`, `hidden` or `compact` it shows none.
- [x] Through the seam: Hide Menu Bar writes `toggle` to the user's settings, and Keep Menu Bar and a notification closed with no answer write nothing to it.
- [x] Through the seam: two cores with one parent pid claim together, and one notification is shown. A core with another parent pid offers too.
- [x] Through the seam: with the agents window's workspace file there's a claim and no offer.
- [x] Through the seam: turning the extension's setting on again brings the offer at the next claim, from the host that holds the name.
- [x] The README's lines on the three settings match what was built.

## To run, and to answer here

Nobody has run these. A result that contradicts an ADR reopens it.

- [ ] Two real windows focused 50 ms apart, with the bus name.
- [ ] `workspace.workspaceFile` in the agents window.
- [ ] That the offer isn't made again in another profile.
- [ ] That `context.globalStorageUri` is under `<user data>/User/globalStorage/` in a window of a profile other than the default, so the agents window is told by its workspace file there too.

## Notes

- VS Code's own Menu Bar command never goes back to `toggle`: it takes `classic` or `visible` to `compact` and anything else to `classic`.

## Done (2026-10-09)

- **Tests first, through the seam:** `caminito-vscode/test/offer.test.mts`, 14 cases, each acceptance criterion's. The fixture gained `another`, a second window's core on the same private bus, with a parent pid of its own or the same. Each fake editor keeps its own settings, which stands for the ~300 ms another host takes to see the offer turned off, so only the bus name keeps the second window from offering. Mutations checked: granting the name always, ignoring the agents window and never forgetting the offer was made each fail a test.
- **A claim that may bring the offer waits for the bus name's reply,** and claims are sent in order through one queue. So a claim the stand-in has had says the offer that came with it is settled, and the tests count notifications without timers. A failed name request is logged and the claim is sent all the same. The notification and the setting's write aren't waited for.
- **The host remembers it made the offer** until it sees `caminito.globalMenu.offerToHideMenuBar` turned on again, so a write that fails doesn't bring the offer at every claim.
- **The settings directory is two levels up from `context.globalStorageUri`,** the default profile's for every window as ticket 12 read it. The agents window is the one whose workspace file is a `file:` URI at `agent-sessions.code-workspace` there; with no directory known, no window is. Added under "To run".
- **`RequestName` is sent with `DO_NOT_QUEUE`,** and a reply of primary or already owner offers. The name is never released, so the host that has it offers again once the setting is turned back on.
- **The setting is contributed in `package.json`:** boolean, default true, `application` scope, `ignoreSync`. **The README** says it's kept for this machine and left out of Settings Sync, and that turning it on asks again the next time a window's menu goes to the panel.
- **Not run:** a real VS Code, so the four items under "To run". `npm run typecheck`, `npm test` (82 pass) and `npm run build` ran on Node 22. The Rust workspace's suite wasn't run: nothing outside `caminito-vscode/`, the README and this file changed.
