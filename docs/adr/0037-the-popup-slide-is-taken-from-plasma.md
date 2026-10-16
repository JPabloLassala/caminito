# 37. The Popup slide is taken from Plasma

Date: 2026-10-16
Status: Accepted

## Context

A menu that opens is there in one frame, and one that closes is gone in the next, as in Marco. ADR 0034 left popups and override-redirect windows out of the Opening and Closing animations, since Plasma animates them with effects of their own. The developer wants menus to open as Plasma's do, and chose its Sliding popups effect over its Fading popups.

Plasma has two effects for popups (KWin's `src/plugins/`):

- **Fading popups** (`fadingpopups/package/contents/code/main.js`), on by default for menus, tooltips and override-redirect windows: a linear fade in over 150 ms, and an OutQuart fade out over 600 ms.
- **Sliding popups** (`slidingpopups/slidingpopups.cpp`): a window slides out of a screen edge over 200 ms on an OutCubic curve, and back into it on the same curve reversed. It travels its own size or 8 lines of the font, whichever is less, fades as well only when that is less than its own size, and is cut off at the edge it comes out of. A slide that is turned round goes back from where it is. It only moves a window that names its edge and offset, through `org_kde_kwin_slide` or the `kwin_slide` property: Plasma's applet popups and the on-screen keyboard. A plain menu never names one.

No MATE client names a slide edge, so Caminito has to choose one.

## Decision

- **Caminito has the Popup slide, taken from Plasma's Sliding popups:** 200 ms in on a cubic ease-out, and 200 ms out on the same curve played backwards. It travels the popup's size along the slide or 8 lines of the interface font, whichever is less, and fades as well only when that is less than its size. It is cut off at its own final rectangle, so it comes out of the edge it is attached to.
- **Caminito chooses the edge from where the popup is attached**, since no client names one:
  - an xdg_popup, of a window or of a Layer surface, slides away from its anchor rectangle, as the positioner placed it after unconstraining: down when it lies below the anchor, up when above, and sideways when beside it. One that overlaps its anchor, as a combo box's list does its button, slides down. A panel's menu thus comes out of the panel, a menubar's menu down from the menubar, and a submenu out of its parent menu.
  - an override-redirect window has no anchor, so it slides away from the pointer as it was when the window mapped: down when the pointer is above its middle, up when below, and so on.
  - the Window menu slides down from where it was asked for, or up when it was turned to fit its output, and its submenu slides out of its parent's side.
- **The popups are Plasma's Fading popups' windows, with menus' edges**: every xdg_popup, every override-redirect window but a Utility or a DND one, and the Window menu with its submenu. The Window switcher and the Workspace popup don't slide: they aren't attached to anything, and Plasma's window switcher doesn't slide either.
- **Closing slides back,** drawn from a picture of the popup's last frame kept as the Closing animation keeps one, after policy has let it go. A popup with no buffer left goes at once. A popup whose window closes leaves with its window's Closing animation, not a slide of its own.
- **It follows ADR 0033's rules.** Policy changes at once and only the drawing takes time: a popup has its grab, the keyboard and clicks at its real place from its first frame. It runs when `compositing-manager` is on, `reduced-resources` off and `org.mate.interface enable-animations` on, and when its own key in `org.mate.Caminito`, `popup-slide`, is on, which it is by default. One key covers in and out.
- **An interrupted slide goes back from where it is,** which is Plasma's own behaviour here.
- **Inside the Overview, during its transition and under the lock, it doesn't run**, as Plasma skips its effects under a full-screen effect. During a Workspace slide a popup moves with its window and slides as well.

The spec has the behaviour in full: [Spec: Popup slide](https://linear.app/jplassala/document/spec-popup-slide-spec-ee64754f78aa).

## Consequences

- Caminito differs from Marco in what a menu opening or closing looks like. A user who wants Marco's turns `popup-slide` off.
- Caminito slides windows that Plasma only fades. A Plasma user sees a slide only on panel popups, and a Caminito user sees one on every menu, tooltip and combo box list.
- Tooltips slide too, since an xdg_popup has no type to tell a tooltip from a menu.
- The Scene draws popups that aren't in policy: a closing popup's picture. It is clicked through, and no query of policy sees it.
- Rejected:
  - Fading popups, Plasma's default for menus (the developer chose the slide);
  - sliding only the panel's popups and fading the rest (the developer wants every menu to slide);
  - one edge for every menu, top-down (a menu above the bottom panel would come down from inside the panel's opposite side);
  - opening only, with closing at once (Plasma slides both ways, and the Closing animation already keeps pictures);
  - a scale as well as the slide (it isn't Plasma's);
  - sliding the Window switcher and the Workspace popup (they aren't attached to anything to come out of);
  - keys for the length, the curve or the distance, as ADR 0033 says.
