---
id: 266
title: Qualify the four operator viewports at their production sizes
status: accepted
date: 2026-10-08
recorded: 2026-10-08
tags: []
supersedes: []
superseded_by: null
amends: []
amended_by: []
sources: []
---

# Qualify the four operator viewports at their production sizes

- Preview and Program share the `720px` production width by design, so the
  operator sees the same frame before and after it goes live. The four
  operator viewports therefore span three distinct production sizes
  (`320`/`480`/`720` before Interface scale).
- The T0.4 `[UI-AUTO]` criterion and E2E qualify the four viewports at those
  production sizes across both viewport presets and every Interface scale,
  which already yields many distinct outer sizes. No test-only fourth size is
  manufactured to satisfy the earlier "four different-size" wording.
- The T0.4 `[MANUAL]` items and compositor-capture comparison are unchanged.
