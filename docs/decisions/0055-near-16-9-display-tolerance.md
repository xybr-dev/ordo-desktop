---
id: 55
title: "Near-16:9 display tolerance"
status: accepted
date: 2026-08-29
recorded: 2026-08-29
tags: [presenter]
supersedes: []
superseded_by: null
amends: []
amended_by: []
sources: []
---

# Near-16:9 display tolerance

- Treat physical display aspect ratios within `0.1%` of canonical `16:9` as exact `16:9` outputs.
- Standard modes such as `1366x768` therefore fill the congregation display without a misleading aspect-ratio warning or a one-pixel black seam.
- The already composed canonical frame is scaled as one unit; output scaling never causes text reflow or layout recomposition.
- Ratios outside the tolerance retain the established proportional fitting, black letterboxing or pillarboxing, and operator warning.
