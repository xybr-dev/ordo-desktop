use std::path::{Path, PathBuf};
use std::sync::atomic::{AtomicUsize, Ordering};

use serde_json::{Value, json};

use super::*;
use crate::snapshot_reader::{DirectorySource, LoadedSnapshot, read_snapshot};
use crate::test_support::write_synthetic_snapshot;

fn load_snapshot() -> LoadedSnapshot {
    static NEXT: AtomicUsize = AtomicUsize::new(0);
    let dir = std::env::temp_dir().join(format!(
        "ordo-journal-snapshot-{}-{}",
        std::process::id(),
        NEXT.fetch_add(1, Ordering::Relaxed)
    ));
    let _ = std::fs::remove_dir_all(&dir);
    write_synthetic_snapshot(&dir).unwrap();
    let loaded = read_snapshot(&DirectorySource::new(&dir));
    let _ = std::fs::remove_dir_all(&dir);
    loaded.expect("the synthetic snapshot loads")
}

fn cue_id(snapshot: &LoadedSnapshot, flat: usize) -> CueId {
    snapshot.snapshot().cue(flat).unwrap().id()
}

fn point_for(snapshot: &LoadedSnapshot) -> RecoveryPoint {
    RecoveryPoint {
        session_id: SessionId::parse("00000000-0000-4000-8000-000000000701").unwrap(),
        seq: 7,
        written_at: WrittenAt::parse("2026-11-07T10:15:30Z").unwrap(),
        snapshot_id: snapshot.snapshot().id(),
        manifest_hash: snapshot.manifest_hash(),
        source_dir: "C:\\Parish\\snapshots\\mass".to_owned(),
        preview: cue_id(snapshot, 3),
        program: Some(cue_id(snapshot, 2)),
        state: LiveIntent::Frozen,
        output: OutputEvidence {
            enabled: true,
            display: Some(DisplayRecord {
                name: Some("Projector \"A\"".to_owned()),
                uuid: None,
                bounds: BoundsRecord {
                    x: -1920,
                    y: 0,
                    width: 1920,
                    height: 1080,
                },
            }),
        },
        ended: false,
    }
}

fn other_uuid() -> String {
    "00000000-0000-4000-8000-0000000007ff".to_owned()
}

/// Re-hash a tampered JSON value so only the tampered field is wrong.
fn rehashed(mut value: Value) -> Vec<u8> {
    value.as_object_mut().unwrap().remove("record_sha256");
    // `serde_json::Value` sorts keys, so go through the typed wire to rehash.
    let wire: Wire = serde_json::from_value({
        let mut v = value;
        v["record_sha256"] = json!("");
        v
    })
    .unwrap();
    let hash = digest_hex(&canonical_body(&wire));
    let mut bytes = canonical_body(&wire);
    bytes.pop();
    bytes.extend_from_slice(format!(",\"record_sha256\":\"{hash}\"}}").as_bytes());
    bytes
}

fn as_value(bytes: &[u8]) -> Value {
    serde_json::from_slice(bytes).unwrap()
}

#[test]
fn a_point_round_trips() {
    let snapshot = load_snapshot();
    let point = point_for(&snapshot);
    assert_eq!(decode(&encode(&point)).unwrap(), point);
}

#[test]
fn a_null_program_and_display_round_trip() {
    let snapshot = load_snapshot();
    let mut point = point_for(&snapshot);
    point.program = None;
    point.state = LiveIntent::OutputDisabled;
    point.output = OutputEvidence {
        enabled: false,
        display: None,
    };
    let bytes = encode(&point);
    assert!(
        std::str::from_utf8(&bytes)
            .unwrap()
            .contains("\"program\":null")
    );
    assert_eq!(decode(&bytes).unwrap(), point);
}

#[test]
fn every_state_round_trips() {
    let snapshot = load_snapshot();
    for state in [
        LiveIntent::OutputDisabled,
        LiveIntent::EnabledBlack,
        LiveIntent::Live,
        LiveIntent::Frozen,
        LiveIntent::Blackout,
        LiveIntent::Disconnected,
    ] {
        let mut point = point_for(&snapshot);
        point.state = state;
        assert_eq!(decode(&encode(&point)).unwrap().state, state);
    }
}

#[test]
fn the_record_is_canonical_compact_json_ending_in_its_hash() {
    let snapshot = load_snapshot();
    let bytes = encode(&point_for(&snapshot));
    let text = std::str::from_utf8(&bytes).unwrap();
    assert!(text.starts_with("{\"schema_version\":2,\"session_id\":"));
    let (body, tail) = text.rsplit_once(",\"record_sha256\":\"").unwrap();
    let hash = tail.strip_suffix("\"}").unwrap();
    assert_eq!(digest_hex(format!("{body}}}").as_bytes()), hash);
}

#[test]
fn truncated_bytes_are_torn() {
    let snapshot = load_snapshot();
    let bytes = encode(&point_for(&snapshot));
    for len in [0, 1, bytes.len() / 2, bytes.len() - 1] {
        assert_eq!(decode(&bytes[..len]), Err(JournalRejection::Torn), "{len}");
    }
}

#[test]
fn one_flipped_byte_inside_a_value_is_torn() {
    let snapshot = load_snapshot();
    let mut bytes = encode(&point_for(&snapshot));
    let text = std::str::from_utf8(&bytes).unwrap();
    // Flip a digit of `seq`: still valid JSON, so only the hash can catch it.
    let at = text.find("\"seq\":7").unwrap() + "\"seq\":".len();
    assert_eq!(bytes[at], b'7');
    bytes[at] = b'8';
    assert_eq!(decode(&bytes), Err(JournalRejection::Torn));
}

#[test]
fn a_tampered_field_with_a_stale_hash_is_torn() {
    let snapshot = load_snapshot();
    let mut value = as_value(&encode(&point_for(&snapshot)));
    value["ended"] = json!(true);
    let tampered = serde_json::to_vec(&value).unwrap();
    assert_eq!(decode(&tampered), Err(JournalRejection::Torn));
}

#[test]
fn wrong_shapes_are_torn() {
    let snapshot = load_snapshot();
    let good = as_value(&encode(&point_for(&snapshot)));

    let mut extra = good.clone();
    extra["surprise"] = json!(1);
    assert_eq!(
        decode(&rehashed_allowing_extra(&extra)),
        Err(JournalRejection::Torn)
    );

    let mut missing = good.clone();
    missing.as_object_mut().unwrap().remove("program");
    assert_eq!(
        decode(&serde_json::to_vec(&missing).unwrap()),
        Err(JournalRejection::Torn)
    );

    let mut bad_state = good.clone();
    bad_state["state"] = json!("playing");
    assert_eq!(decode(&rehashed(bad_state)), Err(JournalRejection::Torn));

    let mut bad_id = good.clone();
    bad_id["preview"]["cue_id"] = json!("not-a-uuid");
    assert_eq!(decode(&rehashed(bad_id)), Err(JournalRejection::Torn));

    let mut bad_time = good.clone();
    bad_time["written_at"] = json!("yesterday");
    assert_eq!(decode(&rehashed(bad_time)), Err(JournalRejection::Torn));

    let mut empty_dir = good;
    empty_dir["snapshot"]["source_dir"] = json!("");
    assert_eq!(decode(&rehashed(empty_dir)), Err(JournalRejection::Torn));

    assert_eq!(decode(b"[]"), Err(JournalRejection::Torn));
    assert_eq!(decode(b"{}"), Err(JournalRejection::Torn));
}

/// Unknown members cannot be rehashed through the typed wire; a stale hash is
/// enough, because deny_unknown_fields already rejects the shape.
fn rehashed_allowing_extra(value: &Value) -> Vec<u8> {
    serde_json::to_vec(value).unwrap()
}

#[test]
fn other_schema_versions_are_unknown_before_the_shape_is_checked() {
    let snapshot = load_snapshot();
    let good = as_value(&encode(&point_for(&snapshot)));
    for version in [1u32, 3, 99] {
        let mut value = good.clone();
        value["schema_version"] = json!(version);
        assert_eq!(
            decode(&serde_json::to_vec(&value).unwrap()),
            Err(JournalRejection::UnknownVersion(version))
        );
    }
    // A real v1 record has a different shape and still says UnknownVersion.
    let v1 = std::fs::read(
        Path::new(env!("CARGO_MANIFEST_DIR"))
            .join("../../contracts/recovery-journal/examples/minimal.json"),
    )
    .unwrap();
    assert_eq!(decode(&v1), Err(JournalRejection::UnknownVersion(1)));
    assert_eq!(
        decode(br#"{"schema_version":"2"}"#),
        Err(JournalRejection::Torn)
    );
}

#[test]
fn a_valid_point_gets_flat_indexes_in_engine_order() {
    let snapshot = load_snapshot();
    let validated = validate(point_for(&snapshot), &snapshot).unwrap();
    assert_eq!(validated.preview, 3);
    assert_eq!(validated.program, Some(2));
    let last = snapshot.snapshot().cue_count() - 1;
    let mut point = point_for(&snapshot);
    point.preview = cue_id(&snapshot, last);
    point.program = None;
    let validated = validate(point, &snapshot).unwrap();
    assert_eq!(validated.preview, last);
    assert_eq!(validated.program, None);
}

#[test]
fn an_ended_record_is_stale_whatever_its_age() {
    let snapshot = load_snapshot();
    for written_at in [
        "1970-01-01T00:00:00Z",
        "2026-11-07T10:15:30Z",
        "9999-12-31T23:59:59Z",
    ] {
        let mut point = point_for(&snapshot);
        point.ended = true;
        point.written_at = WrittenAt::parse(written_at).unwrap();
        let decoded = decode(&encode(&point)).unwrap();
        assert_eq!(validate(decoded, &snapshot), Err(JournalRejection::Stale));
    }
}

#[test]
fn an_old_unended_record_is_not_stale() {
    let snapshot = load_snapshot();
    let mut point = point_for(&snapshot);
    point.written_at = WrittenAt::parse("1970-01-01T00:00:00Z").unwrap();
    assert!(validate(point, &snapshot).is_ok());
}

#[test]
fn another_manifest_hash_or_snapshot_id_is_a_mismatch() {
    let snapshot = load_snapshot();

    let mut point = point_for(&snapshot);
    point.manifest_hash = ContentHash::from_bytes([9; 32]);
    assert_eq!(
        validate(point, &snapshot),
        Err(JournalRejection::SnapshotMismatch)
    );

    let mut point = point_for(&snapshot);
    point.snapshot_id = SnapshotId::parse(&other_uuid()).unwrap();
    assert_eq!(
        validate(point, &snapshot),
        Err(JournalRejection::SnapshotMismatch)
    );
}

#[test]
fn an_unknown_preview_or_program_cue_is_rejected() {
    let snapshot = load_snapshot();

    let mut point = point_for(&snapshot);
    point.preview = CueId::parse(&other_uuid()).unwrap();
    assert_eq!(
        validate(point, &snapshot),
        Err(JournalRejection::UnknownCue)
    );

    let mut point = point_for(&snapshot);
    point.program = Some(CueId::parse(&other_uuid()).unwrap());
    assert_eq!(
        validate(point, &snapshot),
        Err(JournalRejection::UnknownCue)
    );
}

#[test]
fn rejections_display_and_are_errors() {
    let error: &dyn std::error::Error = &JournalRejection::UnknownVersion(4);
    assert!(error.to_string().contains('4'));
    for rejection in [
        JournalRejection::Torn,
        JournalRejection::Stale,
        JournalRejection::SnapshotMismatch,
        JournalRejection::UnknownCue,
    ] {
        assert!(!rejection.to_string().is_empty());
    }
}

fn contracts_dir() -> PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR")).join("../../contracts/recovery-journal")
}

#[test]
fn the_committed_example_decodes_and_is_canonical() {
    let bytes = std::fs::read(contracts_dir().join("examples/minimal-v2.json")).unwrap();
    let trimmed = bytes.strip_suffix(b"\n").unwrap_or(&bytes);
    let point = decode(trimmed).expect("the example decodes with a correct hash");
    assert_eq!(point.state, LiveIntent::Live);
    assert_eq!(encode(&point), trimmed);
}

fn members(schema: &Value) -> (Vec<String>, Vec<String>) {
    let mut properties: Vec<String> = schema["properties"]
        .as_object()
        .unwrap()
        .keys()
        .cloned()
        .collect();
    properties.sort_unstable();
    let mut required: Vec<String> = schema["required"]
        .as_array()
        .unwrap()
        .iter()
        .map(|member| member.as_str().unwrap().to_owned())
        .collect();
    required.sort_unstable();
    (properties, required)
}

fn sorted(members: &[&str]) -> Vec<String> {
    let mut members: Vec<String> = members.iter().map(|member| (*member).to_owned()).collect();
    members.sort_unstable();
    members
}

#[test]
fn the_committed_schema_pins_the_same_version_and_members() {
    // The schema is the contract for other tools; this keeps every level of it
    // from drifting from the wire structs the codec uses. The lists below
    // restate those structs on purpose. Every member is required (nullable
    // ones are present as null).
    let schema: Value = serde_json::from_str(
        &std::fs::read_to_string(contracts_dir().join("v2.schema.json")).unwrap(),
    )
    .unwrap();
    assert_eq!(schema["properties"]["schema_version"]["const"], json!(2));
    assert_eq!(schema["additionalProperties"], json!(false));

    let top = [
        "schema_version",
        "session_id",
        "seq",
        "written_at",
        "snapshot",
        "preview",
        "program",
        "state",
        "output",
        "ended",
        "record_sha256",
    ];
    let display = &schema["$defs"]["display"];
    let levels: [(&Value, &[&str]); 6] = [
        (&schema, &top),
        (
            &schema["properties"]["snapshot"],
            &["id", "manifest_hash", "source_dir"],
        ),
        (&schema["$defs"]["cue_ref"], &["cue_id"]),
        (&schema["properties"]["output"], &["enabled", "display"]),
        (display, &["name", "uuid", "bounds"]),
        (
            &display["properties"]["bounds"],
            &["x", "y", "width", "height"],
        ),
    ];
    for (level, expected) in levels {
        assert_eq!(level["additionalProperties"], json!(false));
        let (properties, required) = members(level);
        assert_eq!(properties, sorted(expected));
        assert_eq!(required, sorted(expected));
    }

    // The state enum is the LiveIntent spelling.
    let mut states: Vec<String> = schema["properties"]["state"]["enum"]
        .as_array()
        .unwrap()
        .iter()
        .map(|state| state.as_str().unwrap().to_owned())
        .collect();
    states.sort_unstable();
    let mut intents: Vec<String> = [
        LiveIntent::OutputDisabled,
        LiveIntent::EnabledBlack,
        LiveIntent::Live,
        LiveIntent::Frozen,
        LiveIntent::Blackout,
        LiveIntent::Disconnected,
    ]
    .iter()
    .map(|intent| intent.as_str().to_owned())
    .collect();
    intents.sort_unstable();
    assert_eq!(states, intents);

    // And the encoder writes exactly those members, in schema order.
    let snapshot = load_snapshot();
    let written = as_value(&encode(&point_for(&snapshot)));
    let mut written_top: Vec<String> = written.as_object().unwrap().keys().cloned().collect();
    written_top.sort_unstable();
    assert_eq!(written_top, sorted(&top));
}
