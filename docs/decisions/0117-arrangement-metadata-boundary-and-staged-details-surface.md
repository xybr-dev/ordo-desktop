---
id: 117
title: Arrangement metadata boundary and staged details surface
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

# Arrangement metadata boundary and staged details surface

- Keep Arrangement metadata in a dedicated, viewport-bounded floating **Arrangement details** surface opened from the focused editor header. Do not add a third primary task tab or mix this metadata into Lyrics or Sequence.
- Require at least one Arrangement language. Allow several languages only when that exact rendition genuinely uses them; language describes the Arrangement's owned lyric rendition rather than the Song work's identity.
- Keep **Display title** optional. When blank, derive the visible library label from the selected language and Choir context rather than forcing operators to maintain redundant names.
- Associate zero, one, or several Choirs with the Arrangement. Multiple Choirs share one Arrangement only when they sing the exact same rendition; Choir membership is discovery/applicability metadata, not ownership of a separate lyric copy.
- Keep translator/adapter attribution, occasion applicability, and source/rendition/rights notes optional and Arrangement-owned. Keep composer and original-author attribution on the canonical Song work.
- Keep exact lyrics, reusable sections, ordered section occurrences, and slide boundaries outside this surface in the adopted **Lyrics** and **Sequence** tasks.
- Reuse the same staged surface for **New Arrangement** and later **Arrangement details** editing. Creation commits explicitly with **Create Arrangement**; editing commits explicitly with **Save details**; Cancel discards staged metadata changes.
- When an edited Arrangement is shared by several Choirs, state that scope before Save. Saving updates the reusable Arrangement and its library filtering/discovery metadata while preserving all existing Mass references.
- Never rewrite an immutable Presentation snapshot after Arrangement metadata changes. A later snapshot may capture the updated metadata under the established snapshot rules.
- Offer **Duplicate Arrangement** from the existing-Arrangement state as a safe path when the operator intends to branch the rendition rather than update the shared record.
