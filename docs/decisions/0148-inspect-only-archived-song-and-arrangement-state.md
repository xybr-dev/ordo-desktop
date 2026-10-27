---
id: 148
title: Inspect-only archived Song and Arrangement state
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

# Inspect-only archived Song and Arrangement state

- Make archived Songs and Arrangements inspect-only until explicitly restored. Keep metadata, lyrics, Sequence, generated slides, and entity-owned History viewable while suppressing editing controls and structural shortcuts.
- Present a clear **Archived** status banner with **Restore** as the prominent lifecycle action. Do not allow ordinary edits to accumulate on content operators reasonably understand to be retired.
- For an archived Song, keep its complete Arrangement list visible for inspection and preserve each child's independent archival state.
- Label an independently active child Arrangement beneath an archived Song as **Available when Song is restored**. Continue labeling an independently archived child as **Archived**; restoring the parent never changes that child state.
- Hide **New arrangement** while the Song is archived. Restore the ordinary authoring controls only after the Song work is restored.
