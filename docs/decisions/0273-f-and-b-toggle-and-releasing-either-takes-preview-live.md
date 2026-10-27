---
id: 273
title: F and B toggle, and releasing either takes Preview live
status: accepted
date: 2026-10-27
recorded: 2026-10-27
tags: [presenter]
supersedes: []
superseded_by: null
amends: [11, 12]
amended_by: []
sources: [MVP-PLAN.md]
---

# F and B toggle, and releasing either takes Preview live

## Context

Decisions [11](0011-program-preview-freeze-and-blackout-semantics.md) and
[12](0012-safe-live-operations.md) made `Space` the only way out of Freeze and Blackout. `B` only
activated Blackout. MVP-PLAN §8.2 lists `F` as "Toggle Freeze" without saying what turning it off
does.

An operator who has just pressed `B` or `F` expects the same key to undo it. Making them reach
for `Space` instead is a second key to remember under pressure, and the mockups showed that the
gap was already being filled by guesswork.

## Decision

- `B` toggles Blackout. `F` toggles Freeze.
- Turning either off does exactly what `Space` does: the current Preview becomes Program and the
  session returns to linked Live. It does not restore the frame that was on Program before.
- `Space` keeps working from both states as before.
- The refusals that apply to `Space` apply to the release too. If Preview is an unavailable Cue,
  Freeze or Blackout stays on and the operator is told why.
- While Blackout is on, `F` does nothing, as before; the operator leaves Blackout first.
- The Presenter buttons follow the state: **Freeze** / **Unfreeze**, **Blackout** /
  **End Blackout**, each keeping its key hint and pressed state.

## Alternatives considered

- **Space only (Decisions 11 and 12 as written).** Rejected: a second key for the undo.
- **Release restores the frame Program had before.** Rejected: the operator usually navigated
  during Blackout or Freeze to prepare the next Cue, so Preview is what they mean to show.
  Restoring the old frame would contradict the linked Live model.

## Consequences

- MVP-PLAN §8.2's `F` and `B` rows change to match.
- A double-tap of `B` now blacks out and immediately shows Preview. Watch for this in the
  Recovery Drill (MVP-PLAN §11.3); if it happens in practice, a short guard against a repeat
  press is the fix, not a return to Space only.
