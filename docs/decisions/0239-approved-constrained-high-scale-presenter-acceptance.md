---
id: 239
title: Approved constrained/high-scale Presenter acceptance reference
status: accepted
date: 2026-08-29
recorded: 2026-08-29
tags: [presenter, ui]
supersedes: []
superseded_by: null
amends: []
amended_by: [294]
sources: []
---

# Approved constrained/high-scale Presenter acceptance reference

> **Amended by Decision [294](0294-the-latest-mockup-page-is-canonical-and-superseded-pages.md).** The reference this record names is now its `-v2` page; the v1 page is kept as superseded. The rest of this record stands.

- Approve `design-references/ordo-presenter-high-scale.html` and its standalone rendering (now [`docs/mockups/presenter-high-scale.html`](../mockups/presenter-high-scale.html), Decision [271](0271-mockups-are-one-page-per-screen-with-a-status-each.md)) as the current visual acceptance reference for Presenter adaptation from `100%` through `200%` Interface scale at the compact physical desktop context.
- Keep the `175%` view as the reference's default because it makes the constrained composition explicit: Ordo is overlay-only; Program and Preview remain simultaneously visible and identical while Live; output status, Freeze, Blackout, and the Live primary state remain fixed beneath them; and the Cue timeline alone receives the remaining scrolling space.
- Retain the interactive scale and Grid/List controls as visual evidence that Interface scaling changes only operator chrome and responsive composition. Canonical frame text and geometry do not reflow, the selected Cue remains represented consistently, and the user's Cue-view preference survives adaptation.
- Treat the reference as information-hierarchy evidence rather than production markup or a source of exact breakpoints. The wider Presenter remains the preferred experience; production thresholds arise from effective available dimensions and qualified minimum region sizes under Decisions [234](0234-operator-only-interface-scaling-isolated-from-canonical.md) and [237](0237-safety-first-constrained-presenter-composition.md).
