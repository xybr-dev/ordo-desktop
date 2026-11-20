---
id: 315
title: Lyric pagination approval is per section
status: proposed
date: 2026-11-16
recorded: 2026-11-16
tags: [songs, rendering]
supersedes: []
superseded_by: null
amends: [18]
amended_by: []
sources: [docs/plans/T2.8-implementation-plan.md, docs/decisions/0018-lyric-pagination.md]
---

# Lyric pagination approval is per section

## Context

Decision [18](0018-lyric-pagination.md) has an operator approve the generated slide sequence before
an Arrangement is presentation-ready, and marks the Arrangement **Needs Review** when reviewed lyrics
or "arrangement order" change. T2.8 adds the Sequence: an ordered list of occurrences of named
sections, where the same section may appear more than once. Approval stored once per Arrangement would
send the whole Arrangement back to review after editing one Chorus, and would treat a reorder as a
lyric change although no slide text or boundary changed.

## Decision

- **Approval is per section.** Each section carries its own pagination status (Needs review or
  Approved). Approve pagination approves one section's slides; an Arrangement is presentation-ready
  when every section its Sequence uses is approved and no other blocker applies.
- **A lyric change moves only that section to Needs review.** This includes Split and Join, which
  change the section's slide boundaries. Other sections keep their approval.
- **Sequence order changes never change approval.** Adding, repeating, moving or removing an
  occurrence changes which slides are presented and in what order, not how any section paginates.
  Decision 18's "arrangement order" trigger no longer applies to Needs Review.
- A section edited once is approved once: every occurrence of it shows the edited slides and shares
  its one status.

## Alternatives considered

- **One approval per Arrangement (Decision 18 as written).** Rejected: one edit invalidates review of
  unrelated sections, and a reorder would force a re-review of unchanged text.
- **Approval per occurrence.** Rejected: occurrences of one section render identical slides, so the
  repeated approval is redundant and the copies could disagree.

## Consequences

- Readiness lists one finding per referenced section, the most severe kind, not one per occurrence.
- Migration 0007 stores the status on each section.
- **Proposed, awaiting owner confirmation.** The behaviour above ships in T2.8 so it can be reviewed
  in the lab; if the owner declines, Decision 18 stands and T2.8 changes accordingly.
