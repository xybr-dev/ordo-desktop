---
id: 297
title: Defer the .ordo inspector promotion; keep T0.10 AC5 on the enforceable reuse rule
status: accepted
date: 2026-10-20
recorded: 2026-11-01
tags: [qualification, portability]
supersedes: []
superseded_by: null
amends: []
amended_by: []
sources: [docs/qualification/milestone-0/EXCEPTIONS.md, docs/qualification/milestone-0/README.md, xtask/src/lib.rs]
---

# Defer the .ordo inspector promotion; keep T0.10 AC5 on the enforceable reuse rule

## Context

T0.10 AC5 asks Phase 0 fixture and instrumentation code to stay reusable in later regression tests
instead of becoming an abandoned spike directory: fixtures, contract cases, inspectors, and
instrumentation outside `tools/*-spike`, and never imported by production crates.

The enforceable half is in place. `validate_spike_reuse_policy` in `xtask/src/lib.rs` fails any
package outside `tests/*` that depends on a spike, by name or manifest path; a unit test proves the
current tree passes and a synthetic violating edge fails. Reusable assets already live outside
spikes: contracts in `contracts/`, fixtures in `tests/fixtures/`, the PowerPoint inspector in
`xtask/src/inspect_pptx.rs`, and instrumentation in `xtask/src/measure_presenter.rs` and
`tests/performance`.

One item remains inside a spike: `.ordo` container inspection in `tools/artifact-container-spike`
(`EXCEPTIONS.md` row 12 owns its NFC gap). Only `tests/artifact-container` reaches it, through
`[dev-dependencies]`, which the reuse rule allows.

## Decision

The tick closes on the rule the criterion exists to enforce. The inspector's promotion into the
`portability` crate belongs to that milestone (`MVP-PLAN.md` section 6.4). The deferral is named
where the gap shows: the tick note in `MILESTONES.md`, the Known gap in
`docs/qualification/milestone-0/README.md`, and the open items in `EXCEPTIONS.md`. This has the
same shape as Decision [295](0295-close-t0-6-s-hot-path-ac-on-what-exists-later-work-proves.md):
close on what exists, name what is deferred and where it lands.

## Alternatives considered

- **Untick AC5 until the promotion.** That would hang a Phase 0 gate on a later milestone's crate
  while the reuse rule it asks for already passes and is tested.

## Consequences

Until the promotion, only `tests/artifact-container` may reach the spike's inspector.
