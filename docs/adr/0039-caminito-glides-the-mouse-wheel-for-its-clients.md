# 39. Caminito glides the mouse wheel for its clients

Date: 2026-10-23
Status: Accepted

## Context

A mouse wheel's click scrolls a GTK or Qt Widgets client one step in one frame. The developer wants the view to glide that step, as it does in browsers and in KDE's own applications.

Who animates the wheel today (toolkit sources at their current branches):

- **GTK3 and GTK4** don't. A click sets the adjustment's value at once; their only animation is the kinetic coast after a touchpad's `axis_stop`. No setting or variable turns one on.
- **Qt Widgets** doesn't. `QAbstractSlider` steps by `angleDelta`, carrying a remainder between events. QScroller is opt-in and for drags.
- **Qt Quick** does: `Flickable` turns a wheel event into a flick, and Plasma 6's Kirigami smooths it. So do **Dolphin**, **Firefox**, **Chromium** and **Electron**, each with its own code. This is why Plasma feels smoother: KWin doesn't smooth the wheel, its own applications do.
- **No compositor** smooths it: not KWin, Mutter, wlroots, niri, Hyprland or Wayfire. The nearest is wheelproxy, a Wayland proxy for Chromium alone.

What a client makes of a wheel frame that carries part of a click:

- **GTK4** (wl_pointer 8 or later) scrolls by `value120`/120 of a step, proportionally.
- **GTK3** binds wl_pointer 5, so Smithay adds the 120ths up and sends `axis_discrete` per whole click. Its scrolled windows scroll by the frame's pixel value all the same, and its spin buttons and combo boxes, which take only discrete events, step once per click.
- **Qt6** adds `value120` into `angleDelta`, proportionally. **Qt5** reads `axis_discrete` alone, so it steps once per click.
- **Xwayland** sets the XI2 scroll valuator to `value120`/120: XI2 clients scroll proportionally, and core clients get a button 4 or 5 per click.

A stream with no `value120`, as a touchpad sends, is worse: GTK3's discrete-only widgets ignore it, and GTK and Chromium wait for a stop to coast.

## Decision

- **Caminito makes a Wheel glide for every client**: each click of a wheel-source scroll reaches the client under the pointer as a short stream of wheel-source frames, each with its share of the click's value and of its `value120`, over about 150 ms on an ease-out curve. The step is as long as without it, and nothing coasts after it.
- **One frame per refresh** of the output under the pointer, and only once the client has drawn the last: a slow client gets fewer, larger frames, and the glide still ends on time.
- **A click during a glide adds its step** to what's left and starts the curve again from where the view is. **A click the other way** drops what's left and starts a new glide. **A change of the pointer's focus** drops what's left.
- **Left as they are**: a click with Ctrl, Alt or Super held, which goes as a whole click so that zooming by event doesn't overshoot; the finger, continuous and tilt sources; and the titlebar's scroll actions, which still act once per click.
- **Settings in `org.mate.Caminito`**: `wheel-glide`, on by default, and `wheel-glide-exceptions`, the app ids whose windows get whole clicks because they glide the wheel themselves. Its default is `firefox`, `chromium`, `google-chrome`, `brave-browser`, `code` and `code-oss`. As the animations of ADR 0033 do, it doesn't run when `org.mate.interface enable-animations` is off or `reduced-resources` is on.

## Consequences

- MATE's GTK3 applications, GTK4 and Qt6 applications and XI2 clients glide on the wheel. Qt5 Widgets applications still step once per click, and their step comes partway through the glide, when its 120ths reach a click.
- Caminito decides a client's wheel behaviour. A client that animates the wheel and isn't in the exceptions, such as a Qt Quick application or an Electron one other than VS Code, glides twice until the user adds it.
- A client redraws on each frame of a glide instead of once per click, as it does under a touchpad.
- Caminito holds state per seat between input events: the glide and its timing.
- Rejected:
  - carried patches to GTK and Qt (two toolkits and every version to keep up with, and still nothing for Xwayland's clients);
  - leaving it to applications (GTK and Qt Widgets have no setting for it);
  - a finger-source stream with no `value120` (GTK3's discrete-only widgets ignore it, and clients take it for a touchpad);
  - a kinetic coast after a fast spin (the developer wants a click's step, no more);
  - queueing a click's glide behind the last (it lags behind the wheel);
  - carrying what's left of a glide to the new surface under the pointer (it scrolls what the user didn't);
  - matching exceptions by Remap group (those are for Key remaps, and VS Code isn't a browser);
  - keys for the duration or the curve, as ADR 0033 says.
