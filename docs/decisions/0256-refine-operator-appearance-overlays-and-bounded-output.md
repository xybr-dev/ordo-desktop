---
id: 256
title: "Refine operator appearance, overlays, and bounded output motion"
status: accepted
date: 2026-08-29
recorded: 2026-08-29
tags: [presenter, ui]
supersedes: []
superseded_by: null
amends: [44, 59, 217]
amended_by: []
sources: []
---

# Refine operator appearance, overlays, and bounded output motion

- This supersedes Decision [59](0059-context-sensitive-application-appearance.md)'s fixed Preparation polarity and Decision [217](0217-restrained-functional-motion-with-immediate-presentation.md)'s
  overlay timing, and narrowly supersedes Decision [44](0044-mvp-slide-transitions.md) only for transitions at
  Blackout/Hold boundaries; content-to-content cuts remain unchanged.
- Preparation offers System (default), Light, and Dark appearance. System
  follows operating-system changes live. Presenter always uses graphite dark
  and restores the Preparation preference on exit.
- Use the same visible dim scrim for dialogs, ordinary slideovers, and unpinned
  Presenter Ordo. It captures the first outside click and dismisses the
  temporary surface without activating content beneath it. Pinning Ordo removes
  the scrim and makes it a fixed pane.
- Keep `O` as open/focus Ordo without changing pin state. Pin/Unpin is an
  explicit visible action; `Esc` closes an unpinned Ordo or returns focus from a
  pinned Ordo without collapsing it.
- Place operator-only toasts bottom-center. Use transient toasts for
  confirmation/Undo and persistent owned notices for failures or hazards.
- Content-to-content Slide changes remain cuts. Content-to-Blackout/Hold and
  Blackout/Hold-to-content fade for `500ms`, independently in Preview and
  Program. A contrary command reverses from current opacity with proportional
  remaining duration; never queue presentation commands.
- Slideovers open in `400ms` with fast-start/slow-finish motion and close in
  `200ms` with smooth acceleration. Motion is interruptible. Reduced motion
  removes translation and limits clarifying opacity to `100ms`.
- Retain fixture-backed HTML acceptance references under `design-references/`
  as behavior and hierarchy evidence. The first addition is one Canonical Slide
  Layout Gallery covering every adopted template, variants, backgrounds, and
  overflow cases through the same scaled canonical frame. Production remains
  native GPUI and aims for behavioral/design-intent parity, not browser pixels.
