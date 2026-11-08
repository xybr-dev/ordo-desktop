use domain::presentation::{AutoPlay, ContentHash, MassSection, ThemeKind, ThemeRef};

use super::*;
use crate::journal::{LiveIntent, OutputEvidence, RecoveryPoint, SessionId, WrittenAt};

fn uuid(n: u64) -> String {
    format!("00000000-0000-4000-8000-{n:012x}")
}

fn frame(n: u16) -> FrameId {
    uuid(0x500 + u64::from(n)).parse().unwrap()
}

fn cue(n: u16, behavior: CueBehavior) -> Cue {
    let context = matches!(&behavior, CueBehavior::OperatorTrigger | CueBehavior::Wait)
        .then(|| format!("Placeholder context {n}"));
    Cue::new(
        uuid(0x300 + u64::from(n)).parse().unwrap(),
        format!("Cue {n}"),
        context,
        behavior,
    )
    .unwrap()
}

fn project(n: u16) -> Cue {
    cue(
        n,
        CueBehavior::Project {
            frame: frame(n),
            duration_seconds: None,
        },
    )
}

fn item(n: u16, cues: Vec<Cue>) -> MassItem {
    MassItem::new(
        uuid(0x200 + u64::from(n)).parse().unwrap(),
        format!("Item {n}"),
        MassItemKind::Song,
        None,
        None,
        cues,
    )
    .unwrap()
}

/// A General Presentation item with auto-play durations, so recovering into
/// it can exercise a running auto-play without a second snapshot.
fn gp(n: u16, auto_seconds: u32, cues: Vec<Cue>) -> MassItem {
    MassItem::new(
        uuid(0x200 + u64::from(n)).parse().unwrap(),
        format!("Item {n}"),
        MassItemKind::GeneralPresentation,
        None,
        Some(AutoPlay {
            default_seconds: auto_seconds,
        }),
        cues,
    )
    .unwrap()
}

/// Flat layout: 0 project, 1 project, 2 unavailable, 3 project, 4 trigger,
/// 5 wait, 6 hold, 7 project, 8 project. Item 2 (Cues 7 and 8) is a
/// General Presentation with a 3-second auto-play.
fn snapshot_with_id(id: u64) -> Arc<PresentationSnapshot> {
    let section = MassSection::new(
        uuid(0x100).parse().unwrap(),
        "Section".to_owned(),
        vec![
            item(0, vec![project(0), project(1)]),
            item(
                1,
                vec![
                    cue(
                        2,
                        CueBehavior::Unavailable {
                            reason: "placeholder asset missing".to_owned(),
                        },
                    ),
                    project(3),
                    cue(4, CueBehavior::OperatorTrigger),
                    cue(5, CueBehavior::Wait),
                    cue(6, CueBehavior::Hold),
                ],
            ),
            gp(2, 3, vec![project(7), project(8)]),
        ],
    )
    .unwrap();
    let theme = uuid(0x400).parse().unwrap();
    Arc::new(
        PresentationSnapshot::new(
            uuid(id).parse().unwrap(),
            "Test".to_owned(),
            theme,
            vec![ThemeRef {
                id: theme,
                name: "T".to_owned(),
                kind: ThemeKind::Fallback,
            }],
            vec![section],
        )
        .unwrap(),
    )
}

fn snapshot() -> Arc<PresentationSnapshot> {
    snapshot_with_id(1)
}

const SESSION: &str = "00000000-0000-4000-8000-000000000705";
const SOURCE_DIR: &str = "placeholder/snapshot";

fn recovery_point(
    snapshot: &PresentationSnapshot,
    preview: usize,
    program: Option<usize>,
    state: LiveIntent,
    enabled: bool,
) -> RecoveryPoint {
    let positions = snapshot.positions();
    RecoveryPoint {
        session_id: SessionId::parse(SESSION).unwrap(),
        seq: 1,
        written_at: WrittenAt::from_unix_seconds(1_794_182_400),
        snapshot_id: snapshot.id(),
        manifest_hash: ContentHash::from_bytes([7; 32]),
        source_dir: SOURCE_DIR.to_owned(),
        preview: positions[preview].cue_id,
        program: program.map(|flat| positions[flat].cue_id),
        state,
        output: OutputEvidence {
            enabled,
            display: None,
        },
        ended: false,
    }
}

fn recovery(
    snapshot: &PresentationSnapshot,
    preview: usize,
    program: Option<usize>,
    state: LiveIntent,
    enabled: bool,
) -> ValidatedRecovery {
    ValidatedRecovery::for_test(
        recovery_point(snapshot, preview, program, state, enabled),
        preview,
        program,
    )
}

fn recovered(
    snapshot: &Arc<PresentationSnapshot>,
    preview: usize,
    program: Option<usize>,
    state: LiveIntent,
    enabled: bool,
    display_recognized: bool,
) -> Engine {
    let recovery = recovery(snapshot, preview, program, state, enabled);
    Engine::recover(Arc::clone(snapshot), &recovery, display_recognized)
        .expect("the recovery matches its snapshot")
}

fn cue_at(snapshot: &PresentationSnapshot, flat: usize) -> CueId {
    snapshot.positions()[flat].cue_id
}

/// The fail-closed state right after recovery: nothing on Program, no
/// auto-play, and the journaled intent kept only as private context.
fn assert_restored(
    engine: &Engine,
    interrupted: LiveIntent,
    preview: usize,
    program: Option<usize>,
) {
    assert_eq!(engine.state().program(), ProgramContent::Black);
    assert!(!engine.state().unavailable_black());
    assert_eq!(engine.state().auto_play(), None);
    assert_eq!(engine.state().preview(), preview);
    assert_eq!(engine.state().revision(), 0);
    let context = engine
        .state()
        .recovered()
        .expect("a recovered session keeps private context");
    assert_eq!(context.program(), program);
    assert_eq!(context.interrupted(), interrupted);
}

/// One deliberate `Take` from recovered black: linked Live on the Preview
/// Cue, the context cleared, and exactly one revision step.
fn assert_take_live(engine: &mut Engine, preview: usize, now: u64) {
    let revision = engine.state().revision();
    assert_eq!(
        engine.apply(Command::Take, Millis(now)),
        Ok(Outcome::Changed)
    );
    assert_eq!(engine.state().live(), LiveState::Live);
    assert_eq!(
        engine.state().program(),
        ProgramContent::Frame {
            frame: frame(preview as u16),
            cue: preview,
        }
    );
    assert_eq!(engine.state().recovered(), None);
    assert_eq!(engine.state().revision(), revision + 1);
}

#[test]
fn recovery_restores_and_then_takes_every_interrupted_state() {
    let snapshot = snapshot();
    for interrupted in [LiveIntent::Live, LiveIntent::Frozen, LiveIntent::Blackout] {
        for recognized in [true, false] {
            let mut engine = recovered(&snapshot, 1, Some(0), interrupted, true, recognized);
            let expected_live = if recognized {
                LiveState::EnabledBlack
            } else {
                LiveState::Disconnected {
                    display_back: false,
                }
            };
            assert_eq!(engine.state().live(), expected_live, "{interrupted:?}");
            assert_restored(&engine, interrupted, 1, Some(0));

            if !recognized {
                assert_eq!(
                    engine.apply(Command::DisplayReturned { recognized: true }, Millis(1)),
                    Ok(Outcome::Changed)
                );
                assert_eq!(engine.state().revision(), 1);
                assert!(engine.state().recovered().is_some());
                assert_eq!(engine.state().program(), ProgramContent::Black);
            }

            // Owner decision 3: Space always resumes the ordinary linked
            // Live state, never Frozen or Blackout.
            assert_take_live(&mut engine, 1, 2);
        }
    }

    // Output was disabled: opening it stays black, then the same Take.
    let mut disabled = recovered(
        &snapshot,
        1,
        Some(0),
        LiveIntent::OutputDisabled,
        false,
        false,
    );
    assert_eq!(disabled.state().live(), LiveState::OutputDisabled);
    assert_restored(&disabled, LiveIntent::OutputDisabled, 1, Some(0));
    assert_eq!(
        disabled.apply(Command::EnableOutput, Millis(1)),
        Ok(Outcome::Changed)
    );
    assert_eq!(disabled.state().live(), LiveState::EnabledBlack);
    assert_eq!(disabled.state().revision(), 1);
    assert!(disabled.state().recovered().is_some());
    assert_eq!(disabled.state().program(), ProgramContent::Black);
    assert_take_live(&mut disabled, 1, 2);
}

#[test]
fn enabled_black_and_disconnected_intents_restore_and_take_too() {
    let snapshot = snapshot();
    for interrupted in [LiveIntent::EnabledBlack, LiveIntent::Disconnected] {
        for recognized in [true, false] {
            let mut engine = recovered(&snapshot, 0, None, interrupted, true, recognized);
            let expected_live = if recognized {
                LiveState::EnabledBlack
            } else {
                LiveState::Disconnected {
                    display_back: false,
                }
            };
            assert_eq!(engine.state().live(), expected_live);
            assert_restored(&engine, interrupted, 0, None);

            if !recognized {
                assert_eq!(
                    engine.apply(Command::DisplayReturned { recognized: true }, Millis(1)),
                    Ok(Outcome::Changed)
                );
                assert_eq!(engine.state().revision(), 1);
                assert!(engine.state().recovered().is_some());
            }
            assert_take_live(&mut engine, 0, 2);
        }
    }
}

#[test]
fn nothing_ever_live_leaves_no_prior_program_but_keeps_the_context() {
    let snapshot = snapshot();
    let engine = recovered(&snapshot, 0, None, LiveIntent::EnabledBlack, true, true);
    let context = engine.state().recovered().expect("context is present");
    assert_eq!(context.program(), None);
    assert_eq!(context.interrupted(), LiveIntent::EnabledBlack);
    assert_eq!(engine.state().program(), ProgramContent::Black);
}

#[test]
fn take_from_recovered_enabled_black_goes_live_and_clears_the_context() {
    let snapshot = snapshot();
    let mut engine = recovered(&snapshot, 1, Some(0), LiveIntent::Live, true, true);
    assert_eq!(engine.state().revision(), 0);

    assert_take_live(&mut engine, 1, 1);
}

#[test]
fn refused_take_on_an_unavailable_preview_keeps_the_context() {
    let snapshot = snapshot();
    let mut engine = recovered(&snapshot, 2, Some(0), LiveIntent::Live, true, true);
    let before = engine.state().clone();
    let now = engine.last_now();

    assert_eq!(
        engine.apply(Command::Take, Millis(1)),
        Err(Refusal::UnavailablePreview {
            cue: cue_at(&snapshot, 2),
        })
    );
    assert_eq!(engine.state(), &before);
    assert_eq!(engine.last_now(), now);
    assert!(engine.state().recovered().is_some());
}

#[test]
fn take_from_recovered_output_disabled_waits_for_enable() {
    let snapshot = snapshot();
    let mut engine = recovered(&snapshot, 0, None, LiveIntent::OutputDisabled, false, false);

    assert_eq!(
        engine.apply(Command::Take, Millis(1)),
        Err(Refusal::OutputDisabled)
    );
    assert!(engine.state().recovered().is_some());

    assert_eq!(
        engine.apply(Command::EnableOutput, Millis(2)),
        Ok(Outcome::Changed)
    );
    assert_eq!(engine.state().live(), LiveState::EnabledBlack);
    assert!(
        engine.state().recovered().is_some(),
        "EnableOutput does not put content on Program"
    );

    assert_eq!(engine.apply(Command::Take, Millis(3)), Ok(Outcome::Changed));
    assert_eq!(engine.state().live(), LiveState::Live);
    assert_eq!(engine.state().recovered(), None);
}

#[test]
fn take_from_recovered_disconnected_waits_for_the_display() {
    let snapshot = snapshot();
    let mut engine = recovered(&snapshot, 1, Some(0), LiveIntent::Disconnected, true, false);
    assert_eq!(
        engine.state().live(),
        LiveState::Disconnected {
            display_back: false,
        }
    );

    assert_eq!(
        engine.apply(Command::Take, Millis(1)),
        Err(Refusal::DisplayNotBack)
    );
    assert!(engine.state().recovered().is_some());

    assert_eq!(
        engine.apply(Command::DisplayReturned { recognized: true }, Millis(2)),
        Ok(Outcome::Changed)
    );
    assert_eq!(
        engine.state().live(),
        LiveState::Disconnected { display_back: true }
    );
    assert!(engine.state().recovered().is_some());

    assert_eq!(engine.apply(Command::Take, Millis(3)), Ok(Outcome::Changed));
    assert_eq!(engine.state().live(), LiveState::Live);
    assert_eq!(engine.state().recovered(), None);
}

#[test]
fn navigation_keeps_the_context_and_moves_preview_only() {
    let snapshot = snapshot();
    let mut engine = recovered(&snapshot, 0, Some(0), LiveIntent::Live, true, true);

    for (command, preview) in [(Command::Next, 1), (Command::GoTo(3), 3)] {
        assert_eq!(engine.apply(command, Millis(1)), Ok(Outcome::Changed));
        assert_eq!(engine.state().preview(), preview, "{command:?}");
        assert_eq!(engine.state().live(), LiveState::EnabledBlack);
        assert_eq!(engine.state().program(), ProgramContent::Black);
        assert!(engine.state().recovered().is_some(), "{command:?}");
    }
}

#[test]
fn auto_play_runs_while_recovered_without_touching_program() {
    let snapshot = snapshot();
    let mut engine = recovered(&snapshot, 7, Some(0), LiveIntent::Frozen, true, true);

    assert_eq!(
        engine.apply(Command::ToggleAutoPlay, Millis(1)),
        Ok(Outcome::Changed)
    );
    assert_eq!(engine.state().preview(), 7);
    assert!(engine.state().auto_play().is_some());
    assert_eq!(engine.state().program(), ProgramContent::Black);
    assert!(engine.state().recovered().is_some());

    assert_eq!(engine.tick(Millis(3_002)), Outcome::Changed);
    assert_eq!(engine.state().preview(), 8);
    assert_eq!(engine.state().program(), ProgramContent::Black);
    assert_eq!(engine.state().live(), LiveState::EnabledBlack);
    assert!(engine.state().recovered().is_some());
}

#[test]
fn frame_unavailable_keeps_the_recovered_context() {
    let snapshot = snapshot();
    let mut engine = recovered(&snapshot, 0, Some(0), LiveIntent::Live, true, true);

    assert_eq!(
        engine.apply(
            Command::FrameUnavailable {
                frame: frame(0),
                reason: UnavailableReason::DecodeFailed,
            },
            Millis(1),
        ),
        Ok(Outcome::Changed)
    );
    assert_eq!(engine.state().live(), LiveState::EnabledBlack);
    assert_eq!(engine.state().program(), ProgramContent::Black);
    assert!(engine.state().recovered().is_some());
}

#[test]
fn display_lost_while_recovered_keeps_the_context() {
    let snapshot = snapshot();
    let mut engine = recovered(&snapshot, 1, Some(0), LiveIntent::Disconnected, true, false);
    let before = engine.state().clone();

    assert_eq!(
        engine.apply(Command::DisplayLost, Millis(1)),
        Ok(Outcome::Unchanged)
    );
    assert_eq!(engine.state(), &before);
    assert!(engine.state().recovered().is_some());
}

#[test]
fn blackout_from_recovered_keeps_the_context_until_released() {
    let snapshot = snapshot();
    let mut engine = recovered(&snapshot, 1, Some(0), LiveIntent::Blackout, true, true);

    assert_eq!(
        engine.apply(Command::ToggleBlackout, Millis(1)),
        Ok(Outcome::Changed)
    );
    assert_eq!(engine.state().live(), LiveState::Blackout);
    assert_eq!(engine.state().program(), ProgramContent::Black);
    assert!(engine.state().recovered().is_some());

    assert_eq!(
        engine.apply(Command::ToggleBlackout, Millis(2)),
        Ok(Outcome::Changed)
    );
    assert_eq!(engine.state().live(), LiveState::Live);
    assert_eq!(
        engine.state().program(),
        ProgramContent::Frame {
            frame: frame(1),
            cue: 1,
        }
    );
    assert_eq!(engine.state().recovered(), None);
}

#[test]
fn disable_output_clears_the_context_even_when_already_disabled() {
    let snapshot = snapshot();
    let mut enabled = recovered(&snapshot, 0, None, LiveIntent::Live, true, true);
    assert_eq!(
        enabled.apply(Command::DisableOutput, Millis(1)),
        Ok(Outcome::Changed)
    );
    assert_eq!(enabled.state().live(), LiveState::OutputDisabled);
    assert_eq!(enabled.state().recovered(), None);

    let mut disabled = recovered(&snapshot, 0, None, LiveIntent::OutputDisabled, false, false);
    assert!(disabled.state().recovered().is_some());
    assert_eq!(
        disabled.apply(Command::DisableOutput, Millis(1)),
        Ok(Outcome::Changed),
        "clearing the private context alone is an observable change"
    );
    assert_eq!(disabled.state().live(), LiveState::OutputDisabled);
    assert_eq!(disabled.state().recovered(), None);
    assert_eq!(disabled.state().revision(), 1);
}

#[test]
fn check_and_apply_agree_when_the_recovery_context_clears() {
    let snapshot = snapshot();
    let mut engine = recovered(&snapshot, 1, Some(0), LiveIntent::Live, true, true);

    let checked = engine.check(Command::Take, Millis(0));
    assert_eq!(checked, Ok(Outcome::Changed));
    assert!(engine.state().recovered().is_some(), "check is a dry run");
    assert_eq!(engine.apply(Command::Take, Millis(0)), checked);
    assert_eq!(engine.state().recovered(), None);
}

#[test]
fn recover_rejects_a_different_snapshot_or_a_shifted_cue() {
    let snapshot = snapshot();
    let other = snapshot_with_id(2);
    let base = recovery(&snapshot, 0, Some(0), LiveIntent::Live, true);
    let base_point = base.point().clone();
    assert_eq!(
        Engine::recover(Arc::clone(&other), &base, true).err(),
        Some(JournalRejection::SnapshotMismatch)
    );

    let mut shifted_preview_point = base_point.clone();
    shifted_preview_point.preview = cue_at(&snapshot, 1);
    let shifted_preview = ValidatedRecovery::for_test(shifted_preview_point, 0, Some(0));
    assert_eq!(
        Engine::recover(Arc::clone(&snapshot), &shifted_preview, true).err(),
        Some(JournalRejection::SnapshotMismatch)
    );

    let mut shifted_program_point = base_point.clone();
    shifted_program_point.program = Some(cue_at(&snapshot, 1));
    let shifted_program = ValidatedRecovery::for_test(shifted_program_point, 0, Some(0));
    assert_eq!(
        Engine::recover(Arc::clone(&snapshot), &shifted_program, true).err(),
        Some(JournalRejection::SnapshotMismatch)
    );

    let missing_program = ValidatedRecovery::for_test(base_point.clone(), 0, None);
    assert_eq!(
        Engine::recover(Arc::clone(&snapshot), &missing_program, true).err(),
        Some(JournalRejection::SnapshotMismatch)
    );

    let preview_out_of_range =
        ValidatedRecovery::for_test(base_point.clone(), snapshot.cue_count(), Some(0));
    assert_eq!(
        Engine::recover(Arc::clone(&snapshot), &preview_out_of_range, true).err(),
        Some(JournalRejection::SnapshotMismatch)
    );

    let program_out_of_range =
        ValidatedRecovery::for_test(base_point.clone(), 0, Some(snapshot.cue_count()));
    assert_eq!(
        Engine::recover(Arc::clone(&snapshot), &program_out_of_range, true).err(),
        Some(JournalRejection::SnapshotMismatch)
    );

    let mut ended_point = base_point;
    ended_point.ended = true;
    let ended = ValidatedRecovery::for_test(ended_point, 0, Some(0));
    assert_eq!(
        Engine::recover(Arc::clone(&snapshot), &ended, true).err(),
        Some(JournalRejection::Stale)
    );
}
