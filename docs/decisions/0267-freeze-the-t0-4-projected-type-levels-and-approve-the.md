---
id: 267
title: Freeze the T0.4 projected type levels and approve the gallery
status: accepted
date: 2026-10-08
recorded: 2026-10-08
tags: [presenter, rendering]
supersedes: []
superseded_by: null
amends: []
amended_by: []
sources: []
---

# Freeze the T0.4 projected type levels and approve the gallery

- The T0.4 manual gate approved the fixture-backed Canonical Slide Layout
  Gallery on 2026-10-08: every case, including the long Cebuano/English
  ones, was judged legible. The candidate levels in Decisions [262](0262-render-reading-identification-in-sentence-case-at-the.md)-[265](0265-content-size-psalm-and-response-bodies-and-keep-authored.md) are now
  frozen as `DESIGN.md` records them: display `90px`, passage `70px`, meta
  `50px`, ReadingPassage `88px`, Celebration/Reading-identification title
  `135px`/`800` with Celebration subtitle `68px`/`300`, Psalm ladder
  `140`-`90px`/`700`, Response ladder `160`-`100px`/`800`, and Song title
  `100px`/`700`.
- The captures move to `design-references/ordo-canonical-slide-layouts/` as an
  Approved Reference. A recapture that changes their bytes needs a fresh
  approval.
- A glyph neither bundled face carries renders the bundled `.notdef` box by
  design; the box proves no system font leaks into canonical output.
- The packaged-Linux manual script is not run; T0.4b rescopes it with the
  move to Windows-only development (Decision [268](0268-make-ordo-windows-only-for-the-mvp-and-defer-linux.md)).
