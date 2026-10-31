---
id: 19
title: Projector-safe area
status: accepted
date: 2026-08-29
recorded: 2026-08-29
tags: [presenter]
supersedes: []
superseded_by: null
amends: []
amended_by: [255]
sources: []
---

# Projector-safe area

> **Superseded in part by Decision [255](0255-use-fixed-semantic-congregation-slide-layouts.md).** The body below is reconciled to the
> `120x80` minimum safe area; full-bleed background and validation rules remain
> in force.

- The minimum congregation-output safe area is `120` horizontal and `80` vertical units on the `1920x1080` canonical canvas.
- Backgrounds and other intentionally full-bleed decorative elements may extend to the canvas edges.
- Lyrics, responses, labels, and important image content remain inside the safe area by default.
- The slide editor visibly indicates the safe-area boundary.
- Any important-content geometry outside the safe area produces a validation warning and cannot be approved by reducing the minimum.
- A Theme may define a larger safe area but never a smaller one.
