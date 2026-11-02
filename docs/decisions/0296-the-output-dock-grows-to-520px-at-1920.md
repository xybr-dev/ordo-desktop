---
id: 296
title: The Output Dock grows to 520px at 1920
status: accepted
date: 2026-11-02
recorded: 2026-11-02
tags: [design, presenter]
supersedes: []
superseded_by: null
amends: []
amended_by: []
sources: []
---

# The Output Dock grows to 520px at 1920

## Context

At `1920x1080` the Output Dock was `400px` wide, so Program was barely larger than a `300px` Cue frame,
while the dock had unused height below its controls. `MVP-PLAN.md` asks for more output-monitor space and
up to four thumbnails per row at that size.

## Decision

- At `1920x1080` the Output Dock is `520px` wide.
- The Cue frame floor drops from `300px` to `270px`, which keeps four frames per row.
- `1366x768` keeps its `360px` dock and three frames.

## Alternatives considered

- **Leave the dock at `400px`.** It keeps larger thumbnails and smaller monitors, against the plan.
- **A wider dock.** `556px` is the limit at which four `270px` frames still fit.

## Consequences

Program and Preview are about 1.75 times a Cue frame's width at `1920x1080`. Cue frames are smaller, so
their captions need checking on real parish content.
