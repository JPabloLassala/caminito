# 12. Policy asks for a window's frame extents in the state it's laying the window out in

Date: 2026-09-29
Status: Accepted

## Context

A window's frame extents depend on the window's facts (type, size hints, transient parent, whether it wants a frame), on the theme, and on the window's state: focused, maximised or shaded. eOS, one of the golden themes (ADR 0009), gives a focused maximised utility window 0px side borders and an unfocused one 1px (`metacity-theme-3.xml:13-38`).

Up to ticket 35, `Mapped` worked the extents out in `update_frame_facts` from the activation and shade it last held, and policy cached them in `Managed::read_frame`. `set_activated` and a shade changed neither copy, so policy laid a window out for one frame while Caminito drew another. Every change to a window's facts had to call `update_frame_facts` before telling policy, and ticket 39's `_NET_FRAME_EXTENTS` would have added another caller.

## Decision

- **`PolicyWindow::frame_extents` takes a `FrameState { maximized, focused, shaded }`**, which policy builds from its own state. The window answers from its facts and the theme alone. Fullscreen isn't in the key, because policy never uses extents for a fullscreen window.
- **Policy owns the extents in use.** `Policy::frame_extents(id)` is what the debug dump, X11 configure requests and `_NET_FRAME_EXTENTS` read. `PolicyWindow::requested_position` takes the extents from policy instead of working them out.
- **Policy lays out again the windows whose focus changed**, because their extents may have changed with it.
- **`caminito`'s `WindowFrame` owns the one derivation** from facts and a `FrameState` to a frame type, flags and extents. It draws with the state policy last sent, so what's drawn and what policy left room for agree by construction.

## Consequences

- Nothing in `caminito` caches transient state for the extents, and nothing reports extent changes back to policy. The facts still have to be read again before policy is told, but in one `Caminito` method.
- Policy's tests can model a theme whose extents depend on focus or shade, with `TestWindow`.
- A focus change can resize a maximised window, as it does in Marco with such a theme.
- `WindowFrame` recomputes its extents table (eight states) whenever the facts or the theme change, which costs a title layout per state.
- Rejected:
  - caching the state in `Mapped` and telling policy in the refresh pass when the extents changed (it keeps the cache, and adds a path back into policy for something policy already knows);
  - handing `Mapped` a shared `FrameContext` so it can compute extents on every call (`Rc<RefCell<_>>` around state that the settings path mutates).
