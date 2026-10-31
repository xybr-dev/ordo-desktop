---
id: 217
title: Restrained functional motion with immediate presentation output
status: accepted
date: 2026-08-29
recorded: 2026-08-29
tags: [presenter, ui]
supersedes: []
superseded_by: null
amends: []
amended_by: [256]
sources: []
---

# Restrained functional motion with immediate presentation output

> **Superseded in part by Decision [256](0256-refine-operator-appearance-overlays-and-bounded-output.md).** Overlay timing is now `400ms` in and
> `200ms` out, and Blackout/Hold boundaries use reversible `500ms` fades.
> Content-to-content cuts and the remaining restraint rules below stay in force;
> the body is reconciled to those boundaries.

- Use `120ms` for hover, pressed, focus-adjacent, and color transitions; `160ms` for menus, popovers, disclosures, and visible row reordering; and the adopted `400ms` entry/`200ms` exit timing for dialogs, slide-overs, nested-slide navigation, and parent recession.
- Use one calm ease-out curve, `cubic-bezier(0.2, 0, 0, 1)`. Do not use spring, bounce, overshoot, decorative looping, or attention-seeking animation.
- Keep navigation and interface transitions interruptible; animation may never delay accepting another valid command or changing focus ownership.
- Apply presentation-critical state changes immediately and never wait for operator-chrome animation. Preview/Program content-to-content changes, Freeze, and Live restoration use `0ms` output motion; only content-to/from Blackout or Hold uses the reversible `500ms` opacity fade adopted in Decision [256](0256-refine-operator-appearance-overlays-and-bounded-output.md).
- Under reduced-motion preference, remove translation, scaling, animated reordering, and nested-parent recession. Limit clarifying opacity, including Blackout/Hold boundaries, to at most `100ms` while retaining immediate state changes.
- Preserve identical interaction order, focus movement, state change, and final layout with or without motion. Reduced motion changes only the visual path between states.
