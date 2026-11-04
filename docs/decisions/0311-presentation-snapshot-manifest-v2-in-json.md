---
id: 311
title: Presentation snapshot manifest v2 in JSON
status: accepted
date: 2026-11-04
recorded: 2026-11-04
tags: [presentation, contracts, snapshot]
supersedes: []
superseded_by: null
amends: []
amended_by: []
sources: [docs/contracts/README.md, contracts/snapshot-manifest/v1.schema.json]
---

# Presentation snapshot manifest v2 in JSON

## Context

`snapshot-manifest` v1 (T0.8 spike contract, `contracts/snapshot-manifest/v1.schema.json`) captures a Mass plan as flat `items[]` with `asset_ids`. It has no Mass sections, no Cues, no Themes and no frames.

T1.1 needed all four, so the Phase-1 reader (`presentation::snapshot_reader`) used a separate keyed-line manifest, `ordo-presentation-snapshot/1` in `snapshot.txt`, and left reconciliation to the Portability phase. The owner judged that deferral to be tech debt: every later consumer (Preflight, recovery, Celebration Package export) would otherwise be written against a format that has to be replaced.

The contract already has a versioning rule (`docs/contracts/README.md` §5). Every payload carries `schema_version`, and a build refuses a higher version with a compatible-version message. It never guesses.

## Decision

- **Version.** Define `snapshot-manifest` **v2** (`schema_version: 2`), a JSON payload that is the single on-disk manifest for a Presentation snapshot. v1 stays frozen as recorded. Nothing has shipped in the v1 shape, so no migration path is needed. A v1 snapshot is refused with the standard unsupported-version message.
- **Top-level fields:**
  - `schema_version`, `id` (SnapshotId), `mass_plan_id` and optional `celebration_id`/`captured_at`, as in v1;
  - `title`;
  - `themes[]` (`id`, `name`, `kind`: `custom` or `fallback`) and `mass_theme_id`;
  - `sections[]`, each with `id`, `title` and `items[]`;
  - `frames[]` (`id`, `path` under `frames/`, `sha256`);
  - `assets[]` (`id`, `path` under `assets/`, `size`, `sha256`), as in v1;
  - `dependencies_fingerprint`.
- **Items.** Each item has `id`, `title`, `kind` (`song`, `reading`, `psalm`, `response_sequence`, `general_presentation` or `rite`), optional `theme_id`, optional `auto_play_seconds`, and `cues[]`.
- **Cues.** Each Cue has `id`, `label`, optional `private_context`, and `behavior`, an internally tagged object:
  - `{ "type": "project", "frame_id", "duration_seconds"? }`
  - `{ "type": "operator_trigger" }`
  - `{ "type": "hold" }`
  - `{ "type": "wait" }`
  - `{ "type": "unavailable", "reason" }`
- **Identifiers** are lowercase UUIDv4, per the contract rule.
- **`dependencies_fingerprint`** applies the contract's fingerprint rule to every `frames[].sha256` and `assets[].sha256`. It covers dependency bytes only. The structure is pinned separately by the SHA-256 of the exact manifest bytes (`manifest_hash`).
- **Frame files stay** canonical `ordo-fixture/1` LF-only text under `frames/`. Only the manifest changes format.
- **Validation is unchanged:**
  - unknown fields, duplicate keys and duplicate ids are rejected;
  - every Project frame is declared and every declared frame is reached, and the same holds for assets;
  - paths are portable and confined to the snapshot root;
  - nothing is allocated from an untrusted count;
  - a whole snapshot whose files do not match the manifest is rejected (Decision 51; owner confirmation 2026-11-04).

## Alternatives considered

- **Keep the line format through Phase 1.** Rejected as tech debt. Preflight, recovery and package export would all bind to a format due for replacement.
- **Change v1 in place.** Rejected. It breaks the contract's own versioning rule and the frozen-schema promise, even though nothing has shipped.

## Consequences

- `presentation::snapshot_reader` reads `snapshot.json` (v2) instead of `snapshot.txt`, and gains exact-pinned `serde`/`serde_json`, the versions the contract spike already uses. `LoadedSnapshot` and its API are unchanged, so the Engine, frame store and Presenter are unaffected.
- `contracts/snapshot-manifest/v2.schema.json` and `docs/contracts/README.md` describe v2. A Celebration Package embedding v2 snapshots is a later contract bump of `celebration-package`.
- **Interrupted-session recovery keys on SnapshotId + CueId** (owner confirmation 2026-11-04). `recovery-journal` v1 records a flat `cue_index`. Moving it to `cue_id` is a later `recovery-journal` v2 change, made when recovery is implemented.
