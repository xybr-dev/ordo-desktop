---
id: 274
title: Program goes black on an unavailable Cue instead of freezing
status: accepted
date: 2026-10-27
recorded: 2026-10-27
tags: [presenter]
supersedes: []
superseded_by: null
amends: [227]
amended_by: []
sources: [MVP-PLAN.md]
---

# Program goes black on an unavailable Cue instead of freezing

## Context

Decision [227](0227-fail-safe-freeze-when-a-future-cue-becomes-unavailable.md) made navigation
onto an unavailable Cue, while Live, keep the previous frame and enter **Frozen · Cue
unavailable**. From then on, navigation stopped updating Program until the operator pressed
`Space` on a valid Cue.

In practice the operator usually passes through: the Offertory Song's image is missing, the
operator steps over it, and the Sanctus should come up as normal. The automatic Freeze turns that
into a hidden mode change. Program keeps showing a slide that no longer matches what is
happening, and the next valid Cue silently does not go live.

## Decision

- While linked Live, selecting an unavailable Cue makes Program black, as an operator-only Cue or
  Hold does. The ordinary 500ms fade applies. Linked Live continues: the next valid Cue the
  operator moves to goes live as usual.
- Program's status reads **LIVE · black · Cue unavailable**. Preview and the Cue card keep the
  operator-only **Output unavailable** placeholder with its reason, unchanged from Decision 227.
- The black frame is Ordo's own canonical black. Nothing from the failed render, no error surface
  and no partial frame, ever reaches Program.
- Unchanged from Decision 227:
  - When Freeze or Blackout is already active, navigating onto an unavailable Cue leaves it
    active.
  - `Space` on an unavailable Preview is refused with the reason.
  - Unavailable Cues stay in navigation.
  - Preflight still reports them before Presenter entry.
  - The font-integrity and repair rules stand.
- The announcement stays urgent (DESIGN.md, Decision 235): the event is unexpected, and the
  protective response is now "Program is black" rather than "Program is frozen".

## Alternatives considered

- **Automatic Freeze (Decision 227 as written).** Rejected: it hides a mode change and keeps a
  stale slide up.
- **Enter the Blackout state.** Rejected: Blackout persists, so the next valid Cue would not go
  live without `Space` or `B`, which is the same trap as the Freeze.

## Consequences

- MVP-PLAN §6.28 and DESIGN.md's urgent-announcement list change to match.
- An unavailable Cue mid-song now shows the congregation a black screen instead of the previous
  verse. That is deliberate: a black gap reads as a pause, a stale verse reads as wrong words.
