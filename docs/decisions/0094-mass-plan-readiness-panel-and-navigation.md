---
id: 94
title: Mass-plan Readiness panel and navigation
status: accepted
date: 2026-08-29
recorded: 2026-08-29
tags: [mass-plan, ui]
supersedes: []
superseded_by: null
amends: []
amended_by: []
sources: []
---

# Mass-plan Readiness panel and navigation

- Activating the plan header's readiness indicator opens a floating **Readiness** slide-over from the right above the Inspector, preserving the Mass Outline and selected-item editor underneath.
- The trigger states active counts in words, such as **1 incomplete · 2 warnings**, rather than hiding severity behind a generic icon or inaccurately calling ordinary missing content a blocker.
- The panel begins with computed readiness and counts, then groups findings under **Incomplete presentation** and **Review recommended**. Plan-level findings precede item-level findings within a group; item findings follow Mass-outline order and identify their owning item.
- Each finding states the concrete consequence and offers both a navigable finding title and a specific corrective action. Activating the title selects and scrolls to the relevant Mass item while preserving the panel; activating the corrective action closes the panel and moves focus to the relevant editor control.
- Findings resolve automatically from the current plan state and disappear when their cause is corrected. The MVP has no manual **Mark resolved** state.
- **Incomplete** means some intended output cannot be faithfully produced, but it is not itself a Presenter-entry blocker. **Present** remains available and requires a prominent acknowledgment; Unavailable Cues remain visible and inspectable but cannot be taken live.
- Only a fatal inability to capture or open a coherent Presentation snapshot blocks Presenter Mode. This exceptional condition must be described as inability to present, not combined with ordinary incomplete-content counts.
