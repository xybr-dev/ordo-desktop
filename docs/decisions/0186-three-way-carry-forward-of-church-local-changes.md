---
id: 186
title: Three-way carry-forward of church-local changes
status: accepted
date: 2026-08-29
recorded: 2026-08-29
tags: []
supersedes: []
superseded_by: null
amends: []
amended_by: []
sources: []
---

# Three-way carry-forward of church-local changes

- When a newer Package version arrives while its ordinary local Mass plans contain saved church-local emergency changes, compare three states: the previous version's imported state as the common base, the current local plan revision, and the incoming version's corresponding plan content.
- Use stable Mass-plan, section, item, Cue, and owned-content identities to classify each church-local change. Automatically carry forward a change only when its affected incoming content remains equivalent to the base and its destination still exists.
- When home and church changed the same owned value or structure after the common base, require an explicit import decision. Choosing the church state keeps that value in the reconciled ordinary local plan; choosing the incoming state accepts the incoming value.
- Require explicit confirmation before discarding any church-local change. A discarded change remains recoverable through the previous Package version and its History until that retained version is deliberately removed under future retention rules.
- If an incoming Package version removed the owning Mass plan, Mass item, Cue, or another required destination, do not guess a new destination. Keep the unmappable change attached to the previous version, disclose why it was not carried, and preserve it for return transfer.
- Never modify either Package version's immutable snapshots. Every retained church value remains part of the ordinary local revision history, records its previous-version provenance, and remains reviewable when returned to the preparation computer.
- Make the incoming Package version Current only after every overlapping or unmappable change has been resolved. Non-overlapping carry-forward remains automatic-first and appears in the staged import summary rather than demanding ceremonial approval.
