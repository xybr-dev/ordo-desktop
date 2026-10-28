---
id: 282
title: Diagnostics opens from its own rail entry
status: proposed
date: 2026-10-28
recorded: 2026-10-28
tags: [ui]
supersedes: []
superseded_by: null
amends: [204, 207]
amended_by: []
sources: [docs/mockups/diagnostics.html, docs/mockups/settings.html]
---

# Diagnostics opens from its own rail entry

## Context

Decisions [204](0204-offline-read-only-diagnostics-surface.md) and
[207](0207-issue-first-single-column-diagnostics-hierarchy.md) describe a **Settings → Diagnostics**
page. Decision [250](0250-use-a-hybrid-global-rail-and-local-tab-navigation-hierarchy.md), recorded
later, anchors **Diagnostics** in the rail beside Settings and Help. Read together, Diagnostics is in
two places.

## Decision

- The rail's **Diagnostics** entry opens the Diagnostics page directly, and the rail shows
  Diagnostics as the current destination.
- Settings keeps a **Diagnostics** link below its own pages, which goes to the same destination.
- Everything else in Decisions 204 and 207 stands: the page is a single column, issue-first and
  read-only, and its findings link to their owners (Displays, Transfer & Backup, a Theme).

## Alternatives considered

- **Diagnostics as a Settings page, with the rail entry opening Settings.** The rail would then
  highlight Settings for a page reached through the Diagnostics entry.
- **Drop the rail entry.** Hides the page an operator looks for when something is wrong, which is
  why Decision 250 made it a destination.

## Consequences

When accepted, this amends where Decisions 204 and 207 place the page; their content is unchanged.
