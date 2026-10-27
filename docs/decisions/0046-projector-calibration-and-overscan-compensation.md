---
id: 46
title: Projector calibration and overscan compensation
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

# Projector calibration and overscan compensation

> **Superseded in part by Decision [255](0255-use-fixed-semantic-congregation-slide-layouts.md).** The calibration guide below is
> reconciled to the `120x80` minimum safe area. Output Fit and the remaining
> calibration behavior are unchanged.

- Provide a built-in congregation-output calibration pattern showing the complete frame edges, center markers, and the `120x80` minimum safe-area boundary.
- Provide a uniform **Output Fit** adjustment from `0%` through `10%`, defaulting to `0%`, for hardware overscan that remains after the operator checks Windows and projector settings.
- Output Fit scales the entire canonical `16:9` frame inward without cropping, stretching, or changing its proportions; any resulting surrounding area is black.
- Preview shows the digitally fitted frame and black margin exactly as Program renders them.
- Projector or operating-system configuration is the preferred correction. Output Fit is a pragmatic fallback rather than a substitute for correct native resolution and aspect-ratio configuration.
- Asymmetric per-edge correction is deferred from the MVP because it can conceal physical alignment problems and makes congregation-output geometry less predictable.
