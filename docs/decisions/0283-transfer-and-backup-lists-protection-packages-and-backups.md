---
id: 283
title: Transfer and Backup lists protection, packages and backups in that order
status: proposed
date: 2026-10-28
recorded: 2026-10-28
tags: [storage]
supersedes: []
superseded_by: null
amends: []
amended_by: []
sources: [docs/mockups/transfer-backup.html]
---

# Transfer and Backup lists protection, packages and backups in that order

## Context

Decision [191](0191-contextual-celebration-package-export-slide-over.md) makes **Transfer & Backup**
the administrative home for Package history, verification details, restoration, removal and the global
export and import actions. Decision [199](0199-reuse-verified-artifact-publication-ux-for-parish-backup.md)
names a **Protect this parish** section, and Decisions
[201](0201-safety-backups-use-ordinary-manual-retention-in-the-mvp.md) and
[231](0231-later-discovered-package-and-backup-availability-versus.md) say what safety backups and
unavailable or damaged artifacts show. No decision arranges the page.

## Decision

- The page is one column with three sections, in this order:
  1. **Protect this parish**: the last portable backup's time and location, the changes since it, the
     safety-backup count, **Create parish backup** (primary) and **Restore parish backup…**.
  2. **Celebration Packages**: **Export celebration…** and **Import .ordo…**, then **Package history**,
     one row per Installed Celebration Package. A row shows the Celebration, its Current version and
     where that version came from, and expands to every retained version.
  3. **Parish Backups**: portable backups and safety backups in one list, newest first. Each row
     shows its time, location, size and state (**Verified**, **Unavailable** or **Damaged**), and only
     the actions that state allows.
- While the portable-backup reminder applies (Decision
  [198](0198-manual-portable-backup-automatic-safety-backup-and-reminder.md)), a warning strip under
  the header repeats it with **Create parish backup**.
- **Export celebration…** opens the export slide-over with a Celebration list first; choosing one shows
  the same content as export started from Masses.

## Alternatives considered

- **Packages first.** Moving Celebrations happens weekly, but losing the parish is the costlier
  failure, and Decision 199 already leads with protection.
- **Separate tabs for packages and backups.** Splits the evidence an operator compares when deciding
  whether the parish is safe.

## Consequences

Import review and restore keep their own full workspaces (Decisions
[181](0181-two-layer-celebration-package-import-review.md) and
[200](0200-replace-only-restore-workspace-with-an-exact-protection-gate.md)); this page only launches
them.
