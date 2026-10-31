---
id: 284
title: "Theme library cards, the Layout group and a shared-save confirmation"
status: accepted
date: 2026-10-28
recorded: 2026-10-28
tags: [themes]
supersedes: []
superseded_by: null
amends: []
amended_by: []
sources: [docs/mockups/themes.html]
---

# Theme library cards, the Layout group and a shared-save confirmation

## Context

Decisions [103](0103-theme-library-management-archival-and-protected-fallback.md) and
[222](0222-protected-fallback-only-theme-library-state.md) give the Theme library its actions and
sections but not how a Theme is shown. Decision
[96](0096-theme-editor-navigation-specimens-and-core-property-groups.md) names a **Layout** group
without its controls. Decision [97](0097-staged-shared-theme-editing-and-safe-duplication.md) says
Ordo "prominently warns" before saving a shared Theme without saying how.

## Decision

- **Library.** Each Theme is a card: a canonical Lyrics specimen through the Theme, its name, how
  many Mass plans use it, and chips for **Built in**, **Parish default**, **Degraded** and
  **Archived**. The card has **Edit** (**View** for Ordo Black) and a `…` menu with the Decision 103
  actions. **Show archived** appears only when a Theme is archived.
- **Layout group.** Two controls: lyric and response **alignment** (Centered or Left) and **vertical
  placement** (Middle or Upper). The 120 × 80 px safe area isn't adjustable.
- **Typography group.** Preferred and minimum size in pixels, weight as 600, 700 or 800, and line
  spacing as three steps. The typeface is shown read-only, because Ordo bundles one projection font
  (Decision [22](0022-guaranteed-fallback-theme.md)).
- **Contrast.** Appearance shows the computed ratio of text to the composed background: 7:1 or more
  reads as good, 4.5:1 to 7:1 as a warning, below 4.5:1 as too low.
- **Shared save.** While a Theme with unsaved changes is used by more than one Mass plan, a strip
  under the editor header names the count. **Save** then asks once, in a centred dialog, with
  **Keep editing**, **Duplicate instead** and **Save for N plans**. A Theme used by one plan or none
  saves without asking. `Ctrl+S` follows the same rule.

## Alternatives considered

- **A plain list of Theme names.** Hides what a Theme looks like, which is the reason to choose one.
- **Only the strip, no dialog.** The strip is easy to stop seeing; a Theme change reaches every
  inheriting plan at once.

## Consequences

The 4.5:1 and 7:1 thresholds are a starting point to check against the church projector (Decision 22).
