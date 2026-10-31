---
id: 271
title: Mockups are one page per screen with a status each
status: accepted
date: 2026-10-27
recorded: 2026-10-27
tags: [design, mockups, repo]
supersedes: []
superseded_by: null
amends: []
amended_by: []
sources: [clockout:docs/mockups/index.html, clockout:docs/mockups/mockups.js, design-references/README.md, DESIGN.md]
---

# Mockups are one page per screen with a status each

## Context

The HTML explorations from the MVP interview lived in `design-references/` as 15 pairs: an
editable fragment and a `*-standalone.html` export wrapping it in an iframe. A flat README listed
them, and which ones were acceptance references, which were post-MVP research, and which details
a later decision had replaced were spread across that README's "Known superseded details",
`DESIGN.md`'s Approved References, and individual decisions. The owner wants to revisit the
screens, flows and tokens, and wants a navigable set of pages that shows what is approved,
proposed and superseded, as clockout's `docs/mockups/` does. Ordo is early, so the move is cheap.

## Decision

- Mockups live in `docs/mockups/`: an `index.html` grouped by area with operator-journey flows,
  one `<slug>.html` per screen, and shared `mockups.css` and `mockups.js`, after clockout's layout.
- `mockups.js` holds a registry of pages. Each page has a status (`approved`, `proposed`,
  `superseded`, `research`), an optional `gate` flag for the acceptance references `DESIGN.md`
  names, the decision records it rests on, and `caveats`: details on the page that a later
  decision replaced. The index, each page's header and its notes are rendered from the registry,
  so status is kept in one place.
- A page is approved only through a decision record; the registry follows the record.
- The 15 explorations move as the baseline, with their markup unchanged inside the shared shell,
  and the standalone copies are dropped. They start `approved`, except the two Song-merge pages,
  which are `research` (Decision 167). The README's superseded details become per-page caveats,
  and the three that hold for every page are listed once on the index.
- The Canonical Slide Layout Gallery stays in `design-references/ordo-canonical-slide-layouts/`,
  because the capture test writes it there; a mockup page shows it.
- The chrome uses `DESIGN.md`'s Preparation tokens in light and Presenter tokens in dark, with a
  System / Light / Dark switch that the embedded screens follow.

## Alternatives considered

- **Keep `design-references/` and add statuses to its README.** Nothing to navigate, and status
  would still sit apart from the pages.
- **Static index HTML as in clockout.** Status would be written twice, on the tile and on the page,
  and drift.
- **Rebuild every exploration in the new shell's styles now.** Wasted work before the screens are
  revisited; the revisit will replace pages through `proposed` versions instead.

## Consequences

- `DESIGN.md`, `MVP-PLAN.md` and decisions 169, 178, 208, 239 and 240 point at `docs/mockups/`;
  the accepted records keep their original paths with a "now" link.
- Redesigned screens arrive as `proposed` pages beside the approved ones and replace them only
  when a decision accepts them, so the index always shows what is in force.
- The pages load fonts and, in some screens, icons from public CDNs, as the explorations did. They
  are documentation, not the app, so the offline runtime rule does not apply to them.
- Nothing checks the registry against the files yet; a page missing from `PAGES` fails loudly in
  the browser, but a stale decision link does not.
