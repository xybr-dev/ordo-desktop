---
id: 11
title: "Program, Preview, Freeze, and Blackout semantics"
status: accepted
date: 2026-08-29
recorded: 2026-08-29
tags: [presenter]
supersedes: []
superseded_by: null
amends: []
amended_by: [169]
sources: []
---

# Program, Preview, Freeze, and Blackout semantics

> **Superseded in part by Decision [169](0169-inspector-navigation-follows-the-selected-mass-item-s-cue.md).** Operator-only triggers use a black
> canonical Preview/Program frame; their private prompt and related response
> remain adjacent operator context rather than entering the Preview frame. The
> body below is reconciled to that distinction; the remaining linked/frozen
> state rules stay adopted.

Ordo uses a linked/frozen model rather than a traditional manual OBS transition model.

#### Live / unfrozen

- Preview and Program are effectively linked.
- Left/Right navigates the timeline.
- The selected projectable output immediately appears in both Preview and Program.
- Entering an operator-only priest cue makes the canonical Preview and Program frames black because the cue has no congregation output.
- The private prompt and related upcoming congregation response remain visible to the operator as adjacent context outside the Preview frame.

#### Frozen

- Program holds the last congregation frame.
- Left/Right continues navigating Preview independently.
- This is the mechanism for retaining previous congregation output while preparing another slide.
- Pressing Space takes the currently selected Preview live and returns to the unfrozen linked state.

#### Blackout

- Pressing `B` immediately blackouts Program.
- Blackout does not prevent Left/Right navigation in Preview.
- Pressing Space takes the current Preview live and returns to the unfrozen linked state.
- Blackout and Freeze are distinct concepts even though both temporarily decouple Program from Preview.

#### Priest-to-congregation example

1. The operator is positioned at `P: Ang Ginoo maanaa kaninyo.`
2. Program is black unless Freeze intentionally retains a previous output.
3. The operator sees the upcoming `T` response.
4. When the priest speaks the trigger, Right advances to `T: MAANAA USAB KANIMO`.
5. In the normal unfrozen state, that response immediately appears on Program and Preview.

Operator-only cues remain clickable/selectable positions in both timeline modes:

- Their canonical congregation-facing output is black.
- Selecting one makes Preview black.
- When Live/unfrozen, Program also becomes black.
- When Frozen, Program retains its held frame while Preview becomes black.
- The private cue text remains visible in the operator-context area, not on the slide frame.
