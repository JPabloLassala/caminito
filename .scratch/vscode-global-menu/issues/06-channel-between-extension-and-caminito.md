# The channel between the extension and Caminito

Type: grilling
Status: resolved
Blocked by: 03, 04
Map: [VS Code's global menu](../map.md)

## Question

How does the extension tell Caminito "this dbusmenu is that window's", and how does Caminito publish it as the global menu?

- A method on `org.mate.Caminito`, a call to the registrar with something other than an X window's id, or another way.
- What names the window, and what Caminito does when it can't tell two windows apart.
- Whether the channel is the extension's alone or one any app without a surface of its own could use.
- Whether it comes and goes with the global menu applet like the other three (ADR 0017), and how the extension is gated on Caminito.
- How it squares with ADR 0002.

## Answer

Decided in a grilling session on 2026-10-08 and recorded as [ADR 0018](../../../docs/adr/0018-a-menu-claim-ties-a-menu-to-its-process-s-focused-window.md), with the term **Menu claim** in [CONTEXT.md](../../../CONTEXT.md).

- **The channel is a method on `org.mate.Caminito`,** `ClaimGlobalMenu(path)`, not the registrar. The menu's bus name is the caller's unique name.
- **The process and focus name the window.** The caller's pid, or its parent's read from `/proc/<pid>/stat`, finds the client, and the claim ties the caller to that client's focused toplevel. Several workbench windows work from the first version.
- **The reply is empty and has no error,** so ADR 0002 holds: nothing about a window crosses the bus.
- **Ties:** the last claim on a toplevel wins, one caller may hold several toplevels, and a tie ends with the toplevel or the caller's bus name. There's no release call.
- **Caminito can't always tell two windows apart, and then it shows less, not a guess.** An unclaimed toplevel has no menu of its own. A late claim can tie the wrong pair until the right window's claim comes.
- **A menu the app set on its own surface wins over a claim.**
- **The applet sees an ordinary dbusmenu** in `GlobalMenu`, so its Caminito backend and the patched package don't change.
- **It comes and goes with the global menu applet.** A read-only property `GlobalMenuOffered` tells when. Caminito drops claims while it's false and forgets every tie when it turns false.
- **The extension is gated** on Linux, an owner of `org.mate.Caminito` and `GlobalMenuOffered`, and watches the last two. It claims on a focus event only, never on the value read at activation, and when the property turns true while its window is focused.
- **A unique bus name, not a well-known one.** A reload empties the menu until the new host claims.
- **The wire is general, the promise is the extension's alone.** Flatpak VS Code is out of scope.
- **A panel frozen by a busy extension host is accepted** until it's measured. The fallback is a carried patch making the importer's calls asynchronous.

### Handed on

- **[09](09-the-in-window-menubar.md):** `GlobalMenuOffered` tells the extension, while it runs, whether the panel is showing the menu.
- **[10](10-thin-end-to-end-slice.md):** the time from Caminito's focus change to the claim; whether the first real value of `focused` comes with an event, and so whether a window focused when its host starts shows a menu without being refocused; whether a floating editor window's focus makes its host claim; the panel's stall under a busy extension host.
- **The spec:** the Caminito side is the method, the property, the ties and the read of `/proc`; the extension side is the gate, the claim rule and the unique name. The settings ADR 0017 documents for VS Code get a line for the extension.
