# The pager's workspace count and names

Epic: caminito-v0.1
Type: implementation
Status: resolved
Milestone: M6
Blocked by: 96
Map: [Caminito v0.1](../caminito-v0.1/map.md)

## What to build

The workspace switcher's preferences set how many workspaces there are and what they're called, as they do under Marco ([06](06-v01-protocol-and-integration-set.md)'s *Workspace count and names* row, [17](17-milestone-slicing.md)).

- **The carried patch:** under Caminito, `mate-panel-caminito`'s pager preferences write Marco's `num-workspaces` and `workspace-names` directly. On X11 they ask the window manager through libwnck; ext-workspace is read-only, so on Wayland the controls do nothing or are greyed today. Find which first.
- **Caminito's side is done:** it applies both keys live since M2 ([29](29-settings-thread.md)), and ext-workspace adds, removes and renames since [66](66-ext-workspace.md). This ticket adds no code to Caminito unless the check finds a gap.
- **The checklist:** one item for 06's row, tagged M6, run in a Session.

## Acceptance criteria

- [x] `mate-panel-caminito` builds with the patch in its series, and its `makepkg` and `namcap` job passes.
- [ ] The checklist has the item, and it was run in a Session: the count goes from 4 to 6 and back to 2 in the preferences, and the pager and Caminito follow; a workspace renamed there shows its name in the pager and in `gsettings`. If it can't be run where the ticket is done, say so in the notes and leave it to [106](106-packaging-and-closing-m6.md).
- [x] On X11 MATE the preferences work as before (the gate, ADR 0011). (Seen by the developer on 2026-10-08, in the X11 session with `mate-panel-caminito` 1.28.7.r111.g58ac6c1-10: the count set to 6 and then 2, and a workspace renamed.)

## Notes

- **Blocked by 96** because both patches change the pager's preferences, and 96's comes first in the series.
- **Resolved on 2026-10-06 with the second and third criteria unticked,** as below.
- **Done before 96, which was still open on its own branch.** The two patches are `0005` and `0006` on top of `0004`, and `pkgrel` is 5. Whichever of 96 and 97 merges second rebases its patches onto the other's and takes the next numbers and `pkgrel`. 96's patch changes the rows preference and `rebuild_ui`'s placing, and these change neither, so the order in the series can still be 96's first if the developer wants it so.
- **Found first: the controls were hidden,** neither greyed nor dead. The switcher learns the window manager from libwnck, on X11 only, so on Wayland it stayed at "unknown", which hides the whole *Workspaces* frame: the count, the names and *Show workspace names in switcher*. Behind it the two handlers did nothing without a `WnckScreen`.
- **`0006`, the carried patch,** gated on `MATE_WM_NAME=Caminito` on a Wayland display:
  - the frame is shown as it is under Marco, and the count and a name are written to `org.mate.Marco.general` `num-workspaces` and `org.mate.Marco.workspace-names` `name-N`;
  - the preferences follow the workspaces as Caminito adds, removes and renames them, through a function the Wayland switcher calls. X11 has libwnck's signals for that;
  - Caminito adds and removes workspaces one at a time, so the spin button passes through the counts between. Those aren't written back: without that, going from 6 to 2 would write 5, 4 and 3 over the 2;
  - a count below the rows lowers the switcher's own `num-rows`, as on X11. 96, which sends the rows to Caminito's keys, has to keep or drop that;
  - a name is trimmed, and an empty one gives `Workspace N` again, both as under Marco.
- **`0005`, an upstream fix with no gate** (ADR 0011): the Wayland switcher drew its buttons again when a workspace was added, removed or made active, and not when one was renamed, so with the names shown a button kept the old name until the next change. Caminito renames on the handle it has already sent (66), so the second half of the criterion needs it. It's right on any compositor, and it's a patch of its own so that it can go upstream.
- **No code in Caminito.** The review checked the patch's assumptions against it: workspaces are added and removed at the end, so row N is `name-N`; a rename is a `name` event on the same handle; the count is clamped to 1–36, which is the spin button's range and the number of `name-N` keys in Marco's schema.
- **Checked on the developer's machine (x86_64):** `makepkg -f` on the six-patch series at `58ac6c1`, in a copy of the package's directory, and `namcap` on the PKGBUILD and the package. Both patches apply with no offset, neither file the patches change gets a compiler warning it didn't have, and `namcap` prints its 17 dependency warnings and no error. Caminito's test suite wasn't run: nothing of Caminito's changed.
- **Not run in a Session:** the checklist's *The workspace switcher's preferences set the count and the names* is the second and third criteria, and it's left to [106](106-packaging-and-closing-m6.md). The package wasn't installed where this was done. The X11 criterion rests on the code until then: the new paths are entered only with the Wayland switcher under Caminito.
- **Two things only that run can show,** since libxfce4windowing's source wasn't at hand: that it emits `name-changed` for ext-workspace's second `name` event, and that `workspace-destroyed` comes after the workspace has left the manager's list. If the second is false, the spin button ends one too high after 6 to 2; the checklist's "the spin button stays at the number that was set" is that.
- **Known and left:**
  - **A rename refills the list of names,** where X11 changes one row, so a selection or an edit under way in another row is lost when a rename arrives.
  - **The spin button shows the counts on the way** for a moment (6, 5, 6 when two clicks outrun Caminito).
  - **Upstream's Wayland switcher never disconnects its handlers on the workspace manager,** so they run on freed data once the applet is removed. Not this ticket's, and a candidate for another upstream fix.
