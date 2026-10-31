---
id: 248
title: "Make safety, output, readability, performance, and fallback failures release-blocking"
status: accepted
date: 2026-08-29
recorded: 2026-08-29
tags: [presenter, platform]
supersedes: []
superseded_by: null
amends: []
amended_by: []
sources: []
---

# Make safety, output, readability, performance, and fallback failures release-blocking

- Fail physical qualification for a crash, hang, data corruption, lost operator input, or inability to recover through the documented operator path.
- Fail for any operator-only text, application chrome, pointer, diagnostic, partial frame, unintended application window, or other non-congregation material appearing on Program.
- Fail for incorrect Preview/Program behavior, Freeze or Blackout failure, wrong display targeting, unsafe display-disconnect behavior, or unsafe interruption/restart recovery.
- Fail for congregation text that is missing, clipped, unexpectedly reflowed, unstable, or not comfortably readable from the adopted observation positions, and for canonical scaling, safe-area, bundled-font/fallback-glyph, contrast, or Output Fit failure.
- Fail for exceeding the adopted `1 GiB` absolute memory ceiling, missing the adopted Presenter command-latency limits, losing rapid input, or otherwise violating the physical performance gates.
- Fail when the generated emergency PowerPoint cannot be opened and run correctly using the church PC's installed PowerPoint version.
- A minor operator-chrome polish issue may remain non-blocking only when it has no effect on meaning, accessibility, timing, recovery, operator confidence in a safety state, or congregation output. Record every accepted non-blocking finding with rationale; ambiguity is blocking until resolved.
