---
id: 264
title: "Render Response at a Response-only 113px candidate, centered vertically"
status: accepted
date: 2026-10-08
recorded: 2026-10-08
tags: [rendering]
supersedes: []
superseded_by: null
amends: [255]
amended_by: [265]
sources: []
---

# Render Response at a Response-only 113px candidate, centered vertically

> **Superseded in part by Decision [265](0265-content-size-psalm-and-response-bodies-and-keep-authored.md).** The fixed `113px`/weight `400` uppercase Response body is replaced; the rest remains authoritative. (Notice added when the ledger was split; the decision said so in its own text.)

- This narrowly adds a bounded Response-only slice; this supersedes Decision [255](0255-use-fixed-semantic-congregation-slide-layouts.md)'s Response size/vertical placement only for Response and makes no change to Lyrics, Psalm, Song identification, Reading, GeneralText, or any later superseding decision.
- Compose `Response` bodies at the pending candidate `113px`/weight `400`
  (`90 * 1.25 = 112.5`, rounded; line advance `130px`), centered
  horizontally and centered vertically in the available body zone. The
  shared `PROVISIONAL_DISPLAY_SIZE` stays at `90px`; uppercase body,
  optional authored header behavior, and the `50px` header size/weight/position
  are unchanged. Present/empty headers reserve the `58px` header zone with
  their one-line body starting at `y=504`; an absent header keeps no header
  rect with its one-line body starting at `y=475`.
- Lyrics, Psalm, Song identification, ReadingPassage top anchoring, GeneralText
  `70px`, operator `24/16/14`, and lab-control typography are unchanged. No
  other fixture, golden, template, schema, parser, model, UI, raster, or
  compositor change; no fixture text change.
