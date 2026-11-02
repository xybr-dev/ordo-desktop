---
id: 295
title: Selection and Preview carry neutral shape cues
status: accepted
date: 2026-11-02
recorded: 2026-11-02
tags: [design, presenter]
supersedes: []
superseded_by: null
amends: []
amended_by: []
sources: []
---

# Selection and Preview carry neutral shape cues

## Context

Selection was a tinted fill alone, at 1.22:1 against the Preparation surface and 1.19:1 on graphite, so
it barely shows in a dim church. On the Presenter timeline, Preview borrowed the blue focus colour in the
same ring shape, on the screen operators drive by keyboard, and `DESIGN.md` named no on-air treatment.

## Decision

- Selection keeps its tinted fill and adds a `3px` leading bar in the neutral text colour plus SemiBold
  primary text.
- The Cue on Program keeps a `2px` ring, Live green or Freeze amber. The Cue on Preview, and the selected
  frame in Preparation, carry a `3px` neutral bar below the frame, outside the canonical composition.
- Rows that are the current item use `aria-current`; `aria-selected` stays on options and tabs.

## Alternatives considered

- **A stronger selection fill.** Reaching 3:1 would fail text contrast on the fill.
- **A green leading bar.** It reads as Live and is the competing green border `DESIGN.md` rejects.
- **Information blue for Preview.** It is nearly the focus colour.

## Consequences

Focus, selection and on-air state are now distinct by shape. The kit's `.o-slide-wrap` no longer draws a
focus-coloured ring, and the three-column reference shows the new marks.
