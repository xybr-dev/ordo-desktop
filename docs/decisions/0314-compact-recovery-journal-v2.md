---
id: 314
title: Compact recovery journal v2
status: accepted
date: 2026-11-07
recorded: 2026-11-07
tags: [presenter, storage, contracts]
supersedes: []
superseded_by: null
amends: [14]
amended_by: []
sources: [contracts/recovery-journal/v1.schema.json, docs/decisions/0311-presentation-snapshot-manifest-v2-in-json.md, docs/plans/T1.7-implementation-plan.md, docs/mockups/presenter-faults.html]
---

# Compact recovery journal v2

## Context

Decision [14](0014-interrupted-session-recovery.md) journals the active Presentation continuously so
an unexpectedly terminated session can reopen. Decision
[228](0228-non-modal-fail-closed-interrupted-session-presentation.md) reopens it non-modally with
external output black. T1.7 implements both.

The frozen `recovery-journal` v1 contract is an **append-only action log** (`cue`, `freeze`,
`blackout`, …) keyed by a flat `cue_index`. It cannot hold what recovery needs:
- separate Preview and Program;
- the output's display evidence;
- the snapshot's manifest hash.

Recovering from it would also mean replaying Engine commands, and a long Mass makes it grow without
bound. Decision [311](0311-presentation-snapshot-manifest-v2-in-json.md) already keys recovery on
SnapshotId + CueId and schedules `recovery-journal` v2 for when recovery is implemented.

## Decision

- **Shape.** `recovery-journal` v2 is a **compact state record**, not a log. Each write replaces the
  previous record whole. It holds:
  - `schema_version` 2, `session_id` (UUIDv4), `seq`, `written_at`;
  - the snapshot's `id`, `manifest_hash` and source directory;
  - Preview's `cue_id`;
  - the last **valid** Program `cue_id` (or none);
  - the live state: output disabled, enabled black, live, frozen, blackout, or disconnected;
  - output evidence (enabled, plus the assigned display's name, uuid and bounds);
  - `ended`;
  - `record_sha256` over the rest of the record.
- **Writing.** Writes happen on a worker, never on the GPUI thread. Each one goes to a temporary
  sibling, is flushed, and is renamed over `session-recovery.json` beside the installation
  preferences. `seq` increases strictly within a session, and a record that does not advance it is
  dropped.
- **Rejected records.** A record is rejected, and Ordo never guesses around it, when it is:
  - **torn:** unparseable, or `record_sha256` does not match;
  - **an unknown version:** any `schema_version` other than 2;
  - **stale:** `ended` is true. Age alone never makes a record stale, because an old interrupted
    session still reopens and ends through Exit Presenter (Decision 228);
  - **mismatched:** the snapshot id or manifest hash differs, or a `cue_id` is not in the snapshot.

  A rejected journal is renamed to `session-recovery.rejected.json`, kept once for Diagnostics, and
  never retried. Ordo then opens Preparation normally.
- **A clean end** writes `ended: true` and then deletes the journal. A clean end is End presentation,
  a direct exit, or a confirmed quit.
- **Resuming.** Space on a recovered session resumes the ordinary linked Live state (Decision 14).
  An interrupted Frozen or Blackout shows only as private context ("FROZEN on Gloria · not on screen
  now"). The operator re-applies F or B if wanted.
- `recovery-journal` v1 stays frozen and is not read by the application.

## Alternatives considered

- **Keep the v1 action log and replay it.** It needs Engine replay, keeps flat indexes that break
  when the snapshot changes, and grows for the whole Mass.
- **SQLite.** The database is for Parish data (Decision 30). A single small file with an atomic
  rename is enough for one live session, and it keeps recovery independent of database health.
- **Treat old journals as stale.** That would lose a real recovery whenever the clock is wrong or a
  service resumes the next day. Exit Presenter already ends an obsolete session safely.

## Consequences

- `crates/presentation/src/journal/` owns the record, its encoding and validation, with no file I/O.
  `crates/ordo/src/recovery.rs` owns the writer worker and the startup read.
- Emergency inserts and overrides (Decision 14) do not exist yet. They will need a later v3.
- `docs/contracts/README.md` lists v2 next to the frozen v1.
