---
id: 310
title: Live output draws the retained Frame; rasterize only for pixels
status: accepted
date: 2026-11-04
recorded: 2026-11-04
tags: [rendering, presenter, performance]
supersedes: []
superseded_by: null
amends: []
amended_by: []
sources: [docs/plans/T0.6-implementation-plan.md, MILESTONES.md, crates/ordo/src/performance/display_path.rs]
---

# Live output draws the retained Frame; rasterize only for pixels

## Context

T0.6 compared the two candidate display paths for the same canonical frame
(`MILESTONES.md` T0.6, `docs/plans/T0.6-implementation-plan.md`, Recorded
results):

- **CPU raster:** `rendering::rasterize` to RGBA, encoded, decoded and
  uploaded. 53.1 ms median on the development machine and 66.8 ms on the
  minimum-profile church PC; about 16.8 MB held per slide.
- **Retained scene:** the logical `Frame` walked straight into the GPUI scene
  through its already-shaped glyph, quad and image painting, with the sprite
  atlas caching glyph masks. 13.3 ms and 17.2 ms median; about 0.24 MB held.

T0.6 recorded the numbers but no decision adopted either path. T1.3 states
that its scope depends on this choice and that both designs must not be built.
Every surveyed presentation engine draws live output from a retained scene and
rasterizes only for thumbnails, transitions, capture and export.

## Decision

- Live congregation output and the Program and Preview monitors draw the
  logical `Frame` through the retained path.
- Rasterization (`rendering::rasterize`) stays for uses that need pixels:
  emergency PowerPoint export, capture and diagnostics, and any future
  transition or thumbnail effect that cannot be drawn from the scene.
- Frame caching works on logical `Frame` values by frame identity, counted
  and pinned (Program and Preview are never evicted). It has no
  full-resolution raster byte budget. Decoded foreground images are cached by
  asset hash.
- T1.3's raster-specific criteria (a byte-budgeted full-resolution frame
  cache, a separate low-resolution thumbnail raster cache, and decoded raster
  eviction) do not apply. Its pinning, preload-bound, cancellation, failure
  propagation and memory-envelope criteria do.

## Alternatives considered

- **CPU raster for live output.** It is about 4× slower to the next presented
  frame and holds about 70× more memory per slide. It would also make T1.3 a
  raster scheduler.
- **Both paths, chosen at runtime.** T1.3 forbids building both designs, and
  a second path doubles the canonical-identity surface to qualify.

## Consequences

- Thumbnails in the Cue timeline (T1.4) are the same retained element at a
  smaller fit scale. The canonical-identity guarantee (T0.4) is unchanged,
  because both draw the same `Frame`.
- `presentation::cache::{FrameCache, ThumbnailCache}` (Phase 0 raster caches)
  remain only for the qualification labs until T1.4 retires them.
- Load-time validation renders each frame once with the default Theme. A
  later render under another Theme or density can still fail; T1.3 maps that
  failure to the structured Unavailable result.
- If a future target needs pixels on the live path (for example a capture
  device), it renders from the same `Frame` through `rasterize` without
  changing this decision.
