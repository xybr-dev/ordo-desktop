---
id: 113
title: "Mass-editor autosave, revision grouping, and Undo"
status: accepted
date: 2026-08-29
recorded: 2026-08-29
tags: [mass-plan, storage]
supersedes: []
superseded_by: null
amends: []
amended_by: []
sources: []
---

# Mass-editor autosave, revision grouping, and Undo

- Use autosave throughout ordinary Mass-plan editing; do not add a routine **Save** button to selected-item editors.
- Display quiet persistence feedback in the Mass-plan header: **Saving…** while a local write is pending and **Saved** after it is durably stored.
- Commit text-field edits after one second of inactivity or focus departure and coalesce one continuous field-editing session into a useful undo unit rather than creating a revision for every keystroke.
- Create distinct undo points for structural and semantic commits such as adding/removing/reordering exchange blocks or section occurrences, changing a Song/Arrangement, creating or discarding customization, and approving pagination changes.
- Show temporary action feedback with **Undo** after structural changes. Keep `Ctrl+Z` available for the most recent Mass-plan edit after the temporary feedback disappears.
- Preserve explicit confirmation before guarded destructive-boundary operations; autosave and Undo do not replace the adopted replacement/customization review flows.
- Flush any pending field edit to local durable storage when the editor or application closes. Saving must not depend on internet connectivity.
