# A thin end-to-end slice

Type: prototype
Status: open
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
