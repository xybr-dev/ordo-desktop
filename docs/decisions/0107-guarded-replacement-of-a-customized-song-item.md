---
id: 107
title: Guarded replacement of a customized Song item
status: accepted
date: 2026-08-29
recorded: 2026-08-29
tags: [songs]
supersedes: []
superseded_by: null
amends: []
amended_by: []
sources: []
---

# Guarded replacement of a customized Song item

- **Change song or arrangement** continues to open the ordinary staged picker even when the current Song item owns a Mass-local override. Merely browsing or selecting a candidate does not change the plan.
- Committing a different Song or Arrangement opens the established nested review slideover. It compares the current customized source with the staged replacement and warns that local lyric, sequence, and pagination changes will be detached from the active item.
- Ordo never attempts to merge Mass-local edits into a different Arrangement. After confirmation, the replacement becomes Arrangement-backed and its warnings, Cue/slide counts, and pagination are recomputed.
- **Return to library arrangement** is a direct action on the local-customization banner. Because it has no preceding picker context, its confirmation opens as one focused slideover rather than as a child nested over the Arrangement picker. The comparison identifies the original library Arrangement and the local changes being removed.
- Both confirmation paths leave the previous customized state recoverable through ordinary Mass-plan Undo and Edit revision history; Presentation snapshots remain immutable.
- Picker-to-review and parent-to-child slideovers must preserve focus, motion direction, stacking, and staged state. Back returns to the unchanged picker selection, cancellation keeps the Mass-local override, and reduced-motion mode removes spatial animation without changing the interaction sequence.
