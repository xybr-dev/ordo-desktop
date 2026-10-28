---
id: 285
title: Masses index rows name the presider and carry the backup reminder
status: proposed
date: 2026-10-28
recorded: 2026-10-28
tags: [mass-plan]
supersedes: []
superseded_by: null
amends: []
amended_by: []
sources: [docs/mockups/masses-index.html]
---

# Masses index rows name the presider and carry the backup reminder

## Context

Decision [83](0083-celebration-grouped-masses-index.md) lists what a Mass-plan row shows: time,
language, choir, readiness and actions. Decision
[89](0089-lightweight-presider-location-and-notes-input.md) gives each plan a presider, which
differs between Masses of one Celebration. Decision
[198](0198-manual-portable-backup-automatic-safety-backup-and-reminder.md) shows the portable-backup
reminder "in Transfer & Backup and the application header", but the preparation shell has a rail and
per-destination headers, not one application header.

## Decision

- A Mass-plan row in Masses shows time, language, choir, **presider** and readiness, in that order.
  A plan with no choir says **No choir**. An Anticipated Mass carries an **Anticipated** chip.
- Clicking anywhere on a row opens the plan. In History the row shows when it was presented instead
  of readiness and offers **Reuse**.
- The portable-backup reminder appears as an amber button in the Masses header, beside
  **New Celebration**: **No backup in N days · M changes**. It opens Transfer & Backup.
- A Celebration header's readiness roll-up reads **All N ready**, **K of N ready** or **No Mass
  plans**.

## Alternatives considered

- **Reminder on every destination header.** Repeats a weekly nudge across screens that are about
  other work.
- **Reminder as a dot on the Transfer & Backup rail entry.** Too easy to miss for something that
  protects every record in the parish.

## Consequences

Masses is the screen Ordo opens on, so the reminder is seen at every start while it applies.
