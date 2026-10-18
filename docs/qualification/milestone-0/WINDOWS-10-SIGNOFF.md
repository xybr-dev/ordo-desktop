# Milestone 0 Windows 10 22H2 packaged sign-off

This blank sheet is safe to keep in source control. Store the completed sheet
beside the exact qualification bundle it references, together with the bundle's
checksums. Do not commit completed sheets, matching the policy in
`docs/qualification/PHYSICAL-QUALIFICATION-TEMPLATE.md`.

This sheet records the manual `[MANUAL]` checks of the T0.10 gate. Automated
proof is in the bundle described in `README.md`. Linux packaged sign-off is
retired to T6.2 (Decision 268) and is not part of this sheet.

Keep failed attempts. A failure is recorded in the attempts table and is never
overwritten. A rerun gets a new row that names the attempt it reruns.

## Bundle identification

- Bundle commit (`provenance.commit` in `index.json`):
- Bundle `dirty` flag (must be `false`):
- Bundle directory (`target/qualification/milestone-0/runs/<UTC>-<commit>/`):
- Aggregate fixture SHA-256 (`provenance.fixtures`, aggregate):
- `index.json` SHA-256:
- `complete` value in `index.json` (must be `true`):
- Test date and local time:
- Operator name or role:
- Observer name or role:
- Location of this completed sheet beside the bundle:

## Machine

- Windows edition, version, and build (must be 10 22H2, build 19045.x):
- `PROCESSOR_ARCHITECTURE`:
- CPU model:
- Installed RAM:
- GPU name and driver version (from `provenance.gpu`):
- Free storage before test:
- Windows display scale:
- Ordo Interface scale:
- Operator display resolution and refresh rate:
- Congregation display or projector resolution and refresh rate:
- Display topology:
- Installed PowerPoint version (`File > Account > About PowerPoint`):
- Network confirmed unavailable: [ ]
- Release `ordo` binary SHA-256 (`target/release/ordo.exe`):
- `walking-skeleton-lab` binary SHA-256 (bundle artifact `walking-skeleton-lab.exe`):

## Checklist

Mark each row with the attempt number it passed on. Record findings in the
attempts table, not here.

- [ ] Launch: `ordo.exe` launches from the kit, offline.
- [ ] Walking skeleton (`walking-skeleton-lab.exe`): the lab's operator window and the congregation output show the same canonical frame for the same Cue.
- [ ] Task supervision (`walking-skeleton-lab.exe`): closing Ordo's operator window closes the congregation window and the lab, and the process exits with no error dialog.
- [ ] Storage (`walking-skeleton-lab.exe`): the Storage Module worker opens a fresh temporary database and stays responsive during navigation.
- [ ] Canonical rendering (`walking-skeleton-lab.exe`): the projected slides' text placement and wrapping look right at the projector's scale.
- [ ] Navigation (`walking-skeleton-lab.exe`): Left/Right, Up/Down, Freeze, Blackout, and Space take effect on Program.
- [ ] Display loss (`walking-skeleton-lab.exe`): unplugging or disabling the congregation display fails to black with no retargeting to another display, the state shows only in the operator's lab status line, and reconnect returns black first until output is restored.
- [ ] Performance: static Program idle CPU, Presenter open time, and memory stay within the adopted gates, and the observations are recorded.
- [ ] Emergency PowerPoint fixture opens and presents in the installed PowerPoint version (`MILESTONES.md` T0.9 `[MANUAL]`). Record the generated file name and SHA-256 below.

Emergency PowerPoint fixture:

- Generated file name:
- Generated file SHA-256:
- Opens in the installed PowerPoint version without a repair prompt:
- Sections and slide order correct:
- Full-frame images, notes, and safety slides correct:
- Slide Show runs on the congregation display without missing or altered content:

## Attempts

Add a row for every run, including failed and aborted runs. Do not delete or
edit an earlier row. If a fix was made, record the corrected build and the
bundle it came from in the notes.

| Attempt # | Date | Bundle commit | Result (pass / fail / ambiguous) | Failure notes | Rerun of # |
|---|---|---|---|---|---|
| 1 | | | | | |
| 2 | | | | | |

## Hardware hard-gate outcome

Complete this section only after profiling on the minimum hardware profile. If a
hard gate cannot be met (p95 Presenter commands under 50 ms, total memory under
the 1 GiB ceiling, or the 512 MiB qualification budget), choose one outcome:

- [ ] Release blocked. No release is produced until the gate is met.
- [ ] Supported hardware profile revised explicitly, with the revision and its
      rationale recorded in `MVP-PLAN.md` or the decision ledger.

A second production shell is not an accepted outcome. Failures are not hidden
by adding one.

- Gate(s) affected:
- Profiling evidence reference:
- Outcome selected:
- Rationale:

## Findings

For each finding, record severity, affected checklist row, reproduction,
corrective build, whether impact was bounded, and the attempt that reran it.
Ambiguity is release-blocking until resolved.

- Finding:
- Blocking: [ ] Yes  [ ] No
- Corrective build and bundle commit:
- Rerun attempt #:

## Sign-off

- Checklist complete (every row passed on the attempt recorded): [ ] Yes  [ ] No
- No unresolved blocking or ambiguous findings: [ ]
- Hardware hard-gate outcome recorded (section above): [ ]
- Operator sign-off (name, date):
- Observer sign-off (name, date):
