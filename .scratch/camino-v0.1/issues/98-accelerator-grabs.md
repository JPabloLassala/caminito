# Accelerator grabs

Type: implementation
Status: open
Milestone: M6
Blocked by: none
Map: [Camino v0.1](../map.md)

## What to build

A program on the session bus can ask Camino for a key combination and be told when it's pressed, which is how msd's media keys and custom shortcuts work under Camino ([06](06-v01-protocol-and-integration-set.md)'s *Media keys and custom shortcuts* row, [15](15-crate-and-module-architecture.md), [17](17-milestone-slicing.md)).

- **D-Bus:** on `org.mate.Camino`, `GrabAccelerators` takes a list of accelerators and returns an action id for each, and the `AcceleratorActivated` signal carries the action id and the event's time, in the style of `org.gnome.Shell`. An ungrab method releases them. Settle the signatures here, before [99](99-msd-media-keys-and-osd.md) and [100](100-msd-custom-shortcuts.md) use them.
- **Accelerators** are strings in GTK's syntax, as MATE's keys store them, with keysyms and with the XF86 media keys. A string that doesn't parse gets action id 0 and one log line.
- **The keyboard path:** the grabs are their own stage, after Camino's bindings and before the client stage (15). A grabbed key reaches no client. Camino's own bindings win over a grab for the same combination.
- **Lifetime:** a caller's grabs go when it leaves the bus.
- **Signals go to the grabber only,** not broadcast, so no other client learns the keys pressed.
- **The debug dump** lists the grabs and their owners.

## Acceptance criteria

- [ ] Headless tests with a D-Bus client, written before the code:
  - a grabbed combination signals its action id once per press, and the key reaches no client;
  - an ungrabbed combination reaches the focused client as before;
  - a second caller's grab of the same combination is refused with action id 0;
  - the grabs of a caller that leaves the bus are released;
  - a combination Camino binds itself isn't signalled;
  - an accelerator that doesn't parse returns 0.
- [ ] Unit tests for the accelerator parser: modifiers in any order and case, XF86 keysyms, and the empty and `disabled` strings.

## Notes

- **While locked** is M7's: the lock's keyboard path decides which grabs still fire.
- **Key repeat and release** aren't signalled. If msd's volume keys need repeat, add it here and say so.
- **Callers in a sandbox** can't reach the session bus's `org.mate.Camino` without a portal, so there's no filter to add.
