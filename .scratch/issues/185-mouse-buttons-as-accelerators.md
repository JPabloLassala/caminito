# Mouse buttons as accelerators

Epic: system-shortcuts
Type: implementation
Status: open: the code is in, and the checklist item's Session run, with its X11 step, is 186's
Blocked by: 184
Spec: [System shortcuts](../system-shortcuts/spec.md)

## Why

A command's System shortcut is mate-settings-daemon's, grabbed from Caminito as an accelerator (98, 100). For a mouse button to run a command, Caminito has to grant a button and msd has to ask for one.

## What to build

### Caminito

- **`GrabAccelerators` takes a button** in [184](184-caminito-binds-mouse-buttons.md)'s syntax ([ADR 0028](../../docs/adr/0028-a-mouse-button-is-written-into-marcos-keybinding-keys.md)), and `AcceleratorActivated` carries its action id on the press. No signature changes.
- **98's rules hold for a button:**
  - a string that doesn't parse gets action id 0 and one log line;
  - a second caller's grab of a taken button is refused;
  - a caller's grabs go when it leaves the bus;
  - the signal goes to the grabber only.
- **Caminito's binding wins** over a grab of the same button, and that follows the settings as 148 has it for keys.
- **The press is handled as 184's is:** no client sees the press or the release, it isn't signalled while the session is locked or while something holds the pointer, a `mouse-button-modifier` click wins over a window, and a shortcuts inhibitor on the focused client keeps the press for that client.
- **The debug dump** lists button grabs and their owners.

### mate-settings-daemon

- **A carried patch** in `packaging/arch/mate-settings-daemon-caminito`, or a change to `0011-keybindings-Grab-the-shortcuts-through-Caminito.patch`, gated on Caminito (ADR 0011): a custom shortcut whose value names a button is passed to `GrabAccelerators` and runs its command when it's signalled.
- **The media keys' plugin** (`0010`) passes a button the same way if it takes no more than the same change. It gets one test and no more.
- **On X11** msd is as stock: a button value is one it can't parse, and the shortcut is unbound.

## Acceptance criteria

- [x] Headless tests with a D-Bus client, written before the code:
  - a grabbed button signals its action id once for each press, and the client under the pointer gets neither the press nor the release;
  - a second caller's grab of it returns 0;
  - the grab goes when its caller leaves the bus;
  - a button Caminito binds itself isn't signalled, and is once Caminito stops binding it;
  - it isn't signalled while the session is locked or during a move;
  - with an active shortcuts inhibitor on the focused surface, the press reaches the client and nothing is signalled.
- [x] `mate-settings-daemon-caminito` builds with the patch in its series, and its `makepkg` and `namcap` job passes.
- [x] [checklist.md](../../docs/testing/checklist.md) has an item, run in a Session with the shortcut set by `dconf`: a custom shortcut on `<Super>Button8` runs its command; changing it to `Button9` moves the grab; removing it gives the button back to the focused app. If it can't be run where the ticket is done, say so and leave it to [186](186-the-dialog-captures-a-mouse-button.md).
- [ ] On X11 MATE a custom shortcut on a key works as before (the gate).

## For the implementing agent to settle

Recorded in the Done section:

- whether msd parses a shortcut's value itself before it asks Caminito, and so what the patch had to change for a button to get through.

## Done (2026-10-12)

- **msd doesn't parse a shortcut's value before it asks Caminito, so no patch changed.** Under Caminito `parse_binding` in `0011` returns before `egg_accelerator_parse_virtual`, having turned away only the empty string and `disabled`, and `0010`'s `caminito_shortcut` checks the same two and no more. Both send the string as the setting has it. The series, `pkgver` and `pkgrel` are as they were, at 1.29.1-7.
- **That was seen, not only read.** In the running Session, with `mate-settings-daemon-caminito` 1.29.1-7 and a Caminito from before this ticket, a custom shortcut was set to `<Super>Button8` and the media keys' `messenger` to `<Super>Button9` with `dconf` and `gsettings`. Caminito's log refused each by name, `can't grab the accelerator "<Super>Button8" for :1.26` and `"<Super>Button9" for :1.24`, one line for each plugin's connection. Both settings were put back at once. That's the media keys' one test.
- **The second criterion is ticked on that:** there's no new patch to build, and the package installed here is the series' build. `makepkg` and `namcap` weren't run again.
- **Tests first.** The headless tests in `caminito/src/tests/accelerators.rs` and the unit test of `Grabs` were written before the code and seen to fail. Two passed from the start, since a refused button reaches the client anyway: the inhibitor's and the one of a button with other modifiers. So each guard in `on_pointer_button` was then taken out in turn, and each break failed its test: the modifier click, the inhibitor, and the lock and the move together.
- **A grab is looked for after Caminito's bindings, under 184's one check,** which `pointer_is_held` now holds for both. So a grabbed button gives way to everything a bound one does, the Window menu, the Window switcher, the Workspace popup and an X11 client's menu among them. Those four have no test of their own for a grab: `button_bindings` has them for a binding.
- **A press that isn't signalled goes on,** to the lock surface or to whatever holds the pointer, as in 184. Only a signalled press is kept from the clients, with its release, and the release is swallowed even if the grab went in between.
- **A held button doesn't repeat.** `AcceleratorRepeated` is a key's.
- **A grab of a button Caminito binds is granted,** as one of a key is, and is signalled once the binding goes (148).
- **The tests map xdg windows only,** as the file's key tests do: they check what a caller is signalled and what a Wayland client sees.
- **`tools/check` stops at caminito-locker's password test,** which fails the same on master on this machine: `libpam_wrapper.so` can't be preloaded. Every step before it passed, with caminito's 1819 tests, and the extension's step after it was run by hand and passed.
- **Not run: the checklist item,** *A mouse button runs a command*. It needs a Session with this build and a mouse with back and forward buttons, and is left to [186](186-the-dialog-captures-a-mouse-button.md), as the third criterion allows.
- **Not seen: the last criterion,** a key's custom shortcut on X11 MATE. No msd file changed, so its X11 code is the one 100 saw working, and step 7 of the checklist item covers it. That a button value is unbound there is read from `egg_accelerator_parse_virtual`, which finds no key for `Button8`, and wasn't run.
