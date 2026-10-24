# 48. Caminito decides where Esmeril goes

Date: 2026-10-23
Status: Proposed

## Context

The developer wants Windows 7's Aero look in a Session, with see-through frames and a see-through panel that show a blur of what's beneath them. Marco themes can already draw frames with alpha, and Caminito draws that alpha, but nothing behind a see-through pixel is blurred, so the theme looks washed out, not like glass.

Plasma does it with KWin's Blur effect, for the surfaces that ask through `org_kde_kwin_blur`, and its Background contrast effect raises the contrast and saturation under them. `ext-background-effect-v1` is the newer protocol, and the vendored Smithay serves it. None of Caminito's clients asks: mate-panel, mate-terminal and the other MATE components are GTK3, which has no API for either protocol, and a Marco theme has no way to say where glass goes.

## Decision

- **Caminito draws Esmeril** (glossary): a dual Kawase blur of everything beneath, with raised saturation and contrast. It's live: a window moving beneath redraws it.
- **Caminito decides where it goes**, by kind, without any client asking:
  - frames, from the theme's own alpha;
  - Caminito's own popups: the Window menu, the Window switcher and the Workspace popup;
  - layer surfaces by namespace: `panel`, `mate-osd`, `desktop-menu` and the notification bubbles. Never the `desktop` or `mate-background`, and no unknown namespace;
  - the windows of the applications in `esmeril-applications`, matched by app id as `wheel-glide-exceptions` is (ADR 0039), behind their own surfaces but not their popups;
  - the wallpaper while the Overview or the Application overview is open, under the dimming and fading with it, at a fixed strength.
- **Each pixel's alpha weights it**, so an opaque pixel shows none and a wholly clear one shows none either: rounded corners and the clear outer band of a `-border` theme stay clear. The theme supplies the tint.
- **It fades with a window's alpha** in the animations. The Overview's thumbnails, the Window switcher's previews and a capture of one window don't get it; a capture of an output does.
- **The shaders are Caminito's own**, written from the published dual Kawase algorithm, and take from KWin its numbers: the passes and offsets of each strength step, and the contrast and saturation. As in ADRs 0033 to 0037, KWin's behaviour is taken and its code isn't.
- **Settings in `org.mate.Caminito`**: `esmeril`, on by default; `esmeril-strength`, 1 to 15 as KWin's, at KWin's default; and `esmeril-applications`, by default `mate-terminal`. caminito-behaviour shows them with the effects. Esmeril isn't drawn while `org.mate.interface reduced-resources` is on; `enable-animations` doesn't touch it, as it doesn't move.
- **Tests**: the in-process tests on Pixman check where Esmeril goes, its mask and its damage, with the element drawing nothing there. A headless GLES backend, on surfaceless EGL, runs the few tests of the blur's pixels, and they're skipped where EGL isn't there.

## Consequences

- A translucent Marco and GTK3 theme looks like glass with no carried patch to any MATE component.
- It's Caminito's first GLSL. It's drawn as a framebuffer-effect element of the vendored Smithay, whose damage tracker redraws what's behind it and lets nothing above stand in for it; such an element is never scanned out.
- Every see-through pixel of a listed application gets Esmeril, wanted or not. Listing an opaque application costs GPU time and changes nothing on screen.
- A client that wants Esmeril can't ask for it until Caminito serves `ext-background-effect-v1`, which is left to a ticket of its own.
- Rejected:
  - blurring every see-through pixel of every client, as Compiz's blur could (drag icons, client-side shadows and tooltips get it too);
  - carried patches to mate-panel and mate-terminal so they ask through `ext-background-effect-v1` (GTK3 has no API for it, so each would speak the protocol itself);
  - a Caminito addition to the Marco theme format saying where glass goes (no theme would use it);
  - blurring only the wallpaper, as Windows 11's Mica does (cheaper, but not Aero);
  - a Caminito tint or a glow behind the title (they'd fight the theme's own);
  - KWin's shaders copied with their copyright (about 20 lines each, and the Plasma ports take no code);
  - a CPU blur on Pixman for the tests (two blurs to keep alike);
  - settings for the contrast and saturation;
  - a strength that grows as the Overview opens (a new blur at every frame of it).
