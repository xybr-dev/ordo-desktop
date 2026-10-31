---
id: 261
title: "Derive display pixels through the compositor, not a CPU raster cache"
status: accepted
date: 2026-09-11
recorded: 2026-09-11
tags: [presenter]
supersedes: []
superseded_by: null
amends: []
amended_by: []
sources: []
---

# Derive display pixels through the compositor, not a CPU raster cache

- This supplements Decision [234](0234-operator-only-interface-scaling-isolated-from-canonical.md), which stays unchanged: canonical composition
  remains an explicit `1920x1080` logical space, and Interface scale, system
  scaling, and output density still change sampling only. What changes is how a
  completed canonical frame reaches a screen.
- Surveyed open-source presentation engines draw live output from a retained
  scene their compositor owns: LibreOffice Impress walks a primitive tree,
  OpenLP renders an HTML display document in an embedded browser engine, Quelea
  builds a JavaFX scene graph, and FreeShow mounts one component tree for both
  preview and projector. Full-frame rasterization appears only at the periphery
  — slide-sorter thumbnails, transition textures, screen capture, and export.
  The engines that do cache one bitmap per slide are PDF-backed and have no
  structured scene to retain, which is not Ordo's situation.
- Treat the logical `Frame` as a display list to be painted, not as input to a
  CPU compositing pass. The pinned GPUI revision paints already-shaped glyphs
  directly and caches their masks in a sprite atlas keyed by face, glyph, size,
  subpixel variant, and scale factor, shared across every slide and viewport.
  Compositing RGBA on the CPU, encoding it, decoding it, and uploading it
  reimplements that atlas per frame and holds the result in process memory.
- Keep the deterministic CPU rasterizer. It remains the qualified path for
  export, golden fixtures, and the density-invariance evidence, and it is the
  reason Ordo can produce exact pixels from a logical frame without capturing a
  live surface the way the surveyed engines must. Retire it from the display
  path only, and only on T0.6's measurement.
- Accept that compositor-painted output cannot be byte-identical to
  `tiny-skia` goldens, because platform glyph rasterization and subpixel
  handling differ. Qualify the display path against canonical composition —
  identical logical geometry, wrapping, and pagination — and keep byte-exact
  golden comparison on the CPU path, which is the separation T0.4 already
  established.
- Rasterize a full frame on the display path only where a retained redraw buys
  nothing: blending two frozen frames during a transition, which is what
  Impress uploads two textures for. Not for static slides, and not before
  transitions exist.
- Decide by measurement, not by this entry. T0.6 compares both paths and
  confirms the bundled canonical faces resolve through the GPUI text system
  rather than a same-named system installation; T1.3's caching scope is
  reconciled against that result before it starts.
