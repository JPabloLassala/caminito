# Plugging or unplugging a monitor blanks the other one

Type: implementation
Status: open
Milestone: before v0.1
Blocked by: none
Map: [Camino v0.1](../map.md)

## What to build

When a monitor is plugged in or unplugged on the tty, the monitor that stays goes black for a short time. The *Hot-plugging a monitor* checklist item expects that it never goes black or freezes. [60](60-tty-checks-left-over.md) saw it on 2026-10-06, in the developer's Session on `camino-git r195.9e4baac-1` (NVIDIA RTX 3060, driver 615.71.09, kernel 7.2.8): the LG on HDMI-A-1 at 3840x2160 and 59.94 Hz, the AOC on DP-1 at 1920x1080 and 144 Hz, with a layout saved for the two ([83](83-saved-layouts.md)).

- **It happened all four times:** unplugging and re-plugging the AOC blanked the LG, and unplugging and re-plugging the LG blanked the AOC. How long it lasted wasn't measured. Nothing froze, and no window was lost.
- **The log has no `can't light` line,** and no warning at an unplug. At each re-plug it has two rounds of `Adding connector` and `Setting new mode` for the monitor that came back, a second apart, and one `Failed to destroy old mode property blob: No such file or directory`.
- **Not known:** whether the blanking is Camino's or the driver's. The first things to look at:
  - `Gpu::connect` lights a re-plugged monitor in its preferred mode, and the saved layout's restore then switches it to the saved one (60 Hz, then 144 Hz for the AOC). That's two mode sets where one would do, and it's what the two rounds in the log look like.
  - Whether the commit that lights or drops one CRTC touches the other's, and whether a mode set on one head blanks all of them on NVIDIA whatever the compositor does. Another compositor on the same machine would tell the two apart.
  - An unplug sets no mode on the monitor that stays, yet it blanked too.

## Acceptance criteria

- [ ] The cause is written down in this ticket's notes: Camino's, the driver's, or both.
- [x] What is Camino's is fixed: a re-plugged monitor with a saved layout is lit once, in the saved mode.
- [ ] On the developer's machine, the monitor that stays shows its picture without a break through an unplug and a re-plug of the other one, or what's left is in the README's *Known limitations* with its cause.

## Notes

- **The cause isn't known yet (2026-10-06).** One thing was Camino's and is fixed, the two mode sets at a re-plug. Nothing was found, in Camino or in the driver's source, that takes the other monitor's picture away, so the first and third criteria stay open for a run on the developer's machine.
- **What Camino did, from the journal of [60](60-tty-checks-left-over.md)'s run and Smithay's code at 928d4a9:**
  - **At a re-plug, two mode sets on the monitor that came back,** 0.27 s apart for the AOC (15:48:32.989 and 15:48:33.258) and 0.34 s for the LG. The first is `initialize_output`'s commit in the preferred mode. The second is the first frame after the restore's `use_mode`. Both monitors' saved modes differ from their preferred ones: the AOC's is 144 Hz against 60, the LG's 59.94 Hz against 60.
  - **At an unplug, one commit and no mode set on the monitor that stays.** No layout is saved for either monitor alone, so nothing is restored, and the journal has no `Setting new mode` then. The commit comes when the monitor's `DrmOutput` is dropped: `AtomicDrmSurface::clear_state` turns its planes, connector and CRTC off, with `ALLOW_MODESET`.
  - **No commit names the other CRTC.** Each `AtomicDrmSurface` builds its requests from its own CRTC, connectors and planes (`surface/atomic.rs`, `commit` and `clear_state`). `DrmOutputManager` redraws the other outputs, in black, only when lighting fails for want of bandwidth, and it warns then (`failed to initialize crtc, trying to lower bandwidth`): the journal has no such line.
  - **Each commit blocks the loop:** Smithay commits a mode set without `NONBLOCK`, so no monitor gets a frame until it returns. That freezes the other monitor's picture and doesn't blank it. A scan blocks it too, about 0.27 s between udev's event and the new output, and an unplug sent three udev events and so three scans.
  - `Failed to destroy old mode property blob` is Smithay's, at a surface's first commit, when there's no old blob. It's harmless.
- **What the driver does, from the source of nvidia-open 615.71.09** (`NVIDIA/open-gpu-kernel-modules`): nvidia-drm asks NVKMS only for the heads in the atomic state (`nv_drm_atomic_apply_modeset_config`, `headsMask`), and NVKMS shuts down only the heads it marks as changed (`nvkms-modeset.c`: "If nothing changed about this head's configuration, then we should not shut it down"). So the source doesn't say a mode set on one head blanks the others. Some steps do run on every head at each mode set, without turning it off: the base channels are idled, raster lock is dropped, and VRR is turned off and on (`nvSetDispModeEvo`, `nvDisableVrr`). What the RM and the firmware do under them (`nvRmBeginEndModeset`) wasn't read. No report of this on another compositor was found; the search was short.
- **Not ruled out, and not Camino's mode sets:** at the LG's re-plug mate-panel died (`general protection fault` in `libxfce4windowing-0.so`, 15:49:35.96), and each hot-plug makes `mate-background` put up new wallpaper surfaces ([88](88-wallpaper-surfaces-pile-up-on-hot-plug.md)). Either can take the desktop or the panels away for a moment. That would leave the windows on screen, where a mode set takes the whole picture, and 60's run didn't say which it was. The panel's crash has no ticket.
- **Fixed: a monitor is lit once, in the mode it's to show.** The tty adds a monitor a scan finds as an output and doesn't light it. `State::monitors_scanned`, which the backends now call in place of `restore_saved_layout`, restores the saved layout and then lights what was found, in the mode its output has: the saved one, or the preferred one with no saved layout. So start-up has one mode set per monitor too, where [83](83-saved-layouts.md) left two. A monitor saved as disabled isn't lit at all. One the GPU can't light in its saved mode is lit in its preferred one, with the rest of the layout restored, and one it can't light at all is left out, as before. The headless backend holds a monitor unlit the same way and counts its mode sets, and `saved_layouts`'s tests read them. The tty's own code isn't run by any test, so the second criterion is ticked on the headless tests and the reading of the code, and the journal of the next run is its check.
- **A gap this leaves:** the layout saved for the monitors found is restored before they're lit. If one of them then can't be lit at all, the others keep what that layout gave them, unless a layout is saved for them alone. Before, such a monitor was gone before any restore. If that layout disabled the others, nothing is lit, as when the only enabled monitor is unplugged from beside a disabled one, which was so before this.
- **Left for the developer's machine,** with a `camino-git` that has this:
  - Unplug and re-plug each monitor, and say whether the other one loses its whole picture (the monitor's own "no signal", or a black screen with no pointer) or only its wallpaper or panels. The journal should have one `Setting new mode` per re-plug.
  - If the whole picture still goes, and at an unplug it wasn't touched by this fix: the same four changes under another compositor (sway, or KWin), which tells Camino and Smithay from the driver. If it's the driver's, it goes in the README's *Known limitations*.
  - `wlr-randr --output DP-1 --off` and `--on` make the same two commits with no cable pulled and no udev event, which tells the commit from the scan.
- **A run in `caminotest`'s Session didn't blank at an unplug (2026-10-06, 16:09 to 16:10).** The developer unplugged a monitor there and the other stayed on. Which monitor, and what the re-plug did, weren't said. It was the build before the fix: `camino-git r210.ad13fa1-1` was installed at 16:08, but no one logged in after that, and the Camino on tty4 had been running since 15:36, so it was r195 still. So the same driver and the same code didn't blank there, which speaks against a mode set on one head blanking the others whatever the compositor does, and for something of the developer's Session. What differs: `caminotest` has no saved layout, so the AOC shows 60 Hz there and not 144 Hz and the LG 60 Hz and not 59.94 Hz, at the default scales; and its desktop's clients may differ. The fix hasn't run on hardware yet: that takes a login after 16:08.
- **The fix ran on hardware (2026-10-06, 16:14),** in the developer's Session on tty1 after a new login, on `camino-git r210.ad13fa1-1`. From the journal:
  - **Start-up:** one mode set per monitor, each straight in its saved mode: the LG's surface with the 59.94 Hz mode (clock 593410) and the AOC's with the 144 Hz one (clock 327140), where the preferred ones are 533250 and 148500.
  - **The AOC unplugged at 16:14:53 and plugged back at 16:14:55:** one surface, in the 144 Hz mode, and one `Setting new mode`, 0.26 s after udev's event. No second round, no `can't light` line, and no crash in the kernel's log.
  - **Not yet said or done:** what the LG showed through it, and the LG's own unplug and re-plug.
