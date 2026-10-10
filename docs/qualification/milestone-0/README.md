# Milestone 0 qualification bundle

This folder is the record for the T0.10 gate in `MILESTONES.md`. It explains
how the Phase 0 qualification bundle is produced, which proof covers which
task, which workarounds were adopted, and where the manual Windows sign-off is
recorded.

Status of the gate is owned by `MILESTONES.md`, not by this file. Nothing here
closes a task. Where a manual box is unticked, this file says so.

## Files

- `README.md` (this file): how to run and read the bundle, and the proof map.
- `EXCEPTIONS.md`: ledger of every workaround and deviation Phase 0 adopted.
- `WINDOWS-10-SIGNOFF.md`: blank manual sheet for the Windows 10 22H2 packaged
  checks. Completed sheets are not committed.

## Producing the bundle

Run from a clean checkout of the commit to qualify:

```
cargo xtask qualify
```

The command lives in `xtask/src/qualify.rs`.

`qualify` runs these steps in order, each with its own log and a status of
`pass`, `fail`, `missing`, or `skipped`:

| Step | Command or check |
|---|---|
| `verify` | `cargo xtask verify` |
| `build-release` | `cargo build -p ordo --release --locked` |
| `canonical-invariance` | `cargo test -p native-runtime-tests --locked --test canonical_invariance` |
| `native-display` | `cargo test -p display-tests --locked --test native_display` |
| `shaping-probe` | `cargo test -p rendering --locked --test shaping_probe` |
| `measure-presenter` | `cargo xtask measure-presenter`, report copied into the bundle |
| `powerpoint-fixture` | `cargo run -p powerpoint-spike -- fixture`, then `cargo xtask inspect-pptx` on the output |

A failed step does not stop later steps, so every result is recorded.

Output goes to `target/qualification/milestone-0/`:

- `runs/<UTC timestamp>-<short commit>/` is one run. It holds `index.json`,
  `index.md`, `logs/<step>.log`, and `artifacts/...`.
- `latest.txt` names the most recent run. Reruns create a new directory, so
  failures and reruns are both kept.
- `index.json` holds the provenance block (commit, dirty flag, rustc, cargo,
  toolchain channel, GPUI supply, `Cargo.lock` SHA-256, OS, GPU, and fixture
  SHA-256 per tree plus one aggregate), the step results with durations and exit
  codes, and the SHA-256 of each artifact.
- `index.md` is the same content as a readable table. Skipped steps are listed
  first.

Each artifact copied into `artifacts/` gets a sidecar
`artifacts/<name>.provenance.json` with `artifact`, `sha256`, `step`, `run`, and
`provenance`. Every `logs/<step>.log` starts with a header that records the
commit, the dirty flag, the run id, and the step. Steps invoke xtask subcommands
through the running xtask executable (`current_exe`), not `cargo xtask`. Each
step has a timeout of 120 minutes, and a timeout is recorded as `fail`.

A provenance problem is any provenance value beginning `unknown:`, a dirty
working tree, or an expected fixture tree that is absent. Provenance problems
are recorded in the bundle, and they fail the run. A dirty working tree fails
the run unless `--allow-dirty` is passed, which records a warning instead. The
bundle is marked `complete: true` only when no step is skipped, failed, or
missing and there is no provenance problem. With `--allow-dirty` the bundle is
still `complete: false`, because qualification needs one clean commit.

`qualify` exits non-zero when any step is `fail` or `missing`, when a step
times out, or when there is a provenance problem.

A report counts as qualification evidence only inside a bundle, meaning the
`index.json` plus the sidecar for each artifact. Standalone reports, such as the
CI-uploaded `presenter-performance-report`, are diagnostic only.

Dependency revisions are identified by the SHA-256 of `Cargo.lock` plus the
explicit GPUI and platform versions (`gpui-pre`, `gpui-pre-platform`, and
`gpui-base` in `Cargo.lock`, and the `[workspace.metadata.ordo.gpui-supply]`
pin). A crate version alone does not identify the GPUI revision.

Manual review: open `target/qualification/milestone-0/latest.txt`, then the
`index.md` it names. `cargo run -p ordo` still launches the walking skeleton.

### CI

The `verify` workflow (`.github/workflows/verify.yml`) runs the `verify`,
`windows-shaping-probe`, `windows-canonical-invariance`,
`windows-display-control`, and `windows-presenter-performance` jobs.

The `windows-qualification` job runs `cargo xtask qualify` with
two steps skipped as covered elsewhere: `verify` (the `verify` job) and
`measure-presenter` (the `windows-presenter-performance` job). Its exit code
fails on any failed step or provenance problem. The CI checkout is clean, so a
dirty tree is not expected there. The job uploads `target/qualification/` as an
artifact. A CI run therefore never produces `complete: true`, because two steps
are skipped. Only a local full run can produce a complete bundle.

## Phase 0 proof map

"Automated" lists the tests, jobs, and xtask commands that carry the proof.
"Bundle step" is the `qualify` step that runs it, or `verify` when the proof
sits inside the workspace test set. Gate status is taken from the ticked
boxes in `MILESTONES.md` at the time of writing.

| Task | What it proves | Automated evidence | Bundle step | Manual gate | Status |
|---|---|---|---|---|---|
| T0.1 Repository, toolchain, harness | Pinned toolchain and lockfile, seven internal crates, verification harness, temporary-location tests | `cargo xtask verify`, `architecture`, `context`; job `verify`; UI harness tests in `crates/ui` and `tools/ui-lab` | verify | None | Closed (all boxes ticked) |
| T0.2 Native runtime, window ownership, supervision | GPUI thread owns lifecycle; typed task ownership and cancellation; congregation window closes with operator | `tests/native-runtime/tests/runtime.rs` | verify | None | Closed (all boxes ticked) |
| T0.3 Operator design system and shell | Appearance, navigation rail, GPUI behavior adoption, overlays, focus, AccessKit | `crates/ui` and `tools/ui-lab` UI suites (see `GPUI-BASE-T0.3.md`); `cargo xtask tokens`, `cargo xtask offline` | verify | `T0.3-MANUAL-GATE.md` (sections A to C recorded complete) | Closed (manual box ticked) |
| T0.4 Canonical rendering and bundled fonts | Deterministic 1920x1080 composition, bundled font hashes, golden frames, packaged invariance | `crates/rendering/tests/` (`golden`, `canonical_shaping`, `fonts`, `raster`, `templates`, `interface`); `tests/native-runtime/tests/canonical_invariance.rs`; job `windows-canonical-invariance` | canonical-invariance | `T0.4-MANUAL-GATE.md` (script A and gallery C; script B retired by T0.4b) | Closed (manual boxes ticked) |
| T0.4a GPUI migration and `gpui-base` re-qualification | One GPUI revision, pin enforcement, behavior re-qualified, pin gaps re-checked | `cargo xtask offline` (GPUI supply check); `crates/rendering/tests/shaping_probe.rs`; job `windows-shaping-probe`; UI suites in `GPUI-BASE-T0.4a.md` | shaping-probe, verify | Recorded as passed in `GPUI-BASE-T0.4a.md` ("Manual gate (passed)") | Closed (manual box ticked) |
| T0.5 Display Control and passive congregation window | Display discovery, one passive output window, disconnect handling, topology, native enumeration | `tests/display/tests/display_control.rs`, `tests/display/tests/native_display.rs`; job `windows-display-control` | native-display | No separate gate file in this folder. Pointer and locator boxes are ticked in `MILESTONES.md` | Closed by checkboxes; no gate record found (see below) |
| T0.6 Presentation Engine and performance | Pure engine, ordered and coalesced commands, caches bounded, measured latency and memory | `tests/presentation/tests/presenter_sequences.rs`, `tests/presentation/tests/caches.rs`, `tests/performance/tests/presenter_bench.rs`; `cargo xtask measure-presenter`; job `windows-presenter-performance` | measure-presenter | None listed. Hardware acceleration box is ticked | Open. Three boxes unticked: hot-path no-I/O check `[AUTO]`; p95 under 50 ms and the 1 GiB ceiling on minimum hardware `[NATIVE-AUTO]`; 500-Cue memory baselines over three runs `[NATIVE-AUTO]` |
| T0.7 SQLite owner worker and migrations | Single worker owns the connection; ordered, transactional migrations; refusal of newer schema; panic and shutdown handling | `tests/storage/tests/acceptance.rs`; `crates/ordo/tests/storage_supervision.rs` | verify | None | Closed (all boxes ticked) |
| T0.8 Versioned contracts and `.ordo` container | JSON Schemas for each contract kind; bounded inspection; atomic create and reopen | `tests/artifact-container/tests/e2e.rs`; `contracts/` (eight kinds); `tools/artifact-container-spike`; runs inside the workspace test set | verify | None | Closed (all boxes ticked) |
| T0.9 Emergency PowerPoint | Image-only 16:9 slides, sections and notes, reopen verification, atomic publication | `tests/powerpoint/tests/e2e.rs`; `tools/powerpoint-spike`; `xtask/src/inspect_pptx.rs` (`cargo xtask inspect-pptx`) | powerpoint-fixture, verify | `PHYSICAL-QUALIFICATION-TEMPLATE.md` (Emergency PowerPoint fallback) and `WINDOWS-10-SIGNOFF.md` | Open. The `[MANUAL]` box for the exact installed PowerPoint version is unticked |

Notes on the table:

- T0.4b is a rescope, not a separate proof. It is recorded in `MILESTONES.md`
  and reflected in the T0.4 row.
- T0.5 has no gate file in this folder even though its manual boxes are ticked.
  Its manual evidence should be found and linked before this row is treated as
  fully recorded.
- T0.6 and T0.9 are open. T0.10 cannot close while their boxes are unticked,
  because the T0.10 manual sign-off depends on them.

## Reuse rule

Phase 0 fixtures, contracts, inspectors, and instrumentation must stay reusable
after Phase 0 (`MILESTONES.md`, T0.10 AC5). The rule is:

- No package outside `tests/*` may depend on any spike package, through any
  dependency kind. `tools/ui-lab` and `xtask` are included. A spike is a package
  whose name ends in `-spike`, or whose manifest lies under `tools/` in a
  directory ending in `-spike`, so a renamed spike is still caught.
- Test packages may reach spikes only through `[dev-dependencies]`. Today the
  only such edges are `tests/artifact-container` and `tests/powerpoint`.
- Reusable assets live outside `tools/*-spike`:
  - contracts in `contracts/`;
  - fixtures in `tests/fixtures/`;
  - the PowerPoint inspector in `xtask/src/inspect_pptx.rs`;
  - instrumentation in `xtask/src/measure_presenter.rs` and `tests/performance`.

`cargo xtask architecture` (and therefore `cargo xtask verify`) enforces the
rule through `validate_spike_reuse_policy` in `xtask/src/lib.rs`.

### Known gap

The `.ordo` container inspector is still spike code in
`tools/artifact-container-spike`. It is promoted into a `portability` crate at
its owning milestone, and until then only tests reach it. See `EXCEPTIONS.md`.
