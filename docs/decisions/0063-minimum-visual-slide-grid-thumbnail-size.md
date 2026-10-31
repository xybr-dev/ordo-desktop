---
id: 63
title: Minimum Visual Slide Grid thumbnail size
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

# Minimum Visual Slide Grid thumbnail size

- In supported Presenter layouts, the canonical 16:9 frame inside a Visual Slide Grid item has a minimum width of `300px`, producing a frame approximately `300x169px` on screen.
- The minimum applies to the canonical image area, not to captions or group labels placed outside it. The frame is always a scaled or density-rerendered copy of the canonical slide and never reflows its content to fit the thumbnail.
- Responsive layout removes grid columns before shrinking below the minimum: approximately three columns at `1366x768` and up to four at `1920x1080`, subject to the actual space left by the Ordo rail and Output Dock.
- A narrower resizable window may therefore show two or one grid columns. It must not silently switch the operator to Compact Cue List, change the persisted timeline mode, or alter Preview, Program, selection, or scroll position.
- The `300px` floor keeps representative 72-canvas-unit projected text near 11 screen pixels in the timeline while column 3's larger canonical Preview remains the place for close reading.
