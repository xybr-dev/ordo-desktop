---
id: 109
title: Viewport-bounded dialogs and slideovers
status: accepted
date: 2026-08-29
recorded: 2026-08-29
tags: [rendering, ui]
supersedes: []
superseded_by: null
amends: []
amended_by: []
sources: []
---

# Viewport-bounded dialogs and slideovers

- Every dialog, floating slideover, and nested overlay is sized and positioned against the currently visible Ordo application viewport—not the document height or a content pane whose height may grow beyond the screen.
- Preserve a visible exterior gutter around floating overlays. A design described as full-application-height means the visible viewport minus that gutter; it never means the full height of an expanded underlying page.
- When content exceeds the available height, keep the overlay header and required commit/cancel actions reachable while scrolling only its content body. The underlying page does not expand the overlay or scroll in its place.
- Nested slideovers share the same viewport boundary. Parent recession, child focus, and transitions may not push either surface outside the visible bounds.
- Implement overlays through the application overlay/portal layer with explicit viewport-relative maximum height, overflow containment, focus trapping, and `Esc`/focus restoration. Verify the behavior at `1366x768`, `1920x1080`, short resized windows, and increased interface text scaling.
