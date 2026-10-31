---
id: 189
title: Symmetric Celebration Package round trip
status: accepted
date: 2026-08-29
recorded: 2026-08-29
tags: [mass-plan, transfer]
supersedes: []
superseded_by: null
amends: []
amended_by: []
sources: []
---

# Symmetric Celebration Package round trip

- Use the same **Celebration Package** contract and export/import workflow in both directions. Do not create a separate Return Package type or a church-only synchronization mode.
- Exporting from the church keeps the durable Celebration UUID, creates a new immutable Package-version UUID and fingerprint, and records the exporting installation plus revision ancestry back to the last shared state.
- Every export remains complete and self-contained: include the Celebration's current Mass plans, one captured-or-reused Presentation snapshot per plan, and the complete current dependency closure used by those plans. Never require the receiving installation to possess an earlier Package version in order to apply a delta.
- On the preparation computer, import through the same verification and reconciliation workflow. Ignore equivalent content, accept church-only changes as safe updates, preserve home-only changes, and require review when both installations changed the same owned content after their common sync anchor.
- After further ordinary editing at home, export another complete Celebration Package and import it at church through the same rules. This round trip may repeat without changing artifact types or turning imported records into package-owned content.
- A change summary such as **7 changes since the version received from Home PC** is explanatory provenance only; it does not alter the contents or semantics of the exported artifact.
