---
id: 293
title: Replace swaps a Mass item from the Insert panel
status: proposed
date: 2026-10-31
recorded: 2026-10-31
tags: [presenter, mass-planning]
supersedes: []
superseded_by: null
amends: []
amended_by: []
sources: [docs/mockups/presenter-replace.html]
---

# Replace swaps a Mass item from the Insert panel

## Context

The operator can prepare one version of a Song and find, as the choir starts, that it is singing
another. Insert (Decision [290](0290-emergency-insert-chooser-and-quick-insert-dialog.md)) only adds
an item, which leaves the wrong one in the timeline. `Ctrl+E`
(Decision [291](0291-emergency-edit-opens-the-selected-slide-in-the-insert-panel.md)) changes one
slide. **Change Song or Arrangement** in the planner goes through the staged review of Decision
[107](0107-guarded-replacement-of-a-customized-song-item.md), which is too slow during Mass. Output
safety is already defined by Decision [12](0012-safe-live-operations.md).

## Decision

- **Where.** `Ctrl+R`, or **Replace Song** / **Replace item** pinned above the Cue timeline, opens
  the Insert panel titled **Replace**. It acts on the Mass item holding the Preview Cue. The header
  names that item, its Song and Arrangement, and the Program state.
- **Output.** `Ctrl+R` turns Blackout on automatically, with the status **BLACKOUT · replacing**,
  because the wrong version must leave the screen at once. This differs from Insert and `Ctrl+E`,
  which Freeze. It also applies if Program is Frozen. Replacing ends at **BLACKOUT · Replace ready**,
  and only Space or `B` puts the new content on screen. Cancelling leaves Blackout on. With output
  disabled, or nothing live yet, Blackout doesn't turn on.
- **Fastest path.** For a Song, **Other versions of {Song}** comes first, with the first one active,
  so `Ctrl+R` then `Enter` fixes the wrong version. Each row says how the version differs. The
  repertoire and the rest of the Library follow, as in Insert, without the Song being replaced.
- **Any Mass item can be replaced**, with any kind the panel offers. It opens on the tab matching
  the item: Song, Response sequence, or otherwise Text or image.
- **What changes.** The item keeps its name and place, so **Entrance Song** stays Entrance Song.
  Only its contents change. It is marked **Replaced live** in the timeline and the Ordo rail.
- **Keeping the place.** Preview lands on the same section where the new version has it, matching
  the occurrence, such as the second Koro. Otherwise it lands on the first slide and the message
  says which section is missing.
- **Edits made live.** If the item has `Ctrl+E` edits, the panel warns before anything changes. There
  is no nested review, unlike Decision 107, because **Put back** recovers them.
- **Put back.** Reopening Replace on a replaced item offers **Put back the prepared version**
  (`Alt+B`), which restores it with its edits. This stays available however many times the item was
  replaced.
- **Scope.** The change is saved to this Mass plan only. The Library and the Presentation snapshot
  are not changed.
- **An item that can't be shown.** Replace is the way to swap out unavailable Cues, and `Ctrl+E`'s
  refusal message says so.
- **Keys.** While the panel is open, single-key Presenter commands are suspended as in Insert, and
  `Esc` closes it without changing anything, leaving Blackout on.

## Alternatives considered

- **Insert the right Song and skip the wrong one.** It leaves the wrong item in the timeline for the
  operator to step over, and in the Mass record.
- **The planner's staged review, during Mass.** It is right in preparation and too many steps when
  the choir has already started.
- **Freezing, as Insert and `Ctrl+E` do.** It would leave the wrong words in front of the
  congregation while the operator finds the right version.
- **Opening on an empty search.** It costs typing in the most common case, a wrong version of the
  right Song.
- **A one-time Undo strip after replacing.** It would disappear on Space, just when the operator
  might notice the mistake.

## Consequences

- `Ctrl+R` joins the Presenter shortcuts in MVP-PLAN §8.2 if this is accepted.
- The Mass plan keeps the replaced item's earlier version until the Mass is over, so Put back has
  something to restore.
- Decision 107 still governs replacement in the planner.
