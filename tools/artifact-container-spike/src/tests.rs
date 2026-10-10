//! Behaviour tests through the public Interface. Every archive lives in a
//! temporary directory; malicious archives are written with the `zip` writer
//! directly, since it accepts arbitrary entry names.

use crate::{
    ArtifactKind, CreateError, Failure, InspectLimits, Inspection, create, inspect, rfc3339_utc,
    sha256_hex, unsupported_message,
};
use serde_json::{Value, json};
use std::{
    fs::{self, File},
    io::Write,
    path::{Path, PathBuf},
};
use zip::{CompressionMethod, ZipWriter, write::SimpleFileOptions};

fn payload() -> Value {
    json!({
        "schema_version": 1,
        "id": "00000000-0000-4000-8000-000000000001",
        "title": "Test payload"
    })
}

fn options() -> SimpleFileOptions {
    SimpleFileOptions::default()
        .compression_method(CompressionMethod::Deflated)
        .unix_permissions(0o644)
}

/// Writes a plain ZIP with the given `(name, bytes)` entries.
fn build_zip(path: &Path, entries: &[(&str, &[u8])]) {
    let mut zip = ZipWriter::new(File::create(path).unwrap());
    for (name, bytes) in entries {
        zip.start_file(*name, options()).unwrap();
        zip.write_all(bytes).unwrap();
    }
    zip.finish().unwrap();
}

/// A `manifest.json` whose hashes describe the given bytes. Tests pass
/// mismatched bytes to the archive to simulate tampering.
fn manifest_bytes(
    kind: &str,
    schema_version: u64,
    payload: &[u8],
    assets: &[(&str, &[u8])],
) -> Vec<u8> {
    let value = json!({
        "format": "ordo-container",
        "format_version": 1,
        "kind": kind,
        "schema_version": schema_version,
        "artifact_id": "test-artifact",
        "created_at": "2026-10-10T00:00:00Z",
        "payload": {
            "path": "payload.json",
            "sha256": sha256_hex(payload),
            "size": payload.len()
        },
        "assets": assets
            .iter()
            .map(|(path, bytes)| json!({
                "path": path,
                "sha256": sha256_hex(bytes),
                "size": bytes.len()
            }))
            .collect::<Vec<_>>()
    });
    serde_json::to_vec(&value).unwrap()
}

/// A temporary work directory, a staging directory, and the archive path.
struct Fixture {
    work: tempfile::TempDir,
    staging: tempfile::TempDir,
}

impl Fixture {
    fn new() -> Self {
        Self {
            work: tempfile::tempdir().unwrap(),
            staging: tempfile::tempdir().unwrap(),
        }
    }

    fn archive(&self) -> PathBuf {
        self.work.path().join("case.ordo")
    }

    fn inspect(&self, limits: &InspectLimits) -> Inspection {
        inspect(&self.archive(), self.staging.path(), limits)
    }

    fn inspect_default(&self) -> Inspection {
        self.inspect(&InspectLimits::default())
    }
}

fn failures_of(outcome: Inspection) -> Vec<Failure> {
    match outcome {
        Inspection::Invalid(failures) => failures,
        other => panic!("expected Invalid, got {other:?}"),
    }
}

#[test]
fn round_trip_each_kind() {
    for kind in ArtifactKind::ALL {
        let fixture = Fixture::new();
        let assets = vec![("assets/cover.png".to_owned(), vec![1, 2, 3, 4])];
        let written = create(&fixture.archive(), kind, &payload(), &assets).unwrap();
        assert_eq!(written.kind, kind);

        match fixture.inspect_default() {
            Inspection::Valid(routed) => {
                assert_eq!(routed.kind, kind, "routed by manifest kind");
                assert_eq!(routed.payload, payload());
                assert_eq!(routed.manifest, written);
                assert_eq!(routed.staged_assets.len(), 1);
                assert_eq!(
                    fs::read(&routed.staged_assets[0]).unwrap(),
                    vec![1, 2, 3, 4]
                );
            }
            other => panic!("{kind:?}: expected Valid, got {other:?}"),
        }
    }
}

#[test]
fn create_replaces_an_existing_container_atomically() {
    let fixture = Fixture::new();
    let mut first = payload();
    first["title"] = json!("first");
    create(&fixture.archive(), ArtifactKind::SongImport, &first, &[]).unwrap();
    let second = payload();
    create(&fixture.archive(), ArtifactKind::SongImport, &second, &[]).unwrap();

    match fixture.inspect_default() {
        Inspection::Valid(routed) => assert_eq!(routed.payload, second),
        other => panic!("expected Valid, got {other:?}"),
    }
}

#[test]
fn traversal_path_is_rejected_and_nothing_escapes_staging() {
    let fixture = Fixture::new();
    let payload_bytes = serde_json::to_vec(&payload()).unwrap();
    let manifest = manifest_bytes("song-import", 1, &payload_bytes, &[]);
    build_zip(
        &fixture.archive(),
        &[
            ("manifest.json", &manifest),
            ("payload.json", &payload_bytes),
            ("../escape.txt", b"x"),
        ],
    );

    let failures = failures_of(fixture.inspect_default());
    assert!(
        failures.iter().any(|f| matches!(
            f,
            Failure::UnsafePath { path, .. } if path == "../escape.txt"
        )),
        "{failures:?}"
    );
    assert!(!fixture.work.path().join("escape.txt").exists());
}

#[test]
fn unsafe_path_shapes_are_rejected() {
    let shapes = [
        "/absolute.txt",
        "C:drive.txt",
        "assets\\windows.png",
        "assets//empty-segment.png",
        "assets/./dot.png",
        "assets/nul\0.png",
    ];
    for shape in shapes {
        let fixture = Fixture::new();
        let payload_bytes = serde_json::to_vec(&payload()).unwrap();
        let manifest = manifest_bytes("song-import", 1, &payload_bytes, &[]);
        build_zip(
            &fixture.archive(),
            &[
                ("manifest.json", &manifest),
                ("payload.json", &payload_bytes),
                (shape, b"x"),
            ],
        );
        let failures = failures_of(fixture.inspect_default());
        assert!(
            failures
                .iter()
                .any(|f| matches!(f, Failure::UnsafePath { .. })),
            "{shape:?} accepted: {failures:?}"
        );
    }
}

#[test]
fn duplicate_path_is_rejected_case_insensitively() {
    let fixture = Fixture::new();
    let payload_bytes = serde_json::to_vec(&payload()).unwrap();
    let manifest = manifest_bytes("song-import", 1, &payload_bytes, &[]);
    build_zip(
        &fixture.archive(),
        &[
            ("manifest.json", &manifest),
            ("payload.json", &payload_bytes),
            ("Payload.json", b"{}"),
        ],
    );

    let failures = failures_of(fixture.inspect_default());
    assert!(
        failures
            .iter()
            .any(|f| matches!(f, Failure::DuplicatePath { path, .. } if path == "Payload.json")),
        "{failures:?}"
    );
}

#[test]
fn tampered_payload_fails_checksum() {
    let fixture = Fixture::new();
    // Same length, different content, so only the hash can catch it.
    let original = br#"{"schema_version":1,"title":"AAAA"}"#;
    let tampered = br#"{"schema_version":1,"title":"BBBB"}"#;
    let manifest = manifest_bytes("song-import", 1, original, &[]);
    build_zip(
        &fixture.archive(),
        &[("manifest.json", &manifest), ("payload.json", tampered)],
    );

    let failures = failures_of(fixture.inspect_default());
    assert!(
        failures
            .iter()
            .any(|f| matches!(f, Failure::Checksum { path, .. } if path == "payload.json")),
        "{failures:?}"
    );
}

#[test]
fn unlisted_and_missing_entries_are_rejected() {
    let fixture = Fixture::new();
    let payload_bytes = serde_json::to_vec(&payload()).unwrap();
    // Manifest lists a cover asset that is absent, and the archive carries an
    // unlisted extra asset.
    let manifest = manifest_bytes(
        "song-import",
        1,
        &payload_bytes,
        &[("assets/cover.png", b"listed")],
    );
    build_zip(
        &fixture.archive(),
        &[
            ("manifest.json", &manifest),
            ("payload.json", &payload_bytes),
            ("assets/extra.png", b"unlisted"),
        ],
    );

    let failures = failures_of(fixture.inspect_default());
    assert!(failures.contains(&Failure::Unlisted {
        path: "assets/extra.png".to_owned()
    }));
    assert!(failures.contains(&Failure::Missing {
        path: "assets/cover.png".to_owned()
    }));
}

#[test]
fn symlink_entry_is_rejected() {
    let fixture = Fixture::new();
    let payload_bytes = serde_json::to_vec(&payload()).unwrap();
    let manifest = manifest_bytes("song-import", 1, &payload_bytes, &[]);
    let mut zip = ZipWriter::new(File::create(fixture.archive()).unwrap());
    zip.start_file("manifest.json", options()).unwrap();
    zip.write_all(&manifest).unwrap();
    zip.start_file("payload.json", options()).unwrap();
    zip.write_all(&payload_bytes).unwrap();
    zip.add_symlink("assets/link.png", "/etc/passwd", options())
        .unwrap();
    zip.finish().unwrap();

    let failures = failures_of(fixture.inspect_default());
    assert!(
        failures
            .iter()
            .any(|f| matches!(f, Failure::Link { path } if path == "assets/link.png")),
        "{failures:?}"
    );
}

#[test]
fn unknown_kind_is_unsupported_not_guessed() {
    let fixture = Fixture::new();
    let payload_bytes = serde_json::to_vec(&payload()).unwrap();
    let manifest = manifest_bytes("future-kind", 1, &payload_bytes, &[]);
    build_zip(
        &fixture.archive(),
        &[
            ("manifest.json", &manifest),
            ("payload.json", &payload_bytes),
        ],
    );

    match fixture.inspect_default() {
        Inspection::Unsupported {
            kind,
            version,
            supported,
        } => {
            assert_eq!(kind, "future-kind");
            assert_eq!(version, 1);
            assert_eq!(
                unsupported_message(&kind, version, supported),
                "requires an Ordo version supporting future-kind v1; this build supports v1"
            );
        }
        other => panic!("expected Unsupported, got {other:?}"),
    }
}

#[test]
fn future_schema_version_is_unsupported() {
    let fixture = Fixture::new();
    let payload_bytes = serde_json::to_vec(&payload()).unwrap();
    let manifest = manifest_bytes("song-import", 2, &payload_bytes, &[]);
    build_zip(
        &fixture.archive(),
        &[
            ("manifest.json", &manifest),
            ("payload.json", &payload_bytes),
        ],
    );

    assert!(matches!(
        fixture.inspect_default(),
        Inspection::Unsupported { version: 2, .. }
    ));
}

#[test]
fn ratio_bomb_is_rejected_before_extraction() {
    let fixture = Fixture::new();
    let payload_bytes = serde_json::to_vec(&payload()).unwrap();
    // One mebibyte of zeros deflates to about a kilobyte: a ratio far above 200.
    let zeros = vec![0u8; 1 << 20];
    let manifest = manifest_bytes(
        "song-import",
        1,
        &payload_bytes,
        &[("assets/zeros.bin", &zeros)],
    );
    build_zip(
        &fixture.archive(),
        &[
            ("manifest.json", &manifest),
            ("payload.json", &payload_bytes),
            ("assets/zeros.bin", &zeros),
        ],
    );

    let failures = failures_of(fixture.inspect_default());
    assert!(
        failures
            .iter()
            .any(|f| matches!(f, Failure::RatioExceeded { .. })),
        "{failures:?}"
    );
    assert!(
        fs::read_dir(fixture.staging.path())
            .unwrap()
            .next()
            .is_none()
    );
}

#[test]
fn archive_and_entry_count_limits_apply() {
    let fixture = Fixture::new();
    create(
        &fixture.archive(),
        ArtifactKind::SongImport,
        &payload(),
        &[],
    )
    .unwrap();

    let tight_count = InspectLimits {
        max_entries: 1,
        ..InspectLimits::default()
    };
    assert!(matches!(
        failures_of(fixture.inspect(&tight_count)).as_slice(),
        [Failure::TooManyEntries { .. }]
    ));

    let tight_size = InspectLimits {
        max_archive: 16,
        ..InspectLimits::default()
    };
    assert!(matches!(
        failures_of(fixture.inspect(&tight_size)).as_slice(),
        [Failure::ArchiveTooLarge { .. }]
    ));
}

#[test]
fn failed_create_leaves_existing_destination_untouched() {
    let fixture = Fixture::new();
    let dest = fixture.archive();
    fs::write(&dest, b"original bytes").unwrap();

    let mut bad = payload();
    bad["schema_version"] = json!(2);
    let error = create(&dest, ArtifactKind::SongImport, &bad, &[]).unwrap_err();
    assert!(matches!(error, CreateError::Payload(_)), "{error:?}");

    assert_eq!(fs::read(&dest).unwrap(), b"original bytes");
    let names: Vec<_> = fs::read_dir(fixture.work.path())
        .unwrap()
        .map(|entry| entry.unwrap().file_name())
        .collect();
    assert_eq!(names, vec![std::ffi::OsString::from("case.ordo")]);
}

#[test]
fn rejected_asset_name_writes_nothing() {
    let fixture = Fixture::new();
    let dest = fixture.archive();
    let assets = vec![("assets/../escape.png".to_owned(), vec![1])];
    let error = create(&dest, ArtifactKind::SongImport, &payload(), &assets).unwrap_err();
    assert!(matches!(error, CreateError::Rejected(_)), "{error:?}");
    assert!(fs::read_dir(fixture.work.path()).unwrap().next().is_none());
}

#[test]
fn rfc3339_formats_known_instants() {
    assert_eq!(rfc3339_utc(0), "1970-01-01T00:00:00Z");
    assert_eq!(rfc3339_utc(1_000_000_000), "2001-09-09T01:46:40Z");
    assert_eq!(rfc3339_utc(951_782_400), "2000-02-29T00:00:00Z");
}
