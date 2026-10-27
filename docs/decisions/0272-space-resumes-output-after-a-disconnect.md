---
id: 272
title: Space resumes output after a disconnect
status: accepted
date: 2026-10-27
recorded: 2026-10-27
tags: [presenter, display]
supersedes: []
superseded_by: null
amends: [49, 61]
amended_by: []
sources: [MVP-PLAN.md]
---

# Space resumes output after a disconnect

## Context

Decision [49](0049-congregation-display-disconnection-and-recovery.md) made recovery after a
congregation-display disconnect take two steps: the operator clicks **Enable Output**, which
recreates the window black, and then presses `Space` to restore Preview. MVP-PLAN §8.1 echoes
this: in the Disconnected state `Space` "has no external effect until output is deliberately
re-enabled."

The click adds nothing to safety. Pressing `Space` is already a deliberate operator act, it is the
command operators use to leave Freeze and Blackout, and it lands on the key their hand is already
on mid-Mass. The two-step path costs time at the moment the congregation is looking at a dark
screen.

What the earlier rule does protect is worth keeping: output never resumes by itself, and it never
lands on the wrong display.

## Decision

- While output is **Disconnected** and the **same recognized congregation display** is present
  again, `Space` recreates the congregation window and takes the current Preview live in one
  step. The session returns to linked Live, exactly as `Space` does from Freeze or Blackout, and
  the black-to-content change uses the ordinary 500ms fade.
- Reconnection alone still never resumes output. Nothing appears until the operator presses
  `Space` or uses the primary control.
- While the display is back, the Presenter primary control reads **Resume Output** with the
  `Space` hint and performs the same command
  (amends Decision [61](0061-context-sensitive-presenter-primary-control.md)).
- If the returning display is not the recognized congregation display, or no congregation display
  is present, `Space` is refused with the reason, and recovery uses **Select display** /
  **Enable Output** as before. Output is still never relocated onto the operator display.
- `Space` on an unavailable Preview is refused as it is everywhere else; output stays
  Disconnected and the operator is told why.
- Unchanged: when a Presentation session starts, output is disabled. In that state **Enable
  Output** stays the only way to create the congregation window, and it starts black.

## Alternatives considered

- **Keep the click (Decision 49 as written).** Rejected: it adds a step at the worst moment
  without adding a safeguard that `Space` itself does not already provide.
- **Space resumes onto any newly attached display.** Rejected: wrong-display targeting is a
  release-qualification failure (MVP-PLAN §10). Only the recognized congregation display
  qualifies.
- **Resume black, then a second Space.** Rejected for the same reason as the click: two presses
  for one intent.

## Consequences

- MVP-PLAN §8.1's Disconnected row changes to match.
- The Presentation Engine gains one transition, Disconnected → Live on `Space`, guarded by
  display identity. The state-model tests in MVP-PLAN §11.1 cover it with the other transitions.
- The Recovery Drill (MVP-PLAN §11.3) checks the one-press path and the refusal on an
  unrecognized display.
