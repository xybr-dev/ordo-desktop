---
id: 180
title: Celebration-scoped portable transfer
status: accepted
date: 2026-08-29
recorded: 2026-08-29
tags: [mass-plan, transfer]
supersedes: []
superseded_by: null
amends: [17]
amended_by: []
sources: []
---

# Celebration-scoped portable transfer

- This supersedes Decision [17](0017-local-storage-and-portable-transfer.md)'s former single-plan **Mass Package** boundary with
  a **Celebration Package** whose scope is exactly one complete Celebration and
  every Mass plan currently under it at export time. Decision [17](0017-local-storage-and-portable-transfer.md)'s reconciled
  text reflects this replacement.
- Capture or reuse one independent immutable Presentation snapshot per included Mass plan. Never combine several scheduled Masses into one Cue timeline or mutable plan.
- On the church PC, preserve the Celebration hierarchy and let the operator choose which included scheduled Mass plan to inspect, rehearse, or present.
- Include the complete offline dependency closure for all plan snapshots. Deduplicate identical assets and other immutable dependencies by stable content identity or checksum inside the package without creating cross-plan edit coupling.
- Let one returned Celebration Package carry Mass-local emergency changes or overrides originating from any of its included plans for later review on the preparation computer.
- Use **Celebration Package** as the canonical product and domain term because the artifact may contain several scheduled Mass plans. Retire **Mass Package** from current MVP language.
- Require at least one Mass plan to export a Celebration Package; an empty Celebration has no presentable snapshot to transfer.
- Defer exact import verification, local package ownership, same-Celebration replacement/version handling, included-plan selection UI, and return-import review to the next grilling branch.
