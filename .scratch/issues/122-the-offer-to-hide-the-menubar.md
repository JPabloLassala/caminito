# The offer to hide the in-window menubar

Epic: global-menu
Type: implementation
Status: resolved
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

All four were run on 2026-10-09, under "From 122's nested run". No result contradicts an ADR.

- [x] Two real windows focused 50 ms apart, with the bus name. **One notification, 15 times out of 15.** Both hosts asked for the name in 13 of them.
- [x] `workspace.workspaceFile` in the agents window. **It's `file://<user data>/User/agent-sessions.code-workspace`,** and that window claims and doesn't offer.
- [x] That the offer isn't made again in another profile. **It isn't.** A window of another profile reads the setting off.
- [x] That `context.globalStorageUri` is under `<user data>/User/globalStorage/` in a window of a profile other than the default, so the agents window is told by its workspace file there too. **It is,** in a window of a new profile and in the agents window opened beside it.

## Notes

- VS Code's own Menu Bar command never goes back to `toggle`: it takes `classic` or `visible` to `compact` and anything else to `classic`.

## Done (2026-10-09)

- **Tests first, through the seam:** `caminito-vscode/test/offer.test.mts`, 14 cases, each acceptance criterion's. The fixture gained `another`, a second window's core on the same private bus, with a parent pid of its own or the same. Each fake editor keeps its own settings, which stands for the ~300 ms another host takes to see the offer turned off, so only the bus name keeps the second window from offering. Mutations checked: granting the name always, ignoring the agents window and never forgetting the offer was made each fail a test.
- **A claim that may bring the offer waits for the bus name's reply,** and claims are sent in order through one queue. So a claim the stand-in has had says the offer that came with it is settled, and the tests count notifications without timers. A failed name request is logged and the claim is sent all the same. The notification and the setting's write aren't waited for.
- **The host remembers it made the offer** until it sees `caminito.globalMenu.offerToHideMenuBar` turned on again, so a write that fails doesn't bring the offer at every claim.
- **The settings directory is `userDir`,** which ticket 120 added on `master` alongside this work: two levels up from `context.globalStorageUri`, the default profile's for every window as tickets 02 and 12 read it. The merge folded this ticket's own `settingsDirectory` into it. The agents window is the one whose workspace file is a `file:` URI at `agent-sessions.code-workspace` there; with no `userDir`, no window is. Added under "To run".
- **`RequestName` is sent with `DO_NOT_QUEUE`,** and a reply of primary or already owner offers. The name is never released, so the host that has it offers again once the setting is turned back on.
- **The setting is contributed in `package.json`:** boolean, default true, `application` scope, `ignoreSync`. **The README** says it's kept for this machine and left out of Settings Sync, and that turning it on asks again the next time a window's menu goes to the panel.
- **Not run:** a real VS Code, so the four items under "To run". `npm run typecheck`, `npm test` (82 pass) and `npm run build` ran on Node 22. The Rust workspace's suite wasn't run: nothing outside `caminito-vscode/`, the README and this file changed.

## From 122's nested run (2026-10-09)

VS Code 1.140.0 and the applet's package 25.04-4 in a nested Caminito, with the scripts in [122-checks](122-checks/). The extension was built from this tree, and the debug build of `caminito` was the morning's. The editor had a scratch `--user-data-dir`, and the extension and a driver were among its built-ins. `focus.py` gives a window the keyboard through `zwlr_foreign_toplevel_manager_v1`. `check.mjs` counts the offer's notifications in each window's page through the DevTools port, and reads the claims and each `RequestName` from a monitor of the session's bus.

**Two windows focused 50 ms apart:**

- **A try:** the offer off, both windows reloaded so each has a new host and nobody holds the name, the offer turned on and read as on by both hosts, then one window activated and the other 50 ms later.
- **One notification in each of 15 tries.** In 13 both hosts asked for the name, 49 to 78 ms apart, so both had read the offer as on. The first to ask got the name, showed the notification and turned the setting off.
- **At 5 ms and at 200 ms, 5 tries each:** one notification every time, with both hosts asking in 3 and 3.
- **In the other 6 of the 25 tries only one window claimed,** though both were activated. Why wasn't looked at. A dropped change of focus is [124](124-vs-code-in-a-real-session.md)'s question, and these were activations by a client, not clicks.
- **Both hosts' parent pid is the editor's main process,** so they ask for the same name.

**The agents window,** opened with `code --agents` beside the two:

- **`workspace.workspaceFile` is `file://<user data>/User/agent-sessions.code-workspace`,** which is what the extension compares it with.
- **It claims and doesn't offer.** With new hosts in the other two windows, the offer on and the menubar at `classic`, a change of its active editor made three claims, no request for the name and no notification, and the setting stayed on. The first window focused after it asked for the name, got it and showed the notification.
- **Its workspace has a folder:** the one of the window that was focused when it opened.
- **Its profile is `builtin/agents`,** which takes its settings from the default's: its settings file is `<user data>/User/settings.json`.

**Another profile,** in an editor started again with one window, `--profile other`, after the offer was made:

- **No offer.** The window's settings file is `<user data>/User/profiles/<id>/settings.json`, its host read `caminito.globalMenu.offerToHideMenuBar` as off and `window.menuBarVisibility` as `classic`, and it claimed with no request for the name and no notification.
- **`context.globalStorageUri` is `<user data>/User/globalStorage/<extension>` there,** as the driver's host read its own. The agents window opened beside that window had the same, and the same workspace file as before.
- **The setting turned on from that window brought the offer at its next claim,** and the offer turned it off in the default profile's settings file.
- **Hide Menu Bar, pressed there,** wrote `toggle` to the default profile's settings file, and the host read `toggle`.

**Left behind:** nothing. The run's three folders were taken out of the recent list, which is shared with the developer's editor ([121](121-open-recent.md)), and the developer's settings file is as it was.
