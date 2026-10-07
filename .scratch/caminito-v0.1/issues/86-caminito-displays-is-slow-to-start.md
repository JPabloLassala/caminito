# caminito-displays takes a long time to start

Type: implementation
Status: resolved
Milestone: M6
Blocked by: none
Map: [Caminito v0.1](../map.md)

## What to build

caminito-displays ([84](84-caminito-displays.md)) takes a long time to show its window. The developer saw it on 2026-10-05, with no time measured. This ticket measures it, finds where the time goes, and says whether start-up time is to be watched from here on.

- **To measure first:** the time from the start of the process to the first frame of the window, cold and warm, in a Session and with `--demo`. `--demo` talks to no compositor beyond GTK's own connection, so the two together say whether the time is GTK's or the program's.
- **Where it may go, none of it checked:**
  - GTK4's start: its renderer (the GL or Vulkan one against cairo, `GSK_RENDERER`), the theme and the fonts.
  - The second Wayland connection, for output management and Identify, and the round trips before the first set of monitors is known.
  - The bus: `org.mate.Caminito`, and a call that waits for its answer before the window is shown.
  - The build: whether what was run was a debug build or the package's.
  - The launcher: the control center's entry and the shadowed desktop entry, before the process starts at all.
- **Whether to keep an eye on it** is to decide here: a number written down for the next ticket to compare with, or a check that fails when it gets worse.

## Acceptance criteria

- [x] The times are written here, with how they were taken and on which build.
- [x] The cause of the longest part is named, and fixed here or left to a ticket of its own.
- [x] It is decided whether start-up time is watched, and how.

## Notes

- **"The panel"** in the developer's report is read as caminito-displays's window, since it came with [85](85-caminito-displays-hangs-on-a-resolution.md). If mate-panel was meant, this ticket is about the wrong program.
- **From 85:** the Session's log of 2026-10-05 (`~/.local/share/sddm/wayland-session.log`) has `Failed to register: Timeout was reached` three times. caminito-displays is a unique application, so a second start asks the first to show its window, and waits for a first one that hangs (85) until the bus gives up. Some of the slow starts may be that, and gone with 85. Not checked.

## Resolution

**A start takes about 0.3 s. The long one is a start beside an instance that hangs:** it waits 25 s for the bus to give up, and shows no window at all. That instance is 85's, which is fixed, so nothing in the program changed here.

### The times

From the start of the process to the first frame, the first commit of a surface with a buffer attached, in milliseconds. Taken with `tools/first-frame`, new here, which runs the program with `WAYLAND_DEBUG=client` and stamps the protocol as it comes. In the Session of 2026-10-05, on Caminito r191.78c06ad with two monitors (HDMI-A-1 and DP-1), an i7-6700K, an RTX 3060 with NVIDIA's driver 615.71.09, GTK 4.22.5, whose renderer here is Vulkan.

| What was run | First frame, ms |
|---|---|
| The package's (`caminito-git` r191.78c06ad), in the Session, warm, 9 runs | 265 to 313 |
| The same from its desktop entry, `gio launch`, 3 runs, the program ended by hand after each | 282 to 368 |
| This checkout's release build (146b5e9), in the Session, 4 runs | 269 to 317 |
| This checkout's debug build, in the Session, 6 runs | 266 to 320 |
| The release build with `--demo`, 7 runs | 231 to 299 |
| The package's with `GSK_RENDERER=cairo`, 7 runs | 103 to 141 |
| The package's with `GSK_RENDERER=gl` or `ngl`, warm, 3 runs | 236 to 279 |
| The package's with `GSK_RENDERER=gl`, its first run | 1154 |
| The package's with an empty `XDG_CACHE_HOME`, 3 runs | 1354 to 1387 |
| The package's beside a first instance that answers | no frame of its own: it exits in 41 to 49 |
| The package's beside a first instance that's stopped (`kill -STOP`) | no frame: `Failed to register: Timeout was reached` after 25.1 s |

### Where the time goes

- **The long start is the wait for a hung instance.** caminito-displays is a unique application, so a start beside another hands itself to it over the bus and exits. If that one doesn't answer, the start waits the bus's 25 s, prints `Failed to register: Timeout was reached` and exits with no window. The Session's log of the day has that line three times, and then `Failed to register: … NoReply: Remote peer disconnected`, which is a fourth start that was waiting when the hung instance was killed. So what the developer saw is taken to be this: a resolution chosen, the hang of 85, and starts that waited for it. It is the note from 85, now checked, with a stopped instance standing in for the hung one. Such a start never shows a window, where the report was of one that took long to show: the window came once the hung instance was killed.
- **The other start that is slow is the first on an empty cache,** below, which does show its window, after 1.4 s. Which of the two the developer meant isn't known. The log speaks for the 25 s.
- **Of a warm start, two thirds is GTK's renderer.** The window is asked for at about 230 to 260 ms with Vulkan and at about 80 to 110 with cairo, so starting the GPU renderer is about 150 ms of the 280. It is GTK's and the driver's: `--demo` takes the same.
- **The first start on an empty cache takes 1.4 s,** 1.1 s of it between the window being asked for and its first frame: the driver compiles the renderer's shaders and keeps them (`$XDG_CACHE_HOME/nvidia/GLCache` here). Once per cache, so after a first login or a driver update. The second run on the same cache is back to 282.
- **The theme, the fonts and the page are within 45 ms.** With cairo the window is asked for about 45 ms after the first message, and that is all of them, with the second connection and the bus. They weren't told apart.
- **The second connection and the bus are small.** The monitors are known about 30 ms after the first message, and 1 to 7 ms after the second connection's registry. The call that waits before the window is shown is the proxy of `org.mate.Caminito` (`for_bus_sync` in `compositor.rs`). It wasn't timed by itself: `--demo`, which makes neither the connection nor the proxy, is within the noise of a start in the Session, and the 45 ms above holds both. A start beside a healthy instance, which is the application's own registration on the bus, is done in 45 ms.
- **The build doesn't matter:** debug and release take the same, since the time is GTK's.
- **The launcher doesn't either:** the desktop entry, through `gio launch`, is within the noise of the program run by itself. The control center's own entry wasn't driven.

### What is done about it

- **The cause of the longest part** is the hang of 85, fixed in 146b5e9. A start beside a hung instance still waits 25 s and shows nothing, whatever makes an instance hang. Nothing is known to now, so it's left: see below.
- **The renderer is left as GTK chooses it.** cairo would take 150 ms off every start and the 1.1 s off the first, but GTK's cairo renderer draws at a whole scale, and 84 has the page sharp at 1.5. 0.3 s isn't the complaint.

### Whether it is watched

**A number written down, and a tool to take it again. Not a check that fails.** The time is GTK's and the GPU driver's, it needs a compositor and a display, which CI doesn't have, and the warm runs are 50 ms apart among themselves.

- `tools/first-frame [-n <runs>] caminito-displays` prints the marks of each run. It works for any Wayland client.
- `docs/testing/checklist.md` has *caminito-displays shows its window at once*, with the figures to compare with: about 0.3 s warm, and a second start beside the first that exits at once.

Not known, or not done:

- **A cold start after boot** wasn't measured. Dropping the page cache needs root, which wasn't used, and `posix_fadvise` evicted none of the program's 114 files. The empty `XDG_CACHE_HOME` is the nearest to it that was.
- **That the developer's slow starts were the 25 s ones** is read from the log and the stopped instance. Nobody timed them on the day.
- **A start that waits for a hung instance** could say so, or show a window of its own after a shorter wait. Not done, with no hang known: it is a ticket of its own if one comes back.
- **Whether a second start raises the window** of the first wasn't looked at: only that it exits at once.
- **With the Session's own renderer only.** Another GPU or driver has other figures, and Mesa's weren't taken.
