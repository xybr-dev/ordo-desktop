---
id: 44
title: MVP slide transitions
status: accepted
date: 2026-08-29
recorded: 2026-08-29
tags: [rendering]
supersedes: []
superseded_by: null
amends: []
amended_by: [256]
sources: []
---

# MVP slide transitions

> **Superseded in part by Decision [256](0256-refine-operator-appearance-overlays-and-bounded-output.md).** Content-to-content changes remain
> cuts, but content-to/from Blackout or Hold now uses the adopted reversible
> `500ms` fade. The body below is reconciled to that boundary.

- Content-to-content changes and Freeze release to nonblank content use
  instantaneous cuts. Content-to/from Blackout or Hold uses the reversible
  `500ms` opacity fade adopted in Decision [256](0256-refine-operator-appearance-overlays-and-bounded-output.md); Presentation state still changes
  immediately.
- Do not expose fade durations, animation styles, or other transition controls in the MVP.
- Instant cuts preserve time-sensitive operator timing, deterministic Preview-to-Program behavior, and the established responsiveness target.
- Keep transition behavior behind the presentation-rendering Seam so a later version may add carefully qualified transitions without changing the Presentation Engine's navigation semantics.
