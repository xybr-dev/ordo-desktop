---
id: 98
title: Revision-based Theme Save and Undo
status: accepted
date: 2026-08-29
recorded: 2026-08-29
tags: [themes, storage]
supersedes: []
superseded_by: null
amends: []
amended_by: []
sources: []
---

# Revision-based Theme Save and Undo

- Every successful Theme **Save** creates a restorable Edit revision before publishing the staged Theme state.
- Save feedback identifies the Theme and the number of editable Mass plans whose inherited rendering was updated, for example: **Parish Dark saved · 12 Mass plans updated**.
- The save result exposes **Undo**. Undo restores the preceding Theme state by creating a later revision rather than erasing history, then rerenders the same currently inheriting editable plans.
- Existing immutable Presentation snapshots are unaffected by both Save and Undo. A later Presenter session uses a newly captured snapshot only when the editable plan or its inherited Theme result has changed.
- The Theme's longer-term revision-history surface and affected-plan review workflow remain to be designed; they do not weaken the immediate Save-and-Undo guarantee.
