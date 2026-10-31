---
id: 214
title: Two-level elevation for temporary ownership
status: accepted
date: 2026-08-29
recorded: 2026-08-29
tags: []
supersedes: []
superseded_by: null
amends: []
amended_by: []
sources: []
---

# Two-level elevation for temporary ownership

- Use exactly two shadow levels in each appearance. **Floating** applies to menus, popovers, dropdowns, and tooltips; **Overlay** applies to dialogs, slide-overs, and the focused child in a nested slide-over sequence.
- Adopt restrained appearance-specific tokens: `0 4px 12px rgba(39, 35, 31, 0.14)` and `0 18px 48px rgba(39, 35, 31, 0.20)` in Preparation; `0 4px 14px rgba(0, 0, 0, 0.40)` and `0 18px 52px rgba(0, 0, 0, 0.58)` in Presenter. These may receive small visual-qualification adjustments without creating additional semantic levels.
- Retain the appropriate neutral border on every elevated surface so its edge remains perceptible without relying on shadow alone.
- Keep fixed panes, rows, buttons, selected items, and canonical slide frames flat. Selection, hover, and ordinary interactivity do not create elevation.
- Combine a nested child's single Overlay shadow with the adopted visual recession of its parent. Never accumulate successively stronger or multiple stacked shadows merely because nesting is deeper.
- Keep all shadows neutral. Presenter uses deeper neutral black for visibility against graphite; neither appearance uses colored glows or shadows for Live, Freeze, Blackout, warning, danger, focus, selection, or liturgical meaning.
