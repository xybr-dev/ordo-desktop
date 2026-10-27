---
id: 168
title: Owner-directed pagination correction instead of a review workspace
status: accepted
date: 2026-08-29
recorded: 2026-08-29
tags: [rendering, ui]
supersedes: []
superseded_by: null
amends: []
amended_by: [257]
sources: []
---

# Owner-directed pagination correction instead of a review workspace

> **Superseded in part by Decision [257](0257-make-library-a-multi-view-destination-with-revision-bound.md).** General Presentation Text replaces
> the former Custom text editor label; owner-directed correction remains
> adopted.

- Remove the standalone **Pagination Review** workspace from the MVP. Pagination remains a rendering concern and warning category, but not a second place for editing projectable text.
- Expose contextual **Fix pagination** from the authoritative Inspector warning and from Readiness or Theme affected-plan findings. Navigate directly to the exact affected content in its owning editor and retain the Inspector as the canonical preview.
- Route a reusable Arrangement finding to its affected Lyrics section in the Song Library; route a Mass-local Song override to its local Lyrics section; and route Response sequences, Readings, and General Presentation Text to their own Mass-item text editors.
- Keep pagination boundary changes, text changes, and their Undo/revision ownership with that source editor. Do not duplicate those controls inside the Inspector or create an intermediate review surface.
- Use the Readiness panel as the plan-wide overview for unresolved pagination findings. Resolving source content recomputes warnings automatically.
