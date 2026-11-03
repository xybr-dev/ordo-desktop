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

/// The published minimal example for `kind`.
fn example(kind: ArtifactKind) -> Value {
    let text = match kind {
        ArtifactKind::SongImport => {
            include_str!("../../../contracts/song-import/examples/minimal.json")
        }
        ArtifactKind::LiturgyImport => {
            include_str!("../../../contracts/liturgy-import/examples/minimal.json")
        }
        ArtifactKind::CelebrationPackage => {
            include_str!("../../../contracts/celebration-package/examples/minimal.json")
        }
        ArtifactKind::ParishBackup => {
            include_str!("../../../contracts/parish-backup/examples/minimal.json")
        }
        ArtifactKind::DiagnosticReport => {
            include_str!("../../../contracts/diagnostic-report/examples/minimal.json")
        }
    };
    serde_json::from_str(text).expect("minimal example parses")
}

/// The published minimal Song import document, so container tests run against
/// a payload the real contract accepts.
fn payload() -> Value {
    serde_json::from_str(include_str!(
        "../../../contracts/song-import/examples/minimal.json"
    ))
    .expect("minimal song-import example parses")
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
        let written = create(&fixture.archive(), kind, &example(kind), &assets).unwrap();
        assert_eq!(written.kind, kind);

        match fixture.inspect_default() {
            Inspection::Valid(routed) => {
                assert_eq!(routed.kind, kind, "routed by manifest kind");
                assert_eq!(routed.payload, example(kind));
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

/// Rewrites the central directory record for `name` as if an MS-DOS/Windows
/// archiver wrote it, with the given external attribute byte.
fn set_dos_attributes(archive: &Path, name: &[u8], attributes: u8) {
    let mut bytes = fs::read(archive).unwrap();
    let record = (0..bytes.len() - 46)
        .find(|&at| {
            bytes[at..at + 4] == [0x50, 0x4b, 0x01, 0x02]
                && bytes[at + 46..].starts_with(name)
                && usize::from(u16::from_le_bytes([bytes[at + 28], bytes[at + 29]])) == name.len()
        })
        .expect("central directory record");
    // Version made by: high byte 0 is MS-DOS (FAT). External attributes: low
    // byte holds the DOS attribute bits.
    bytes[record + 5] = 0;
    bytes[record + 38..record + 42].copy_from_slice(&u32::from(attributes).to_le_bytes());
    fs::write(archive, bytes).unwrap();
}

#[test]
fn dos_directory_attribute_on_a_file_name_is_rejected() {
    let fixture = Fixture::new();
    create(
        &fixture.archive(),
        ArtifactKind::SongImport,
        &payload(),
        &[],
    )
    .unwrap();
    // 0x10 is the DOS directory bit, set on an entry without a trailing slash.
    set_dos_attributes(&fixture.archive(), b"payload.json", 0x10);

    let failures = failures_of(fixture.inspect_default());
    assert!(
        failures
            .iter()
            .any(|f| matches!(f, Failure::NonRegular { path, .. } if path == "payload.json")),
        "{failures:?}"
    );
}

#[test]
fn dos_archive_attribute_on_a_file_is_accepted() {
    let fixture = Fixture::new();
    create(
        &fixture.archive(),
        ArtifactKind::SongImport,
        &payload(),
        &[],
    )
    .unwrap();
    // 0x20 is the DOS archive bit that Windows archivers set on ordinary files.
    set_dos_attributes(&fixture.archive(), b"payload.json", 0x20);

    let outcome = fixture.inspect_default();
    assert!(matches!(outcome, Inspection::Valid(_)), "{outcome:?}");
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

/// Rewrites every `from` to `to` (equal length) in an archive file, in the local
/// headers and the central directory alike. The zip writer refuses duplicate
/// names, so exact duplicates are made this way.
fn rename_in_archive(path: &Path, from: &[u8], to: &[u8]) {
    assert_eq!(from.len(), to.len());
    let mut bytes = fs::read(path).unwrap();
    let mut i = 0;
    while i + from.len() <= bytes.len() {
        if &bytes[i..i + from.len()] == from {
            bytes[i..i + from.len()].copy_from_slice(to);
            i += from.len();
        } else {
            i += 1;
        }
    }
    fs::write(path, bytes).unwrap();
}

/// Builds `assets/a.txt` twice under the same name. The manifest lists the
/// second copy's bytes, so only the duplicate itself is wrong. The raw record
/// count is 4 (manifest, payload, and two `a.txt` records); the reader sees 3.
fn exact_duplicate_archive(fixture: &Fixture) {
    let payload_bytes = serde_json::to_vec(&payload()).unwrap();
    let manifest = manifest_bytes(
        "song-import",
        1,
        &payload_bytes,
        &[("assets/a.txt", b"two")],
    );
    build_zip(
        &fixture.archive(),
        &[
            ("manifest.json", &manifest),
            ("payload.json", &payload_bytes),
            ("assets/a.txt", b"one"),
            ("assets/b.txt", b"two"),
        ],
    );
    rename_in_archive(&fixture.archive(), b"assets/b.txt", b"assets/a.txt");
}

#[test]
fn exact_duplicate_path_is_rejected() {
    let fixture = Fixture::new();
    exact_duplicate_archive(&fixture);

    assert_eq!(
        failures_of(fixture.inspect_default()),
        vec![Failure::EntryCountMismatch {
            declared: 4,
            unique: 3
        }]
    );
}

#[test]
fn raw_count_limit_counts_duplicate_records() {
    let fixture = Fixture::new();
    exact_duplicate_archive(&fixture);

    // Three unique names would fit under this limit; four raw records do not.
    let limits = InspectLimits {
        max_entries: 3,
        ..InspectLimits::default()
    };
    assert_eq!(
        failures_of(fixture.inspect(&limits)),
        vec![Failure::TooManyEntries { count: 4, limit: 3 }]
    );
}

#[test]
fn declared_count_of_0xffff_is_read_as_is() {
    let fixture = Fixture::new();
    create(
        &fixture.archive(),
        ArtifactKind::SongImport,
        &payload(),
        &[],
    )
    .unwrap();
    // Patch the EOCD's entry-count fields (offsets 8 and 10 of the record, which
    // has no comment, so it starts 22 bytes from the end) to 0xFFFF. No ZIP64
    // locator precedes it.
    let mut bytes = fs::read(fixture.archive()).unwrap();
    let eocd = bytes.len() - 22;
    bytes[eocd + 8..eocd + 10].copy_from_slice(&0xFFFFu16.to_le_bytes());
    bytes[eocd + 10..eocd + 12].copy_from_slice(&0xFFFFu16.to_le_bytes());
    fs::write(fixture.archive(), bytes).unwrap();

    assert_eq!(
        failures_of(fixture.inspect_default()),
        vec![Failure::TooManyEntries {
            count: 0xFFFF,
            limit: 4096
        }]
    );
}

/// Writes a v1 song container whose manifest has `field` replaced by `value`.
fn manifest_with_field(archive: &Path, field: &str, value: Value) {
    let payload_bytes = serde_json::to_vec(&payload()).unwrap();
    let mut manifest: Value =
        serde_json::from_slice(&manifest_bytes("song-import", 1, &payload_bytes, &[])).unwrap();
    manifest[field] = value;
    let manifest = serde_json::to_vec(&manifest).unwrap();
    build_zip(
        archive,
        &[
            ("manifest.json", &manifest),
            ("payload.json", &payload_bytes),
        ],
    );
}

#[test]
fn newer_format_version_is_unsupported_container() {
    let fixture = Fixture::new();
    manifest_with_field(&fixture.archive(), "format_version", json!(2));

    match fixture.inspect_default() {
        Inspection::Unsupported {
            kind,
            version,
            supported,
        } => {
            assert_eq!(kind, "container");
            assert_eq!(version, 2);
            assert_eq!(supported, &[1]);
            assert_eq!(
                unsupported_message(&kind, version, supported),
                "requires an Ordo version supporting container v2; this build supports v1"
            );
        }
        other => panic!("expected Unsupported, got {other:?}"),
    }
}

#[test]
fn malformed_format_version_is_a_bad_manifest() {
    for value in [json!("1"), json!(0), json!(1.5), json!(null)] {
        let fixture = Fixture::new();
        manifest_with_field(&fixture.archive(), "format_version", value.clone());
        assert!(
            failures_of(fixture.inspect_default())
                .iter()
                .any(|f| matches!(f, Failure::BadManifest { .. })),
            "format_version {value} was not a bad manifest"
        );
    }
}

#[test]
fn oversized_manifest_is_refused_by_max_payload() {
    let fixture = Fixture::new();
    create(
        &fixture.archive(),
        ArtifactKind::SongImport,
        &payload(),
        &[],
    )
    .unwrap();
    let limits = InspectLimits {
        max_payload: 10,
        ..InspectLimits::default()
    };
    let failures = failures_of(fixture.inspect(&limits));
    for path in ["manifest.json", "payload.json"] {
        assert!(
            failures.iter().any(|f| matches!(
                f,
                Failure::EntryTooLarge { path: p, limit: 10, .. } if p == path
            )),
            "{path} not refused: {failures:?}"
        );
    }
}

#[test]
fn windows_hostile_names_are_rejected() {
    for name in [
        "assets/CON",
        "assets/con.txt",
        "NUL.tar.gz",
        "assets/Lpt9.bin",
        "assets/com1",
        "assets/x/PRN.",
        "assets/trailing.",
        "assets/trailing ",
        "assets/dir /x",
    ] {
        assert!(
            crate::inspect::check_entry_name(name).is_err(),
            "{name:?} accepted"
        );
    }
    for name in [
        "assets/CONSOLE.png",
        "assets/com10.bin",
        "assets/lpt0",
        "assets/nullify.txt",
        "assets/a.b.c",
    ] {
        assert!(
            crate::inspect::check_entry_name(name).is_ok(),
            "{name:?} refused"
        );
    }
}

#[test]
fn non_empty_staging_is_refused_up_front() {
    let fixture = Fixture::new();
    create(
        &fixture.archive(),
        ArtifactKind::SongImport,
        &payload(),
        &[],
    )
    .unwrap();
    fs::write(fixture.staging.path().join("stale.txt"), b"left over").unwrap();

    assert_eq!(
        failures_of(fixture.inspect_default()),
        vec![Failure::Unreadable {
            message: "staging directory must be empty".to_owned()
        }]
    );
}
