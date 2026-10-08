# A thin end-to-end slice

Type: prototype
Status: resolved
Blocked by: 06
Map: [VS Code's global menu](../map.md)

## Question

One menu with one item, served by a throwaway extension and shown by the global menu applet for a native Wayland VS Code with its custom title bar, the item running its command. Does it behave as the channel's decision said it would, with two windows open, across a window reload and across a restart of the panel?

To measure, from [06](06-channel-between-extension-and-caminito.md) and ADR 0018:

- The time from Caminito's focus change to the extension's claim, and how often moving fast between two windows ties the wrong pair.
- Whether the first real value of `focused` comes with an `onDidChangeWindowState` event, so that a window already focused when its host starts, or reloads, gets its menu without being refocused.
- Whether focusing a floating editor window gives its host a focus event, and so a claim.
- How long the panel stalls while the extension host is busy.

From [07](07-source-of-the-menu-structure.md) and ADR 0019:

- Whether the extension runs in the agent sessions window, and what the ordinary menu does there.

From [09](09-the-in-window-menubar.md) and ADR 0021:

- Whether a press of Alt shows the in-window menubar under `window.menuBarVisibility: toggle` in a Caminito session, and whether Alt+F opens its File menu.
- Whether two windows' extension hosts, claiming one after the other, can both show the offer to hide the menubar.

## Answer

The slice works, and three things didn't behave as the decisions said: a window that's focused when its extension host starts gets no focus event, a floating editor window gets no claim when the focus comes from its own workbench window, and two hosts do both show the offer.

The throwaway code is on branch `prototype/vscode-global-menu-slice` (commit 23b9697): the menu claim in `caminito/src/dbus/menu_claim_prototype.rs`, and the extension and the scripts in `.scratch/vscode-global-menu/issues/10-prototype/`. Run on 2026-10-08 with VS Code 1.140.0 (`07f806f`) on native Wayland with its custom title bar, in a nested debug build of Caminito on a private bus, with the installed `mate-panel-caminito` and `vala-panel-appmenu-mate-caminito 25.04-4`, which is the working tree's package and not yet a commit's. The focus was moved through a debug call that activates a toplevel as a click on a window list does, and keys were pressed the same way. Nobody clicked or typed, and nothing drawn was looked at except VS Code's own DOM. The panel's menubar was read, and its item activated, through AT-SPI with the `look.py` of ticket 111's checks, which isn't committed either.

### The slice

- **It works end to end.** The extension served one menu with one item through `@jellybrick/dbus-next`, bundled as plain files, in a live extension host. The panel showed the menu, and activating the item in the panel ran `workbench.action.files.newUntitledFile` in the focused window, within 70 ms of the click's `Event`.
- **Two windows:** each window's host claimed for its own toplevel, the panel followed the focus, and the item ran in the window that had it.
- **The parent step is right.** Each extension host's parent was the main process, which was the Wayland client, so every claim was matched by the parent's pid. That's from each host's own `ppid` beside the client's pid, compared by hand: Caminito's log doesn't say which of the two matched.
- **A reload** emptied the menu when the old host left the bus. The new host was on the bus about 2 s later (1.8 to 2.5 over five reloads).
- **A restart of the panel** changed nothing: `GlobalMenuOffered` stayed true, the ties were kept, and the new panel imported the menu with no new claim. With the panel gone for 13 s the property turned false and `GlobalMenu` emptied. When the panel came back the focused window's host claimed 2 ms after the signal.

### What ticket 06 asked

- **From the focus change to the claim:** 4 to 18 ms, median 9, over 40 switches 300 ms apart. Single switches took up to 63 ms. The time runs from the refresh pass in which Caminito sees the new focus, which is when it tells the client, to its handling of the claim.
- **A wrong tie that lasts** needed two switches 10 ms apart or less, closer than a person makes them. One that the next claim put right was also seen once at 100 ms. Alpha was focused and then beta, twenty times for each gap:

  | Gap | A claim tied to another window than its caller's | Beta left showing another window's menu |
  | --- | --- | --- |
  | 0 ms | 2 | 0 |
  | 5 ms | 13 | 1 |
  | 10 ms | 8 | 1 |
  | 20 ms | 0 | 0 |
  | 50 ms | 0 | 0 |
  | 100 ms | 1 | 0 |
  | 200 ms | 0 | 0 |

  Beta was never left with no menu. A wrong tie that lasts stays until that window is focused again.
- **The value of `focused` at activation was the real one, and no event followed it.** With `onStartupFinished`, a window that was focused when its host started read true, one that wasn't read false, at start-up and after a reload, in every run: a dozen windows that were focused and two that weren't. None got an `onDidChangeWindowState` event until the focus next changed. So under ADR 0018's rule, to claim on an event only, a new or reloaded window has no menu until it's focused again. The prototype then claimed at activation when `focused` read true, and the menu came back after a reload with nothing refocused, in three runs.
- **Some focus changes reach no host.** In one run of 20 focus changes to a window, 3 got no claim. In the one looked into, the other window's host was told it lost the focus, and this window's host got no event: its `focused` stayed false while the window had the focus. Whether VS Code drops the event or Caminito's nested run delivers the focus differently from a click wasn't told apart. The window's tie was already there, so its menu was shown. A window whose first focus is dropped has no menu until the next.
- **A floating editor window gets a claim only when the focus comes from another window.** Moving between it and its workbench window gives the host no event either way. So it had no menu when it opened, nor when it was reached from its workbench window. Reached from another window, its host claimed and the tie was to the floating window, which then kept it. It has no transient parent, so it doesn't show its workbench window's menu. The item clicked with it focused ran in it.
- **A busy extension host stalls the panel only while the panel is waiting on it.** With the host blocked for 5 s, a click on the item 1 s in stalled the panel for 3.9 s, the rest of the 5. Focusing that window while its host was blocked didn't stall the panel: the first `GetLayout` is asynchronous, so the menu was late and nothing else. A busy host of an unfocused window stalled nothing. The panel's slowest answer during ordinary switches was 29 ms. The stall while a real menu's submenus are preloaded wasn't run, since the slice has one.

### What ticket 07 asked

- **In the agents window the extension runs only if it's a built-in.** Installed in the user's extensions directory it wasn't activated there: that window's host started only built-in extensions. Put among the built-in ones (`--builtin-extensions-dir`, a directory of links to VS Code's own and the extension), it was activated, claimed, and the panel showed the menu.
- **The item worked there:** New Text File opened an editor in the agents window's editor area. No other item was tried.
- **The agents window has no in-window menubar,** so the panel's menu is the only one it has.

### What ticket 09 asked

- **A press of Alt shows the menubar under `toggle`,** and Escape hides it again.
- **Alt+F opened the File menu** in 40 of 42 tries, with F pressed from 0 to 300 ms after Alt. Alt and then F opened it in 7 of 7. The two failures were single tries early on and weren't reproduced.
- **Two hosts do both make the offer.** What was counted is each host reaching the notification's call, not the notifications on screen. The host that makes it turns the setting off, and the other window's host learned of it 250 to 350 ms later. With beta focused 50 or 150 ms after alpha both made it in 9 of 10 tries, at 300 ms in 3 of 5, and at 500 ms or more in none.

### Not run, or not explained

- A window was never clicked and the panel's menu was never opened with the pointer, so `focused` while a panel menu has the keyboard wasn't seen.
- Activation with `*`, a fork, a remote window, and a menu of more than one submenu.
- How GTK draws the stray action of a submenu whose `label` comes first, which [04](04-serving-dbusmenu-from-an-extension.md) handed on: the slice sends its properties in alphabetical order.
- A window with a menu on its own surface and a claim, which ADR 0018 says shows the surface's.
- Once, a window closed 0.7 s after a reload finished. It didn't happen in four more reloads.

### Handed on

- **ADR 0018's claim rule doesn't hold as written.** "Never on the value it reads at activation, which is true before the real one arrives" wasn't what 1.140 did with `onStartupFinished`. Claiming at activation when `focused` reads true is what gave a new or reloaded window its menu. This is a decision to reopen, not one this ticket made.
- **The floating editor window** shows nothing until it's reached from another window. ADR 0018 left which it would be to this run. What an unclaimed toplevel of a client with claimed ones shows is still to decide: ADR 0018 rejected the process's only claimed menu, and a floating window's own host is the case that rule didn't have.
- **[11](11-how-the-package-installs-the-extension.md):** the built-in directory isn't only the system-wide place, it's the one where the extension reaches the agents window.
- **ADR 0019's "the agents window gets the ordinary menu"** holds for a built-in extension, with one item tried.
- **ADR 0021's offer** needs more than the setting to be made once: a second claim within about 300 ms makes it again.
- **The spec:** the claim costs about 10 ms and a wrong tie lasted only for switches 10 ms apart or less, so nothing here asks for a number to reject a late claim with. A dropped focus event leaves `focused` false, so "the host of a focused window claims when the property turns true" misses that window.
- **The prototype's own comments** are older than its findings in two places: `extension.js` still says the value read at activation is true before the real one arrives, and the claim it makes when `GlobalMenuOffered` turns true asks for no earlier focus event while the one for a restarted Caminito does.
- **The harness** nests Caminito with a runtime directory of its own. Without one, the accessibility bus the private session bus starts takes the desktop's socket path in `/run/user/<uid>/at-spi/`. The first run here did that, and the earlier tickets' check scripts, which share the desktop's runtime directory, may have too.
