# Picture of the day

Epic: wallpaper
Type: implementation
Status: ready-for-agent
Blocked by: 210, 212
Spec: [Wallpaper](../wallpaper/spec.md)

## What to build

The Picture of the day kind, under "The kinds" in [the spec](../wallpaper/spec.md).

- **Three sources** behind one interface: NASA's Astronomy Picture of the Day, Bing and Wikimedia Commons. None with a key.
- **The drawer fetches** at session start, on wake from suspend and shortly after local midnight. With no network, or when the day's item isn't a usable picture, the last one stays. Only the latest of each source is kept, in `~/.cache`, with its title and credit.
- **The dialog:** the three sources with a line on each, today's title and credit, "Keep this picture" (copies it to `~/Pictures/Wallpapers` and adds it to the grid), and Positioning.

## Acceptance criteria

- [ ] Each source's parsing is tested against a saved response, including a NASA day that is a video.
- [ ] Tests with a fake source: a failed fetch keeps the last picture, and the cache holds one picture per source.
- [ ] No test reaches the network.
- [ ] [checklist.md](../../docs/testing/checklist.md) has an item: each source shows today's picture with its credit, "Keep this picture" puts it in the grid, and with the network off the picture stays.

## For the implementing agent to settle

Recorded in the Done section: each source's endpoint and its terms of use, and the HTTP client, which must not add a second TLS stack to the workspace if one is already there.
