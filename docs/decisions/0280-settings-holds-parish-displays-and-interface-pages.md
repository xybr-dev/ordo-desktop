---
id: 280
title: "Settings holds Parish, Displays and Interface pages"
status: proposed
date: 2026-10-28
recorded: 2026-10-28
tags: [ui]
supersedes: []
superseded_by: null
amends: []
amended_by: []
sources: [docs/mockups/settings.html]
---

# Settings holds Parish, Displays and Interface pages

## Context

Decision [64](0064-preparation-shell-primary-navigation.md) says Settings owns parish, application and
display configuration. Separate decisions name individual pages:
[89](0089-lightweight-presider-location-and-notes-input.md) has **Parish Settings → Defaults**,
[202](0202-installation-specific-displays-settings-page.md) has **Settings → Displays**, and
[234](0234-operator-only-interface-scaling-isolated-from-canonical.md) puts **Interface scale** in
Settings. None of them says how Settings is laid out or which pages it has.

## Decision

- Settings is a list of pages on the left with the selected page on the right, like the Library's
  master-detail. The pages are **Parish**, **Displays** and **Interface**.
- **Parish** holds the parish name (required, as in Decision
  [218](0218-minimal-first-run-parish-setup-with-regular-presider.md)) and the defaults new Mass plans copy:
  **Regular presiders**, a **Default presider** chosen from them, and a **Default Mass location**.
  Changes save as they're typed. Removing a regular presider doesn't change plans that name them.
- **Displays** is the page Decision 202 describes.
- **Interface** holds Interface scale as a five-step segmented control (100% to 200%), with the
  `Ctrl++`, `Ctrl+-` and `Ctrl+0` keys shown beside it.
- Under the pages, a separator and a **Diagnostics** link go to the Diagnostics destination
  (proposed Decision [282](0282-diagnostics-opens-from-its-own-rail-entry.md)).

## Alternatives considered

- **One long scrolling Settings page.** Displays needs room for its topology and calibration
  preview, and would push Parish and Interface out of sight.
- **Tabs across the top.** Tabs are reserved for views inside a destination (Decision
  [250](0250-use-a-hybrid-global-rail-and-local-tab-navigation-hierarchy.md)); a page list leaves room
  for more pages later.

## Consequences

Later settings get a page in the same list rather than a new rail entry. The Parish page is where
the values typed during Set up Ordo can be changed afterwards.
