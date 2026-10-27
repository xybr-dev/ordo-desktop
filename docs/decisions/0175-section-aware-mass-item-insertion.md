---
id: 175
title: Section-aware Mass-item insertion
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

# Section-aware Mass-item insertion

- Refine the adopted **+ Add Mass item** workflow so an item selected in the outline inserts the new item immediately after itself within the same Mass section.
- Let interaction with a Mass-section header make that section the active insertion target; adding then appends the new item within that section.
- When neither a Mass item nor section is active, including a completely item-empty plan that still owns at least one section, require selection of a destination section before the item type may be chosen. Do not silently default to the bottom of the Ordo or to After Mass.
- State the complete current destination inside the anchored Add dropdown and let the operator change it before creation. Reuse the same destination semantics for per-item **Insert before** and **Insert after** commands.
- After creation, select the new Mass item, expand its section, autosave it, and preserve the existing direct Undo behavior.
