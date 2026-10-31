---
id: 233
title: "Predictable multi-pane focus order with `F6` region navigation"
status: accepted
date: 2026-08-29
recorded: 2026-08-29
tags: [ui]
supersedes: []
superseded_by: null
amends: []
amended_by: []
sources: []
---

# Predictable multi-pane focus order with `F6` region navigation

- Keep ordinary `Tab` and `Shift+Tab` traversal in the same visible reading order as the GPUI focus tree. Visual reordering may never make keyboard order disagree with the interface, and ordinary panes may not trap focus.
- Add conventional desktop `F6` and `Shift+F6` navigation for jumping forward or backward among visible major regions. In Preparation, cycle **Global navigation → Workspace header/actions → Primary list or Mass outline → Main editor → Inspector**; skip every absent, hidden, or collapsed region.
- Keep selection and focus independent. Selecting a Song, Mass item, Cue, or other row may update contextual editor and Inspector content, but it never steals focus from the selected row. Moving into another region requires an explicit navigation action.
- Within lists, trees, tab sets, grids, menus, and Cue collections, use the expected component-level arrow-key pattern without converting `Tab` into row-by-row navigation. Preserve each component's selected item and meaningful focus target when the operator leaves and later returns to its region.
- Let a focused modal dialog or focused slide-over own and contain focus until it closes. Closing or completing it restores focus to the invoking control or the most relevant surviving entity when that exact control no longer exists.
- In Presenter Mode, use the smaller visible-region cycle **Ordo rail → Cue timeline → Output Dock and controls**. Skip a closed unpinned rail; treat an open overlay rail as the active focused surface; never let an `F6` jump change Preview, Program, Freeze, Blackout, or output enablement.
- Implement region registration and traversal once through the Ordo workspace shell/design-system seam. Workspaces declare their ordered regions and active targets without independently owning global key handling.
