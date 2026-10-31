---
id: 227
title: Fail-safe Freeze when a future Cue becomes unavailable
status: accepted
date: 2026-08-29
recorded: 2026-08-29
tags: [presenter]
supersedes: []
superseded_by: null
amends: []
amended_by: [274]
sources: []
---

# Fail-safe Freeze when a future Cue becomes unavailable

> **Amended by Decision [274](0274-program-goes-black-on-an-unavailable-cue-instead-of-freezing.md).** While linked Live, an unavailable Cue makes Program black and linked Live continues; there is no automatic Freeze. The rest of this record stands.

- Never send an error surface, partial frame, renderer placeholder, or accidental black frame to congregation output when the selected future Cue cannot produce its complete canonical frame.
- If Program is Live when the operator navigates to an unavailable Cue, preserve the previous valid Program output and automatically enter Freeze with the explicit state **Frozen · Cue unavailable**.
- Show the same unmistakable operator-only **Output unavailable** placeholder and specific failure reason in the Cue card and Preview viewport. It is not a canonical projectable frame and must not resemble an ordinary black Hold Cue or Blackout.
- Keep unavailable Cues in normal Preview navigation rather than silently skipping liturgical context. Left/Right and Up/Down continue navigating; selecting a later valid Cue does not change Program while Freeze remains active.
- Let Space take a later valid Preview live through the ordinary unfreeze transition. If the currently selected Preview remains unavailable, Space preserves Freeze and reports why it cannot be taken live.
- If Blackout was already active, preserve Blackout rather than replacing it with Freeze. Navigation continues in Preview, and Space cannot restore an unavailable Cue; Blackout remains until a valid Preview is deliberately restored.
- Preflight should identify snapshot/frame failures before Presenter entry, but the same normalized Cue-availability contract must protect against corruption, decode failure, or renderer failure discovered during the active session.
