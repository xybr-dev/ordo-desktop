---
id: 119
title: Arrangement-section creation and rename surface
status: accepted
date: 2026-08-29
recorded: 2026-08-29
tags: [songs, mass-plan]
supersedes: []
superseded_by: null
amends: []
amended_by: [275]
sources: []
---

# Arrangement-section creation and rename surface

> **Amended by Decision [275](0275-rename-a-section-in-place-in-the-lyric-editor-header.md).** A section is renamed in place in the lyric editor header, not through the action button and popover. Section creation stands.

- Give every Arrangement section one required free-form **Section name**. Do not add a separate Section Type field in the MVP; the operator's final label is the section's visible identity.
- Open **Add first section** and later **+ Add section** in the same compact, anchored creation popover rather than another full-height slideover or dialog.
- Offer editable naming suggestions such as **Verse 1**, **Chorus**, **Refrain**, **Bridge**, and **Ending**. Suggestions fill the ordinary name field and never restrict custom names such as **Kyrie**, **Verse 1A**, or **Memorial Acclamation**.
- Propose the next sensible numbered label from the current Arrangement, such as **Verse 3** after Verse 1 and Verse 2, while leaving the proposal fully editable.
- Commit explicitly with **Add section**. After creation, select the new section and focus its lyric editor so keyboard entry can begin immediately.
- Expose the selected section's compact action button beside its row. **Rename section** reuses the small naming surface with the current name populated; editing does not need a separate general-purpose Section-details screen.
- Renaming a section preserves its lyrics, slide boundaries, approval state, and every Sequence occurrence referencing it. Linked occurrences immediately display the new name.
