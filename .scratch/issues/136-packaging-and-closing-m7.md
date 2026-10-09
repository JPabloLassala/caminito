# Packaging and closing M7

Epic: caminito-v0.1
Type: implementation
Status: open
Milestone: M7
Blocked by: 107, 125, 126, 127, 128, 129, 130, 131, 132, 133, 134, 135
Map: [Caminito v0.1](../caminito-v0.1/map.md)

## What to build

M7's exit criterion holds on the developer's machine, and the closing commit says so ([17](17-milestone-slicing.md)). From this commit the developer uses *MATE on Caminito* as their daily session.

- **Packaging ([16](16-packaging.md)):** `caminito-git` installs `caminito-locker` and depends on gtk-session-lock, mate-desktop and mate-screensaver. `pkgver` and every `pkgrel` are bumped, and all the packages are installed on the developer's machine from the built files.
- **The README** loses "the session can't lock yet" and says how to recover a session whose locker gave up.
- **The checklist:** 17's four items are written by [126](126-the-locker-command.md), [130](130-logind-lock-unlock-and-sleep.md) and [135](135-keyboard-monitor-for-orca.md). Write any that no ticket did.
- **The closing run:** every item tagged M7 or earlier that still applies, on the installed packages, in a Session on the developer's machine, with a wizard script beside this ticket as [73](73-packaging-and-closing-m5.md) had. **M6's items are all part of it:** [106](106-packaging-and-closing-m6.md) closed M6 without recording them one by one, and its notes list them. So is [107](107-brightness-and-power-keys.md)'s check on a laptop.
- **The closing commit,** `Close M7 Lock, idle and accessibility`, with the results in its message. It moves the tag of any item that's deferred, and updates the README's status and the map.

## Acceptance criteria

- [ ] CI is green on push, and the Packaging job with `caminito-locker` in `caminito-git`.
- [ ] 17's exit, run by hand in a Session:
  - the session locks from the panel, from `org.mate.ScreenSaver`, on idle and before suspend;
  - killing `caminito-locker` while locked leaves the session locked and brings it back;
  - idle blanks, then powers the outputs off, and a playing video inhibits both;
  - Orca reads the panel and `caminito-locker`, and sticky keys work;
  - the brightness keys change the backlight on a laptop with a Wayland window focused.
- [ ] X11 MATE and MATE on Wayfire still work with all the Patched packages installed, and mate-screensaver still locks there (ADR 0011).
- [ ] Every checklist item tagged M1 to M7 that still applies was run, and the results are in the closing commit's message.
- [ ] 06's rows with no checklist item (the bell, window types, struts) each have one, or the notes say why not: 106 left that here.

## Notes

- **The laptop:** 107's check needs one. If the developer has none at hand, the closing says so and the item moves.
- **125's swaylock item** was run nested but for its last step, the bindings pressed while locked, which a nested run can't do: the host's compositor has them. That step is on the tty, and its result ticks [125](125-lock-state-and-ext-session-lock.md)'s last criterion and resolves it.
