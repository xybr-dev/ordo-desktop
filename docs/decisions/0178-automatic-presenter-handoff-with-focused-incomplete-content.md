---
id: 178
title: Automatic Presenter handoff with focused incomplete-content acknowledgment
status: accepted
date: 2026-08-29
recorded: 2026-08-29
tags: [presenter, ui]
supersedes: []
superseded_by: null
amends: []
amended_by: []
sources: []
---

# Automatic Presenter handoff with focused incomplete-content acknowledgment

- Make **Present** finish pending autosave and prepare a presentation candidate from the exact current Mass-plan revision. Reuse the latest Presentation snapshot when presentation-relevant content is unchanged; otherwise persist a new immutable snapshot only after the operator actually proceeds into Presenter Mode.
- Run automatic preflight against that exact candidate. Enter Presenter Mode directly for a clean result or review-only warnings; do not impose an approval dialog or checklist on ordinary entry.
- When content is incomplete, open one centered, viewport-bounded modal above the visibly dimmed planner. Summarize affected Mass items and unavailable Cue counts, then list findings compactly in Mass-section and item context with an internally bounded scroll area.
- Make **Review and fix** the recommended, initially focused action. It closes the modal, opens the existing Readiness surface, and focuses the first incomplete finding. Let `Esc`, Close, or **Cancel** return unchanged to the planner.
- Provide **Present available content** as an explicit warning-toned secondary action. The action itself is the acknowledgment; do not add an **I understand** checkbox. Unavailable Cues remain visible and inspectable in Presenter Mode but can never be taken live.
- If the operator cancels or chooses review, do not create an unused Presentation snapshot. If snapshot capture or opening fails after proceeding, remain in the planner and show the fatal Presenter-entry blocker with recovery guidance.
- Open Presenter Mode with congregation output disabled and without creating an output window merely because **Present** was clicked.
- Treat `ordo-incomplete-preflight*` as the approved interaction and information-hierarchy reference for the incomplete-content handoff.
