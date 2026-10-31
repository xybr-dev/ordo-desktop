---
id: 250
title: Use a hybrid global-rail and local-tab navigation hierarchy
status: accepted
date: 2026-08-29
recorded: 2026-08-29
tags: [ui]
supersedes: []
superseded_by: null
amends: []
amended_by: [257]
sources: []
---

# Use a hybrid global-rail and local-tab navigation hierarchy

> **Superseded in part by Decision [257](0257-make-library-a-multi-view-destination-with-revision-bound.md).** Rename the global **Songs** destination
> to **Library** and place Songs, Responses & Prayers, and General Presentations
> in its local navigation. The global-rail/local-tab hierarchy remains adopted.

- Use one persistent vertical application rail for global Preparation destinations rather than presenting global workspaces as a horizontal tab strip.
- Make **Masses** the default global destination, followed by **Songs**, **Themes**, and **Transfer & Backup**. Anchor **Settings**, **Help**, and **Diagnostics** at the bottom of the same rail.
- Reserve horizontal tabs for views or tasks within the current global destination or selected entity. Examples include **Upcoming / History / Archive / Templates** inside Masses, **Lyrics / Sequence** inside an Arrangement, and the adopted task navigation inside the Theme Editor.
- At comfortable effective widths, show rail icons with persistent labels. At constrained effective widths or increased Interface scale, collapse it to accessible labeled icons with matching tooltips and a non-color-only active treatment.
- Keep the rail compact when necessary in focused width-hungry editors, while preserving current destination, breadcrumb context, primary action, selection, focus, and return state. Responsive rail presentation is operator UI state, never persisted Parish content.
- Presenter Mode replaces the Preparation application shell with Live Control and therefore does not show this global rail; its separate collapsible **Ordo rail** remains a Presenter-specific navigation surface.
