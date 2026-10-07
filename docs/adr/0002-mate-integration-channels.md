# 2. Integrate with MATE through standard protocols, then environment, then D-Bus

Date: 2026-09-28
Status: Accepted

## Context

On X11, Marco talks to the rest of MATE through root-window properties and ClientMessages:

- `_MATE_PANEL_ACTION` for Alt+F1 and Alt+F2
- `_NET_DESKTOP_LAYOUT` for the pager
- `_NET_SHOWING_DESKTOP` for Show Desktop
- `_NET_WM_NAME` and `_MATE_WM_KEYBINDINGS` for mate-control-center

None of these exist on Wayland. On Wayland, MATE components speak only wlroots/ext protocols, and several of Marco's conversations have no standard protocol at all. The v0.1 feature cut requires all of these conversations, with the MATE-side changes carried downstream rather than upstreamed.

## Decision

For each conversation, Caminito uses the first channel that works:

1. **A standard Wayland protocol** that the MATE component already speaks, or can be made to speak through its existing library. Examples are wlr-foreign-toplevel `set_rectangle` and ext-workspace `coordinates`.
2. **An environment variable** for static facts, such as `MATE_WM_NAME=Caminito`.
3. **D-Bus on the session bus.** Caminito owns `org.mate.Caminito`; for example it exposes `ShowingDesktop`, `GetActiveToplevel` and `GrabAccelerators`. A patched component may also export its own object, as `org.mate.Panel` does for `MainMenu` and `RunDialog`.

A Caminito-private Wayland protocol is used only when a channel must refer to a particular Wayland surface. None does in v0.1. The MATE side of any non-standard channel is a carried patch, and each patched component falls back to its current behaviour when Caminito is absent.

## Consequences

- The patches stay small and GLib-idiomatic: GDBus or `g_getenv`, with no wayland-scanner or protocol XML to rebase.
- D-Bus isn't tied to the Wayland connection, so any session process can call Caminito's methods. That's acceptable for a single-user session, and it's why nothing that exposes other clients' surfaces goes over D-Bus.
- Where a D-Bus call leads to a Wayland action the client has no input serial for, Caminito has to bridge it itself. The Alt+F1 menu needs a one-shot popup-grab allowance.
- Rejected:
  - private Wayland protocols throughout (bigger patches in every component);
  - patching libxfce4windowing (a shared Xfce library);
  - reimplementing MATE logic such as media keys inside Caminito (duplicates msd).
