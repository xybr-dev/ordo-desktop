---
id: 251
title: Separate strategic planning from checkbox-driven execution milestones
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

# Separate strategic planning from checkbox-driven execution milestones

- Assign authority by subject: `MVP-PLAN.md` owns product outcome, scope, journeys, strategic architecture, contracts, verification strategy, release gates, risks, and deferred work; `CONTEXT.md` owns domain terms and invariants; `DESIGN.md` owns visible behavior and visual tokens; and `MILESTONES.md` owns task order, status, acceptance criteria, and delivery gates. Use this ledger to resolve gaps and reconcile any conflict into the applicable shorter document before implementation.
- Move the implementation sequence into `MILESTONES.md` and make it the sole task-status document so parallel milestone descriptions cannot drift.
- Give every execution task a Goal, intended Changes/locality, dependencies, objectively checkable checkbox acceptance criteria, and at least one E2E scenario.
- Tag verification honestly as automated CI, automated native GPUI UI testing, native packaged automation, scripted manual work, or physical church qualification. Human judgment and actual projector/PowerPoint behavior may never be relabeled as automatic merely to produce a green CI result.
- Order work as executable vertical slices and risk qualification rather than disconnected screens. Test domain behavior through the owning deep Module Interface and retain Phase gates that leave a demonstrable application state.
- Keep post-MVP Song merge, cloud/login, arbitrary PowerPoint import, media playback, remote control, and other deferred features out of the MVP task graph.
