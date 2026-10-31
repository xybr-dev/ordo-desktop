---
id: 234
title: Operator-only interface scaling isolated from canonical slide rendering
status: accepted
date: 2026-08-29
recorded: 2026-08-29
tags: [rendering, ui]
supersedes: []
superseded_by: null
amends: []
amended_by: [268]
sources: []
---

# Operator-only interface scaling isolated from canonical slide rendering

> **Superseded in part by Decision [268](0268-make-ordo-windows-only-for-the-mvp-and-defer-linux.md).** Qualification uses Windows system-scale factors only. The isolation contract remains.

- Provide installation-specific **Interface scale** choices of `100%`, `125%`, `150%`, `175%`, and `200%`, persisted as an installation preference and controlled through Settings plus standard `Ctrl++`, `Ctrl+-`, and `Ctrl+0` controls. Scale operator text, controls, spacing, hit areas, and focus treatment together rather than enlarging glyphs inside fixed-height chrome.
- Treat Windows/Linux system display scaling and Ordo Interface scale as operator-environment concerns only. They may resize and reflow Ordo's non-slide chrome but may never change the content geometry, font metrics, wrapping, pagination, safe area, or Theme typography of a canonical Slide.
- Do not implement Interface scale as an indiscriminate GPUI/window scale that also multiplies canonical Slide layout. Apply it through operator-shell layout and typography tokens outside the Canonical Slide Renderer seam; fit the passive congregation view through the qualified Output Fit path.
- Compose every Slide in an explicit `1920x1080` logical coordinate space independent of GPUI viewport dimensions, operating-system scale, Ordo Interface scale, and physical output density. A target display's physical resolution or scale factor may request a sharper raster, but density changes only sampling quality: logical glyph positions, line breaks, element bounds, and pagination remain identical.
- Treat thumbnails, Inspector, Preview, and Program as viewports onto a completed canonical frame. Interface scaling may change those viewport sizes and grid column counts, but it only scales the finished frame uniformly and never causes the Slide Renderer to reflow it.
- At increased scale, grow or reflow operator rows, reduce visual-grid columns as necessary, disclose secondary metadata, and keep viewport-bounded overlays internally scrollable. Never clip or overlap primary actions; retain the high-stakes Presenter controls even when the timeline reaches one column.
- Qualify the isolation contract with golden fixtures at Windows and Linux system-scale factors and every Ordo Interface-scale step. Assert invariant logical layout/pagination independently from output-raster density, and verify both supported desktop sizes plus the physical church display.
