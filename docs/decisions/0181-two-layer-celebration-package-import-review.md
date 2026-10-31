---
id: 181
title: Two-layer Celebration Package import review
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

# Two-layer Celebration Package import review

- Import a Celebration Package through one dedicated temporary review workspace rather than compressing the package-wide workflow into a slide-over.
- Present two explicit but coordinated layers: **Exact presentation** installs the package's immutable per-plan Presentation snapshots and dependencies, while **Library reconciliation** stages editable Celebration, Mass-plan, Song, Arrangement, Theme, and related source records for the receiving parish library.
- Keep exact presentation fidelity independent of reconciliation. A library identity choice, accepted update, rejected update, or newly created local record may never rewrite the embedded Presentation snapshots.
- Classify staged library records as **New**, **Safe update**, **Unchanged**, or **Needs decision**. Resolve stable UUID identity and a remembered package-to-local identity mapping before using normalized metadata and content similarity to suggest a possible semantic match.
- Preserve incoming UUIDs for genuinely new records. When an operator deliberately maps an incoming identity to a different existing local identity or keeps it as a separate copy, retain that reviewed mapping so succeeding home-to-church imports do not repeat the same question without new evidence.
- Keep the primary import commit unavailable while consequential decisions remain unresolved. Show package verification and the included plan snapshots separately from reconciliation counts so readiness to render is not confused with readiness to modify editable library state.
- Use the reviewed interaction hierarchy: full import workspace, compact two-layer summaries, staged-result table, and a focused decision slide-over.
