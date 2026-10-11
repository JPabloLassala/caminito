# Mouse buttons as accelerators

Epic: system-shortcuts
Type: implementation
Status: ready-for-agent
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

- [ ] Headless tests with a D-Bus client, written before the code:
  - a grabbed button signals its action id once for each press, and the client under the pointer gets neither the press nor the release;
  - a second caller's grab of it returns 0;
  - the grab goes when its caller leaves the bus;
  - a button Caminito binds itself isn't signalled, and is once Caminito stops binding it;
  - it isn't signalled while the session is locked or during a move;
  - with an active shortcuts inhibitor on the focused surface, the press reaches the client and nothing is signalled.
- [ ] `mate-settings-daemon-caminito` builds with the patch in its series, and its `makepkg` and `namcap` job passes.
- [ ] [checklist.md](../../docs/testing/checklist.md) has an item, run in a Session with the shortcut set by `dconf`: a custom shortcut on `<Super>Button8` runs its command; changing it to `Button9` moves the grab; removing it gives the button back to the focused app. If it can't be run where the ticket is done, say so and leave it to [186](186-the-dialog-captures-a-mouse-button.md).
- [ ] On X11 MATE a custom shortcut on a key works as before (the gate).

## For the implementing agent to settle

Recorded in the Done section:

- whether msd parses a shortcut's value itself before it asks Caminito, and so what the patch had to change for a button to get through.
