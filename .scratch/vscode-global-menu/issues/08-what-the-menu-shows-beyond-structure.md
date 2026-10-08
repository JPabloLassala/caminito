# What the menu shows beyond structure

Type: grilling
Status: open
Blocked by: 01, 02
Map: [VS Code's global menu](../map.md)

## Question

Within the first version's bar (the map's Notes): which checkmarks are shown, how a keybinding label is chosen, what Open Recent holds, what is done with items that depend on a state the extension can't read, and when the menu is rebuilt (on `AboutToShow`, on a settings or keybindings change, on a timer).

From [07](07-source-of-the-menu-structure.md): the table is a snapshot with a hand-written annotations file keyed by command id, where the checkmarks that mirror a setting are listed. The snapshot also holds 108 default keybinding labels. Open Recent's entries are stripped from the table and filled at run time.
