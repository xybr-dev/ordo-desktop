---
id: 124
title: Independent Lyrics-section authoring order
status: accepted
date: 2026-08-29
recorded: 2026-08-29
tags: [songs, mass-plan]
supersedes: []
superseded_by: null
amends: []
amended_by: []
sources: []
---

# Independent Lyrics-section authoring order

- Let operators manually reorder reusable sections in the **Lyrics** task so sections created later can still be organized into a sensible authoring/navigation order.
- Treat this as authoring order only. Reordering sections never adds, removes, or repositions any Sequence occurrence and therefore never changes the Arrangement's performance order.
- Support both visible drag handles and `Alt+Up` / `Alt+Down` while a section row is focused. Keep the resulting move autosaved and recoverable through Undo.
- Preserve every section's content, slide boundaries, pagination approval state, and occurrence references during the move; only its position in the Lyrics section list changes.
