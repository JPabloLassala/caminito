# 38. Key remaps are combinations swapped in Caminito's key path

Date: 2026-10-22
Status: Accepted

## Context

The developer wants macOS's shortcuts on Caminito: Alt, where Cmd sits, doing what Ctrl does (Alt+T a new tab), Alt+Right going to the end of a line, and a terminal or a browser having shortcuts of its own. On Linux this is done by Toshy and Kinto, which grab the keyboard's evdev device, rewrite its events in a process of their own and send them back through uinput. Their configs swap modifiers first (a modmap makes the Cmd key Ctrl) and then write every per-app rule in terms of the swapped keys. The developer wants it fast, with no key monitor of that kind.

Caminito already decodes every key itself, in `on_key`'s stages (`caminito/src/input/mod.rs`): the Keyboard monitor's, then System shortcuts' bindings, then accelerators, before a key is forwarded to the focused client.

## Decision

- **Key remaps are a stage of Caminito's key path,** after the bindings and the accelerators and before the key is forwarded. There's no other process, device or uinput, and a remap costs a lookup.
- **A remap swaps one combination for another, for a sequence of others, or for nothing.** It's matched by the keysym of the key and the modifiers, Alt from either Alt key and never AltGr.
- **There is no modifier swap.** Alt acting as Cmd is written as one rule per combination (Alt+T → Ctrl+T, Alt+W → Ctrl+W, …), so the page lists every remap there is, and a rule never has to be read against a swap made beneath it.
- **Remaps are listed for all applications and for named Remap groups of app ids,** and a group's rule replaces All applications' for its combination, or the group has none at all. An app id is in one group at most, so no two groups compete.
- **A System shortcut is matched first,** so it is never remapped. A target that is a System shortcut runs it instead of reaching the client (Alt+Q sent as Alt+F4 closes the window), and a target is never remapped again.
- **A sequence is sent in the same dispatch,** each step pressed and released in turn, with the client seeing only each step's own modifiers. The last step stays pressed while the key is held, so the client repeats it.
- **An Alt that starts a rule is held back** until the next key: a key that isn't remapped gets it first, and an Alt released alone reaches the client as a whole tap, so Firefox's menubar and GTK's mnemonics don't flash on a remap.
- **No remaps on the lock screen or during an Interaction.**
- **Toshy's macOS config is converted once into a Remap template,** by a script in `tools/`, not read by Caminito. The template is marked GPL-3 and credits Toshy and Kinto.

The spec has the behaviour in full: [Spec: Key remaps](https://linear.app/jplassala/document/spec-key-remaps-spec-1217258e4fa3).

## Consequences

- Remaps reach Wayland and XWayland clients alike, since both get their keys from Caminito, but not a client that reads the device itself.
- A combination the user wants as Alt+letter in an app (Alt+F for a File menu) is taken by an All applications rule, unless that app's group replaces it or has no remaps.
- The macOS template is long (one rule per letter, digit and punctuation key), which the page shows one application at a time.
- Rejected:
  - an evdev and uinput daemon, Toshy's way (a round trip through another process for every key);
  - a modifier swap with rules over it (two layers to read, and a swap can't be told apart per key);
  - extending System shortcuts (Caminito acts on those, and a remap's combination still reaches the client);
  - a GTK key theme, or text-input-v3's `delete_surrounding_text`, for the editing sequences (each reaches only some clients);
  - reading Toshy's Python config at run time;
  - Toshy's tap-or-hold keys, matching by window title or keyboard type, its per-desktop rules and its Option special characters, which XKB's `mac` variants give.
