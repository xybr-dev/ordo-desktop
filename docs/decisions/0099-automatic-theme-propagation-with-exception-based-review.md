---
id: 99
title: Automatic Theme propagation with exception-based review
status: accepted
date: 2026-08-29
recorded: 2026-08-29
tags: [themes]
supersedes: []
superseded_by: null
amends: []
amended_by: []
sources: []
---

# Automatic Theme propagation with exception-based review

- After a Theme Save or Theme Undo, Ordo automatically rerenders all editable Mass plans that currently inherit the Theme. Clean rerenders require no approval step and do not interrupt the editor with a modal workflow.
- Human review is requested only for meaningful deltas introduced by the Theme change: new overflow or contrast warnings, changed pagination or slide count, or a newly incomplete projectable Mass item.
- When such deltas exist, Save feedback offers **Review affected plans** and reports the number of plans needing attention. Plans without meaningful deltas are omitted from this review.
- The affected-plan review groups findings by Mass plan and affected Mass item. Activating a finding opens that plan at the owning item and exact affected content rather than leaving the operator to locate the changed slide manually.
- Review is advisory and exception-based. The Theme revision is already saved and remains recoverable through Undo; immutable Presentation snapshots remain unchanged.
