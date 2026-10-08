# When a claim is made, and what an unclaimed window shows

Type: grilling
Status: resolved
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

## Answer

Decided on 2026-10-08 and recorded as amendments to [ADR 0018](../../../docs/adr/0018-a-menu-claim-ties-a-menu-to-its-process-s-focused-window.md) and [ADR 0021](../../../docs/adr/0021-vs-code-s-in-window-menubar-is-the-user-s-to-hide.md). The developer took the recommended answer to all five questions, in two rounds. The facts are from VS Code's source at tag 1.140.0 (`07f806f`), the commit of the installed one. Nothing was run, Caminito's side doesn't change, and no glossary term changed.

### The facts

- **Focus is one boolean for a workbench window and its floating windows.** `nativeHostService.ts:46-52` folds the focus of the main window and every auxiliary one into one latched value, so moving between them reads true and then true. This is why ticket 10 saw no event.
- **Two events reach the host around a floating window.** `onDidChangeActiveTextEditor`, and the notebook one, fires when the focus lands in an editor group of another window's editor part (`editorParts.ts:297-305`), and not when both windows show editors of another kind. `tabGroups.onDidChangeTabGroups` fires when a floating window opens or closes, an empty one too. `activeTabGroup` doesn't follow the focus across windows.
- **Nothing names the window.** No stable or proposed API says which window a tab group is in. The proposed `window.nativeHandle` changes with the active window and has no event.
- **The agents window can be told.** `workspace.workspaceFile` is `<user settings directory>/agent-sessions.code-workspace` there (`windowsMainService.ts:1597`). That it reaches the host unchanged is inferred. The proposed `workspace.isAgentSessionsWorkspace` says it directly. `env.appHost` reads `desktop` in both.
- **Nothing in the API is atomic between two hosts.** `globalState` is one blob for an extension with the last writer winning, takes 200 ms or more to reach another window, has no change event, and is kept for each profile; the agents window has a profile of its own. A file created exclusively in `globalStorageUri` is atomic, and that directory is the default profile's for every window. A bus name asked for without queueing is atomic and goes when the host does.

### The decisions

- **The extension claims at activation when `focused` reads true,** with `onStartupFinished` as its activation event, and on focus gained as before. An editor where the value is true before the real one arrives would tie a window to the wrong host until it's focused again.
- **A dropped focus event is lived with.** Caminito can't cover it within ADR 0002: every host of one VS Code has the same parent, so it would have to name a window over the bus. A window that was claimed once keeps its tie, so what's exposed is a window that starts unfocused and whose first focus event is dropped.
- **For a floating editor window, the extension also claims while `focused` is true when the active text or notebook editor changes and when the tab groups change.** An unclaimed toplevel still shows its transient parent's menu or none, and ADR 0018's rejection of the process's only claimed menu stands. A floating window that holds only editors of another kind, a terminal say, and is reached from its own workbench window still has no menu.
- **A bus name makes the offer once.** Before offering, a host asks for a well-known name that carries the main process's pid, which is its parent, without queueing. Only the host that gets it offers, and it keeps the name while it lives. ADR 0021's setting still records that the offer was made.
- **No offer comes from the agents window,** told by its workspace file. Its host claims and serves, and the offer waits for a claim from another window.

### Handed on

- **[13](13-the-extension-s-name-and-the-spec.md):** the bus name's spelling follows the extension's name.
- **The spec, to run on a real session with a person clicking:** whether a click's focus change is ever dropped, as 3 of 20 were through the nested run's debug call. If it is, the dropped event is reopened.
- **The spec, to run:** the value of `focused` at activation in a fork; a floating window opened, and reached from its workbench window, with a text editor and with only a terminal, and whether the claim made on opening lands on it or on the workbench window; two windows focused 50 ms apart with the bus name; `workspace.workspaceFile` in the agents window.
- **The spec:** "the host of a focused window claims when `GlobalMenuOffered` turns true" misses a window whose `focused` is stuck at false, and nothing covers it.
