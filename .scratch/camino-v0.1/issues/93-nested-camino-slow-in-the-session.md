# A Camino nested in the Session is too slow to use with foot and xterm in it

Type: implementation
Status: open
Milestone: none (proposed: it gates no milestone, since a nested run is the development loop's and not a user's)
Blocked by: none
Map: [Camino v0.1](../map.md)

## What to build

A Camino started with `tools/dev-nested` inside the Session, so Camino on winit in a window of Camino on the tty, was too slow to use once foot and xterm were open in it. The developer saw it on 2026-10-06 in [73](73-packaging-and-closing-m5.md)'s closing run, on `camino-git r225.c221860-1`, and moved that run's last two items to the Session itself.

- **What's known:**
  - It was usable earlier in the same run, with pluma, caja and gtk3-demo in it.
  - Measured while it was slow: the nested Camino at 0.9% of a CPU and the Session's at 9%, so neither was busy.
  - Nothing repeated in the nested Camino's log. It had `Xwayland glamor: GBM Wayland interfaces not available` once, at its start.
  - The nested runs before that day had KDE as their host, and weren't slow.
- **Not known:** what was slow, typing, the pointer or redrawing; whether it's foot, xterm or the two together; and whether it's slow with another host.
- **To find out:**
  - What is slow, and with which client alone.
  - Whether the nested Camino's frames wait on the Session's frame callbacks, and at what rate they come: a nested window that isn't drawn isn't sent them.
  - ~~The same nested run with KDE or another compositor as its host~~: measured below, and it isn't slow there.

## Acceptance criteria

- [ ] What is slow is described: which input or drawing, with which clients, and on which hosts.
- [ ] The cause is named, and whether it's the nested Camino's, the host Camino's, or a client's.
- [ ] If Camino changes, a test written before the change where a seam can show it, and the nested run with foot and xterm is usable in the Session, by the developer.

## Measured on other hosts (2026-10-06, night)

By Claude with commands alone, on the developer's machine in Plasma, with c14283a's debug build. Nobody was in the Session, so the first two criteria are still open: what follows narrows where to look, and names no cause.

- **[`93-measure.sh`](93-measure.sh)** starts `tools/dev-nested` with its side of the host's protocol logged (`WAYLAND_DEBUG=client`), runs foot, xterm and then both in it, each printing a character ten times a second for 8 s, and prints two waits for each: foot's for the nested Camino's frame callback after a commit (`client`), and the nested Camino's for the host's after its own (`host`). Then it leaves both windows open and asks what is slow by hand. It needs no eyes until that question.
- **KDE as the host isn't slow with foot and xterm.** Medians: `client` 4.4 ms with foot and 8.3 ms with both; `host` 8.4 ms with foot, 11.7 ms with xterm and 10.6 ms with both; the longest `host` wait was 41.8 ms. No commit was left unanswered but the last of a run.
- **Camino as the host, on winit, isn't slow either.** A Camino nested in a Camino nested in Plasma: foot's median wait was 11.4 ms and its longest 36.6 ms, with xterm printing beside it. So the frame callbacks of `State::post_render`, which both backends share, come at the rate they should.
- **What's left is the tty backend as the host, with this client.** That inner Camino wasn't the client the Session had: winit offers no linux-dmabuf, so it drew with llvmpipe into `wl_shm`. In the Session the nested Camino draws with the NVIDIA driver (`egl-wayland2` 1.0.2, driver 615.71.09) and, as its log in Plasma shows:
  - every commit carries a dmabuf with an acquire and a release point, so on the tty each one goes through `block_on_dmabuf`'s wait on the acquire point. [57](57-drm-syncobj.md) never ran a client that sets them, on any GPU: its check went to [60](60-tty-checks-left-over.md). pluma, caja, gtk3-demo, foot and xterm in the Session all draw into `wl_shm`, so the nested Camino may be the first such client the tty backend has had.
  - it has three buffers, used in turn, each with a timeline of its own.
  - the EGL library asks for a frame callback of its own at every swap, on its own queue, beside winit's: Smithay's `vsync: false` only picks the EGL config and never calls `eglSwapInterval`, so the swap interval is EGL's default of 1. It reads that callback at the next swap, so a swap waits there for as long as the host hasn't answered the frame before.
- **Why it may have been usable with pluma, caja and gtk3-demo:** those two items were worked with the pointer, on frames, which keeps both Caminos drawing. Typing into a terminal is one frame at a time with nothing after it. A frame that's held until something else makes the host draw would pass unseen in the first and be unusable in the second, with both Caminos idle, as they were measured. That's a guess to check, not a finding.
- **The Session's journal for that hour** has no line from Camino between 18:41:57 and 18:58:28, so the host logged nothing at `info` while it was slow. `can't wait on an acquire point` is logged at `debug`.

To do, in the Session (`MATE on Camino` from sddm), from this directory:

1. `./93-measure.sh`, and answer its question. A `host` wait near the 100 ms between characters, or commits left unanswered, says the host holds a frame until the next one; a quick `host` and a slow `client` says the nested Camino does.
2. If the host holds them: the same with the Session's Camino started with `RUST_LOG=info,camino=debug`, for `block_on_dmabuf`'s lines, and with `__NV_DISABLE_EXPLICIT_SYNC=1 ./93-measure.sh`, which takes the acquire points away (checked in Plasma: 83 commits, none with an acquire point).
