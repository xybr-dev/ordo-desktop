---
id: 104
title: Visible Theme revision history and restoration
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

# Visible Theme revision history and restoration

- The Theme Editor header exposes a labeled **History** action in the MVP; revision recovery is not confined to the transient post-Save Undo notice.
- **History** opens a right slide-over listing saved Theme revisions with date and time, a concise generated change summary, and the number of editable inheriting Mass plans affected when that revision was saved.
- A revision may be previewed through the authoritative canonical frame without changing the current staged editor state or publishing it.
- **Restore this revision** uses the same current-impact warning as an ordinary shared Theme Save. Restoration creates a new latest Edit revision from the historical state and never erases the intervening history.
- Restoring rerenders currently inheriting editable plans and uses the same exception-based affected-plan review. Existing Presentation snapshots remain immutable.
