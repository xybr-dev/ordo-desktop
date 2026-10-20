# Milestone 0 exceptions ledger

This ledger lists every workaround, deviation, and recorded gap that Phase 0
adopted. It is taken from the qualification documents in
`docs/qualification/milestone-0/`, the Deviations sections of
`docs/plans/T0.8-implementation-plan.md` and
`docs/plans/T0.9-implementation-plan.md`, the recorded results and caveats in
`docs/plans/T0.6-implementation-plan.md`, and the sign-off findings and owner
review in `docs/plans/T0.10-implementation-plan.md`. T0.3 to T0.5 and T0.7
plans have no Deviations section.

Each row names the owner, the proof that requalifies it, and the domain
statement. "Requalifying proof" is the test or record that must pass again
when the item changes or closes.

## Domain independence

`crates/domain/Cargo.toml` has no dependencies. `cargo xtask architecture`
checks the dependencies of `domain` and `presentation` and fails if either
gains a GPUI, SQLite, filesystem, network, or OS-window dependency. Every row
below is a platform, adapter, or qualification-scope exception, so `domain`
stays independent of all of them.

## Ledger

### GPUI and `ui` workarounds

| # | Exception | Owner | Requalifying proof | Domain independence | Source |
|---|---|---|---|---|---|
| 1 | Private test window: `TestAppContext::test_window` is `pub(crate)`, so OS appearance changes cannot be driven in tests. | `ui` (test harness) | Manual appearance check in `T0.3-MANUAL-GATE.md` section A; re-check when `gpui-pre` changes | Domain has no GPUI dependency | `docs/qualification/GPUI-BASE-T0.3.md` "Test-harness gaps" and "Pinned platform limitations"; `docs/qualification/GPUI-BASE-T0.4a.md` "Pin gaps re-checked" |
| 2 | Accessibility tree is empty under the test platform (`debug_a11y_tree_json` is `None`). | `ui` (test harness) | UI suites in `crates/ui` and `tools/ui-lab` that read Ordo's own accessibility elements | Domain has no GPUI dependency | `docs/qualification/GPUI-BASE-T0.3.md` "Pinned platform limitations"; `docs/qualification/GPUI-BASE-T0.4a.md` "Pin gaps re-checked" |
| 3 | Tooltips do not open on keyboard focus. Full text reaches keyboard users through the accessible name instead. | `ui` (`ui::primitives`) | T0.3 keyboard-only visual pass, `T0.3-MANUAL-GATE.md` section B | Domain has no GPUI dependency | `docs/qualification/GPUI-BASE-T0.4a.md` "Pin gaps re-checked" |
| 4 | No high-contrast or forced-colors signal from GPUI. Ordo paints its own focus treatment in every mode. | `ui` | T0.3 Windows High Contrast script, `T0.3-MANUAL-GATE.md` section C | Domain has no GPUI dependency | `docs/qualification/GPUI-BASE-T0.4a.md` "Pin gaps re-checked" |
| 5 | No AccessKit live-region setter. Ordo uses its own `LiveElement`. | `ui` (`ui::accessibility::LiveElement`) | UI suites that assert live announcements | Domain has no GPUI dependency | `docs/qualification/GPUI-BASE-T0.4a.md` "Pin gaps re-checked" |
| 6 | No disabled or read-only AccessKit setter. Read-only state is announced through the description. | `ui` | UI accessibility suites in `crates/ui` | Domain has no GPUI dependency | `docs/qualification/GPUI-BASE-T0.4a.md` "Behavior still recorded as a gap" |
| 7 | GPUI ships no scrollbar element. Ordo owns the scroll indicator. | `ui` (`ui::scroll`) | T0.3 scroll-indicator evidence in `GPUI-BASE-T0.3.md` section T0.3.8; `gpui-base` scrollbar adoption is a new qualification | Domain has no GPUI dependency | `docs/qualification/GPUI-BASE-T0.4a.md` "Pin gaps re-checked" |
| 8 | Image-asset completion test asserts behavior, not the old `get`/`use` mechanism. The retire path is no longer tested for `use` versus `get`. | `ordo` (`crates/ordo/examples/canonical-slide-lab.rs`) | `lab_keys_cycle_fixture_viewport_scale_and_density` and the retire-path assertion that the incoming slide takes the screen | Domain has no GPUI dependency | `docs/qualification/GPUI-BASE-T0.4a.md` "Recorded difference: image-asset completion" |
| 9 | `InputState::set_value` needs `InputContent`. `ui::primitives::Field::set_value` converts from `SharedString` first. | `ui` (`ui::primitives::Field`) | Existing field and input tests in `crates/ui` | Domain has no GPUI dependency | `docs/qualification/GPUI-BASE-T0.4a.md` "Source changes required by the new revision" |

### Native adapter workaround

| # | Exception | Owner | Requalifying proof | Domain independence | Source |
|---|---|---|---|---|---|
| 10 | wgpu atlas uploads are unpadded. Ordo pads each row to 64 px. | `ordo` native adapter (`crates/ordo/src/canonical_views.rs`) | Canonical-invariance test (`tests/native-runtime/tests/canonical_invariance.rs`) and job `windows-canonical-invariance` | Domain has no GPUI or wgpu dependency | `docs/qualification/GPUI-BASE-T0.4a.md` "Pin gaps re-checked" |

### Rendering and qualification scope

| # | Exception | Owner | Requalifying proof | Domain independence | Source |
|---|---|---|---|---|---|
| 11 | Linux packaged and rendering evidence is retired. Windows is the only MVP platform for development and native evidence. | Qualification process (`MILESTONES.md`, T0.4b and the Phase 0 gate) | Windows native jobs and the Windows 10 22H2 sheet in `WINDOWS-10-SIGNOFF.md`. Linux packaged sign-off moves to T6.2 | Not applicable to domain | `MILESTONES.md` T0.4b; `MVP-PLAN.md` section 1 and section 3 notes (Decision 268); `docs/qualification/T0.4-MANUAL-GATE.md` script B (retired) |

### `.ordo` container and PowerPoint spikes (T0.8, T0.9)

All rows below live in `tools/artifact-container-spike` or `tools/powerpoint-spike`,
which are development-only spikes. Until each one is promoted, production
crates do not depend on them (see the reuse rule in `README.md`).

| # | Exception | Owner | Requalifying proof | Domain independence | Source |
|---|---|---|---|---|---|
| 12 | Unicode NFC normalisation is not applied to duplicate names. The check folds case only, because the workspace has no normalisation crate. | `.ordo` inspector (`tools/artifact-container-spike`); promoted to `portability` at its owning milestone | Container inspection tests in `tests/artifact-container/tests/e2e.rs`; requalify when a normalisation crate is adopted | Domain has no filesystem or archive dependency | `docs/plans/T0.8-implementation-plan.md` "Deviations" (NFC normalisation gap) |
| 13 | `uuid` is a normal dependency, not fixtures-only. `create` assigns `artifact_id` with UUID v4. | `.ordo` container spike | `tests/artifact-container/tests/e2e.rs` | Domain has no `uuid` dependency | `docs/plans/T0.8-implementation-plan.md` "Deviations" (`uuid` is a normal dependency) |
| 14 | Every archive kind, Parish Backups included, is capped at 64 MiB. Owner decision, 2026-10-10. | `.ordo` container spike | Limit tests in `tests/artifact-container/tests/e2e.rs`; revisit when needed | Domain has no archive dependency | `docs/plans/T0.8-implementation-plan.md` "Deviations" (Parish Backups capped at 64 MiB) |
| 15 | Duplicate detection reads the raw End Of Central Directory count and refuses mismatches with `Failure::EntryCountMismatch`, because the zip crate collapses exact-duplicate names. | `.ordo` container spike | `tests/artifact-container/tests/e2e.rs` (EntryCountMismatch and DuplicatePath cases) | Domain has no archive dependency | `docs/plans/T0.8-implementation-plan.md` "Deviations" (Duplicate detection) |
| 16 | Interface additions beyond the plan: `InspectLimits::max_payload`, `Failure::EntryCountMismatch`, refusal of Windows device names and trailing dot or space segments, refusal of non-empty staging directories. | `.ordo` container spike | `tests/artifact-container/tests/e2e.rs` | Domain has no archive dependency | `docs/plans/T0.8-implementation-plan.md` "Deviations" (Interface additions) |
| 17 | A `format_version` above 1 is reported as `Unsupported`, not as a bad manifest. | `.ordo` container spike | Version tests in `tests/artifact-container/tests/e2e.rs`; contract text in `docs/contracts/README.md` section 5 | Domain has no archive dependency | `docs/plans/T0.8-implementation-plan.md` "Deviations" (Format version) |
| 18 | The `powerpoint-tests` package was registered in WP3, not WP1, because xtask compares members as an exact set. | `tests/powerpoint` (test package) | xtask workspace member check (`cargo xtask verify`) | Not applicable to domain | `docs/plans/T0.9-implementation-plan.md` "Deviations" (No `powerpoint-tests` placeholder) |
| 19 | `export_with` is `pub(crate)` so unit tests can force a post-write validation failure. Public `export` delegates to it. | `tools/powerpoint-spike` | Unit tests in `tools/powerpoint-spike/src/tests.rs` that force validation failure and check that the destination is untouched; `tests/powerpoint/tests/e2e.rs` | Domain has no PowerPoint dependency | `docs/plans/T0.9-implementation-plan.md` "Deviations" (Test seam) |
| 20 | PowerPoint structure is checked by the inspector, not by PowerPoint. Opening without a repair prompt is not proved by automation. | `tools/powerpoint-spike`; `xtask/src/inspect_pptx.rs` | Manual: "Emergency PowerPoint fallback" in `WINDOWS-10-SIGNOFF.md`, in the installed PowerPoint version. Still open | Domain has no PowerPoint dependency | `docs/plans/T0.9-implementation-plan.md` "Deviations" (Not verified here); `MILESTONES.md` T0.9 `[MANUAL]` |

### Phase 0 sign-off scope and T0.6 evidence (T0.5, T0.6, T0.10)

Rows 21 to 23 reconcile the sign-off sheets with what Milestone 0 delivered,
and rows 24 to 26 record the caveats and deferrals under which T0.6's evidence
is accepted.

| # | Exception | Owner | Requalifying proof | Domain independence | Source |
|---|---|---|---|---|---|
| 21 | Hand checks 2-6 of the sign-off (both windows, navigation, canonical rendering, display loss, supervision) run on the `walking-skeleton-lab` example, not `ordo.exe`: the product shell has no output, navigation, or Presenter controls before Phase 1. | `ordo` (development-only example) | The attempt-2 sign-off rows in `WINDOWS-10-SIGNOFF.md` on the lab binary; the lab's tests, `cargo test -p ordo --example walking-skeleton-lab` | Domain has no GPUI dependency | `docs/plans/T0.10-implementation-plan.md` "Church-PC finding 2"; `USB-KIT-README.md`; Decision 271 |
| 22 | Display loss is checked as T0.5 delivers it: fail to black, no retargeting, black until restored. An operator message in the product shell is Phase 1; the lab shows output state in its own operator-only status line. | `ordo` runtime (`crates/ordo/src/output.rs`; lab status line) | The display-loss row in `WINDOWS-10-SIGNOFF.md`; display tests in `tests/display/` | Domain has no GPUI dependency | `docs/plans/T0.10-implementation-plan.md` "Church-PC finding 2"; Decision 271 |
| 23 | The sign-off's storage row runs on a fresh temporary database owned by the Storage Module worker; the runtime does not open SQLite in Milestone 0. | `storage` (temporary database) | The storage row in `WINDOWS-10-SIGNOFF.md`; `tests/storage/tests/acceptance.rs` | Domain has no SQLite dependency | `docs/plans/T0.10-implementation-plan.md` "Church-PC finding 2"; Decision 271 |
| 24 | T0.6's "large-library Preparation" memory baseline is a stand-in: a synthetic 10,000-entry library in memory, because no Preparation UI or storage exists yet. | `ordo` performance harness (T0.6) | The three-run baselines recorded for the church PC; close when a Preparation workspace backs the benchmark | Domain has no GPUI dependency | `docs/plans/T0.6-implementation-plan.md` "Recorded results (church PC, minimum profile)" caveats |
| 25 | T0.6's native harness is the release `presenter-lab.exe` example, not an installed package; there is no installer before T6. | `ordo` performance harness (T0.6) | The church-PC run and its recorded hashes; requalify when T6 packaging lands | Domain has no GPUI dependency | `docs/plans/T0.6-implementation-plan.md` "Recorded results (church PC, minimum profile)" caveats; Decision 271 |
| 26 | T0.6's hot-path AC closes on existing subsystems (Decision 270): SQLite, autosave, pagination, and a Cue timeline do not exist yet, so their checks are reserved (`HotPathKind::Pagination`, `HotPathKind::Autosave`) or move to the task that first builds them (T1.2's timeline). | `ordo` performance probe and `presentation` | `crates/ordo/tests/hot_path_blocking.rs` and the zero hot-path violations in the church-PC run; close as the reserved checks land | Domain has no GPUI dependency | `MILESTONES.md` T0.6 hot-path AC; Decision 270 |
| 27 | Operator display after a hot-plug: with the pinned GPUI, `Window::display` goes stale because Windows reissues `HMONITOR` ids, so the operator window's display could read `None` or name another monitor. The operator display is resolved from the window's live bounds against a fresh display list; ambiguous mixed-DPI overlap keeps the last known display instead of guessing. | `ordo` runtime (`crates/platform/src/display/gpui_adapter.rs`, `crates/ordo/src/output.rs`) | Adapter tests (`display_containing` resolutions, `resolved_display`) and the hot-plug row in `WINDOWS-10-SIGNOFF.md` on attempt 2; re-check when the GPUI supply changes | Domain has no GPUI dependency | `docs/plans/T0.10-implementation-plan.md` "Owner review of the walking-skeleton lab (PR #46)"; `MILESTONES.md` T0.5 hot-plug note |

## Open items in this ledger

- Rows 1 to 7, row 10, and row 27 close only when the pinned GPUI, `gpui-base`,
  or wgpu revision changes the gap. Re-check them at every GPUI supply change,
  as `GPUI-BASE-T0.4a.md` does.
- Rows 21, 22, 23, and 27 requalify together on the Phase 0 sign-off attempt 2
  (issue #50), when the sign-off sheets run against the rebuilt kit.
- Row 25 stays open until T6 packaging lands, when the packaged checks rerun
  against the installed package.
- Row 24 closes when a Preparation workspace backs the benchmark.
- Row 26 closes as the reserved hot-path checks land with their first
  production sites (Decision 270).
- Row 11 remains until T6.2 completes Linux packaged sign-off, or the decision
  is revisited.
- Row 12 remains open. The NFC gap is not closed by any Phase 0 proof.
- Row 20 remains open until the manual PowerPoint check is recorded.
- Known gap, not a workaround row: the `.ordo` container inspector is still
  spike code. It is promoted into a `portability` crate at its owning milestone
  (`MVP-PLAN.md` section 6.4); T0.10 AC5 stays ticked on the enforceable reuse
  rule and the deferral is recorded in Decision 272. Until the promotion, only
  tests reach it through `[dev-dependencies]`.
