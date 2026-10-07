# Image capture

Type: implementation
Status: open
Milestone: M6
Blocked by: none
Map: [Camino v0.1](../map.md)

## What to build

A client can capture an output or a window: ext-image-capture-source with output and toplevel sources, and ext-image-copy-capture ([06](06-v01-protocol-and-integration-set.md), [17](17-milestone-slicing.md)). mate-screenshot is the user ([104](104-screenshots.md)).

- **Smithay's states** for both protocols, as 06 found them at the pinned `rev`. Check what they leave to the compositor before starting.
- **Output sources** render what the output shows, at its mode's size, through the scene's own element order, so a capture can't differ from the screen.
- **Toplevel sources** are made from an ext-foreign-toplevel-list handle. The capture is the window with its Frame and without its shadow, alone, at the window's scale, whatever covers it and whichever workspace it's on. A minimised or shaded window is captured as it was last drawn, or the session is stopped: decide here and write it down.
- **Sessions:** shm buffers at least, and dmabuf if Smithay's state makes it cheap. Damage is reported, and a session whose source changes size gets new constraints. A source that goes away stops its sessions.
- **The cursor:** the session's `paint_cursors` option is honoured. Cursor sessions aren't needed by mate-screenshot; offer them only if Smithay's state does it without more code.
- **Privileged:** both globals join the ones hidden from `wp_security_context_v1` clients ([68](68-data-control-and-privileged-globals.md) left a place for them).
- **Test client:** both protocols in `camino-test-client`, as a screenshot tool would use them ([14](14-development-and-testing-loop.md)).

## Acceptance criteria

- [ ] Headless tests, written before the code:
  - an output capture has the output's size and the pixels of a client's known colour where its window is;
  - a toplevel capture has the window's size with its Frame, and is the same when another window covers it;
  - a capture at an output scale of 2 has twice the pixels;
  - `paint_cursors` puts the pointer in the capture, and without it the pointer is absent;
  - resizing the window sends the session new constraints;
  - closing the window stops the session.
- [ ] `a_client_lists_the_globals` gains the three globals, and the security-context test's list of privileged globals gains them.

## Notes

- **Shadows are M8's,** so "without the shadow" has nothing to leave out yet. Write the test that pins the capture's size to the Frame's, so M8 can't grow it.
- **While locked** a capture shows the lock, never the session behind it. That's M7's (17), since there's no lock until then.
- **wlr-screencopy stays deferred** (06).
