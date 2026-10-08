# Test fixture and client

Epic: caminito-v0.1
Type: implementation
Status: resolved
Milestone: M1
Blocked by: 20
Map: [Caminito v0.1](../caminito-v0.1/map.md)

## What to build

The headless integration tier from 14, ready for [22](22-xdg-shell-and-input.md) to use test-first.

- **`Fixture`** in `caminito/src/tests/` (15): Caminito's `State` on the headless backend and any number of test clients in one calloop. Clients are joined over `UnixStream::pair()` with `insert_client`, and there are `roundtrip` and `dispatch` helpers. This is niri's `tests/fixture.rs` shape.
- **`caminito-test-client`:** a `wayland-client` client, starting with the globals list and xdg-shell. It can create a toplevel, record every configure it gets, ack it, and commit a buffer (shm, solid colour) of a given size. It can also set a title, app id and parent, and send move, resize and minimise requests. More protocols come with the tickets that need them (14's list).
- **Snapshots:** insta, with small helpers that turn a client's recorded events into stable text.
- **D-Bus isolation (14):** CI runs `caminito`'s lib tests under `dbus-run-session`. Nothing uses D-Bus yet, but the wrapper is in place.

## Acceptance criteria

- [x] A fixture test connects a client, lists the globals and snapshots the list.
- [x] A fixture test creates an xdg toplevel and snapshots its first configure. The snapshot is expected to change when [22](22-xdg-shell-and-input.md) lands.
- [x] The headless tier runs in CI with no GPU or EGL.
