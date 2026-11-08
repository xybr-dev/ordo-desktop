use domain::presentation::{ContentHash, MassSection, ThemeKind, ThemeRef};

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

/// Flat layout: 0 project, 1 project, 2 unavailable, 3 project, 4 trigger,
/// 5 wait, 6 hold, 7 project, 8 project.
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
            item(2, vec![project(7), project(8)]),
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

fn recovery(
    snapshot: &PresentationSnapshot,
    preview: usize,
    program: Option<usize>,
    state: LiveIntent,
    enabled: bool,
) -> ValidatedRecovery {
    let positions = snapshot.positions();
    ValidatedRecovery {
        point: RecoveryPoint {
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
        },
        preview,
        program,
    }
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

#[test]
fn recovery_restores_every_interrupted_state_and_output_condition() {
    let snapshot = snapshot();
    for interrupted in [LiveIntent::Live, LiveIntent::Frozen, LiveIntent::Blackout] {
        let cases = [
            (interrupted, true, true, LiveState::EnabledBlack),
            (
                interrupted,
                true,
                false,
                LiveState::Disconnected {
                    display_back: false,
                },
            ),
            (
                LiveIntent::OutputDisabled,
                false,
                false,
                LiveState::OutputDisabled,
            ),
        ];
        for (state, enabled, recognized, expected_live) in cases {
            let engine = recovered(&snapshot, 1, Some(0), state, enabled, recognized);
            assert_eq!(engine.state().live(), expected_live, "{interrupted:?}");
            assert_eq!(
                engine.state().program(),
                ProgramContent::Black,
                "nothing reaches Program before Take"
            );
            assert_eq!(engine.state().preview(), 1);
            assert_eq!(engine.state().revision(), 0);
            let context = engine
                .state()
                .recovered()
                .expect("a recovered session keeps private context");
            assert_eq!(context.program(), Some(0));
            assert_eq!(context.interrupted(), state);
        }
    }

    for interrupted in [LiveIntent::EnabledBlack, LiveIntent::Disconnected] {
        let engine = recovered(&snapshot, 0, None, interrupted, true, true);
        assert_eq!(engine.state().live(), LiveState::EnabledBlack);
        assert_eq!(
            engine
                .state()
                .recovered()
                .map(|context| context.interrupted()),
            Some(interrupted)
        );
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

    assert_eq!(engine.apply(Command::Take, Millis(1)), Ok(Outcome::Changed));
    assert_eq!(engine.state().live(), LiveState::Live);
    assert_eq!(
        engine.state().program(),
        ProgramContent::Frame {
            frame: frame(1),
            cue: 1,
        }
    );
    assert_eq!(engine.state().recovered(), None);
    assert_eq!(engine.state().revision(), 1);
}

#[test]
fn refused_take_on_an_unavailable_preview_keeps_the_context() {
    let snapshot = snapshot();
    let mut engine = recovered(&snapshot, 2, Some(0), LiveIntent::Live, true, true);

    assert_eq!(
        engine.apply(Command::Take, Millis(1)),
        Err(Refusal::UnavailablePreview {
            cue: cue_at(&snapshot, 2),
        })
    );
    assert_eq!(engine.state().live(), LiveState::EnabledBlack);
    assert_eq!(engine.state().program(), ProgramContent::Black);
    assert_eq!(engine.state().revision(), 0);
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
    let point = recovery(&snapshot, 0, Some(0), LiveIntent::Live, true);
    assert_eq!(
        Engine::recover(Arc::clone(&other), &point, true).err(),
        Some(JournalRejection::SnapshotMismatch)
    );

    let mut shifted_preview = point.clone();
    shifted_preview.point.preview = cue_at(&snapshot, 1);
    assert_eq!(
        Engine::recover(Arc::clone(&snapshot), &shifted_preview, true).err(),
        Some(JournalRejection::SnapshotMismatch)
    );

    let mut shifted_program = point.clone();
    shifted_program.point.program = Some(cue_at(&snapshot, 1));
    assert_eq!(
        Engine::recover(Arc::clone(&snapshot), &shifted_program, true).err(),
        Some(JournalRejection::SnapshotMismatch)
    );

    let mut missing_program = point.clone();
    missing_program.program = None;
    assert_eq!(
        Engine::recover(Arc::clone(&snapshot), &missing_program, true).err(),
        Some(JournalRejection::SnapshotMismatch)
    );

    let mut preview_out_of_range = point.clone();
    preview_out_of_range.preview = snapshot.cue_count();
    assert_eq!(
        Engine::recover(Arc::clone(&snapshot), &preview_out_of_range, true).err(),
        Some(JournalRejection::SnapshotMismatch)
    );

    let mut program_out_of_range = point;
    program_out_of_range.program = Some(snapshot.cue_count());
    assert_eq!(
        Engine::recover(Arc::clone(&snapshot), &program_out_of_range, true).err(),
        Some(JournalRejection::SnapshotMismatch)
    );
}
