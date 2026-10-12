# Slideshow

Epic: wallpaper
Type: implementation
Status: ready-for-agent
Blocked by: 210, 212
Spec: [Wallpaper](../wallpaper/spec.md)

## What to build

The Slideshow kind, under "The kinds" in [the spec](../wallpaper/spec.md).

- **The drawer:** shows the pictures of a folder one after another. The interval is one of 5 minutes, 15 minutes, 1 hour and 1 day; the order is random or by name. Each output draws independently, so two outputs on one folder show different pictures. It picks up pictures added to or removed from the folder, and survives an empty or missing folder by showing the output's colour.
- **The dialog:** the folder, the interval, "In order, not at random", the number of pictures in the folder, and Positioning.

## Acceptance criteria

- [ ] Unit tests with a clock that is passed in: the next change's time for each interval, a random order that shows every picture before any repeats, two outputs differing, and an empty folder.
- [ ] [checklist.md](../../docs/testing/checklist.md) has an item: a folder at 5 minutes changes with a cross-fade, and differs on two outputs.
