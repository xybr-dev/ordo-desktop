# Milestone 0 exceptions ledger

This ledger lists every workaround, deviation, and recorded gap that Phase 0
adopted. It is taken from the qualification documents in `docs/qualification/`
and the Deviations sections of `docs/plans/T0.8-implementation-plan.md` and
`docs/plans/T0.9-implementation-plan.md`. T0.3 to T0.7 plans have no Deviations
section.

Each row names the owner, the proof that requalifies it, and the domain
statement. "Requalifying proof" is the test or record that must pass again
when the item changes or closes.

## Domain independence

`crates/domain/Cargo.toml` has no dependencies. `cargo xtask architecture`
checks the dependencies of `domain` and `presentation` and fails if either
gains a GPUI, SQLite, filesystem, network, or OS-window dependency. Every row
below is a platform or adapter workaround, so `domain` stays independent of all
of them.

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

## Open items in this ledger

- Rows 1 to 7 and row 10 close only when the pinned GPUI, `gpui-base`, or wgpu
  revision changes the gap. Re-check them at every GPUI supply change, as
  `GPUI-BASE-T0.4a.md` does.
- Row 11 remains until T6.2 completes Linux packaged sign-off, or the decision
  is revisited.
- Row 12 remains open. The NFC gap is not closed by any Phase 0 proof.
- Row 20 remains open until the manual PowerPoint check is recorded.
- Known gap, not a workaround row: the `.ordo` container inspector is still
  spike code. It is promoted into a `portability` crate at its owning milestone
  (`MVP-PLAN.md` section 6.4). Until then only tests reach it through
  `[dev-dependencies]`.
