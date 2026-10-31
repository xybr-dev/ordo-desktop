---
id: 121
title: Atomic recoverable Arrangement-section removal
status: accepted
date: 2026-08-29
recorded: 2026-08-29
tags: [songs, mass-plan, storage]
supersedes: []
superseded_by: null
amends: []
amended_by: []
sources: []
---

# Atomic recoverable Arrangement-section removal

- When a section has no Sequence occurrences, let **Remove section** commit immediately as an ordinary structural edit and show temporary **Undo** feedback.
- When one or several occurrences reference the section, require a guarded confirmation that names both consequences, such as **Remove “Chorus” and its 2 Sequence occurrences?**
- On confirmation, remove the reusable section and every occurrence referencing it in one atomic operation. Never retain broken occurrence placeholders or require the operator to hunt down each occurrence manually first.
- Capture enough revision state for **Undo** to restore the section name, lyrics, slide boundaries, pagination/approval state, and all removed occurrences in their original Sequence positions.
- Keep immutable Presentation snapshots unchanged. Removing the current library section cannot rewrite a snapshot already captured for presentation or export.
