---
id: 97
title: Staged shared-Theme editing and safe duplication
status: accepted
date: 2026-08-29
recorded: 2026-08-29
tags: [themes]
supersedes: []
superseded_by: null
amends: []
amended_by: []
sources: []
---

# Staged shared-Theme editing and safe duplication

- Editing a reusable Theme occurs in a staged Theme Editor session. The authoritative preview reflects pending changes immediately, but Mass plans and the saved Theme remain unchanged until the operator explicitly saves.
- A Theme may be edited in place even when several editable Mass plans inherit it. Before saving, Ordo prominently warns that this is a shared Theme and identifies how many Mass plans will be affected.
- Saving replaces the reusable Theme's current editable state and causes every editable Mass plan inheriting it to render from the saved changes. Existing immutable Presentation snapshots remain unchanged.
- **Duplicate Theme** is the safe alternative to shared propagation: it creates a new independent Theme from the staged result without changing the original Theme or plans that inherit the original.
- Closing or navigating away with unsaved staged changes must not silently publish them. Exact discard confirmation, Theme revision history, Undo semantics, and affected-plan review behavior remain to be finalized.
