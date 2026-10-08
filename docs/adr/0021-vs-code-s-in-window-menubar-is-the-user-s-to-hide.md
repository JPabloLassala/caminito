# 21. VS Code's in-window menubar is the user's to hide, and the extension offers once

Date: 2026-10-08
Status: Accepted

## Context

With the extension serving VS Code's menubar as the global menu (ADR 0018), a window with its custom title bar shows the menu twice: in the panel and in its own title bar. What the window shows is `window.menuBarVisibility`, one of `classic` (the default on Linux), `visible`, `toggle`, `hidden` and `compact`. With `toggle` the menubar is hidden and a press of Alt shows it.

The setting is application-scoped: it's in the user's settings, the same for every profile and window, and settings sync carries it to the user's other machines, where there may be no Caminito. An extension can write it, and nothing else an extension can do hides the menubar. It can also contribute a setting of its own that sync leaves out.

ADR 0017 left Firefox's prefs and VS Code's settings to the user, and left Alt+F and F10 opening the panel's menu to a later ticket. The extension knows while it runs whether the panel shows the menu, through `GlobalMenuOffered`.

## Decision

- **The extension never writes `window.menuBarVisibility` on its own.** Not when it starts serving, and not when the global menu applet goes away.
- **It offers once to hide the menubar.** When it makes a menu claim while the setting is `classic` or `visible`, it shows a notification with two buttons. Hide Menu Bar sets `window.menuBarVisibility` to `toggle` in the user's settings. Keep Menu Bar leaves it as it is. A user who already has `toggle`, `hidden` or `compact` gets no offer.
- **The value it writes is `toggle`.** A press of Alt shows the menubar again, on this machine or on one the setting was synced to, with nothing to undo. Until Alt+F opens the panel's menu, it's also the keyboard's way to the menus.
- **The offer is made once for a machine.** A setting of the extension's own, application-scoped and left out of settings sync, says whether the offer is still to be made. It's true by default and the extension turns it off when it shows the notification, whatever the answer. A user can turn it on again.
- **A bus name keeps two windows from both offering.** Another window's extension host learns that that setting was turned off about 300 ms late. So before it offers, a host asks the session bus for a well-known name that carries the pid of VS Code's main process, which is its parent, without queueing, and only the host that gets the name offers. It keeps the name while it lives.
- **No offer comes from the agents window,** which has no in-window menubar. Its host claims and serves, and the offer waits for a claim from another window. The extension knows the agents window by its workspace file, `agent-sessions.code-workspace` in the user's settings directory.
- **Nothing brings the menubar back by itself.** When the applet is removed, or VS Code runs outside a Caminito session, the window shows what the setting says. Alt, or View: Toggle Menu Bar in the command palette, is the user's way back.
- **The extension follows the applet for the panel's menu only.** It serves and claims as `GlobalMenuOffered` comes and goes (ADR 0018), and the in-window menubar doesn't change with it.
- **View > Appearance > Menu Bar stays in the panel's menu** and runs VS Code's own command. Its checkmark is on when the setting is `classic` or `visible`, as in VS Code's own menu. It mirrors the setting, as ADR 0020 has it, so a menubar that Alt brought up under `toggle` leaves it off.
- **The settings are documented** with the ones ADR 0017 documents for VS Code: `window.menuBarVisibility`, the extension's own, and `settingsSync.ignoredSettings` for a user who wants the menubar hidden on this machine only.

## Consequences

- Until the user answers the offer or sets the setting, the menu is shown twice.
- A user who took the offer has no menubar in the window on another machine the setting is synced to, until Alt is pressed. On macOS the setting does nothing.
- After the applet is removed, a user who hid the menubar has neither menu on screen until Alt is pressed.
- VS Code's command doesn't go back to `toggle`: with the custom title bar it switches `classic` and `visible` to `compact` and anything else to `classic`. A user who brings the menubar back from the panel's item and wants it gone again sets the setting by hand.
- A notification closed without an answer isn't shown again. The documentation is then the only place that tells of the setting.
- That the extension's stored state is kept for each profile is from VS Code's documentation of profiles and wasn't run.
- With several windows open, each window's extension host reads the same setting, so the first to show the offer turns it off for the others. Without the bus name, two hosts claiming within 300 ms of each other both made the offer in 12 of 15 tries. The name hasn't been run.
- A user who turns the extension's setting on again gets the offer from the window whose host holds the name, or from any window once that one is closed or reloaded.
- A user who only ever opens the agents window gets no offer. That the workspace file's path reaches the extension as VS Code's source has it hasn't been run.
- That Alt shows a `toggle` menubar in a Caminito session, and that Alt+F opens its File menu, haven't been run there.
- Rejected:
  - Setting `window.menuBarVisibility` when the extension starts serving (a user's setting changed with nobody asking, and synced to machines with no global menu).
  - Hiding the menubar while `GlobalMenuOffered` is true and putting it back when it turns false (every window's host would write the user's settings file each time the applet comes or goes, a crash or a kill leaves it hidden, and each change is synced).
  - Leaving it alone and only documenting it (every user of the extension sees two menus, and the fix is a setting whose name they'd have to look for).
  - Writing `hidden` (only the command palette brings the menubar back, and a synced machine with no panel menu has no menu at all).
  - Writing `compact` (a menu button stays in the window beside the panel's menu).
  - Keeping whether the offer was made in the extension's stored state (it's kept for each profile, so the offer would come again in every profile, and a user couldn't turn it back on). It's no quicker than the setting between two windows either.
  - A file created exclusively in the extension's global storage, in place of the setting (a crash between creating it and showing the offer loses the offer, and a user can't turn it back on in the settings).
  - A bus name without the main process's pid (two editors running side by side would keep each other from offering).
  - Telling the agents window by the proposed `workspace.isAgentSessionsWorkspace` (a proposal can change with a release, and a fork may not have it).
  - Offering from the agents window too (it would hide a menubar the user isn't looking at).
  - A Don't Ask Again button (the offer is made once whatever the answer, so there's no second asking to turn off).
  - Writing the user's `settingsSync.ignoredSettings` along with the setting (a second setting of the user's changed, for something only some users want).
  - An offer to show the menubar again when the applet goes away (Alt already does it, and the applet's ten seconds of grace and a panel restart would have to be told apart from a removal).
