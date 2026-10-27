---
id: 172
title: Editable and independently owned Mass-section structure
status: accepted
date: 2026-08-29
recorded: 2026-08-29
tags: [mass-plan]
supersedes: []
superseded_by: null
amends: []
amended_by: []
sources: []
---

# Editable and independently owned Mass-section structure

- Treat the six-section default as an editable starting scaffold rather than a locked schema.
- Let a Mass template add, rename, reorder, or remove its Mass sections so special celebrations and parish-specific structures can be modeled without bypassing the section concept.
- Copy the template's section structure into each new Mass plan. Let that plan add, rename, reorder, or remove sections independently; later template or plan edits never synchronize across the copy boundary.
- Never cascade-delete Mass items when removing a section. Require a non-empty section's items to be moved to another section or explicitly removed from the Mass before the section itself can be removed.
- Keep section changes within the existing Mass-template or Mass-plan autosave, Undo, and revision ownership rather than creating a separate shared section library.
