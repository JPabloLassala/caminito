# Dynamic wallpapers from HEIC files

Epic: wallpaper
Type: implementation
Status: open: the code is in, and running the checklist item with a HEIC of their own, which also checks that Apple's pictures are numbered as heif-dec writes them, is the developer's (or 220's)
Blocked by: 216
Spec: [Wallpaper](../wallpaper/spec.md)

## What to build

The third Dynamic format in [the spec](../wallpaper/spec.md): Apple's HEIC files, whose pictures are keyed to the sun's position, or to clock times in the older variant.

- **The drawer** reads the file's pictures and its metadata, and shows the picture for the sun's elevation and azimuth Caminito publishes, cross-fading between neighbours.
- **`libheif` is an optional dependency** of `caminito-git`. Without it the build and the programs still work, and the dialog greys these files out with a line that says why.
- **The dialog's "Add…"** takes a `.heic`.

## Acceptance criteria

- [x] Unit tests of the metadata's parsing and of the picture chosen for a sun position, from fixtures that are not Apple's pictures.
- [x] The workspace builds and its tests pass with and without `libheif`.
- [ ] [checklist.md](../../docs/testing/checklist.md) has an item, by the developer with a HEIC file of their own: it is on the right picture at noon and at night.

## Done (2026-10-15)

- **The metadata is read without libheif.** `heic.rs` finds the XMP item through the container's `meta`, `iinf` and `iloc` (in an `mdat` or in `idat`), then the `apple_desktop:` attribute or element, base64 and a binary property list, all parsed here. Of the three variants, `solar` follows the sun, `h24` the clock and `apr` is a light and a dark picture, as a Plasma pair. Without `ap`, a solar file's light picture is its highest and its dark its lowest; an h24's is noon's and midnight's. Broken containers and property lists, every byte of each cut or changed, are errors and never a panic, and a property list that contains itself is refused.
- **The pictures are decoded by libheif's program, not its library.** `heif-dec` (or `heif-convert`, its name before 1.16 and on Ubuntu 24.04) writes each picture once to `~/.cache/caminito-wallpaper/heic/<name>-<hash>/picture-<n>.jpg`, named for the file's path, size and time, decoded beside the folder and renamed into place. So the crate stays `forbid(unsafe_code)`, builds and links nothing of libheif (`ldd` shows none), and a file that crashes the decoder doesn't take the drawer down. The drawer shows the output's colour while a HEIC decodes, then cross-fades to it. A failed decode is logged once and not tried again until the program restarts. A missing libheif doesn't count as a failure, so libheif installed later is used. Old folders in the cache are never removed, which is left for later.
- **Picture `i` of the metadata is taken as heif-dec's `-<i+1>`,** its top-level images in order. That is checked on files libheif made, not on Apple's: the checklist item does that.
- **How the sun picks the picture.** This was the design question, decided here. A day goes round the positions: up the eastern half of the sky by elevation, down the western, across the top and the bottom by the east-west part of the azimuth. Each cross-fades linearly between the two positions either side. So a northern file keeps its noon at noon in the south, where the nearest position by azimuth would show it at midnight. A sun that crosses north or south in the middle band, as a winter noon below the morning's highest picture or a summer midnight above the night's lowest, crosses over 15° of azimuth either side, without a cut. A test walks four made-up days, spring, winter, summer and a southern spring, a minute at a time, and no minute changes more than 0.3 of a picture. The first draft cut at a winter noon, which the spec review found.
- **The sun's azimuth is read from Caminito's `Sun`,** beside its elevation (215). Without Caminito, outside a Session, a solar HEIC is its light and dark pictures by the fixed times, as a pair is (216). An h24 file follows the local clock: whether Apple's times are local or UTC was not checked.
- **The dialog.** *Add…* takes `.xml`, `.heic` and `.heif`. A HEIC goes under *Your dynamic wallpapers*, grey until its thumbnail's pictures are decoded. Without libheif a HEIC is added but not chosen, greyed out with the reason in its tooltip, and the line under the grid adds "HEIC files are greyed out: libheif's heif-dec isn't installed…" after its usual sentence. The schema's `dynamic` and `dynamics` describe HEICs.
- **Packaging.** `libheif` is an optdepend of `caminito-git`, and `libheif-examples` a Suggests of the Debian package, where `heif-dec` is. That Arch's `libheif` ships `heif-dec` was not checked here.
- **The fixtures aren't Apple's.** The unit tests build their containers and property lists byte by byte. `heic/solar.heic` (1.6 KB) was made by libheif 1.17's `heif-enc` from three flat-colour JPEGs, the first carrying a made-up `solar` XMP. It is read by a unit test without libheif, and the drawer's new ignored test decodes it and draws its day picture at noon and its night picture at midnight, within 4 of each channel after HEVC and JPEG (`Fixture::wait_for_pixel_near_on`). One more unit test makes a HEIC with `heif-enc` and decodes it through `decode` when libheif's programs are installed. Without them, that test checks that `decode` says why. The drawer test returns early.
- **Run in an Ubuntu 24.04 cloud container, with and without `libheif-examples`.** Under `dbus-run-session`, with `--no-fail-fast` and without libheif, every crate passes: caminito's 2085, caminito-wallpaper's 122, and the rest. With libheif, caminito-wallpaper's tests and the drawer's 7 ignored tests pass, the HEIC one included. caminito-theme's `goldens` test fails in both, 342 cases differing from Marco's pixels and none in geometry. That crate isn't touched here, so it's this container's theme rendering. `cargo clippy -p caminito-wallpaper --all-targets -D warnings` is clean, and the schema compiles with `--strict`. The dialog's `--demo` was seen under Xvfb with a HEIC added, with `heif-convert` on `PATH` and without it. It wasn't seen on Wayland or in a Session. `tools/check`'s meson, cargo-deny and locker steps weren't run, and no crate was added.
