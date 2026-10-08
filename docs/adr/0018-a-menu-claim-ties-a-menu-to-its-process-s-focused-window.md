# 18. A menu claim ties a menu to its process's focused window

Date: 2026-10-08
Status: Accepted

## Context

VS Code on native Wayland with its custom title bar builds no native menu, so none of ADR 0017's three channels carries one. A VS Code extension can rebuild the menubar and serve it as a `com.canonical.dbusmenu`, but it runs in an extension host: a process of its own, a child of the Electron main process, that owns no Wayland surface.

Nothing an extension knows names its window. It has no title, id or handle, and every VS Code window has the same app id. It does know when its window gains focus. Each workbench window has its own extension host.

ADR 0002 keeps other clients' surfaces off the bus.

The rule for when the extension claims was amended on 2026-10-08, after a thin slice was run on VS Code 1.140. There the focus an extension reads is one boolean for a workbench window and its floating editor windows, so moving between them gives the extension host no event. With `onStartupFinished` the value read at activation was the real one and no event followed it. And in one run 3 of 20 focus changes reached no host, with nobody clicking a window.

## Decision

- **A fourth channel, the menu claim.** `org.mate.Caminito` has a method `ClaimGlobalMenu(path)`. The menu's bus name is the caller's unique name and is never an argument.
- **The process names the client, and focus names the window.** Caminito takes the caller's pid from the bus and that process's parent from `/proc/<pid>/stat`, one step up and no further. If a focused toplevel belongs to a client with either pid, Caminito ties it to the caller and the path. If none does, the claim is dropped.
- **The reply is empty either way,** with no error, so a claim tells its caller nothing about windows.
- **The last claim on a toplevel wins,** and one caller may hold several toplevels. A tie ends when the toplevel is destroyed or the caller's bus name leaves. There is no call to release one.
- **A toplevel nobody claimed has no menu of its own.** ADR 0017's rule for such a window applies: its transient parent's menu, or none.
- **A menu the app set on its own surface wins over a claim.** The claim is kept and not shown.
- **A claimed menu is published as any other dbusmenu,** in the `GlobalMenu` property, with the application's name taken from the toplevel. The global menu applet needs no change.
- **It follows the same switch as the three.** `org.mate.Caminito` has a read-only boolean property `GlobalMenuOffered`, true while Caminito offers the other channels, the ten seconds of grace included. Caminito drops a claim made while it's false and forgets every tie when it turns false.
- **The extension is gated on Caminito.** It serves and claims only on Linux, while `org.mate.Caminito` has an owner and `GlobalMenuOffered` is true. It watches both, so it follows a restart of Caminito and the applet coming and going without a reload. When the property turns true, the host of a focused window claims at once.
- **The extension claims when its window gains focus, and at activation when `focused` reads true.** Its activation event is `onStartupFinished`, where the value read is the real one. It never acts on focus lost, which a panel menu taking the keyboard also causes.
- **While `focused` is true it also claims when the active text or notebook editor changes, and when the tab groups change.** These are the events a host gets when the focus lands in a floating editor window's editor or one opens. A claim only ties the process's focused toplevel to its caller again, so in the workbench window they change nothing.
- **A focus event that never reaches the host isn't made up for.** Caminito sends nothing on a focus change.
- **The wire is general and the promise isn't.** Nothing in the method is VS Code's, but it's documented as the extension's, and no other client is promised it.

## Consequences

- The claim comes after Caminito's own focus change. Moving fast between two VS Code windows can tie one window's menu to the other until that window's own claim arrives. In the slice a claim came 4 to 18 ms after the focus change, and a wrong tie that lasted needed two switches 10 ms apart or less. It stays until that window is focused again.
- A VS Code window that's focused when its extension host starts has its menu once the host has activated. After a reload the new host was on the bus in about 2 s. One that isn't has none until it's focused. A reload ends the host and its bus name, so the menu empties and comes back with the new host's claim.
- A version or a fork of VS Code where `focused` reads true at activation before the real value arrives would have an unfocused window's host claim another window of its process. The wrong tie would last until that window is focused again. Only 1.140 was run, and a fork hasn't been.
- A floating editor window shares its workbench window's extension host. It has no menu until a claim is made while it has the focus: when it's reached from another window, or when the focus lands in a text or notebook editor of it. One that holds only other editors, a terminal say, and is reached from its own workbench window has none. The claim made when it opens can come before Caminito has focused it, and then it lands on the workbench window. The claims on an editor change and a tab-groups change are from VS Code's source and haven't been run.
- A window whose focus event is dropped keeps the tie it had. One with no tie yet has no menu until the focus next changes, and its `focused` stays false, so the claim made when `GlobalMenuOffered` turns true misses it too. Whether a person's click drops one hasn't been run, and if it does this is to reopen.
- Reading `/proc` is new to Caminito. A caller can't lie about its parent, and the most a claim can do is put a menu on its own or its parent's focused window.
- A Flatpak VS Code is left out. Its bus caller is `xdg-dbus-proxy`, which isn't a child of the main process, and the Flathub manifests don't let `org.mate.Caminito` through.
- The applet's dbusmenu importer calls the menu's server synchronously from inside mate-panel, so a busy extension host freezes the panel. This is accepted until it's measured. The fix, if one is needed, is a carried patch that makes the importer's calls asynchronous, which would help every app.
- ADR 0017's "VS Code has a global menu only on XWayland" no longer holds once the extension exists.
- Rejected:
  - `RegisterWindow` at the registrar with a number that's no X window's id (it works from a Flatpak, but it gives the id a second meaning and shares a map with the ids Electron registers by itself).
  - The pid alone, with no claim (two windows of one process would share a menu, and an item would run in the wrong window).
  - The extension sending its parent's pid (Caminito would trust what a caller says about itself).
  - A title or a window name passed between the extension and Caminito, or a number to reject a late claim with (Caminito would hand out window identities over the bus, against ADR 0002).
  - A well-known bus name for each window, to keep the tie through a reload (nothing names a window to build it from).
  - An unclaimed toplevel showing its process's only claimed menu (a second window still starting would show the first one's menu, and a click would run there). It would give a floating editor window its menu, but Caminito can't tell one from a second window still starting.
  - Claiming on a focus event only (a new or reloaded window would have no menu until it's focused again).
  - Claiming at activation after a delay or a second read (a timer for a value nobody has seen wrong).
  - A signal from Caminito on a focus change, or a reply that tells a host it has the focus, to cover a dropped focus event (every host of one VS Code has the same parent, so Caminito would have to name a window over the bus, against ADR 0002).
  - Polling the proposed `window.nativeHandle` to see the focus move into a floating window (a timer, and a proposed API a fork may not have).
  - A helper program, or Caminito itself, serving the dbusmenu so the panel can't freeze (a second program and protocol, or a menu server inside the compositor).
