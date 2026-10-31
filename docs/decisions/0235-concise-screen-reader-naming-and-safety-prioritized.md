---
id: 235
title: Concise screen-reader naming and safety-prioritized announcements
status: accepted
date: 2026-08-29
recorded: 2026-08-29
tags: [ui]
supersedes: []
superseded_by: null
amends: []
amended_by: []
sources: []
---

# Concise screen-reader naming and safety-prioritized announcements

- Give every major operator region an explicit stable accessible name, including **Global navigation**, **Workspace actions**, **Mass outline** or the applicable primary list, **Mass editor**, **Inspector**, **Cue timeline**, **Program**, **Preview**, and **Output controls**. Keep names aligned with visible product vocabulary rather than exposing implementation terms.
- Give each interactive Cue one concise accessible name containing its position, meaningful title or first line, Cue type, and whether it projects, waits as an operator-only trigger, holds, or Blackouts. Expose complete slide or private-context text through a separately associated description or structured child content rather than turning the control's name into an entire paragraph.
- Represent focus, selection, current Preview, current Program, expanded/collapsed state, and availability through appropriate semantic state as well as visible styling. Do not force a focus jump merely to announce a state change.
- Let ordinary arrow-key Cue navigation speak through the newly focused Cue. Do not duplicate every movement through a live region. When linked Live causes that focused Cue to become Program, include concise current-state context without rereading the full slide.
- Use one restrained polite status channel for expected operator actions such as **Frozen**, **Blackout active**, **Preview taken live**, **Congregation output enabled**, and successful output restoration. Deduplicate repeated or unchanged messages so rapid operation does not build an announcement queue.
- Reserve urgent interruption for unexpected safety events that require immediate awareness: congregation display disconnection, an unavailable selected Cue, automatic Freeze, or loss of usable Program output. State both the event and the protective result, such as **Cue unavailable; previous Program held and Freeze activated**.
- Keep priest/minister prompts, timing context, diagnostics, and other operator-only information available to assistive technology solely in the operator workspace. The passive congregation renderer exposes and displays only congregation-facing canonical output.
- Centralize semantic labels and operational announcement events through shared workspace/Presentation interfaces so visual state, registered commands, and assistive output cannot drift. Qualify with keyboard-only and Windows screen-reader testing without requiring a screen reader for ordinary operation.
