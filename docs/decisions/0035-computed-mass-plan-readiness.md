---
id: 35
title: Computed Mass-plan readiness
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

# Computed Mass-plan readiness

- Do not add a manually assigned `Draft`, `Approved`, or `Completed` state machine to Mass plans in the MVP.
- Ordo computes Mass-plan readiness from current content and validation results. The operator does not click a button to declare a plan ready.
- Readiness distinguishes `Incomplete`, `Ready with warnings`, and `Ready` for preparation guidance; these are computed assessments rather than organizational approval states.
- `Incomplete` is narrow: it means Ordo cannot faithfully produce part of the intended presentation because a concrete dependency is missing, broken, empty, structurally invalid, or unrenderable.
- Examples include an unfilled required placeholder, no Arrangement selected for an assigned Song, a reference to deleted content or a missing asset, a projectable Cue with no renderable content, an invalid canonical frame, or an empty or corrupted plan.
- Optional omissions, applicability-tag mismatches, unreviewed pagination, missing attribution or provenance, and differences from prior use are warnings rather than causes of incompleteness.
- Projector or display disconnection is runtime Presenter readiness, not Mass-plan incompleteness.
- A Presentation session records that a Presentation snapshot was actually used. A past dated Mass plan remains available as history without an operator-assigned `Completed` status.
- An `Incomplete` assessment does not by itself prevent entry into Presenter Mode. The operator receives a prominent acknowledgment identifying affected Mass items and Cues, then may continue using the valid portions of the plan.
- Unavailable Cues remain clearly marked in Preview. Attempting to take an unavailable Cue live must leave Program unchanged rather than expose an error or partial frame to the congregation.
- Only a fatal inability to capture or open a coherent Presentation snapshot, such as unreadable or irrecoverably inconsistent storage, blocks Presenter Mode entirely.
