# Accelerator grabs

Type: implementation
Status: resolved
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

- [x] Headless tests with a D-Bus client, written before the code:
  - a grabbed combination signals its action id once per press, and the key reaches no client;
  - an ungrabbed combination reaches the focused client as before;
  - a second caller's grab of the same combination is refused with action id 0;
  - the grabs of a caller that leaves the bus are released;
  - a combination Camino binds itself isn't signalled;
  - an accelerator that doesn't parse returns 0.
- [x] Unit tests for the accelerator parser: modifiers in any order and case, XF86 keysyms, and the empty and `disabled` strings.

## Notes

- **While locked** is M7's: the lock's keyboard path decides which grabs still fire.
- **Key repeat and release** aren't signalled. If msd's volume keys need repeat, add it here and say so.
- **Callers in a sandbox** can't reach the session bus's `org.mate.Camino` without a portal, so there's no filter to add.

## Resolution

The code is `camino/src/input/bindings/accelerators.rs` (the parser and the table of grabs), the `grabs_stage` of the keyboard path in `camino/src/input/mod.rs`, and the methods and the signal in `camino/src/dbus/camino.rs`. The headless tests are `camino/src/tests/accelerators.rs`, and the parser's are beside it.

**The signatures,** on `org.mate.Camino` at `/org/mate/Camino`, for [99](99-msd-media-keys-and-osd.md) and [100](100-msd-custom-shortcuts.md):

| Member | Signature | |
|---|---|---|
| `GrabAccelerators` | in `as`, out `au` | An action id for each accelerator, in order. 0 is never an id: it's the answer for the empty string and `disabled`, for a string that doesn't parse, and for a combination another caller has. A combination the caller already has gets the id it had. |
| `UngrabAccelerators` | in `au` | Releases the caller's grabs with those ids. An id that isn't one of the caller's is passed over. |
| `AcceleratorActivated` | signal `uu` | The action id, then the key event's time in milliseconds, the time Wayland clients get with the key. It's sent to the grabber's unique name only. |

`org.gnome.Shell`'s takes `a(suu)` and signals `(ua{sv})`. The mode and grab flags, and the parameters other than the time, had nothing to carry here.

**What parses:** the modifiers `<Control>`, `<Ctrl>`, `<Ctl>` and `<Primary>`; `<Shift>` and `<Shft>`; `<Alt>`, `<Meta>` and `<Mod1>`; `<Super>`, `<Hyper>` and `<Mod4>`, in any order and case, then a keysym's name as xkbcommon knows it. A letter matches in either case. A grab is of exactly those modifiers, with the locks not counted, as Camino's bindings are.

**What doesn't,** each with its log line: `<Release>`, since only the press is signalled; `<Mod2>`, `<Mod3>` and `<Mod5>`, the lock and the level shifts, which Camino's bindings don't tell apart either; and a key by its code, `0x1f`, which msd writes for a key with no keysym. If 99 or 100 finds one of these in a default or in what Keyboard Shortcuts writes, it's added here.

**A key is matched by its first level,** as Camino's bindings are: `<Shift>1` fires, and `<Shift>exclam`, which GTK can write for the same press, is granted and never does. So with a keypad key that NumLock changes. [100](100-msd-custom-shortcuts.md) finds out what Keyboard Shortcuts writes, and this is fixed there if it writes those.

**Decided here:**

- A grab of a combination Camino binds is granted, and never signalled while Camino binds it. M8 makes the bindings a setting, and a refusal would be out of date when they change.
- A second caller's grab is refused however the first wrote the combination: `XF86AudioMute` and `xf86audiomute` are one.
- An active shortcuts inhibitor on the focused surface passes the grabbed keys to the client, as it does Camino's bindings, and as Mutter's does. A remote desktop or a virtual machine then gets the media keys. This is the one place a grabbed key reaches a client, which the ticket's text doesn't allow for: it's the developer's to confirm.
- Each activation is signalled from a task of its own on zbus's executor, which runs them in the order they were made. Nothing else orders two quick presses, but each signal has its time.
- The watch for callers that leave the bus was the Screensaver shim's ([62](62-screensaver-names.md)). It's `dbus/mod.rs`'s now, and drops the shim's cookies and the grabs alike.

**Not run in a Session:** nothing calls it until 99's and 100's patches do. Their checklist items are its check.
