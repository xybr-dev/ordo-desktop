---
id: 25
title: MVP edit history and restoration
status: accepted
date: 2026-08-29
recorded: 2026-08-29
tags: [storage]
supersedes: []
superseded_by: null
amends: []
amended_by: []
sources: []
---

# MVP edit history and restoration

- Ordinary edits support Undo and Redo during preparation.
- Durable automatic checkpoints cover both Mass plans and reusable library content, including Songs and Arrangements.
- Ordo creates checkpoints at meaningful safety boundaries such as before imports, bulk edits, Celebration Package export, and entry into Presenter Mode; entering Presenter Mode again without relevant changes does not create a duplicate content revision.
- Restoring an earlier checkpoint makes its state the new current revision while preserving the later history, so restoration itself can be undone.
- Complex branches, user-named revisions, and advanced visual diffs are deferred beyond the MVP.
