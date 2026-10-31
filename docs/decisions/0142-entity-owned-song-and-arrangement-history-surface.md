---
id: 142
title: Entity-owned Song and Arrangement History surface
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

# Entity-owned Song and Arrangement History surface

- Expose a visible labeled **History** action in the Song-detail header for Song-work identity and metadata history, and another in the Arrangement-editor header for that Arrangement's details, lyrics, Sequence, and pagination history.
- Open History as a viewport-bounded floating right slideover spanning the full application chrome. Preserve the current editor, selection, and navigation context underneath rather than routing to a separate page.
- Flush any pending autosave before opening History. If that flush fails, retain the unsaved edit and do not present an ambiguous supposedly current comparison state.
- List grouped editing sessions with date and time, a generated change summary, revision count, and provenance such as manual edit, import, or restore.
- Expand grouped sessions inline to expose their individual durable revisions without replacing the readable session-level overview.
- Keep Song History and Arrangement History separate and entity-owned. A Song's History does not aggregate its child Arrangements, and an Arrangement History does not mix Song-work metadata revisions into its timeline.
