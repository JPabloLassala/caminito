# A Caminito nested in the Session is too slow to use with foot and xterm in it

Type: implementation
Status: open
Milestone: none (proposed: it gates no milestone, since a nested run is the development loop's and not a user's)
Blocked by: none
Map: [Caminito v0.1](../map.md)

## What to build

A Caminito started with `tools/dev-nested` inside the Session, so Caminito on winit in a window of Caminito on the tty, was too slow to use once foot and xterm were open in it. The developer saw it on 2026-10-06 in [73](73-packaging-and-closing-m5.md)'s closing run, on `caminito-git r225.c221860-1`, and moved that run's last two items to the Session itself.

- **What's known:**
  - It was usable earlier in the same run, with pluma, caja and gtk3-demo in it.
  - Measured while it was slow: the nested Caminito at 0.9% of a CPU and the Session's at 9%, so neither was busy.
  - Nothing repeated in the nested Caminito's log. It had `Xwayland glamor: GBM Wayland interfaces not available` once, at its start.
  - The nested runs before that day had KDE as their host, and weren't slow.
- **Not known:** what was slow, typing, the pointer or redrawing; whether it's foot, xterm or the two together; and whether it's slow with another host.
- **To find out:**
  - What is slow, and with which client alone.
  - Whether the nested Caminito's frames wait on the Session's frame callbacks, and at what rate they come: a nested window that isn't drawn isn't sent them.
  - ~~The same nested run with KDE or another compositor as its host~~: measured below, and it isn't slow there.

## Acceptance criteria

- [x] What is slow is described: which input or drawing, with which clients, and on which hosts. Every frame of the nested Caminito, whichever client drew in it, and only with Caminito on the tty as its host; by measurement, below. What it looked like by hand wasn't asked.
- [x] The cause is named, and whether it's the nested Caminito's, the host Caminito's, or a client's. The host Caminito's.
- [ ] If Caminito changes, a test written before the change where a seam can show it, and the nested run with foot and xterm is usable in the Session, by the developer.

## Measured on other hosts (2026-10-06, night)

By Claude with commands alone, on the developer's machine in Plasma, with c14283a's debug build. Nobody was in the Session, so the first two criteria are still open: what follows narrows where to look, and names no cause.

- **[`93-measure.sh`](93-measure.sh)** starts `tools/dev-nested` with its side of the host's protocol logged (`WAYLAND_DEBUG=client`), runs foot, xterm and then both in it, each printing a character ten times a second for 8 s, and prints two waits for each: foot's for the nested Caminito's frame callback after a commit (`client`), and the nested Caminito's for the host's after its own (`host`). Then it leaves both windows open and asks what is slow by hand. It needs no eyes until that question.
- **KDE as the host isn't slow with foot and xterm.** Medians: `client` 4.4 ms with foot and 8.3 ms with both; `host` 8.4 ms with foot, 11.7 ms with xterm and 10.6 ms with both; the longest `host` wait was 41.8 ms. No commit was left unanswered but the last of a run.
- **Caminito as the host, on winit, isn't slow either.** A Caminito nested in a Caminito nested in Plasma: foot's median wait was 11.4 ms and its longest 36.6 ms, with xterm printing beside it. So the frame callbacks of `State::post_render`, which both backends share, come at the rate they should.
- **What's left is the tty backend as the host, with this client.** That inner Caminito wasn't the client the Session had: winit offers no linux-dmabuf, so it drew with llvmpipe into `wl_shm`. In the Session the nested Caminito draws with the NVIDIA driver (`egl-wayland2` 1.0.2, driver 615.71.09) and, as its log in Plasma shows:
  - every commit carries a dmabuf with an acquire and a release point, so on the tty each one goes through `block_on_dmabuf`'s wait on the acquire point. [57](57-drm-syncobj.md) never ran a client that sets them, on any GPU: its check went to [60](60-tty-checks-left-over.md). pluma, caja, gtk3-demo, foot and xterm in the Session all draw into `wl_shm`, so the nested Caminito may be the first such client the tty backend has had.
  - it has three buffers, used in turn, each with a timeline of its own.
  - the EGL library asks for a frame callback of its own at every swap, on its own queue, beside winit's: Smithay's `vsync: false` only picks the EGL config and never calls `eglSwapInterval`, so the swap interval is EGL's default of 1. It reads that callback at the next swap, so a swap waits there for as long as the host hasn't answered the frame before.
- **Why it may have been usable with pluma, caja and gtk3-demo:** those two items were worked with the pointer, on frames, which keeps both Caminitos drawing. Typing into a terminal is one frame at a time with nothing after it. A frame that's held until something else makes the host draw would pass unseen in the first and be unusable in the second, with both Caminitos idle, as they were measured. That's a guess to check, not a finding.
- **The Session's journal for that hour** has no line from Caminito between 18:41:57 and 18:58:28, so the host logged nothing at `info` while it was slow. `can't wait on an acquire point` is logged at `debug`.

To do, in the Session (`MATE on Caminito` from sddm), from this directory:

1. `./93-measure.sh`, and answer its question. A `host` wait near the 100 ms between characters, or commits left unanswered, says the host holds a frame until the next one; a quick `host` and a slow `client` says the nested Caminito does.
2. If the host holds them: the same with the Session's Caminito started with `RUST_LOG=info,caminito=debug`, for `block_on_dmabuf`'s lines, and with `__NV_DISABLE_EXPLICIT_SYNC=1 ./93-measure.sh`, which takes the acquire points away (checked in Plasma: 83 commits, none with an acquire point).

## Measured in the Session, and the cause (2026-10-06, 23:26)

By Claude with commands alone, in *MATE on Caminito* on `caminito-git r239.c14283a-1`, with `./93-measure.sh` and the installed build nested. Its question wasn't asked: the script had no terminal.

- **The host is what's slow.** The nested Caminito waited for the Session's frame callback 43.6 ms at the median with foot, 134 to 145 ms at the 90th percentile and up to 249.6 ms, where KDE answered in 8.4 ms, 16 ms and 21 ms. foot's own wait for the nested Caminito had a median of 3.8 ms, as on KDE, with a 90th percentile of 58 ms that follows from the host's. xterm alone, and both, measured the same as foot, so it isn't the clients.
- **The client is another one than on KDE.** Caminito offers fifo and commit-timing, which KDE doesn't, so NVIDIA's EGL paces itself with them instead of a frame callback. Each swap is two commits: the buffer with its acquire and release points, `wp_fifo_v1.set_barrier` and `wait_barrier`, and `wp_commit_timer_v1.set_timestamp` at the last presentation plus one refresh; then an empty one with `wait_barrier`.
- **The cause is the host Caminito's:** a commit-timing barrier was signalled only in `State::post_render`, after an output is drawn, and a commit that's held back never reaches `CompositorHandler::commit`, so nothing queued a draw for it. The frame waited for the Session to draw for some other reason: its own empty second commit a refresh later when the time had passed by then, and otherwise the pointer, the clock or another client. That's the two or three refreshes at the median and the quarter of a second at worst, with both Caminitos idle.
- **Why pluma, caja and gtk3-demo were usable:** those items were worked with the pointer, and each motion made the Session draw.

Fixed: a timer at each timed commit's timestamp signals the surface's barriers and lets the client's commits through (`State::release_at_commit_time`), beside `post_render`'s. Tested with a headless test written first, a commit timed 30 ms ahead with nothing else drawing, which was never shown before the change. The whole suite passes on a private session bus.

Left as they are:

- **The fifo barrier is still signalled only after a draw.** In this client's sequence the commit that sets it is applied, which queues one. A fifo client whose window is on no output, minimized or on another workspace, is never let through.
- **A timed frame is let through at its time and drawn after it,** so it's shown a refresh later than a compositor that draws ahead of the time would show it, and the empty second commit costs the tty a refresh with nothing to draw. Whether that holds the nested Caminito under the monitor's rate is for the run below.
- **A timer and its surface live until the timestamp,** however far ahead a client sets it.

To do, for the third criterion: install a build with the fix, log into *MATE on Caminito* again, and run `./93-measure.sh` from a terminal. The `host` medians should be near KDE's, and its question records what the developer sees.
