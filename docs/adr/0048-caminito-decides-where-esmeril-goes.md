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
  - layer surfaces by namespace: `panel`, `mate-osd`, `desktop-menu` and the notification bubbles. Never the `desktop` or `mate-background`, and no unknown namespace. Their popups get it too, the panel's menus and tooltips, so a client-side shadow on one does (CMN-232);
  - the windows of the applications in `esmeril-applications`, matched by app id as `wheel-glide-exceptions` is (ADR 0039), behind their own surfaces but not their popups;
  - the wallpaper while the Overview or the Application overview is open, under the dimming and fading with it, at the user's strength, which doesn't grow as it opens.
- **Each pixel's alpha weights it**, so an opaque pixel shows none and a wholly clear one shows none either: rounded corners and the clear outer band of a `-border` theme stay clear. The theme supplies the tint.
- **It fades with a window's alpha** in the animations. The Overview's thumbnails, the Window switcher's previews and a capture of one window don't get it; a capture of an output does.
- **The shaders are Caminito's own**, written from the published dual Kawase algorithm, and take from KWin its numbers: the passes and offsets of each strength step, and the colour transform with the saturation of Plasma's default. The contrast of 1.2 is Caminito's own, as Plasma's default contrasts are below 1 and go with an intensity that makes up for them (CMN-231). As in ADRs 0033 to 0037, KWin's behaviour is taken and its code isn't.
- **Settings in `org.mate.Caminito`**: `esmeril`, on by default; `esmeril-strength`, 1 to 15 as KWin's, at KWin's default; and `esmeril-applications`, by default `mate-terminal`. caminito-behaviour shows them with the effects. Esmeril isn't drawn while `org.mate.Marco.general reduced-resources` is on; `enable-animations` doesn't touch it, as it doesn't move.
- **Tests**: the in-process tests on Pixman check where Esmeril goes, its mask and its damage, with the element drawing nothing there. A headless GLES backend, on surfaceless EGL, runs the few tests of the blur's pixels, and they're skipped where EGL isn't there.

## Clients that ask (CMN-238)

- Caminito serves `ext-background-effect-v1`, for any surface of a window, a popup or a layer surface. `org_kde_kwin_blur` isn't served; a later ticket can add it if a Qt5 client needs it.
- A client's region replaces a listed application's or a namespace's whole-surface Esmeril, on that surface only. The window's other surfaces keep theirs.
- In the region, Esmeril goes under every pixel, a wholly clear one too, as the protocol has the region alone say where the blur goes. The surface is drawn over it as usual. The alpha rule above stays for the places Caminito chooses.
- `esmeril` off and `reduced-resources` win over a client that asks. `capabilities` then has no `blur`, and is sent again when that changes. It follows the settings alone, not the renderer.
- A window's or a layer surface's Esmeril for its own surfaces goes under all of them, so a subsurface below its parent hides the Esmeril of its parent's region, and an opaque parent hides the Esmeril of its subsurface's region. A popup's goes just under the popup. Only a lock surface's tree has it between its layers, right under the surface that asks (CMN-301).
- As for a listed window, a region has no Esmeril in the animations, and a closing popup's picture has none as it slides. An empty region asks for none, and so removes a listed surface's.

## Consequences

- A translucent Marco and GTK3 theme looks like glass with no carried patch to any MATE component.
- It's Caminito's first GLSL. It's drawn as a framebuffer-effect element of the vendored Smithay, whose damage tracker redraws what's behind it and lets nothing above stand in for it; such an element is never scanned out.
- Every see-through pixel of a listed application gets Esmeril, wanted or not. Listing an opaque application costs GPU time and changes nothing on screen.
- A client that wants Esmeril can ask for it through `ext-background-effect-v1` (CMN-238, above).
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
