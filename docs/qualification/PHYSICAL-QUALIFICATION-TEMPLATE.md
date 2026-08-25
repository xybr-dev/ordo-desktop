# Ordo physical church-system qualification

This blank template is safe to keep in source control. Store completed reports
locally beside the exact qualified release bundle and checksums. Do not commit
completed church reports.

## Release candidate

- Ordo version:
- Build identifier/commit:
- Installer filename:
- Installer SHA-256:
- Qualification Package version:
- Qualification Package SHA-256:
- Test date and local time:
- Operator name or role:
- Congregation-area observer name or role:
- Final report location beside release bundle:

## Church system

- Windows edition/version/build:
- CPU:
- Installed RAM:
- Free storage before test:
- Windows display scale:
- Ordo Interface scale:
- Operator display resolution and refresh rate:
- Congregation display/projector resolution and refresh rate:
- Display topology:
- Cable/adapter/splitter path:
- Projector mode or relevant processing settings:
- Representative room lighting:
- Installed PowerPoint version:
- Network confirmed unavailable: [ ]

## Preflight

- [ ] Installer checksum matches the published checksum.
- [ ] Install or upgrade completes offline.
- [ ] Parish data remains available or the expected clean fixture state opens.
- [ ] Sanitized qualification Package imports successfully.
- [ ] Display assignment, calibration, aspect handling, and Output Fit are checked.
- [ ] Presenter cold-starts with congregation output disabled.
- Notes:

## Normal Mass pass

- Start time:
- End time:
- Presenter duration (minimum 60 minutes or longer typical Sunday duration):
- Starting Ordo memory:
- Maximum observed Ordo memory:
- Representative idle intervals exercised:
- Farthest routinely occupied observation position:
- Off-center observation position:

### Required observations

- [ ] Ordinary Cue navigation and operator flow remain reliable.
- [ ] Both adopted congregation Themes render correctly.
- [ ] Identification, response, lyric, reading, repeated-section, Hold, and Blackout content behaves correctly.
- [ ] One-to-four-line lyrics and deliberately long wrapped text remain comfortably readable.
- [ ] Cebuano and fallback-font glyphs render correctly.
- [ ] Canonical composition, line breaks, 5% safe area, contrast, and Output Fit remain correct.
- [ ] No clipping, soft/unstable rendering, timing artifact, or strained readability is observed.
- [ ] Cursor remains absent from congregation output and the locator works when required.
- [ ] CPU, memory, thermal behavior, and Presenter latency remain within adopted gates.
- [ ] Program never exposes operator material, chrome, diagnostics, partial frames, or unintended windows.
- Result: [ ] Pass  [ ] Fail  [ ] Ambiguous
- Findings:

## Recovery Drill pass

- Start time:
- End time:

### Required scenarios

- [ ] Rapid Left/Right and Up/Down navigation loses no input.
- [ ] Preview and Program remain correct while Live.
- [ ] Freeze holds Program while Preview navigation continues.
- [ ] Unfreeze takes the selected Preview live correctly.
- [ ] Blackout forces Program to black while Preview navigation continues.
- [ ] Space restores Preview to Program from Blackout.
- [ ] Pointer recovery and topology-based locator direction are correct.
- [ ] Congregation-display disconnect fails safely and reports only to the operator.
- [ ] Reconnect follows the documented black-first recovery path.
- [ ] Emergency insert/edit preserves protected Program behavior.
- [ ] Simulated application interruption restores or fails closed through the documented path.
- [ ] Numbered Cues confirm that no wrong, partial, stale, or operator-only frame reaches Program.
- Result: [ ] Pass  [ ] Fail  [ ] Ambiguous
- Findings:

## Emergency PowerPoint fallback

- Generated file name:
- Generated file SHA-256:
- [ ] Opens in the installed PowerPoint version.
- [ ] Sections and slide order are correct.
- [ ] Full-frame images, notes, and safety slides are correct.
- [ ] Slide Show runs on the congregation display without missing or altered content.
- Result: [ ] Pass  [ ] Fail  [ ] Ambiguous
- Findings:

## Performance evidence

- Maximum total Ordo memory:
- Static Program idle CPU observation:
- Presenter open time:
- Navigation/Freeze/Blackout/Space latency observation:
- Rapid-input result:
- Other measurements or sanitized diagnostic references:

## Findings and reruns

For each finding, record severity, affected pass, reproduction, corrective
build, whether impact was bounded, and the pass or passes rerun. Ambiguity is
release-blocking until resolved.

- Finding:
- Blocking: [ ] Yes  [ ] No
- Non-blocking rationale, if applicable:
- Correction/build:
- Rerun evidence:

## Privacy check

- [ ] No parish-authored lyrics or other parish content is embedded.
- [ ] No congregation photograph, audio, or video is included.
- [ ] No unnecessary personal data is included.
- [ ] Diagnostics and screenshots are sanitized.
- [ ] The completed report will remain outside source control.

## Final sign-off

- Normal Mass pass: [ ] Pass
- Recovery Drill pass: [ ] Pass
- Emergency PowerPoint fallback: [ ] Pass
- No unresolved blocking or ambiguous findings: [ ]
- Release candidate is church-qualified: [ ] Yes  [ ] No
- Operator sign-off and date:
- Observer sign-off and date:
- Requalification trigger or bounded-waiver rationale for this build:
