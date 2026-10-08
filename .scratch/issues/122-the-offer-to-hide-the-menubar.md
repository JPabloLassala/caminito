# The offer to hide the in-window menubar

Epic: global-menu
Type: implementation
Status: open
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

- [ ] Through the seam: a claim with the setting at `classic` or `visible` shows the notification once and turns the extension's setting off; at `toggle`, `hidden` or `compact` it shows none.
- [ ] Through the seam: Hide Menu Bar writes `toggle` to the user's settings, and Keep Menu Bar and a notification closed with no answer write nothing to it.
- [ ] Through the seam: two cores with one parent pid claim together, and one notification is shown. A core with another parent pid offers too.
- [ ] Through the seam: with the agents window's workspace file there's a claim and no offer.
- [ ] Through the seam: turning the extension's setting on again brings the offer at the next claim, from the host that holds the name.
- [ ] The README's lines on the three settings match what was built.

## To run, and to answer here

Nobody has run these. A result that contradicts an ADR reopens it.

- [ ] Two real windows focused 50 ms apart, with the bus name.
- [ ] `workspace.workspaceFile` in the agents window.
- [ ] That the offer isn't made again in another profile.

## Notes

- VS Code's own Menu Bar command never goes back to `toggle`: it takes `classic` or `visible` to `compact` and anything else to `classic`.
