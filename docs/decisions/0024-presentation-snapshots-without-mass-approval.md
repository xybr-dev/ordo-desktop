---
id: 24
title: Presentation snapshots without Mass approval
status: accepted
date: 2026-08-29
recorded: 2026-08-29
tags: [mass-plan]
supersedes: []
superseded_by: null
amends: []
amended_by: []
sources: []
---

# Presentation snapshots without Mass approval

- MVP has no Mass-level approval status, approval button, or approver role.
- The working Mass plan remains editable and autosaved.
- Starting Presenter Mode selects one immutable **Presentation snapshot** for its Mass plan. Exporting a Celebration Package performs the same capture-or-reuse operation independently for every Mass plan under the exported Celebration.
- If the current Mass plan and all rendering dependencies exactly match an existing Presentation snapshot, Ordo reuses that snapshot instead of creating a duplicate.
- Therefore, quitting and revisiting Presenter Mode without making relevant edits opens a new Presentation session from the same Presentation snapshot; it does not create another content version.
- A cleanly started Presentation session begins with congregation output disabled. When the operator explicitly enables it, external Program begins black. Interrupted-session recovery remains governed by section 14.
- Validation problems remain visible. Starting presentation or exporting with unresolved warnings requires operator acknowledgment rather than formal approval.
- Later edits create a difference from the captured snapshot and never mutate an already exported package or an active presentation snapshot.
- These automatic snapshot boundaries provide a foundation for version history and restoration without forcing an organizational approval workflow.
