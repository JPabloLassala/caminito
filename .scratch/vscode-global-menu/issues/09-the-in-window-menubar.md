# What happens to the in-window menubar

Type: grilling
Status: open
Blocked by: 
Map: [VS Code's global menu](../map.md)

## Question

With the menu in the panel, what becomes of the one in the window?

- Does the extension set `window.menuBarVisibility` itself, ask the user to, or leave it alone? The setting is the user's, and with settings sync it follows them to machines without Caminito.
- When the global menu applet goes away, or VS Code runs outside a Caminito session, does the menubar come back, and who brings it?
- A Qt app, Firefox and an Electron app follow the applet only from their next start (ADR 0017). The extension could follow it while running. Should it?

From [06](06-channel-between-extension-and-caminito.md): `org.mate.Caminito` gets a `GlobalMenuOffered` property the extension watches, so it knows while running whether the panel shows the menu.
