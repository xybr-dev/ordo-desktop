use std::path::PathBuf;
use std::sync::Arc;
use std::sync::atomic::{AtomicUsize, Ordering};

use super::*;
use crate::commands::Command;
use crate::journal::{BoundsRecord, JournalRejection, decode, encode, validate};
use crate::snapshot_reader::{DirectorySource, LoadedSnapshot, read_snapshot};
use crate::state::{LiveState, Millis, ProgramContent};
use crate::test_support::write_synthetic_snapshot;

fn load_snapshot() -> LoadedSnapshot {
    static NEXT: AtomicUsize = AtomicUsize::new(0);
    let dir: PathBuf = std::env::temp_dir().join(format!(
        "ordo-recorder-snapshot-{}-{}",
        std::process::id(),
        NEXT.fetch_add(1, Ordering::Relaxed)
    ));
    let _ = std::fs::remove_dir_all(&dir);
    write_synthetic_snapshot(&dir).unwrap();
    let loaded = read_snapshot(&DirectorySource::new(&dir));
    let _ = std::fs::remove_dir_all(&dir);
    loaded.expect("the synthetic snapshot loads")
}

fn engine_on(snapshot: &LoadedSnapshot) -> Engine {
    Engine::new(Arc::new(snapshot.snapshot().clone()))
}

const SOURCE_DIR: &str = "C:\\Parish\\snapshots\\mass";

fn session() -> SessionId {
    SessionId::parse("00000000-0000-4000-8000-000000000702").unwrap()
}

fn recorder_for(snapshot: &LoadedSnapshot) -> JournalRecorder {
    JournalRecorder::new(SessionIdentity::of(session(), snapshot, SOURCE_DIR))
}

fn cue(snapshot: &LoadedSnapshot, flat: usize) -> CueId {
    snapshot.snapshot().cue(flat).unwrap().id()
}

fn at(seconds: u64) -> WrittenAt {
    WrittenAt::from_unix_seconds(seconds)
}

fn run(engine: &mut Engine, command: Command) {
    engine.apply(command, Millis(0)).unwrap();
}

fn display() -> DisplayRecord {
    DisplayRecord {
        name: Some("Projector".to_owned()),
        uuid: None,
        bounds: BoundsRecord {
            x: 1920,
            y: 0,
            width: 1280,
            height: 720,
        },
    }
}

#[test]
fn every_live_state_has_its_journal_spelling() {
    let cases = [
        (LiveState::OutputDisabled, LiveIntent::OutputDisabled),
        (LiveState::EnabledBlack, LiveIntent::EnabledBlack),
        (LiveState::Live, LiveIntent::Live),
        (LiveState::Frozen, LiveIntent::Frozen),
        (LiveState::Blackout, LiveIntent::Blackout),
        (
            LiveState::Disconnected {
                display_back: false,
            },
            LiveIntent::Disconnected,
        ),
        (
            LiveState::Disconnected { display_back: true },
            LiveIntent::Disconnected,
        ),
    ];
    for (live, intent) in cases {
        assert_eq!(LiveIntent::of(live), intent, "{live:?}");
    }
}

#[test]
fn unix_seconds_become_utc_timestamps() {
    for (seconds, text) in [
        (0, "1970-01-01T00:00:00Z"),
        (951_782_400, "2000-02-29T00:00:00Z"),
        (1_709_164_800, "2024-02-29T00:00:00Z"),
        (1_794_182_399, "2026-11-08T23:59:59Z"),
    ] {
        let written = WrittenAt::from_unix_seconds(seconds);
        assert_eq!(written.as_str(), text);
        assert_eq!(WrittenAt::parse(text).unwrap(), written);
    }
}

#[test]
fn seconds_past_year_9999_clamp_to_its_last_second() {
    for seconds in [253_402_300_799, 253_402_300_800, u64::MAX] {
        assert_eq!(
            WrittenAt::from_unix_seconds(seconds).as_str(),
            "9999-12-31T23:59:59Z"
        );
    }
    assert_eq!(
        WrittenAt::from_unix_seconds(253_402_300_798).as_str(),
        "9999-12-31T23:59:58Z"
    );
}

#[test]
fn every_formatted_timestamp_parses_and_rises_with_its_seconds() {
    // A stride that is not a whole number of days, so the sweep crosses
    // month, leap-year and century boundaries at varying offsets.
    let mut previous: Option<String> = None;
    let mut seconds = 0u64;
    while seconds <= 253_402_300_799 {
        let written = WrittenAt::from_unix_seconds(seconds);
        assert!(WrittenAt::parse(written.as_str()).is_ok());
        if let Some(previous) = &previous {
            assert!(previous.as_str() < written.as_str(), "{seconds}");
        }
        previous = Some(written.as_str().to_owned());
        seconds += 86_400 * 365 + 7_919;
    }
}

#[test]
fn the_first_observe_emits_seq_one_and_an_unchanged_engine_emits_nothing() {
    let snapshot = load_snapshot();
    let engine = engine_on(&snapshot);
    let mut recorder = recorder_for(&snapshot);
    let first = recorder
        .observe(&engine, None, at(1))
        .expect("the first observe emits");
    assert_eq!(first.seq, 1);
    assert!(!first.ended);
    assert_eq!(recorder.observe(&engine, None, at(2)), None);
    assert_eq!(recorder.observe(&engine, None, at(3)), None);
}

#[test]
fn seq_rises_by_one_per_point_and_holds_still_on_none() {
    let snapshot = load_snapshot();
    let mut engine = engine_on(&snapshot);
    let mut recorder = recorder_for(&snapshot);
    let mut seqs = Vec::new();
    let steps = [
        Command::EnableOutput,
        Command::Next,
        Command::Take,
        Command::Next,
        Command::ToggleFreeze,
    ];

    seqs.push(recorder.observe(&engine, None, at(1)).unwrap().seq);
    for command in steps {
        run(&mut engine, command);
        seqs.push(recorder.observe(&engine, None, at(2)).unwrap().seq);
        assert_eq!(recorder.observe(&engine, None, at(3)), None);
    }
    let expected: Vec<u64> = (1..=steps.len() as u64 + 1).collect();
    assert_eq!(seqs, expected);
}

#[test]
fn a_display_change_alone_is_a_change() {
    let snapshot = load_snapshot();
    let mut engine = engine_on(&snapshot);
    run(&mut engine, Command::EnableOutput);
    let mut recorder = recorder_for(&snapshot);

    let first = recorder.observe(&engine, None, at(1)).unwrap();
    assert_eq!(first.output.display, None);

    let assigned = recorder
        .observe(&engine, Some(display()), at(2))
        .expect("a display appearing is a change");
    assert_eq!(assigned.seq, 2);
    assert_eq!(assigned.output.display, Some(display()));
    assert_eq!(recorder.observe(&engine, Some(display()), at(3)), None);

    let lost = recorder
        .observe(&engine, None, at(4))
        .expect("a display disappearing is a change");
    assert_eq!(lost.seq, 3);
    assert_eq!(lost.output.display, None);
}

#[test]
fn end_is_always_emitted_bumps_seq_and_stops_observe() {
    let snapshot = load_snapshot();
    let mut engine = engine_on(&snapshot);
    run(&mut engine, Command::EnableOutput);
    let mut recorder = recorder_for(&snapshot);

    let observed = recorder.observe(&engine, None, at(1)).unwrap();
    assert!(!observed.ended);

    // Nothing changed since the last point, and `end` still writes one.
    let ended = recorder.end(&engine, None, at(2));
    assert!(ended.ended);
    assert_eq!(ended.seq, observed.seq + 1);
    assert_eq!(ended.written_at, at(2));

    // A clean end is final: the session cannot record a live point again.
    run(&mut engine, Command::Take);
    assert_eq!(recorder.observe(&engine, None, at(3)), None);

    let again = recorder.end(&engine, None, at(4));
    assert!(again.ended);
    assert_eq!(again.seq, ended.seq + 1);
    assert_eq!(again.state, LiveIntent::Live);
}

#[test]
fn every_point_carries_the_session_identity() {
    let snapshot = load_snapshot();
    let mut engine = engine_on(&snapshot);
    let mut recorder = recorder_for(&snapshot);
    let mut points = vec![recorder.observe(&engine, None, at(1)).unwrap()];
    run(&mut engine, Command::EnableOutput);
    points.push(recorder.observe(&engine, None, at(2)).unwrap());
    points.push(recorder.end(&engine, None, at(3)));

    for point in &points {
        assert_eq!(point.session_id, session());
        assert_eq!(point.snapshot_id, snapshot.snapshot().id());
        assert_eq!(point.manifest_hash, snapshot.manifest_hash());
        assert_eq!(point.source_dir, SOURCE_DIR);
    }
}

#[test]
fn every_live_state_is_recorded_with_its_output_evidence() {
    let snapshot = load_snapshot();
    let cases: [(&[Command], LiveIntent, bool); 7] = [
        (&[], LiveIntent::OutputDisabled, false),
        (&[Command::EnableOutput], LiveIntent::EnabledBlack, true),
        (
            &[Command::EnableOutput, Command::Take],
            LiveIntent::Live,
            true,
        ),
        (
            &[Command::EnableOutput, Command::Take, Command::ToggleFreeze],
            LiveIntent::Frozen,
            true,
        ),
        (
            &[
                Command::EnableOutput,
                Command::Take,
                Command::ToggleBlackout,
            ],
            LiveIntent::Blackout,
            true,
        ),
        (
            &[Command::EnableOutput, Command::Take, Command::DisplayLost],
            LiveIntent::Disconnected,
            true,
        ),
        (
            &[
                Command::EnableOutput,
                Command::Take,
                Command::DisplayLost,
                Command::DisplayReturned { recognized: true },
            ],
            LiveIntent::Disconnected,
            true,
        ),
    ];
    for (commands, intent, enabled) in cases {
        let mut engine = engine_on(&snapshot);
        for command in commands {
            run(&mut engine, *command);
        }
        let mut recorder = recorder_for(&snapshot);
        let point = recorder.observe(&engine, None, at(1)).unwrap();
        assert_eq!(point.state, intent, "{commands:?}");
        assert_eq!(point.output.enabled, enabled, "{commands:?}");
    }
}

#[test]
fn program_is_none_until_a_frame_has_been_on_program() {
    let snapshot = load_snapshot();
    let mut engine = engine_on(&snapshot);
    run(&mut engine, Command::EnableOutput);
    let mut recorder = recorder_for(&snapshot);

    let point = recorder.observe(&engine, None, at(1)).unwrap();
    assert_eq!(point.program, None);
    for command in [Command::Next, Command::Next, Command::GoTo(4)] {
        run(&mut engine, command);
        let point = recorder.observe(&engine, None, at(2)).unwrap();
        assert_eq!(point.program, None, "{command:?}");
        assert_eq!(point.preview, cue(&snapshot, engine.state().preview()));
    }
}

#[test]
fn program_keeps_the_last_valid_cue_through_black() {
    let snapshot = load_snapshot();
    for black in [
        Command::ToggleBlackout,
        Command::DisableOutput,
        Command::DisplayLost,
    ] {
        let mut engine = engine_on(&snapshot);
        run(&mut engine, Command::EnableOutput);
        run(&mut engine, Command::Take);
        let mut recorder = recorder_for(&snapshot);
        assert_eq!(
            recorder.observe(&engine, None, at(1)).unwrap().program,
            Some(cue(&snapshot, 0))
        );

        run(&mut engine, black);
        assert_eq!(engine.state().program(), ProgramContent::Black, "{black:?}");
        let point = recorder.observe(&engine, None, at(2)).unwrap();
        assert_eq!(point.program, Some(cue(&snapshot, 0)), "{black:?}");
    }
}

#[test]
fn a_black_landing_keeps_the_last_frame_cue_until_a_new_frame_replaces_it() {
    let snapshot = load_snapshot();
    let mut engine = engine_on(&snapshot);
    run(&mut engine, Command::EnableOutput);
    run(&mut engine, Command::Take);
    let mut recorder = recorder_for(&snapshot);

    // Linked Live: Program follows Preview through flat 1..=4 (Frames).
    for _ in 0..4 {
        run(&mut engine, Command::Next);
    }
    assert_eq!(
        recorder.observe(&engine, None, at(1)).unwrap().program,
        Some(cue(&snapshot, 4))
    );

    // Flat 5 is an operator trigger: Program goes black, the cue is kept.
    run(&mut engine, Command::Next);
    assert_eq!(engine.state().program(), ProgramContent::Black);
    assert_eq!(
        recorder.observe(&engine, None, at(2)).unwrap().program,
        Some(cue(&snapshot, 4))
    );

    // Flat 6 is a Frame again: the new cue replaces the remembered one.
    run(&mut engine, Command::Next);
    assert!(matches!(
        engine.state().program(),
        ProgramContent::Frame { cue: 6, .. }
    ));
    assert_eq!(
        recorder.observe(&engine, None, at(3)).unwrap().program,
        Some(cue(&snapshot, 6))
    );
}

#[test]
fn recorded_points_round_trip_and_validate_against_their_snapshot() {
    let snapshot = load_snapshot();
    let mut engine = engine_on(&snapshot);
    let mut recorder = recorder_for(&snapshot);
    let mut points = Vec::new();
    points.extend(recorder.observe(&engine, None, at(1)));
    run(&mut engine, Command::EnableOutput);
    points.extend(recorder.observe(&engine, Some(display()), at(2)));
    run(&mut engine, Command::Next);
    run(&mut engine, Command::Take);
    run(&mut engine, Command::Next);
    points.extend(recorder.observe(&engine, Some(display()), at(3)));
    run(&mut engine, Command::ToggleFreeze);
    points.extend(recorder.observe(&engine, Some(display()), at(4)));
    run(&mut engine, Command::DisplayLost);
    points.extend(recorder.observe(&engine, None, at(5)));
    points.push(recorder.end(&engine, None, at(6)));
    assert_eq!(points.len(), 6);

    for point in &points {
        let decoded = decode(&encode(point)).unwrap();
        assert_eq!(&decoded, point);
        if point.ended {
            assert_eq!(
                validate(decoded, &snapshot),
                Err(JournalRejection::Stale),
                "the end point is never reopened"
            );
            continue;
        }
        let validated = validate(decoded, &snapshot).unwrap();
        assert_eq!(
            snapshot.snapshot().cue(validated.preview).unwrap().id(),
            point.preview
        );
        assert_eq!(
            validated
                .program
                .map(|flat| snapshot.snapshot().cue(flat).unwrap().id()),
            point.program
        );
    }
}
