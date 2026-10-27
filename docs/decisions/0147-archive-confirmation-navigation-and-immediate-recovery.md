---
id: 147
title: "Archive confirmation, navigation, and immediate recovery"
status: accepted
date: 2026-08-29
recorded: 2026-08-29
tags: [storage, ui]
supersedes: []
superseded_by: null
amends: []
amended_by: []
sources: []
---

# Archive confirmation, navigation, and immediate recovery

- Confirm Song and Arrangement archival in a compact, viewport-bounded centered dialog rather than a nested slideover or full workflow surface.
- Name the affected entity and summarize the planning impact. For a Song, disclose its child-Arrangement count; for an Arrangement, disclose Choir associations and editable Mass references. State explicitly that existing plans and immutable Presentation snapshots remain unchanged.
- Require only **Cancel** and the clearly labeled Archive action. Do not require title entry or another high-friction confirmation for this recoverable operation.
- After archiving a Song, select the next visible Song in the current active-library result set, otherwise the previous visible Song. When neither exists, focus the active-list empty state while keeping Undo available. After archiving an Arrangement, return to its parent Song detail.
- Show immediate success feedback with **Undo**. Provide an explicit `Status: Archived` Song Library filter for later discovery and restoration.
