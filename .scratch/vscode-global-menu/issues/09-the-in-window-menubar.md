# What happens to the in-window menubar

Type: grilling
Status: resolved
Blocked by: 
Map: [VS Code's global menu](../map.md)

## Question

With the menu in the panel, what becomes of the one in the window?

- Does the extension set `window.menuBarVisibility` itself, ask the user to, or leave it alone? The setting is the user's, and with settings sync it follows them to machines without Caminito.
- When the global menu applet goes away, or VS Code runs outside a Caminito session, does the menubar come back, and who brings it?
- A Qt app, Firefox and an Electron app follow the applet only from their next start (ADR 0017). The extension could follow it while running. Should it?

From [06](06-channel-between-extension-and-caminito.md): `org.mate.Caminito` gets a `GlobalMenuOffered` property the extension watches, so it knows while running whether the panel shows the menu.

## Answer

Decided on 2026-10-08 and recorded as [ADR 0021](../../../docs/adr/0021-vs-code-s-in-window-menubar-is-the-user-s-to-hide.md). The developer took the recommended answer to every question without a session, so each line below is a recommendation that stood. The setting's values, scope and default, what settings sync leaves out, and what VS Code's toggle command writes were read from the installed VS Code 1.140.0's bundle. Nothing was run, and no glossary term changed.

- **The extension never writes `window.menuBarVisibility` on its own.** It offers once: a notification when it claims while the setting is `classic` or `visible`, with Hide Menu Bar and Keep Menu Bar. A user already at `toggle`, `hidden` or `compact` gets none.
- **Hide Menu Bar writes `toggle`,** not `hidden` or `compact`. Alt shows the menubar again on any machine, and it's the keyboard's way to the menus until Alt+F opens the panel's (ADR 0017).
- **Once for a machine.** A setting of the extension's own, application-scoped and with `ignoreSync`, is turned off when the notification is shown. The stored state wasn't used because it's kept for each profile (not run).
- **Settings sync:** `window.menuBarVisibility` is application-scoped and synced. The extension doesn't touch `settingsSync.ignoredSettings`; the documentation names it.
- **Nobody brings the menubar back.** With the applet gone or outside a Caminito session the window shows what the setting says, and Alt or View: Toggle Menu Bar in the command palette is the user's way back.
- **Following the applet while running:** for the panel's menu only, as ADR 0018 has it. The in-window menubar doesn't follow it.
- **View > Appearance > Menu Bar** ([08](08-what-the-menu-shows-beyond-structure.md)'s note) stays, runs VS Code's command, and is checked when the setting is `classic` or `visible`. It needs no rule of its own, since the extension writes the setting only on the user's click.

### Handed on

- **[10](10-thin-end-to-end-slice.md):** whether Alt shows a `toggle` menubar in a Caminito session and Alt+F opens its File menu; whether two windows' hosts can both show the offer.
- **The spec:** the offer's wording and its two buttons; the extension's own setting, whose name waits on the extension's; a line for `window.menuBarVisibility` and `settingsSync.ignoredSettings` in what ADR 0017 documents for VS Code. VS Code's toggle command goes from `classic` or `visible` to `compact` under the custom title bar and from anything else to `classic`, never to `toggle`.
