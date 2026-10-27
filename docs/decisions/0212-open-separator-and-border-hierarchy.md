---
id: 212
title: Open separator and border hierarchy
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

# Open separator and border hierarchy

- Use `1px` neutral borders for ordinary pane boundaries, list-row separators, inputs, and structural divisions in both appearances. Prefer shared dividers and restrained surface changes over enclosing every section or row in a separate box.
- Keep adjacent surfaces on one shared edge so their borders never double into an unintended heavier line. Lists generally use row separators; panes use one boundary divider; section hierarchy may use a subtle tonal band without requiring a complete container border.
- Retain the existing `preparation-border` and `presenter-border` tokens for subtle structure. Add `preparation-border-strong: #8E8476` and `presenter-border-strong: #7A857B` for edges whose own visibility is necessary, including otherwise ambiguous controls, canonical frame boundaries, active drop targets, and temporary overlay boundaries.
- Do not use the strong treatment to box every group or simulate a card grid. Ordinary content remains visually part of the workspace.
- Keep selection as a forest-green tinted fill rather than a competing green outline. Keep the adopted blue two-layer keyboard focus independent of borders and selection.
- Continue reserving shadows for temporary elevated ownership such as menus, dialogs, slide-overs, and nested focused children. Major fixed panes and ordinary rows do not receive card shadows.
