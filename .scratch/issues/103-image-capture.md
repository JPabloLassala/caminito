# Image capture

Epic: caminito-v0.1
Type: implementation
Status: resolved
Milestone: M6
Blocked by: none
Map: [Caminito v0.1](../caminito-v0.1/map.md)

## What to build

A client can capture an output or a window: ext-image-capture-source with output and toplevel sources, and ext-image-copy-capture ([06](06-v01-protocol-and-integration-set.md), [17](17-milestone-slicing.md)). mate-screenshot is the user ([104](104-screenshots.md)).

- **Smithay's states** for both protocols, as 06 found them at the pinned `rev`. Check what they leave to the compositor before starting.
- **Output sources** render what the output shows, at its mode's size, through the scene's own element order, so a capture can't differ from the screen.
- **Toplevel sources** are made from an ext-foreign-toplevel-list handle. The capture is the window with its Frame and without its shadow, alone, at the window's scale, whatever covers it and whichever workspace it's on. A minimised or shaded window is captured as it was last drawn, or the session is stopped: decide here and write it down.
- **Sessions:** shm buffers at least, and dmabuf if Smithay's state makes it cheap. Damage is reported, and a session whose source changes size gets new constraints. A source that goes away stops its sessions.
- **The cursor:** the session's `paint_cursors` option is honoured. Cursor sessions aren't needed by mate-screenshot; offer them only if Smithay's state does it without more code.
- **Privileged:** both globals join the ones hidden from `wp_security_context_v1` clients ([68](68-data-control-and-privileged-globals.md) left a place for them).
- **Test client:** both protocols in `caminito-test-client`, as a screenshot tool would use them ([14](14-development-and-testing-loop.md)).

## Acceptance criteria

- [x] Headless tests, written before the code:
  - an output capture has the output's size and the pixels of a client's known colour where its window is;
  - a toplevel capture has the window's size with its Frame, and is the same when another window covers it;
  - a capture at an output scale of 2 has twice the pixels;
  - `paint_cursors` puts the pointer in the capture, and without it the pointer is absent;
  - resizing the window sends the session new constraints;
  - closing the window stops the session.
- [x] `a_client_lists_the_globals` gains the three globals, and the security-context test's list of privileged globals gains them.

## Notes

- **Shadows are M8's,** so "without the shadow" has nothing to leave out yet. Write the test that pins the capture's size to the Frame's, so M8 can't grow it.
- **While locked** a capture shows the lock, never the session behind it. That's M7's (17), since there's no lock until then.
- **wlr-screencopy stays deferred** (06).

## Decided while building

- **What Smithay's states leave to Caminito:** what a source is (Caminito keeps an output or a window's id in its user data), the constraints, every frame's pixels, and when a session stops or gets new constraints. Smithay keeps the protocol objects, checks a frame's buffer against the constraints, and fails a stopped session's frames.
- **A minimised window, and one on another workspace,** is captured as it was last drawn, with its Frame. **A shaded window** is captured as it's drawn: its Frame's titlebar alone, and its sessions get constraints of that size. No session stops for either.
- **shm buffers only.** Smithay's state would carry dmabuf constraints, but only the tty could draw into a dmabuf, and nothing in v0.1 asks for one.
- **A session's first frame is copied at once,** and each later one when its source has changed, with the damage since the frame before. So a client that asks for frame after frame is paced by its source.
- **Cursor sessions aren't offered:** Smithay's state leaves their frames and the cursor's position to the compositor. `paint_cursors` puts the cursor in a window's capture only while the window is showing and the pointer is over the capture.
- **A drag's icon is in an output's capture** with or without `paint_cursors`, which is about the pointer's cursor alone. A window's capture has the cursor whether or not another window covers it at the pointer, as the capture ignores the cover.
- **An output that's turned off is gone,** as an unplugged one is: its sessions stop.
- **The capture's size is policy's frame rect,** `Policy::frame_rect`, which is what M8's shadow mustn't grow: `a_window_capture_is_the_window_with_its_frame` pins it.
- **The tests cover the pixman renderer.** GLES, which nested runs and the tty draw with, was checked nested on Xvfb; mate-screenshot on it is [104](104-screenshots.md)'s.
