# caminito-wallpaperd draws a picture or a colour on each output

Epic: wallpaper
Type: implementation
Status: ready-for-agent
Spec: [Wallpaper](../wallpaper/spec.md)

## What to build

The tracer bullet of the epic: the new crate `caminito-wallpaper` and its drawer, `caminito-wallpaperd`, under "Who draws", "An output's wallpaper" and "The settings" in [the spec](../wallpaper/spec.md), and [ADR 0031](../../docs/adr/0031-caminito-wallpaper-draws-the-wallpaper-in-place-of-msds-plugin.md). Only the Picture and Colour kinds; the other three are later tickets and their settings are read as a Colour until then.

- **The drawer** is a layer-shell client with one surface per output in the background layer, in the namespace Caminito's scene already orders behind caja's desktop. It links no GTK.
- **The store** is gsettings under `org.mate.Caminito`: one relocatable set of keys per monitor identity, a "same on all outputs" key, and the lists of added pictures and folders. Changes are read live.
- **A monitor's identity** is a saved layout's: make, model and serial, or the connector when it reports none.
- **An output never seen before** takes the primary output's wallpaper. While "same on all outputs" is on, every output has the same settings.
- **Positioning:** Fill (the default), Fit, Stretch, Centre and Tile, with the output's colour in the bars Fit and Centre leave. It follows the output's mode and scale.
- **Every change cross-fades** over about a second.
- **In a Session** the drawer starts with the session and is restarted if it exits, and mate-settings-daemon's background plugin is off. Nothing changes outside a Session.
- **On first run** the picture `org.mate.background` names, with its positioning, becomes every output's wallpaper.
- `caminito-git`'s PKGBUILD installs the drawer.

## Acceptance criteria

- [ ] Unit tests of the toolkit-free parts: each positioning's rectangle for a picture and an output of given sizes; the settings an output gets from the store by identity, with "same on all outputs" on and off, and for a monitor never seen.
- [ ] A headless test with the real drawer against Caminito: one background-layer surface per output, a second output added gets one, and a removed output's is gone with none left over (ticket 88's failure).
- [ ] A change of a key is drawn without a restart.
- [ ] First run takes MATE's picture, and a second run doesn't take it again.
- [ ] [checklist.md](../../docs/testing/checklist.md) has an item, in a Session with two monitors: a different picture on each set with `gsettings`, each positioning by eye, the cross-fade, unplugging and plugging a monitor, and `killall caminito-wallpaperd` bringing the wallpaper back.

## For the implementing agent to settle

Recorded in the Done section:

- the schema's keys and the path a monitor's identity becomes;
- which crates draw and decode, given that the drawer links no GTK;
- how the Session starts and restarts the drawer, and how the plugin is turned off there and nowhere else.
