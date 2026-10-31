---
id: 127
title: Recoverable Sequence-occurrence removal
status: accepted
date: 2026-08-29
recorded: 2026-08-29
tags: [songs, storage]
supersedes: []
superseded_by: null
amends: []
amended_by: []
sources: []
---

# Recoverable Sequence-occurrence removal

- Remove a selected Sequence occurrence immediately without confirmation because this operation never deletes its reusable section, lyrics, or generated slides.
- Expose removal through the occurrence action surface and the `Delete` key while the occurrence row or Sequence list owns keyboard focus. Never intercept `Delete` from a lyric editor, text field, metadata control, or other nested input.
- After removal, select the occurrence that moves into the removed position. If none follows, select the preceding occurrence; if the Sequence becomes empty, move focus to its empty-state action.
- Let removing the final occurrence leave a valid authoring state whose Sequence is explicitly **Incomplete**. The reusable section remains available for later placement.
- Autosave the removal and provide both temporary Undo feedback and `Ctrl+Z`; restoration returns the occurrence to its exact former position.
