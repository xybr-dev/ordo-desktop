---
id: 177
title: Mass-section header controls and structural invariant
status: accepted
date: 2026-08-29
recorded: 2026-08-29
tags: [mass-plan]
supersedes: []
superseded_by: null
amends: []
amended_by: []
sources: []
---

# Mass-section header controls and structural invariant

- Compose each Mass-section header from a dedicated drag handle, disclosure caret, section name, item/readiness counts, and a hover-or-focus `...` action. Right-click and `...` open the same scoped menu.
- Let the disclosure caret only expand or collapse. Let the rest of the header activate that section as the current insertion target, clear Mass-item selection, and place the selected-item editor and Inspector into a quiet **Select an item** state.
- Provide **Rename**, **Add section above**, **Add section below**, **Move up**, **Move down**, and **Remove section** in the section menu. Deliberately omit section duplication from the MVP.
- Rename inline, committing with `Enter` and cancelling with `Esc`. Require a nonblank display name unique under Unicode-aware case-insensitive comparison within its owning template or plan so insertion destinations remain unambiguous.
- Disable **Remove section** while the section contains Mass items, and disallow removing the final remaining section.
- Require every Mass template and Mass plan to contain at least one Mass section and every Mass item to belong to exactly one section.
- Autosave section creation, rename, movement, and eligible removal through the owning template or plan, with ordinary Undo and revision behavior.
