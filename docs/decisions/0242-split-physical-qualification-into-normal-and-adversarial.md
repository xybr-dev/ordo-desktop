---
id: 242
title: Split physical qualification into normal and adversarial passes
status: accepted
date: 2026-08-29
recorded: 2026-08-29
tags: [repo]
supersedes: []
superseded_by: null
amends: []
amended_by: []
sources: []
---

# Split physical qualification into normal and adversarial passes

- Qualify a release candidate through two distinct runs on the actual church system rather than mixing deliberate faults into one undifferentiated rehearsal.
- **Normal session pass:** run a representative Mass plan uninterrupted and at realistic pacing for a full Mass-length duration. Use it to assess congregation readability, ordinary operator flow, projector/output stability, sustained CPU and memory behavior, thermal behavior, cursor hiding, and absence of unintended Program exposure.
- **Adversarial recovery pass:** run a shorter scripted sequence covering rapid navigation, Freeze, Blackout, cursor recovery, emergency insert/edit, congregation-display disconnect/reconnect, and simulated application interruption/recovery. Verify the protected Program result and the operator's recovery path for each event.
- Require both passes. A failure in one does not erase the evidence from the other, but the build is not church-qualified until the failed pass is corrected and rerun as required by the later requalification policy.
