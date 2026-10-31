---
id: 36
title: Reusing a past Mass plan
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

# Reusing a past Mass plan

- The operator-facing action is **Reuse for another Celebration**.
- Reuse always creates a new, independent Mass plan and requires assigning its new Celebration and date.
- The source Mass plan remains tied to its original Celebration and is never repurposed by changing its date.
- The new plan does not remain synchronized with the source plan; later edits to either plan do not affect the other.
- Avoid the operator-facing term `Replicate`, because it may imply continuing synchronization, and avoid relying on the generic term `Duplicate`, because it does not communicate the liturgical intent or required new Celebration.
- Reuse copies the source plan's latest editable state, including its Mass-item order, selected Songs and Arrangements, responses, readings, Cues, Theme choices, and Mass-local overrides.
- Reuse does not copy the source plan's identity, Celebration/date, Presentation snapshots, Presentation sessions, live state, or Undo/Redo history.
- The operator must assign the new Celebration and date, after which Ordo recomputes readiness and warns about date-specific or liturgical mismatches such as old readings or questionable seasonal applicability.
- The result is a predictable editable starting point, not a claim that the copied content is correct for the new Celebration.
