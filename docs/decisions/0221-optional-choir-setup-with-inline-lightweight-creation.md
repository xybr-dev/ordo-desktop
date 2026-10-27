---
id: 221
title: Optional Choir setup with inline lightweight creation
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

# Optional Choir setup with inline lightweight creation

- Preserve the ordinary compact list-detail workspace under **Songs → Choirs** before the first Choir exists. Show **No choirs yet**, explain that Choir records support Arrangement discovery and Mass-plan defaults without owning separate lyric copies, and make **Add choir** the primary action.
- Require only a Choir name at creation. Keep short label, preferred languages, private notes, and Active or Archived state as optional details in the Choir management workspace.
- Never require Choir setup before creating Songs or Arrangements. Retain **Not assigned** as a valid choice wherever Choir association is optional; existing readiness rules continue to identify only genuinely unresolved choir-dependent Song slots.
- In editable Choir selectors, keep **Not assigned** available and offer **Create “{typed name}”** when normalized, case-insensitive matching finds no active Choir. Quick creation persists the lightweight record with only its required name and immediately selects it without leaving the Song or Mass task.
- Route both quick creation and the management workspace through the same Choir Module interface so normalization, duplicate prevention, identity generation, and persistence do not diverge across callers.
- Exclude Archived Choirs from ordinary selection suggestions. Let the operator deliberately reveal or manage them from **Songs → Choirs** rather than silently reactivating one through a selector.
