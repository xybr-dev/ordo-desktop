---
id: 296
title: The walking-skeleton lab stands in for packaged checks; the sign-off is restated to delivered scope
status: accepted
date: 2026-10-20
recorded: 2026-11-01
tags: [qualification, platform]
supersedes: []
superseded_by: null
amends: []
amended_by: []
sources: [docs/plans/T0.10-implementation-plan.md, docs/qualification/milestone-0/WINDOWS-10-SIGNOFF.md, crates/ordo/examples/walking-skeleton-lab.rs]
---

# The walking-skeleton lab stands in for packaged checks; the sign-off is restated to delivered scope

## Context

T0.10's `[MANUAL]` criterion asks for "Windows 10 22H2 packaged checks". There is no installer
before T6, so "native package" has meant the release binaries since the T0.3 and T0.4a gates
(`docs/plans/T0.10-implementation-plan.md`, "Packaged checks"). The USB kit ships those release
binaries.

Attempt 1 of the sign-off (bundle `20261011T105721Z-b03f8fc`) found that checks 2-6 had no program
to run: `ordo.exe` opens the operator shell and the runtime but has no output, navigation, or
Presenter controls before Phase 1, and the development labs each held one piece
(`docs/plans/T0.10-implementation-plan.md`, "Church-PC finding 2").

## Decision

The `walking-skeleton-lab` example composes the T0.2, T0.4, T0.5, T0.6, and T0.7 Module Interfaces,
adds no product behaviour, and is built in release and recorded in the bundle (`index.json`,
SHA-256). Checks 2-6 run on it, restated to what Milestone 0 delivers:

- Display loss is fail-to-black, no retargeting, and black until restored. An operator message in
  the product shell is Phase 1, and the lab shows output state in its own operator-only status line.
- Storage runs on a fresh temporary database owned by the Storage Module worker, because the runtime
  does not open SQLite in Milestone 0.

The T0.10 manual box is therefore signed off against release-profile kit binaries, not an installed
package; the attempt-2 sheet records the lab binary and its SHA-256. The lab is a development
example, not a second production shell, and this decision does not promote it.

## Alternatives considered

- **Declare the manual box "not triggered".** It is the gate's manual half, and there are release
  binaries to sign off.

## Consequences

When T6 packaging lands, the packaged checks must rerun against the installed package. The lab's
checks stay repeatable meanwhile.
