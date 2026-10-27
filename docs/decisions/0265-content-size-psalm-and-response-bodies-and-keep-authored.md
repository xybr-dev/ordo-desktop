---
id: 265
title: Content-size Psalm and Response bodies and keep authored case
status: accepted
date: 2026-10-08
recorded: 2026-10-08
tags: []
supersedes: []
superseded_by: null
amends: [255, 264]
amended_by: []
sources: []
---

# Content-size Psalm and Response bodies and keep authored case

- This supersedes Decision [264](0264-render-response-at-a-response-only-113px-candidate-centered.md)'s fixed `113px`/weight `400` uppercase
  Response body, and Decision [255](0255-use-fixed-semantic-congregation-slide-layouts.md)'s display-size Psalm refrain, uppercase
  Song title, and left-aligned Lyrics body; every other rule stands.
- `Response` bodies render in authored case at weight `800`, matching the
  `projected-response` token, at the largest pending candidate size in
  `160`/`140`/`120`/`100px` whose block fits the body zone; `100px` is the
  floor and carries any overflow warning. Centering is unchanged.
- `Psalm` refrains render at weight `700` from the `140`/`120`/`105`/`90px`
  ladder by the same largest-fit rule, floored at the `90px` display size.
- Song titles render in authored case at `100px`/weight `700`, shaped and
  rasterized at that weight.
- Lyrics lines center horizontally inside the optically centered block.
- All sizes and weights remain pending the T0.4 manual gate.
