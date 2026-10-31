---
id: 294
title: The latest mockup page is canonical and superseded pages are hidden
status: accepted
date: 2026-10-31
recorded: 2026-10-31
tags: [design, mockups, repo]
supersedes: []
superseded_by: null
amends: [169, 178, 208, 239, 240]
amended_by: []
sources: [docs/mockups/index.html, docs/mockups/mockups.js]
---

# The latest mockup page is canonical and superseded pages are hidden

## Context

Decision [271](0271-mockups-are-one-page-per-screen-with-a-status-each.md) keeps replaced pages for
history. When the regenerated `-v2` pages were approved, each v1 page still read as approved, and
`DESIGN.md` and `MVP-PLAN.md` still named v1 pages as references. Two approved pages for one screen
leave a later reader to guess which one counts.

## Decision

- The newest page of a screen is canonical. Each v1 page with a `-v2` replacement is `superseded`,
  with `supersededBy` naming it.
- Acceptance references move with this: `gate: true` and the references in `DESIGN.md` and
  `MVP-PLAN.md` now name the `-v2` pages. The Flows on the index link to them as well.
- The index opens on **Current**, which hides superseded pages. The **Superseded** and **All**
  filters show them, and the pages stay in the repository, reachable by link.

## Alternatives considered

- **Leave v1 approved until a later cleanup.** It keeps two versions of each screen in `main`.
- **Delete the v1 pages.** Decision 271 keeps them for history, and older records link to them.

## Consequences

Decisions 169, 178, 208, 239 and 240 keep their history but point at the `-v2` pages for acceptance.
A v1 page's caveats still show on that page.
