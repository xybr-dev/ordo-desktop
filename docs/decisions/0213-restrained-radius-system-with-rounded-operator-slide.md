---
id: 213
title: Restrained radius system with rounded operator slide viewports
status: accepted
date: 2026-08-29
recorded: 2026-08-29
tags: [rendering]
supersedes: []
superseded_by: null
amends: []
amended_by: []
sources: []
---

# Restrained radius system with rounded operator slide viewports

- Adopt exactly two ordinary corner radii: `4px` for controls, interactive-row treatments, compact elements, and slide-viewport chrome; `8px` for menus, dialogs, slide-overs, and other temporary elevated surfaces. Fixed panes, shared dividers, tables, and ordinary open list structures remain square.
- Reserve fully rounded shapes for genuine circles, status dots, and compact status labels whose semantics warrant them. Do not render ordinary buttons, containers, or rows as pills.
- Keep the canonical Slide Renderer square-cornered at `1920x1080`, but display its result through consistently clipped `4px` operator viewports in Cue thumbnails, generated-frame navigation, Inspector, Preview, and Program. Preview and Program use identical viewport geometry.
- Keep congregation output square and edge-to-edge on the physical display. The operator viewport radius is application chrome; it never becomes Slide Theme content, saved slide content, or exported output.
- The small corner mask is compatible with the adopted safe-text margin but may not influence content layout, scale, pagination, or canonical `16:9` geometry. Keyboard focus follows the rounded outer wrapper without entering the rendered canvas.
