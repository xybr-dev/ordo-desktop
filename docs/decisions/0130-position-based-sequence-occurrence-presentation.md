---
id: 130
title: Position-based Sequence-occurrence presentation
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

# Position-based Sequence-occurrence presentation

- Display each Sequence occurrence with its current one-based position and unchanged reusable-section name, such as **02 Chorus** and **04 Chorus**. Never rename repeated occurrences to artificial section identities such as Chorus 1 and Chorus 2.
- Treat the position as a transient display label rather than stored content identity. Recompute it immediately after insertion, removal, or reordering.
- Show the occurrence's generated-slide count in its collapsed summary. Expanding it reveals those fixed-order generated slides; selecting a child slide drives the authoritative Inspector.
- Keep generated slides subordinate to their occurrence and section. Repetition may produce visually identical expanded children, but each occurrence remains independently selectable and movable in performance order.
