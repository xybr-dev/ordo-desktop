---
id: 303
title: "Cue captions are one line; indicators keep their width"
status: accepted
date: 2026-11-03
recorded: 2026-11-03
tags: [design, presenter, layout]
supersedes: []
superseded_by: null
amends: [238]
amended_by: []
sources: [DESIGN.md, docs/mockups/ordo.css, docs/mockups/ordo-kit.js]
---

# Cue captions are one line; indicators keep their width

## Context

Decision [238](0238-explicit-responsive-truncation-and-disclosure-hierarchy.md) let a Cue caption
outside its canonical frame use up to two lines. In the Presenter slide grid a caption also carries
indicators: **Edited**, and **Program**, **Preview** or **Held** for the Cues on air or selected. When
one of them appeared, a one-line caption could wrap to two and every tile below moved, often while the
operator was navigating during Mass. The Program and Preview words could also wrap onto a second row.

## Decision

- A Cue caption is one line. Its label ellipsizes to make room, and the full label is on hover, on
  focus and in its accessible name.
- The number and the indicators never shrink or wrap, so a tile's height never depends on which
  indicators it shows.
- The canonical slide thumbnail above the caption is unchanged: it still shows the whole slide.
- This applies wherever Cue captions appear: the Presenter grid, the plan's Cue thumbnails and the
  Arrangement section grid.

## Alternatives considered

- **Reserving a second line in every caption.** Every tile grows by a line most of them never use.
- **Hiding the indicators when space is short.** Program, Preview and Held are state the operator
  needs, and Decision 238 says not to cut them.

## Consequences

- Decision 238's two-line allowance for Cue captions no longer applies. Its other rules are unchanged.
- GPUI's caption layout needs a shrinking label slot beside fixed-width indicator slots.
