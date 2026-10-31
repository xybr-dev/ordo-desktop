---
id: 295
title: "Close T0.6's hot-path AC on what exists; later work proves its own"
status: accepted
date: 2026-10-11
recorded: 2026-11-01
tags: [performance, presenter, qualification]
supersedes: []
superseded_by: null
amends: []
amended_by: []
sources: [crates/ordo/tests/hot_path_blocking.rs, docs/plans/T0.6-implementation-plan.md]
---

# Close T0.6's hot-path AC on what exists; later work proves its own

## Context

T0.6 asks that the measured hot path contain no SQLite or filesystem access, autosave, pagination,
image decode, blocking worker wait, or full-timeline rerender. At T0.6 there is no Presenter-path
SQLite use, autosave, pagination, or Cue timeline to rerender.

## Decision

Close the criterion on what exists, and require later work to prove its own part.

- What exists is proven. The live probe counts file access, preference saves, worker sends,
  canonical recomposition, and image encode and decode between keydown and paint, and reports zero
  on the church PC run (2,000 commands). `crates/ordo/tests/hot_path_blocking.rs` lists every
  blocking receive, blocking send, `block_on`, and join in production `ordo` source; each runs on a
  worker thread or at shutdown, and a new one fails the test until reviewed.
- What does not exist yet is proven where it lands. `HotPathKind::Pagination` and
  `HotPathKind::Autosave` are reserved, so their first production sites must call `note`. The
  full-timeline rerender check moves to the Presenter timeline task that first renders a timeline
  (T1.2's Live Control and the T1 cache and layout tasks), which already gate on not disturbing
  Program.

## Alternatives considered

- **Leave the criterion open until Phase 1 code exists.** That would make a Phase 0 gate depend on
  Phase 1 work it is meant to precede.

## Consequences

T0.6 closes on the church-PC run. The first production pagination, autosave and timeline code each
carry their own hot-path proof.
