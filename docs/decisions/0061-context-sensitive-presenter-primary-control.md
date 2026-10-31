---
id: 61
title: Context-sensitive Presenter primary control
status: accepted
date: 2026-08-29
recorded: 2026-08-29
tags: [presenter]
supersedes: []
superseded_by: null
amends: []
amended_by: []
sources: []
---

# Context-sensitive Presenter primary control

- Presenter Mode does not show a permanent Take button. In Ordo's linked Live state, Cue navigation already updates Preview and Program together, so an always-present Take control would falsely imply a traditional manual transition model.
- When congregation output is disabled, the primary control is **Enable Output**. Enabling remains explicit and follows the established black-first behavior; it never exposes Preview immediately.
- When output is enabled but black, Frozen, or Blacked out, the primary control is **Take Preview Live**. It performs the same transition as `Space`: the selected Preview becomes Program and the session returns to linked Live state.
- While Live, the primary-control area becomes an unmistakable informational state, **LIVE — navigation updates Program**, rather than presenting an inapplicable action.
- Freeze and Blackout remain directly available at all applicable times and retain their keyboard shortcuts. Their availability and state are not hidden behind the context-sensitive primary area.
- Button labels describe the immediate outcome, while Program and Preview state indicators remain separately visible. Color alone is never the only distinction between Live, Frozen, Blackout, or output-disabled states.
