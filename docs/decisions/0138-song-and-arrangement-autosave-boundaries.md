---
id: 138
title: Song and Arrangement autosave boundaries
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

# Song and Arrangement autosave boundaries

- Autosave ordinary Song-detail fields and Arrangement lyric text after one second of inactivity or when focus leaves the edited field. Keep authored content local-first and require no network or routine global Save action.
- Coalesce one continuous typing session into one persisted, undoable edit rather than creating a revision for every keystroke.
- Persist section creation, rename, removal, and reorder; Sequence insertion, repeat, removal, and reorder; and slide-boundary changes immediately as discrete undo points.
- Preserve explicit transactional boundaries where they already communicate staged intent: **Create song**, **Create Arrangement**, and **Save details** commit their respective New Song, New Arrangement, and Arrangement-details surfaces; Cancel discards those staged changes.
- Show quiet editor-header persistence feedback as **Saving…**, **Saved**, or **Couldn’t save**. Do not use successful saves as disruptive notifications.
- Flush pending local edits before navigation or application close. If persistence fails, keep the recoverable edit visible and warn before allowing the operator to leave rather than silently discarding it.
