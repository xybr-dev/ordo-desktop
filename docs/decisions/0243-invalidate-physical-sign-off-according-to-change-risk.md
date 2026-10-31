---
id: 243
title: Invalidate physical sign-off according to change risk
status: accepted
date: 2026-08-29
recorded: 2026-08-29
tags: []
supersedes: []
superseded_by: null
amends: []
amended_by: []
sources: []
---

# Invalidate physical sign-off according to change risk

- Tie physical qualification to the exact release candidate rather than treating one successful church test as permanent approval for all later builds.
- Require both the normal and adversarial physical passes again after changes to Presenter state, canonical Slide rendering, bundled fonts, congregation-output windows, display handling, GPUI/platform or other rendering dependencies, packaging, interruption/recovery, or performance-sensitive code.
- For changes confined to Preparation behavior with no path into snapshot capture, Presenter, rendering, output, recovery, or packaging, require the normal automated/native gates plus a short church-PC sanity check rather than another full Mass-length run.
- When a physical check fails or its result is ambiguous, correct the candidate and rerun at least the affected pass. Rerun both passes whenever the correction or root cause crosses both stability and recovery concerns, or when impact cannot be bounded confidently.
- Record which code/build change triggered or waived full requalification in the external release evidence; never infer validity merely from a version-number category such as patch, minor, or major.
