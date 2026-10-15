# Picture of the day

Epic: wallpaper
Type: implementation
Status: open: the code is in, and the saved answers were written from each source's format because its host was blocked where this was built, so capturing real ones (and running the checklist item, with 220) is the developer's
Blocked by: 210, 212
Spec: [Wallpaper](../wallpaper/spec.md)

## What to build

The Picture of the day kind, under "The kinds" in [the spec](../wallpaper/spec.md).

- **Three sources** behind one interface: NASA's Astronomy Picture of the Day, Bing and Wikimedia Commons. None with a key.
- **The drawer fetches** at session start, on wake from suspend and shortly after local midnight. With no network, or when the day's item isn't a usable picture, the last one stays. Only the latest of each source is kept, in `~/.cache`, with its title and credit.
- **The dialog:** the three sources with a line on each, today's title and credit, "Keep this picture" (copies it to `~/Pictures/Wallpapers` and adds it to the grid), and Positioning.

## Acceptance criteria

- [ ] Each source's parsing is tested against a saved response, including a NASA day that is a video.
- [x] Tests with a fake source: a failed fetch keeps the last picture, and the cache holds one picture per source.
- [x] No test reaches the network.
- [x] [checklist.md](../../docs/testing/checklist.md) has an item: each source shows today's picture with its credit, "Keep this picture" puts it in the grid, and with the network off the picture stays.

## For the implementing agent to settle

Recorded in the Done section: each source's endpoint and its terms of use, and the HTTP client, which must not add a second TLS stack to the workspace if one is already there.

## Done (2026-10-15)

- **The parsers came before their saved answers could be checked against the real ones.** The sandbox this was built in refused `apod.nasa.gov`, `www.bing.com` and `en.wikipedia.org`, so `caminito-wallpaper/src/daily/apod.html`, `apod-video.html`, `bing.json` and `commons.json` were written from each source's format as it is published, not captured. They're the first acceptance criterion's box left open. Replacing each with a `curl` of the endpoint below, and adjusting a parser if the real answer differs, is the developer's.
- **Endpoints and terms.** None of the three needs a key.
  - NASA: `https://apod.nasa.gov/apod/astropix.html`, the page of today, read as HTML because `api.nasa.gov` needs a key, and not the dated `apNNNNNN.html`, which 404s for hours after local midnight east of US Eastern time. APOD's pictures belong to whoever the page credits, often not NASA. The credit line is shown whole, "Image Credit & Copyright: …". A video day (an `<iframe>`, no `href="image/…"`) is no picture, and the last one stays.
  - Bing: `https://www.bing.com/HPImageArchive.aspx?format=js&idx=0&n=1&mkt=en-US`, the feed its home page reads, and the picture at `urlbase` + `_UHD.jpg` (3840×2160). Bing's own tooltip says the pictures are for wallpaper use only, and a day whose `wp` is `false` is refused as no picture. The credit is the `(© …)` of `copyright`.
  - Commons: `https://en.wikipedia.org/api/rest_v1/feed/featured/YYYY/MM/DD`, whose `image` is Commons' Picture of the Day, with its artist and licence: the credit is "artist, licence", as CC BY-SA asks. Wikimedia's API policy wants a named client, so every request sends `User-Agent: caminito-wallpaper/<version> (<repository>)`. A picture wider than 3840 px is asked of Commons' scaler at 3840; a TIFF or SVG comes scaled, and a video or sound is no picture. The title is the short English caption, then the description.
- **The HTTP client is GIO's sockets with a GET of our own,** in `http.rs`. The workspace had no TLS stack in Rust (no rustls or openssl in `Cargo.lock`). GLib's TLS, glib-networking, is the one GTK and gvfs use already, so it's that one. libsoup would be the same TLS, but it adds a system library and two crates. It asks in HTTP/1.0 with `Connection: close`, so the body runs to the end. It reads a chunked body anyway, follows five redirects, and stops at 64 MiB and 30 s of silence. glib-networking joins the PKGBUILD's `depends` and the Debian package's `Depends`. `serde_json` reads the two JSON feeds. It was in the lock already, and is now one line in `[workspace.dependencies]`, which `caminito` and `caminito-theme` now use too.
- **When it fetches.** The ticket's three times are kept, plus two more:
  - The three: the first refresh that shows a source in a session; the first after local midnight, which the drawer's 60-second tick notices; and logind's `PrepareForSleep(false)`.
  - Added: within the hour, while the day's picture isn't in, and when GIO's network monitor says the network is back after a failure. A session that starts offline, and a source whose day starts later than local midnight (NASA's and Bing's do, for most of the world), would otherwise wait for the next midnight. An APOD video day costs one page an hour.
  - A failed fetch logs a warning, and a day that isn't a picture logs at debug level.
- **The cache** is `~/.cache/caminito-wallpaper/picture-of-the-day/`. Each source has `<source>.about`, with its day, file, URL, title and credit, and `<source>-<day>-<hash>.<ext>`. The name is new for each picture because the drawer knows a picture by its file. Both are written to `.part` and renamed, and the source's other pictures are removed after. A picture kept already isn't downloaded again, and what comes back must start like a JPEG, PNG, WebP or GIF, or it's refused.
- **The settings.** An output's set gains `picture-of-the-day`, the enum `org.mate.Caminito.PictureOfTheDaySource` (`apod`, `bing`, `commons`), defaulting to `bing`, the one that is always a landscape picture. Choosing the kind applies at once, as Colour does. Before the first fetch, the output is its colour.
- **The dialog's pane:** the three sources with a line each, a "Today" heading with the title and the credit with its day, then "Keep this picture" and Positioning. A file monitor on the cache shows the drawer's new picture while the window is open. "Keep this picture" copies to the XDG pictures folder's `Wallpapers`, which is `~/Pictures/Wallpapers` unless the user's pictures folder is another, as on a system in Spanish. The copy is named "<day> <title>" and added to the user's pictures, while the output stays on the picture of the day. `--demo` keeps nothing outside the window.
- **Seen in `--demo` under Xvfb,** with a cache seeded by hand through `XDG_CACHE_HOME`:
  - Bing's title, credit and picture in the pane and both sidebar previews.
  - NASA with nothing kept: the colour, the "no picture yet" line and a greyed button.
  - Keep's demo line.
  - The real drawer wasn't run on a picture of the day: it would reach the network, and none of `caminito_wallpaperd.rs`'s tests may. The checklist item is that test.
- **Run in an Ubuntu 24.04 cloud container,** with the build's dependencies from apt.
  - Under `dbus-run-session`, the workspace's tests pass: caminito's 2085, then the other crates', and the wallpaper library's 102.
  - With `CAMINITO_WALLPAPERD` set, the six ignored drawer tests and the dialog's test against Caminito pass on the new drawer.
  - `cargo clippy -p caminito-wallpaper --all-targets -D warnings` is clean. `caminito-theme` has three `manual_range_contains` errors with this toolchain on the base branch too, and they weren't touched.
  - The schema compiles with `--strict`.
  - `tools/check`'s meson, cargo-deny, locker and extension steps weren't run, and no package was built.
