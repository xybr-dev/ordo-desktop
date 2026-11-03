---
id: 309
title: "Floating surfaces fade and rise instead of zooming"
status: accepted
date: 2026-11-03
recorded: 2026-11-03
tags: [design, motion, gpui]
supersedes: []
superseded_by: null
amends: []
amended_by: []
sources: [DESIGN.md, docs/mockups/ordo-kit.js, crates/ui/src/overlay.rs, crates/ui/src/primitives/selection.rs]
---

# Floating surfaces fade and rise instead of zooming

## Context

`DESIGN.md` had dialogs fade in while zooming from `95%` in place, and menus and popovers zoom from
`95%` while dropping `4px` from their trigger, after the shadcn pattern the mockups adopted. GPUI
cannot draw that. Its element `Style` carries opacity but no transform: only an SVG can be rotated
or scaled. The pinned `gpui-pre` 0.3.8 has none, and neither does the latest release line checked on
3 Nov 2026 (`gpui-kit` 0.9.0, with `gpui-pre` 0.3.11 and `gpui-base` 0.9.0). `gpui-base` names a
`MotionTransform` with a scale, but nothing applies it to an element, and `gpui-component`'s own
dialogs enter by sliding and fading for the same reason.

Two workarounds were rejected. Animating the surface's width and height scales its box but rewraps
its text on every frame. Patching GPUI to transform every scene primitive would be a much larger
vendored patch than the small display-change patch the T0.10 work carries.

## Decision

- **Dialogs** (a confirmation, a name prompt) fade in while rising one spacing step (`8px`) into
  place, over `400ms` with their scrim, and fall back while fading out in `200ms`.
- **Menus and popovers** fade in while dropping `4px` from their trigger, in `160ms`, and reverse in
  `120ms`.
- **Toasts** rise `8px` while fading in, in `160ms`, as before.
- No surface zooms. The mockups draw the same motion, so the reference shows what the product does.
- Reduced motion keeps only the fades, at most `100ms`, as before.

## Alternatives considered

- **Keep the zoom in the specification as an aspiration.** The mockups and the product would then
  disagree on every dialog, with nothing to say which one is right.
- **Size animation.** Rejected above: it rewraps the body as it moves.
- **A vendored GPUI transform patch.** Rejected above: its cost is far larger than the effect.

## Consequences

- Revisit if a GPUI release adds element transforms; the zoom can then return by amending this
  record.
