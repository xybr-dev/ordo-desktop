---
id: 100
title: Theme Appearance and MVP background model
status: accepted
date: 2026-08-29
recorded: 2026-08-29
tags: [themes]
supersedes: []
superseded_by: null
amends: []
amended_by: [255]
sources: []
---

# Theme Appearance and MVP background model

> **Superseded in part by Decision [255](0255-use-fixed-semantic-congregation-slide-layouts.md).** Themes still own ordinary backgrounds,
> but the generated Celebration-title Cue gains the narrow Mass-local custom
> image override defined there. Other per-slide backgrounds remain deferred.

- Rename the Theme Editor's **Colors** property group to **Appearance** so its label matches its responsibility for both foreground colors and the rendered background.
- An MVP Theme background is either a **Solid color** or one **Image**. It is a property of the reusable Theme, not of an individual specimen or canonical slide.
- Image backgrounds use **Cover** fitting and expose focal-position control so an operator can choose which region survives cropping within the canonical `16:9` frame.
- An image background may be combined with one color overlay and adjustable opacity. This is the Theme's built-in legibility treatment; the canonical preview and computed contrast feedback update from the resulting composition.
- Every image background stores a required fallback color. If the image is missing or corrupt, rendering remains coherent and reports the degraded asset rather than emitting a partial frame.
- Theme background assets are portable dependencies and must be included in Celebration Packages and Parish Backups when referenced by captured content.
- Gradients, multiple background or decorative layers, textures, and per-specimen backgrounds are deferred from the MVP.
