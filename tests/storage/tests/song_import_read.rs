//! T2.10 WP4 storage acceptance tests: reading a song-import `.ordo` file.
//! Archives are built in memory (or in a unique temporary directory) with the
//! storage crate's test packers; nothing reads or writes real Parish data.

use std::fs;
use std::path::{Path, PathBuf};
use std::sync::atomic::{AtomicU64, Ordering};

use domain::song_library::import::ImportDocument;
use serde_json::{Value, json};
use storage::exchange::{
    Finding, FindingCode, InspectLimits, ManifestOverrides, RawEntry, SongImportRead,
    encode_song_import, pack_for_tests, pack_raw_for_tests, read_song_import,
    read_song_import_bytes,
};

const FIXTURES: &str = concat!(env!("CARGO_MANIFEST_DIR"), "/../fixtures/song-import");
const EXAMPLE_V2: &str = concat!(
    env!("CARGO_MANIFEST_DIR"),
    "/../../contracts/song-import/examples/minimal-v2.json"
);

fn limits() -> InspectLimits {
    InspectLimits::default()
}

fn fixture_files(dir: &str, suffix: &str) -> Vec<PathBuf> {
    let mut files: Vec<PathBuf> = fs::read_dir(Path::new(FIXTURES).join(dir))
        .expect("fixture directory")
        .map(|entry| entry.expect("entry").path())
        .filter(|path| path.to_string_lossy().ends_with(suffix))
        .collect();
    files.sort();
    files
}

fn pack(payload: &[u8]) -> Vec<u8> {
    pack_for_tests(&ManifestOverrides::default(), payload, &[])
}

fn read(payload: &[u8]) -> SongImportRead {
    read_song_import_bytes(&pack(payload), &limits())
}

fn valid(read: SongImportRead) -> ImportDocument {
    match read {
        SongImportRead::Valid(document) => document,
        other => panic!("expected Valid, got {other:?}"),
    }
}

fn invalid(read: SongImportRead) -> Vec<Finding> {
    match read {
        SongImportRead::Invalid(findings) => findings,
        other => panic!("expected Invalid, got {other:?}"),
    }
}

fn codes(findings: &[Finding]) -> Vec<FindingCode> {
    findings.iter().map(|finding| finding.code).collect()
}

fn example() -> Value {
    serde_json::from_str(&fs::read_to_string(EXAMPLE_V2).expect("example")).expect("json")
}

fn bytes_of(value: &Value) -> Vec<u8> {
    serde_json::to_vec(value).expect("serialize")
}

struct TempDir(PathBuf);

impl TempDir {
    fn new(tag: &str) -> Self {
        static COUNTER: AtomicU64 = AtomicU64::new(0);
        let path = std::env::temp_dir().join(format!(
            "ordo-song-import-read-{tag}-{}-{}",
            std::process::id(),
            COUNTER.fetch_add(1, Ordering::Relaxed)
        ));
        fs::create_dir_all(&path).expect("create temp dir");
        Self(path)
    }
}

impl Drop for TempDir {
    fn drop(&mut self) {
        let _ = fs::remove_dir_all(&self.0);
    }
}

#[test]
fn ac1_reads_one_valid_song_import() {
    let document = valid(read(&bytes_of(&example())));
    assert_eq!(
        document.id.to_string(),
        "00000000-0000-4000-8000-000000000001"
    );
    assert_eq!(document.song.title, "Hymn of Thanks");
    assert_eq!(document.arrangements.len(), 1);
    let arrangement = &document.arrangements[0];
    assert_eq!(arrangement.languages, vec!["Cebuano".to_owned()]);
    assert_eq!(arrangement.sections.len(), 1);
    assert_eq!(arrangement.sections[0].lyrics, "Salamat, Ginoo");
    assert_eq!(arrangement.sequence.len(), 1);
}

#[test]
fn ac1_every_valid_contract_fixture_decodes() {
    let files = fixture_files("contract", ".valid.json");
    assert!(files.len() >= 3, "minimal, representative and boundary");
    for path in files {
        let payload = fs::read(&path).expect("fixture");
        match read(&payload) {
            SongImportRead::Valid(_) => {}
            other => panic!("{} should decode: {other:?}", path.display()),
        }
    }
}

#[test]
fn ac1_every_reject_fixture_fails_with_its_expected_code() {
    let files = fixture_files("contract", ".reject.json");
    assert!(files.len() >= 6);
    for path in files {
        let expect = fs::read_to_string(path.with_extension("expect")).expect("expect file");
        let expected = match expect.trim() {
            "schema" => FindingCode::Schema,
            "unsupported-version" => FindingCode::UnsupportedVersion,
            "missing-reference" => FindingCode::MissingReference,
            "hash-mismatch" => FindingCode::HashMismatch,
            other => panic!("unknown expectation {other}"),
        };
        let findings = invalid(read(&fs::read(&path).expect("fixture")));
        assert!(
            codes(&findings).contains(&expected),
            "{}: wanted {expected:?}, got {findings:?}",
            path.display()
        );
    }
}

#[test]
fn ac1_a_v1_payload_is_unsupported_with_the_compatible_version_message() {
    let payload = fs::read(Path::new(FIXTURES).join("contract/unknown-version.reject.json"))
        .expect("fixture");
    let bytes = pack_for_tests(
        &ManifestOverrides {
            schema_version: Some(1),
            ..ManifestOverrides::default()
        },
        &payload,
        &[],
    );
    assert_eq!(
        read_song_import_bytes(&bytes, &limits()),
        SongImportRead::Unsupported {
            message: "requires an Ordo version supporting song-import v1; this build supports v2"
                .to_owned()
        }
    );
    // A newer container format is refused the same way, and an unknown kind too.
    let newer = pack_for_tests(
        &ManifestOverrides {
            format_version: Some(2),
            ..ManifestOverrides::default()
        },
        &payload,
        &[],
    );
    assert!(matches!(
        read_song_import_bytes(&newer, &limits()),
        SongImportRead::Unsupported { .. }
    ));
    let unknown = pack_for_tests(
        &ManifestOverrides {
            kind: Some("mystery-box".to_owned()),
            ..ManifestOverrides::default()
        },
        &payload,
        &[],
    );
    assert!(matches!(
        read_song_import_bytes(&unknown, &limits()),
        SongImportRead::Unsupported { .. }
    ));
}

#[test]
fn ac1_a_renamed_pptx_is_not_an_ordo_file_and_nothing_parses_it() {
    let pptx = pack_raw_for_tests(&[
        RawEntry::file("[Content_Types].xml", b"<Types/>"),
        RawEntry::file("ppt/presentation.xml", b"<p:presentation/>"),
        RawEntry::file("ppt/slides/slide1.xml", b"<p:sld/>"),
    ]);
    let findings = invalid(read_song_import_bytes(&pptx, &limits()));
    assert_eq!(codes(&findings), vec![FindingCode::Container]);
    assert!(findings[0].message.contains("It isn't an Ordo file."));
    // Bytes that are not even a ZIP archive say the same.
    let findings = invalid(read_song_import_bytes(b"just some text", &limits()));
    assert!(findings[0].message.contains("It isn't an Ordo file."));
}

#[test]
fn ac1_another_artifact_kind_is_refused() {
    let bytes = pack_for_tests(
        &ManifestOverrides {
            kind: Some("liturgy-import".to_owned()),
            ..ManifestOverrides::default()
        },
        &bytes_of(&example()),
        &[],
    );
    let findings = invalid(read_song_import_bytes(&bytes, &limits()));
    assert_eq!(codes(&findings), vec![FindingCode::WrongKind]);
}

#[test]
fn ac8_container_traversal_duplicate_unlisted_and_ratio_cases_are_invalid() {
    let payload = bytes_of(&example());
    let good = pack(&payload);
    let manifest_json = manifest_of(&payload);

    let cases: Vec<(&str, Vec<RawEntry>)> = vec![
        (
            "traversal",
            vec![
                RawEntry::file("manifest.json", &manifest_json),
                RawEntry::file("payload.json", &payload),
                RawEntry::file("../escape.txt", b"x"),
            ],
        ),
        (
            "absolute",
            vec![
                RawEntry::file("manifest.json", &manifest_json),
                RawEntry::file("payload.json", &payload),
                RawEntry::file("/etc/passwd", b"x"),
            ],
        ),
        (
            "case duplicate",
            vec![
                RawEntry::file("manifest.json", &manifest_json),
                RawEntry::file("payload.json", &payload),
                RawEntry::file("Payload.json", &payload),
            ],
        ),
        (
            "unlisted",
            vec![
                RawEntry::file("manifest.json", &manifest_json),
                RawEntry::file("payload.json", &payload),
                RawEntry::file("extra.txt", b"hello"),
            ],
        ),
        (
            "missing payload",
            vec![RawEntry::file("manifest.json", &manifest_json)],
        ),
        (
            "symlink",
            vec![
                RawEntry::file("manifest.json", &manifest_json),
                RawEntry::file("payload.json", &payload),
                RawEntry {
                    name: "link".to_owned(),
                    bytes: b"/etc/passwd".to_vec(),
                    symlink: true,
                },
            ],
        ),
        (
            "ratio bomb",
            vec![
                RawEntry::file("manifest.json", &manifest_json),
                RawEntry::file("payload.json", &payload),
                RawEntry::file("bomb.bin", &vec![0_u8; 4 * 1024 * 1024]),
            ],
        ),
        (
            "windows device name",
            vec![
                RawEntry::file("manifest.json", &manifest_json),
                RawEntry::file("payload.json", &payload),
                RawEntry::file("NUL.txt", b"x"),
            ],
        ),
    ];
    for (name, entries) in cases {
        let findings = invalid(read_song_import_bytes(
            &pack_raw_for_tests(&entries),
            &limits(),
        ));
        assert!(
            findings
                .iter()
                .all(|finding| finding.code == FindingCode::Container),
            "{name}: {findings:?}"
        );
        assert!(!findings.is_empty(), "{name} must be refused");
    }

    // A tampered payload no longer matches the hash the manifest records.
    let tampered = pack_for_tests(
        &ManifestOverrides {
            payload_sha256: Some("0".repeat(64)),
            ..ManifestOverrides::default()
        },
        &payload,
        &[],
    );
    let findings = invalid(read_song_import_bytes(&tampered, &limits()));
    assert_eq!(codes(&findings), vec![FindingCode::Container]);

    // Limits apply: archive size and entry count.
    let tiny = InspectLimits {
        max_archive: 10,
        ..limits()
    };
    assert!(matches!(
        read_song_import_bytes(&good, &tiny),
        SongImportRead::Invalid(_)
    ));
    let few = InspectLimits {
        max_entries: 1,
        ..limits()
    };
    assert!(matches!(
        read_song_import_bytes(&good, &few),
        SongImportRead::Invalid(_)
    ));
    // And the untouched archive still reads.
    assert!(matches!(
        read_song_import_bytes(&good, &limits()),
        SongImportRead::Valid(_)
    ));
}

/// The manifest bytes `pack_for_tests` writes for `payload`.
fn manifest_of(payload: &[u8]) -> Vec<u8> {
    use sha2::{Digest, Sha256};
    let sha: String = Sha256::digest(payload)
        .iter()
        .map(|byte| format!("{byte:02x}"))
        .collect();
    bytes_of(&json!({
        "format": "ordo-container",
        "format_version": 1,
        "kind": "song-import",
        "schema_version": 2,
        "artifact_id": "test-artifact",
        "created_at": "2026-10-10T00:00:00Z",
        "payload": { "path": "payload.json", "sha256": sha, "size": payload.len() },
        "assets": [],
    }))
}

#[test]
fn ac8_reading_writes_no_file() {
    let dir = TempDir::new("no-write");
    let path = dir.0.join("song.ordo");
    fs::write(&path, pack(&bytes_of(&example()))).expect("write archive");
    let listing = || {
        let mut names: Vec<(String, u64)> = fs::read_dir(&dir.0)
            .expect("list")
            .map(|entry| {
                let entry = entry.expect("entry");
                (
                    entry.file_name().to_string_lossy().into_owned(),
                    entry.metadata().expect("metadata").len(),
                )
            })
            .collect();
        names.sort();
        names
    };
    let before = listing();
    assert!(matches!(
        read_song_import(&path, &limits()),
        SongImportRead::Valid(_)
    ));
    // A refused file and a missing file leave the directory alone too.
    let bad = dir.0.join("bad.ordo");
    fs::write(&bad, b"not an archive").expect("write bad");
    let with_bad = listing();
    assert!(matches!(
        read_song_import(&bad, &limits()),
        SongImportRead::Invalid(_)
    ));
    assert!(matches!(
        read_song_import(&dir.0.join("missing.ordo"), &limits()),
        SongImportRead::Invalid(_)
    ));
    assert_eq!(listing(), with_bad);
    assert_eq!(before.len() + 1, with_bad.len());
}

#[test]
fn ac9_any_invalid_arrangement_makes_the_document_invalid() {
    let path = Path::new(FIXTURES).join("contract/representative.valid.json");
    let mut value: Value = serde_json::from_slice(&fs::read(path).expect("fixture")).expect("json");
    assert!(valid(read(&bytes_of(&value))).arrangements.len() >= 2);
    // One bad language in the last Arrangement excludes the whole document.
    let last = value["arrangements"].as_array().expect("array").len() - 1;
    value["arrangements"][last]["languages"] = json!(["Klingon"]);
    let findings = invalid(read(&bytes_of(&value)));
    assert_eq!(findings.len(), 1, "{findings:?}");
    assert_eq!(
        findings[0].pointer,
        format!("/arrangements/{last}/languages")
    );
}

#[test]
fn ac9_findings_carry_json_pointers() {
    let mut value = example();
    value["arrangements"][0]["sequence"] = json!(["00000000-0000-4000-8000-0000000000ff"]);
    let findings = invalid(read(&bytes_of(&value)));
    assert_eq!(findings.len(), 1);
    assert_eq!(findings[0].code, FindingCode::MissingReference);
    assert_eq!(findings[0].pointer, "/arrangements/0/sequence/0");

    let mut value = example();
    value["arrangements"][0]["sections"][0]["slides"][0]["lines"][0] = json!("two\nlines");
    value["title"] = json!("   ");
    value["extra"] = json!(1);
    let findings = invalid(read(&bytes_of(&value)));
    let pointers: Vec<&str> = findings.iter().map(|f| f.pointer.as_str()).collect();
    assert!(pointers.contains(&"/title"), "{pointers:?}");
    assert!(pointers.contains(&"/extra"), "{pointers:?}");
    assert!(
        pointers.contains(&"/arrangements/0/sections/0/slides/0/lines/0"),
        "{pointers:?}"
    );
    assert!(findings.iter().all(|f| f.code == FindingCode::Schema));

    let mut value = example();
    value["arrangements"][0]["content_sha256"] = json!("0".repeat(64));
    let findings = invalid(read(&bytes_of(&value)));
    assert_eq!(findings[0].code, FindingCode::HashMismatch);
    assert_eq!(findings[0].pointer, "/arrangements/0/content_sha256");
}

#[test]
fn gate_every_valid_fixture_round_trips_through_ordo() {
    let mut files = fixture_files("contract", ".valid.json");
    files.extend(
        fixture_files("batch", ".json")
            .into_iter()
            .filter(|path| !path.to_string_lossy().contains("seed.json"))
            .filter(|path| !path.to_string_lossy().contains("05-invalid")),
    );
    files.push(PathBuf::from(EXAMPLE_V2));
    assert!(files.len() >= 7);
    for path in files {
        let payload = fs::read(&path).expect("fixture");
        let document = valid(read(&payload));
        let encoded = bytes_of(&encode_song_import(&document));
        let again = valid(read(&encoded));
        assert_eq!(
            again,
            document,
            "{} changed in the round trip",
            path.display()
        );
        // Encoding is canonical: the second encoding is byte-identical.
        assert_eq!(bytes_of(&encode_song_import(&again)), encoded);
    }
}

#[test]
fn non_empty_assets_are_refused_for_song_import() {
    let bytes = pack_for_tests(
        &ManifestOverrides::default(),
        &bytes_of(&example()),
        &[("assets/cover.png", &[1, 2, 3])],
    );
    let findings = invalid(read_song_import_bytes(&bytes, &limits()));
    assert_eq!(codes(&findings), vec![FindingCode::Assets]);
    assert_eq!(findings[0].message, "song-import v2 carries no assets");
}

const SCHEMA_V2: &str = concat!(
    env!("CARGO_MANIFEST_DIR"),
    "/../../contracts/song-import/v2.schema.json"
);

/// Fixtures the schema accepts but the Rust reader refuses: rules JSON Schema
/// cannot state: identity uniqueness (duplicate-id), references between
/// records (missing-reference) and the recorded content hash (tampered-hash).
const RUST_ONLY_REJECTIONS: &[&str] = &[
    "duplicate-id.reject.json",
    "missing-reference.reject.json",
    "tampered-hash.reject.json",
];

#[test]
fn ac1_v2_schema_and_rust_validator_agree_on_every_contract_fixture() {
    let schema: Value =
        serde_json::from_str(&fs::read_to_string(SCHEMA_V2).expect("schema")).expect("schema json");
    let validator = jsonschema::validator_for(&schema).expect("schema compiles");
    let mut checked = 0;
    for path in fixture_files("contract", ".json") {
        let name = path
            .file_name()
            .expect("name")
            .to_string_lossy()
            .into_owned();
        let bytes = fs::read(&path).expect("fixture");
        let Ok(document) = serde_json::from_slice::<Value>(&bytes) else {
            continue;
        };
        let schema_ok = validator.is_valid(&document);
        let rust_ok = matches!(read(&bytes), SongImportRead::Valid(_));
        if RUST_ONLY_REJECTIONS.contains(&name.as_str()) {
            assert!(schema_ok && !rust_ok, "{name}: schema-valid, Rust-invalid");
        } else {
            assert_eq!(
                schema_ok, rust_ok,
                "{name}: schema {schema_ok}, Rust {rust_ok}"
            );
        }
        checked += 1;
    }
    assert!(checked >= 8, "only {checked} fixtures compared");
}

#[test]
fn ac1_contract_example_is_the_minimal_fixture() {
    let fixture = Path::new(FIXTURES).join("contract/minimal.valid.json");
    let fixture: Value =
        serde_json::from_str(&fs::read_to_string(fixture).expect("fixture")).expect("json");
    assert_eq!(example(), fixture);
}

#[test]
fn ac8_container_limits_and_manifest_faults_name_their_cause() {
    let payload = bytes_of(&example());
    let good = pack(&payload);
    let message_of = |bytes: &[u8], limits: &InspectLimits| -> String {
        let findings = invalid(read_song_import_bytes(bytes, limits));
        assert!(
            findings
                .iter()
                .all(|finding| finding.code == FindingCode::Container),
            "{findings:?}"
        );
        findings
            .iter()
            .map(|finding| finding.message.clone())
            .collect::<Vec<_>>()
            .join(" | ")
    };

    let small_entry = InspectLimits {
        max_entry: 10,
        ..limits()
    };
    assert!(message_of(&good, &small_entry).contains("uncompressed; the limit is 10"));
    let small_total = InspectLimits {
        max_total: 10,
        ..limits()
    };
    assert!(message_of(&good, &small_total).contains("The entries expand to"));
    let small_payload = InspectLimits {
        max_payload: 10,
        ..limits()
    };
    assert!(message_of(&good, &small_payload).contains("The payload is"));

    // A manifest that lists an unsafe path, or one path twice.
    let manifest = |assets: Value, size: usize| -> Vec<u8> {
        use sha2::{Digest, Sha256};
        let sha: String = Sha256::digest(&payload)
            .iter()
            .map(|byte| format!("{byte:02x}"))
            .collect();
        bytes_of(&json!({
            "format": "ordo-container",
            "format_version": 1,
            "kind": "song-import",
            "schema_version": 2,
            "artifact_id": "test-artifact",
            "created_at": "2026-10-10T00:00:00Z",
            "payload": { "path": "payload.json", "sha256": sha, "size": size },
            "assets": assets,
        }))
    };
    let entry = |path: &str| json!({ "path": path, "sha256": "0".repeat(64), "size": 1 });
    let unsafe_path = manifest(json!([entry("../x")]), payload.len());
    let found = message_of(
        &pack_raw_for_tests(&[
            RawEntry::file("manifest.json", &unsafe_path),
            RawEntry::file("payload.json", &payload),
        ]),
        &limits(),
    );
    assert!(found.contains("listed path ../x"), "{found}");
    let twice = manifest(json!([entry("payload.json")]), payload.len());
    let found = message_of(
        &pack_raw_for_tests(&[
            RawEntry::file("manifest.json", &twice),
            RawEntry::file("payload.json", &payload),
        ]),
        &limits(),
    );
    assert!(found.contains("more than once"), "{found}");
    let wrong_size = manifest(json!([]), payload.len() + 1);
    let found = message_of(
        &pack_raw_for_tests(&[
            RawEntry::file("manifest.json", &wrong_size),
            RawEntry::file("payload.json", &payload),
        ]),
        &limits(),
    );
    assert!(found.contains("the manifest says"), "{found}");

    // A newer container format on a known kind is unsupported, not invalid.
    let newer = pack_for_tests(
        &ManifestOverrides {
            format_version: Some(2),
            ..ManifestOverrides::default()
        },
        &payload,
        &[],
    );
    assert!(
        matches!(
            read_song_import_bytes(&newer, &limits()),
            SongImportRead::Unsupported { .. }
        ),
        "a newer format_version"
    );

    // An entry stored with a compressed size of zero but content.
    let mut zeroed = good.clone();
    let mut patched = 0;
    for at in 0..zeroed.len().saturating_sub(24) {
        if zeroed[at..at + 4] == *b"PK\x01\x02" {
            zeroed[at + 20..at + 24].copy_from_slice(&[0, 0, 0, 0]);
            patched += 1;
        }
    }
    assert!(patched >= 2);
    let found = message_of(&zeroed, &limits());
    assert!(found.contains("expands by more than"), "{found}");
}
