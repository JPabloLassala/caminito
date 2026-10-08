# Caminito takes a menu claim

Type: implementation
Status: open
Milestone: none: after v0.1
Blocked by:

## What to build

Caminito's side of [ADR 0018](../../../docs/adr/0018-a-menu-claim-ties-a-menu-to-its-process-s-focused-window.md), under "Caminito" in [the spec](../../vscode-global-menu/spec.md). A program with no window of its own calls `ClaimGlobalMenu(path)` on `org.mate.Caminito`, and the menu it serves becomes the global menu of its process's focused toplevel. The prototype's `menu_claim_prototype` module, on branch `prototype/vscode-global-menu-slice`, has it in throwaway form: read it, don't merge it.

- **The method.** The menu's bus name is the caller's unique name. The reply is empty whatever happened.
- **The client is found by pid.** The caller's pid comes from the bus and its parent's from `/proc/<pid>/stat`, one step up. A focused toplevel of a client with either pid is tied to the caller and the path. With none, the claim is dropped.
- **Ties.** The last claim on a toplevel wins, a caller may hold several, and a tie ends with the toplevel or with the caller's bus name. There's no release.
- **What's published.** A claimed menu is in `GlobalMenu` as a dbusmenu, with the application's name from the toplevel. A menu the app set on its own surface wins over a claim, which is kept. An unclaimed toplevel shows its transient parent's menu or none.
- **`GlobalMenuOffered`,** a read-only boolean property that tells when it changes, true while Caminito offers the other three channels, the grace period included. A claim made while it's false is dropped, and every tie is forgotten when it turns false.
- **The log says which pid matched,** the caller's or its parent's.
- **Document the method** as the global menu extension's, promised to no other client.

## Acceptance criteria

- [ ] A headless test: a claim from the client's own process ties its focused toplevel, and `GlobalMenu` carries the caller's unique name and the path.
- [ ] A headless test: a claim from a child process of the client's ties the same way.
- [ ] With two toplevels of one client, each claimed by another caller, `GlobalMenu` follows the focus.
- [ ] A claim from a process with no focused toplevel is dropped, and the reply is the same empty one.
- [ ] A tie ends when its toplevel is destroyed and when its caller leaves the bus.
- [ ] A menu set on the surface is shown in place of a claim, and the claim shows again when that menu is unset.
- [ ] `GlobalMenuOffered` follows the applet's bus name with the grace period, a claim made while it's false is dropped, and the ties are gone after it turns false. No test sleeps.
- [ ] The global menu applet and its patched package are unchanged.

## Notes

- Caminito sends nothing on a focus change, and nothing rejects a late claim: one came about 10 ms after the focus change in the prototype's runs.
- The prototype's debug calls, Activate and Key, come over only if a test needs them.
