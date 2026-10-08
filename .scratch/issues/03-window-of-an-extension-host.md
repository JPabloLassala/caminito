# How Caminito can tell which window an extension host belongs to

Epic: vscode-global-menu
Type: research
Status: resolved
Blocked by: 
Map: [VS Code's global menu](../vscode-global-menu/map.md)

## Question

An extension host is a process of its own that owns no Wayland surface. Caminito's global menu code is in `caminito/src/dbus/` (`registrar.rs` among it) and the broker is ADR 0017. What can tie a menu served by an extension host to one xdg toplevel?

- **The process:** from a bus caller's unique name to its pid, and from there to the Electron main process that's the Wayland client. What does the process tree look like in 1.140, with the shared extension host and with one per window? Does Smithay give Caminito a client's pid? What about a Flatpak or Snap VS Code, where pids are namespaced?
- **Several windows in one process:** what does an extension know that tells its window from the others: `vscode.window.state.focused` and its event, the window's title, anything else? Is focus reported soon enough to follow Caminito's own focus change?
- **Anything on the surface:** does Electron on native Wayland set an app id, a title or anything an extension can also know or set (`window.title`)?
- **Auxiliary windows:** a floating editor window has the same extension host. Whose menu does it show?
- **ADR 0002** keeps window identities off the bus. Which of these ways would break it?

## Answer

Full findings are on branch `research/vscode-window-of-an-extension-host` (commit e967d58), in `research/vscode-window-of-an-extension-host.md`. Observed on the running session with one VS Code window, and read from VS Code `07f806f`, Electron v43.7.3, Chromium 150.0.7871.250, Smithay `928d4a9` and Caminito `52e4dca`. Two windows, a floating window and all timing were read, not run.

- **The process can be found with what Caminito already does.** A bus caller's pid comes from `GetConnectionUnixProcessID` (`caller_process`, `caminito/src/dbus/caminito.rs`). Every extension host is a direct child of the Electron main process, which is the only VS Code process on the Wayland socket, and Smithay gives a client's pid (`client_process`, `caminito/src/state.rs`). The step from the host to its parent would be Caminito's first read under `/proc`.
- **Each workbench window has its own extension host** in 1.140. There's no shared one.
- **Nothing an extension knows names its window.** The stable API has `window.state.focused` and `active` with `onDidChangeWindowState`, and no title, id or handle. `window.nativeHandle` is a proposed API, and on Wayland it's Ozone's counter, which names no toplevel. Every window has the app id `com.microsoft.VSCode`. The title is set by the workbench from the user's `window.title`, and the extension can't read it.
- **The only way found to tell two windows apart is a focus claim:** the extension calls when `focused` turns true, and Caminito ties the caller to that process's focused toplevel and keeps the tie. The claim always comes after Caminito's own focus change, so a late one can tie the wrong pair until the next. A window shows nothing until it's focused once after its host starts.
- **Two traps in `focused`:** it reads true before the first real value comes, and it follows `wl_keyboard` focus, so it turns false when a panel menu takes the keyboard. Only a claim of focus can count, never its loss.
- **A floating editor window** is a toplevel of the same client with no parent set, and shares its workbench window's extension host, which can't tell the two apart. It would show its workbench window's menu, and Caminito would tie more than one toplevel to one host.
- **Flatpak:** the bus caller is `xdg-dbus-proxy`, not a descendant of the main process, so the parent walk fails. A match by the sandbox's instance id is possible. The Flathub manifests allow the registrar's name and nothing of `org.mate.Caminito`. Snap is classic confinement, with no such trouble.
- **ADR 0002:** the pid alone and a focus claim that answers nothing keep it. A title sent by the extension bends it. An ext-foreign-toplevel identifier or Caminito handing out window names breaks it.

### Handed on

- **[06](06-channel-between-extension-and-caminito.md):** one workbench window (the pid alone) or several (the pid and a focus claim) in the first version; what a claim is on the wire and what an unclaimed toplevel shows; whether Caminito may read `/proc`; whether Flatpak is in scope, which would make the channel the registrar's name; that a number at the registrar shares one map with Electron's own ids.
- **The map's "several windows and reloads":** a restarted host is a new pid and bus name, so the tie is remade; a floating window shows its workbench window's menu.
- **[10](10-thin-end-to-end-slice.md):** the time from Caminito's focus change to the extension's event, `focused` while a panel menu is open, a floating window's parent and focus events, and the process tree with two windows.
- **[Ticket 113](113-global-menu-for-xwayland-windows.md)'s Electron question** has the same shape: the pid finds the client, Electron's number finds no toplevel.
