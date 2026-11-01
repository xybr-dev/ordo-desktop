---
id: 298
title: Watch the display topology while output is enabled
status: superseded
date: 2026-11-01
recorded: 2026-11-01
tags: [display, qualification]
supersedes: []
superseded_by: 299
amends: []
amended_by: []
sources: [crates/ordo/src/output.rs, tests/display/tests/display_control.rs, docs/qualification/milestone-0/USB-KIT-README.md]
---

# Watch the display topology while output is enabled

## Context

The pinned GPUI snapshot has no display-change event, so `crates/ordo/src/output.rs` noticed a lost
congregation display only when Windows moved the window off it, and the module's rule was "never
polled". Sign-off attempt 2 on the church PC (bundle `20261031T213536Z-4e8ea3d`) disproved that
assumption: after the projector was unplugged, Program stayed on until `F8` forced a refresh.
T0.5 promises fail-to-black, and an operator keypress cannot be part of that promise.

## Decision

While output is enabled, `CongregationOutput` compares a fresh topology snapshot with the last one
every `DISPLAY_WATCH_INTERVAL` (500 ms). A difference goes through the existing refresh path, so
loss still means Program black, no retargeting, and black until restored. An unchanged topology
wakes no observer, so a static Program is not redrawn. The task is owned in `display_watch` and is
dropped with the window. No watch runs while output is disabled. The bounds observer stays,
because it can notice a loss sooner.

## Alternatives considered

- **Hook `WM_DISPLAYCHANGE` directly.** It would need its own Windows message handling around GPUI's
  window, and `unsafe` is denied outside the one module Decision 269 audits.
- **Keep `F8` and document it.** That makes fail-to-black depend on the operator noticing.

## Consequences

A lost display blacks out within about half a second of Windows reporting it. The kit's display-loss
check now fails if `F8` is needed. A later GPUI with a display-change event can replace the watch.
