---
id: 263
title: Render Reading passage at a ReadingPassage-only 88px candidate
status: accepted
date: 2026-10-08
recorded: 2026-10-08
tags: [rendering]
supersedes: []
superseded_by: null
amends: [255]
amended_by: []
sources: []
---

# Render Reading passage at a ReadingPassage-only 88px candidate

- This narrowly adds a bounded ReadingPassage-only slice; this supersedes Decision [255](0255-use-fixed-semantic-congregation-slide-layouts.md)'s shared 70px passage size only for ReadingPassage and makes no change to GeneralText or any later superseding decision.
- Compose `ReadingPassage` bodies at the pending candidate `88px`/weight
  `400` (`70 * 1.25 = 87.5`, rounded; line advance `102px`), left-aligned
  to the safe-area left edge and top-anchored in the body zone, with the
  unchanged `50px` bottom-left reference. Reading behavior, blank-line
  boundaries, and authored casing are unchanged.
- `GeneralText` stays at `70px`; operator, UI, and lab-control typography
  are unchanged. No other fixture, golden, template, schema, parser, UI,
  raster, or compositor change.
- The existing short English fixture wording matches the Catholic USCCB
  text at https://bible.usccb.org/bible/john/1; no additional Scripture
  text is added.
