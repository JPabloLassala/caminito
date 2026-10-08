# When a claim is made, and what an unclaimed window shows

Type: grilling
Status: open
Blocked by:
Map: [VS Code's global menu](../map.md)

## Question

The slice ([10](10-thin-end-to-end-slice.md)) worked, and four things didn't behave as ADR 0018 and ADR 0021 said. Each is a rule to amend, not a new channel.

- **The claim rule.** ADR 0018 has the extension claim "on a focus event only, never on the value of `focused` it reads at activation". On 1.140 with `onStartupFinished` that value was the real one and no event followed it, so a new or reloaded window has no menu until it's focused again. Does the extension claim at activation when `focused` reads true? The prototype did, and the menu came back after a reload.
- **A dropped focus event.** In one run 3 of 20 focus changes reached no host, and that host's `focused` stayed false while its window had the focus. Whether VS Code or the nested run dropped them wasn't told apart. Is it looked into first, lived with, or covered by something Caminito does (a signal on a focus change, say, which ADR 0002 may not allow)? "The host of a focused window claims when `GlobalMenuOffered` turns true" misses such a window too.
- **The floating editor window.** Its host gets no event when the focus moves between it and its workbench window, so it has a menu only once it's been reached from another window. What does an unclaimed toplevel of a client with claimed ones show? ADR 0018 rejected the process's only claimed menu, for a second window still starting. A floating window has no transient parent and shares its workbench window's host.
- **The offer to hide the menubar, made twice.** Two hosts claiming within about 300 ms of each other both make ADR 0021's offer, since the setting that turns it off takes that long to reach the other host. What makes it once?
- **The offer in the agents window.** As a built-in (ADR 0022) the extension claims there, and that window has no in-window menubar to hide. Is the offer made from it?

## Facts to look up, not to ask

- Whether an extension can tell a floating editor window's focus from its workbench window's, and whether it can tell it runs in the agents window.
- What two extension hosts share that is quicker than a setting.
