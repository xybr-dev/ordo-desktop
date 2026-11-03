//! T0.8 WP3 end-to-end acceptance: every artifact kind round-trips through
//! `create` and `inspect`, and a malformed corpus is refused without any file
//! escaping its staging directory or touching a sentinel Parish tree.

use artifact_container_spike::{
    ArtifactKind, CreateError, Failure, InspectLimits, Inspection, create, inspect,
    unsupported_message,
};
use serde_json::{Value, json};
use sha2::{Digest, Sha256};
use std::{
    collections::BTreeMap,
    fs,
    io::{Cursor, Write},
    path::Path,
};
use zip::{CompressionMethod, ZipWriter, write::SimpleFileOptions};

const MIB: u64 = 1024 * 1024;
const FIXTURES: &str = concat!(env!("CARGO_MANIFEST_DIR"), "/../fixtures/contracts");
const SONG: &str = "song-import";
const STAGING: &str = "staging";

fn fixture(kind: &str, name: &str) -> Vec<u8> {
    let path = Path::new(FIXTURES).join(kind).join(name);
    fs::read(&path).unwrap_or_else(|e| panic!("cannot read {}: {e}", path.display()))
}

fn sha_hex(bytes: &[u8]) -> String {
    Sha256::digest(bytes)
        .iter()
        .map(|byte| format!("{byte:02x}"))
        .collect()
}

/// One raw archive entry. `Link` is written as a Unix symbolic link.
enum Item {
    File(String, Vec<u8>),
    Link(String, String),
}

fn file(name: &str, bytes: impl Into<Vec<u8>>) -> Item {
    Item::File(name.to_owned(), bytes.into())
}

/// Writes entries verbatim, so hostile names and modes reach the reader.
fn build_zip(items: &[Item], method: CompressionMethod) -> Vec<u8> {
    let mut zip = ZipWriter::new(Cursor::new(Vec::new()));
    let options = SimpleFileOptions::default()
        .compression_method(method)
        .unix_permissions(0o644);
    for item in items {
        match item {
            Item::File(name, bytes) => {
                zip.start_file(name.as_str(), options).expect("start entry");
                zip.write_all(bytes).expect("write entry");
            }
            Item::Link(name, target) => zip
                .add_symlink(name.as_str(), target.as_str(), options)
                .expect("add symlink"),
        }
    }
    zip.finish().expect("finish zip").into_inner()
}

/// Replaces every `from` with `to` (equal length) in raw archive bytes.
fn retarget(bytes: &[u8], from: &[u8], to: &[u8]) -> Vec<u8> {
    assert_eq!(from.len(), to.len(), "retarget needs equal lengths");
    let mut out = bytes.to_vec();
    let mut i = 0;
    while i + from.len() <= out.len() {
        if &out[i..i + from.len()] == from {
            out[i..i + from.len()].copy_from_slice(to);
            i += from.len();
        } else {
            i += 1;
        }
    }
    out
}

/// A hand-built v1 manifest whose hashes match the given bytes.
fn manifest(kind: &str, schema_version: u64, payload: &[u8], assets: &[(&str, &[u8])]) -> Value {
    json!({
        "format": "ordo-container",
        "format_version": 1,
        "kind": kind,
        "schema_version": schema_version,
        "artifact_id": "00000000-0000-4000-8000-0000000000aa",
        "created_at": "2026-10-10T00:00:00Z",
        "payload": {"path": "payload.json", "sha256": sha_hex(payload), "size": payload.len()},
        "assets": assets
            .iter()
            .map(|(path, bytes)| json!({"path": path, "sha256": sha_hex(bytes), "size": bytes.len()}))
            .collect::<Vec<_>>(),
    })
}

/// `manifest.json`, `payload.json`, then the listed assets.
fn container(manifest: &Value, payload: &[u8], assets: &[(&str, &[u8])]) -> Vec<Item> {
    let mut items = vec![
        file(
            "manifest.json",
            serde_json::to_vec_pretty(manifest).unwrap(),
        ),
        file("payload.json", payload.to_vec()),
    ];
    items.extend(
        assets
            .iter()
            .map(|(path, bytes)| file(path, bytes.to_vec())),
    );
    items
}

/// A valid song container with no assets, plus extra raw entries.
fn song_with(extra: Vec<Item>) -> Vec<Item> {
    let payload = fixture(SONG, "minimal.valid.json");
    let mut items = container(&manifest(SONG, 1, &payload, &[]), &payload, &[]);
    items.extend(extra);
    items
}

/// Relative path (forward slashes) to content hash, or `dir`. With
/// `skip_staging`, the staging subtree is excluded: it is the only place
/// inspection may write.
fn snapshot(base: &Path, dir: &Path, skip_staging: bool, out: &mut BTreeMap<String, String>) {
    for entry in fs::read_dir(dir).expect("read dir") {
        let path = entry.expect("dir entry").path();
        let rel = path
            .strip_prefix(base)
            .expect("under base")
            .to_string_lossy()
            .replace('\\', "/");
        if skip_staging && rel == STAGING {
            continue;
        }
        if path.is_dir() {
            out.insert(rel, "dir".to_owned());
            snapshot(base, &path, skip_staging, out);
        } else {
            out.insert(rel, sha_hex(&fs::read(&path).expect("read file")));
        }
    }
}

fn tree(base: &Path, skip_staging: bool) -> BTreeMap<String, String> {
    let mut out = BTreeMap::new();
    snapshot(base, base, skip_staging, &mut out);
    out
}

/// Sentinel Parish tree plus an inbox holding the archive under test. The
/// archive is written before any snapshot, so it is part of the baseline.
fn sentinel_root(name: &str, bytes: &[u8]) -> (tempfile::TempDir, std::path::PathBuf) {
    let root = tempfile::tempdir().expect("temp root");
    let parish = root.path().join("parish");
    fs::create_dir_all(parish.join("songs")).expect("parish dirs");
    fs::write(parish.join("parish.db"), b"sentinel parish bytes").expect("parish db");
    fs::write(parish.join("songs").join("hymn.json"), b"{}").expect("parish song");
    let inbox = root.path().join("inbox");
    fs::create_dir_all(&inbox).expect("inbox");
    let archive = inbox.join(format!("{name}.ordo"));
    fs::write(&archive, bytes).expect("archive");
    (root, archive)
}

/// Inspects `bytes` and checks the no-escape invariants around it: nothing
/// outside `staging/` changed, and no file named `evil` exists anywhere.
fn inspect_contained(name: &str, bytes: &[u8], limits: &InspectLimits) -> Inspection {
    let (root, archive) = sentinel_root(name, bytes);
    let staging = root.path().join(STAGING).join(name);
    fs::create_dir_all(&staging).expect("staging");
    let before = tree(root.path(), true);
    let outcome = inspect(&archive, &staging, limits);
    let after = tree(root.path(), true);
    assert_eq!(before, after, "{name}: a path outside staging changed");
    let everything = tree(root.path(), false);
    assert!(
        !everything
            .keys()
            .any(|key| key.rsplit('/').next() == Some("evil")),
        "{name}: a file named evil exists under the root"
    );
    outcome
}

#[derive(Clone)]
enum Want {
    /// Invalid, with a failure satisfying the predicate.
    Failure(fn(&Failure) -> bool),
    /// Unsupported (unknown kind or future version).
    Unsupported,
    /// Invalid(Payload) containing a contract error with this code.
    Payload(String),
}

struct Case {
    name: String,
    bytes: Vec<u8>,
    limits: InspectLimits,
    want: Want,
}

fn satisfies(outcome: &Inspection, want: &Want) -> bool {
    match (outcome, want) {
        (Inspection::Invalid(failures), Want::Failure(predicate)) => {
            failures.iter().any(predicate)
        }
        (Inspection::Unsupported { .. }, Want::Unsupported) => true,
        (Inspection::Invalid(failures), Want::Payload(code)) => failures.iter().any(|failure| {
            matches!(failure, Failure::Payload(errors) if errors.iter().any(|e| e.code == code.as_str()))
        }),
        _ => false,
    }
}

fn corpus() -> Vec<Case> {
    let payload = fixture(SONG, "minimal.valid.json");
    let cover = b"cover bytes".to_vec();
    let mut cases = Vec::new();
    let mut add = |name: &str, bytes: Vec<u8>, limits: InspectLimits, want: Want| {
        cases.push(Case {
            name: name.to_owned(),
            bytes,
            limits,
            want,
        });
    };
    let zip_of = |items: Vec<Item>| build_zip(&items, CompressionMethod::Deflated);
    let default = InspectLimits::default;

    add(
        "traversal-dotdot",
        zip_of(song_with(vec![file("../evil", b"x".to_vec())])),
        default(),
        Want::Failure(|f| matches!(f, Failure::UnsafePath { .. })),
    );
    add(
        "traversal-backslash",
        zip_of(song_with(vec![file("..\\evil", b"x".to_vec())])),
        default(),
        Want::Failure(|f| matches!(f, Failure::UnsafePath { .. })),
    );
    add(
        "absolute-path",
        zip_of(song_with(vec![file("/etc/x", b"x".to_vec())])),
        default(),
        Want::Failure(|f| matches!(f, Failure::UnsafePath { .. })),
    );
    add(
        "drive-prefix",
        zip_of(song_with(vec![file("C:/x", b"x".to_vec())])),
        default(),
        Want::Failure(|f| matches!(f, Failure::UnsafePath { .. })),
    );
    add(
        "nul-byte",
        zip_of(song_with(vec![file("evil\0x", b"x".to_vec())])),
        default(),
        Want::Failure(|f| matches!(f, Failure::UnsafePath { .. })),
    );
    // The manifest lists `assets/a.txt` with the hash of the surviving copy, so
    // the only defect is the duplicate record itself. The writer refuses
    // duplicate names, so write distinct names of equal length and rewrite the
    // second to the first in the raw bytes (local header and central directory).
    let dup_manifest = manifest(SONG, 1, &payload, &[("assets/a.txt", b"two")]);
    add(
        "duplicate-path",
        retarget(
            &zip_of(container(
                &dup_manifest,
                &payload,
                &[("assets/a.txt", b"one"), ("assets/b.txt", b"two")],
            )),
            b"assets/b.txt",
            b"assets/a.txt",
        ),
        default(),
        // The raw record count (4) exceeds the reader's unique count (3), so
        // the archive is refused as a count mismatch before any listing check.
        Want::Failure(|f| {
            matches!(
                f,
                Failure::EntryCountMismatch { .. } | Failure::DuplicatePath { .. }
            )
        }),
    );
    add(
        "windows-device-name",
        zip_of(song_with(vec![file("assets/CON.txt", b"x".to_vec())])),
        default(),
        Want::Failure(|f| matches!(f, Failure::UnsafePath { .. })),
    );
    add(
        "windows-trailing-dot",
        zip_of(song_with(vec![file("assets/name.", b"x".to_vec())])),
        default(),
        Want::Failure(|f| matches!(f, Failure::UnsafePath { .. })),
    );
    add(
        "case-duplicate-path",
        zip_of(song_with(vec![
            file("assets/A.txt", b"one".to_vec()),
            file("assets/a.txt", b"two".to_vec()),
        ])),
        default(),
        Want::Failure(|f| matches!(f, Failure::DuplicatePath { .. })),
    );
    add(
        "symlink-entry",
        zip_of(song_with(vec![Item::Link(
            "assets/link".to_owned(),
            "/etc/passwd".to_owned(),
        )])),
        default(),
        Want::Failure(|f| matches!(f, Failure::Link { .. })),
    );
    add(
        "missing-manifest",
        zip_of(vec![file("payload.json", payload.clone())]),
        default(),
        Want::Failure(|f| matches!(f, Failure::MissingManifest)),
    );
    add(
        "malformed-manifest-json",
        zip_of(vec![
            file("manifest.json", b"{not json".to_vec()),
            file("payload.json", payload.clone()),
        ]),
        default(),
        Want::Failure(|f| matches!(f, Failure::BadManifest { .. })),
    );
    let mut wrong_format = manifest(SONG, 1, &payload, &[]);
    wrong_format["format"] = json!("zip");
    add(
        "wrong-format",
        zip_of(container(&wrong_format, &payload, &[])),
        default(),
        Want::Failure(|f| matches!(f, Failure::BadManifest { .. })),
    );
    let mut future_format = manifest(SONG, 1, &payload, &[]);
    future_format["format_version"] = json!(2);
    add(
        "future-format-version",
        zip_of(container(&future_format, &payload, &[])),
        default(),
        Want::Unsupported,
    );
    let mut text_format = manifest(SONG, 1, &payload, &[]);
    text_format["format_version"] = json!("1");
    add(
        "text-format-version",
        zip_of(container(&text_format, &payload, &[])),
        default(),
        Want::Failure(|f| matches!(f, Failure::BadManifest { .. })),
    );
    add(
        "unknown-kind",
        zip_of(container(
            &manifest("mystery-kind", 1, &payload, &[]),
            &payload,
            &[],
        )),
        default(),
        Want::Unsupported,
    );
    add(
        "future-schema-version",
        zip_of(container(&manifest(SONG, 2, &payload, &[]), &payload, &[])),
        default(),
        Want::Unsupported,
    );
    add(
        "unlisted-entry",
        zip_of(song_with(vec![file("assets/extra.bin", b"extra".to_vec())])),
        default(),
        Want::Failure(|f| matches!(f, Failure::Unlisted { .. })),
    );
    let listed_missing = manifest(SONG, 1, &payload, &[("assets/cover.bin", &cover)]);
    add(
        "listed-missing-asset",
        zip_of(vec![
            file(
                "manifest.json",
                serde_json::to_vec(&listed_missing).unwrap(),
            ),
            file("payload.json", payload.clone()),
        ]),
        default(),
        Want::Failure(|f| matches!(f, Failure::Missing { .. })),
    );
    let mut tampered_payload = payload.clone();
    let flip = tampered_payload.len() - 2;
    tampered_payload[flip] ^= 0x01;
    add(
        "tampered-payload-hash",
        zip_of(container(
            &manifest(SONG, 1, &payload, &[]),
            &tampered_payload,
            &[],
        )),
        default(),
        Want::Failure(|f| matches!(f, Failure::Checksum { path, .. } if path == "payload.json")),
    );
    add(
        "tampered-asset-hash",
        zip_of(container(
            &manifest(SONG, 1, &payload, &[("assets/cover.bin", &cover)]),
            &payload,
            &[("assets/cover.bin", b"COVER BYTES")],
        )),
        default(),
        Want::Failure(
            |f| matches!(f, Failure::Checksum { path, .. } if path == "assets/cover.bin"),
        ),
    );
    let mut wrong_size = manifest(SONG, 1, &payload, &[]);
    wrong_size["payload"]["size"] = json!(payload.len() + 1);
    add(
        "size-mismatch",
        zip_of(container(&wrong_size, &payload, &[])),
        default(),
        Want::Failure(|f| matches!(f, Failure::SizeMismatch { .. })),
    );
    let bomb = vec![0u8; (50 * MIB) as usize];
    add(
        "zip-bomb",
        zip_of(container(
            &manifest(SONG, 1, &payload, &[("assets/bomb.bin", &bomb)]),
            &payload,
            &[("assets/bomb.bin", &bomb)],
        )),
        InspectLimits {
            max_ratio: 100,
            max_total: MIB,
            ..default()
        },
        Want::Failure(|f| matches!(f, Failure::RatioExceeded { .. })),
    );
    add(
        "too-many-entries",
        zip_of(container(
            &manifest(
                SONG,
                1,
                &payload,
                &[("assets/a.bin", b"a"), ("assets/b.bin", b"b")],
            ),
            &payload,
            &[("assets/a.bin", b"a"), ("assets/b.bin", b"b")],
        )),
        InspectLimits {
            max_entries: 3,
            ..default()
        },
        Want::Failure(|f| matches!(f, Failure::TooManyEntries { .. })),
    );
    add(
        "oversize-archive",
        zip_of(song_with(vec![])),
        InspectLimits {
            max_archive: 16,
            ..default()
        },
        Want::Failure(|f| matches!(f, Failure::ArchiveTooLarge { .. })),
    );

    // Payloads that fail their contract, wrapped in otherwise valid containers
    // with correct hashes. One case per song-import reject fixture; the code
    // comes from the fixture's sibling `.expect` file.
    let mut rejects: Vec<_> = fs::read_dir(Path::new(FIXTURES).join(SONG))
        .expect("song fixtures")
        .map(|entry| entry.expect("entry").path())
        .filter(|path| path.to_string_lossy().ends_with(".reject.json"))
        .collect();
    rejects.sort();
    assert!(!rejects.is_empty(), "song-import reject fixtures missing");
    for path in rejects {
        let stem = path.file_name().unwrap().to_string_lossy().into_owned();
        let expect = path
            .to_string_lossy()
            .replace(".reject.json", ".reject.expect");
        let code = fs::read_to_string(&expect)
            .unwrap_or_else(|e| panic!("missing {expect}: {e}"))
            .trim()
            .to_owned();
        let bytes = fs::read(&path).expect("reject fixture");
        add(
            &format!("payload-{stem}"),
            zip_of(container(&manifest(SONG, 1, &bytes, &[]), &bytes, &[])),
            default(),
            Want::Payload(code),
        );
    }
    cases
}

#[test]
fn every_kind_round_trips_through_create_and_inspect() {
    for kind in ArtifactKind::ALL {
        let name = kind.as_str();
        let payload_bytes = fixture(name, "minimal.valid.json");
        let payload: Value = serde_json::from_slice(&payload_bytes).expect("fixture json");
        let root = tempfile::tempdir().expect("temp dir");
        let dest = root.path().join(format!("{name}.ordo"));
        let note = b"small asset".to_vec();
        let written = create(
            &dest,
            kind,
            &payload,
            &[("assets/note.txt".to_owned(), note.clone())],
        )
        .unwrap_or_else(|e| panic!("{name}: create failed: {e}"));
        let staging = root.path().join(STAGING);
        fs::create_dir_all(&staging).expect("staging");

        match inspect(&dest, &staging, &InspectLimits::default()) {
            Inspection::Valid(routed) => {
                assert_eq!(routed.kind, kind, "{name}: routed kind");
                assert_eq!(routed.manifest.kind, kind, "{name}: manifest kind");
                assert_eq!(routed.manifest, written, "{name}: manifest round trip");
                assert_eq!(routed.payload, payload, "{name}: payload round trip");
                assert_eq!(routed.staged_assets.len(), 1, "{name}: staged assets");
                assert_eq!(fs::read(&routed.staged_assets[0]).unwrap(), note);
            }
            other => panic!("{name}: expected Valid, got {other:?}"),
        }
    }
}

#[test]
fn malformed_corpus_is_refused_without_escaping_staging() {
    let cases = corpus();
    assert!(cases.len() >= 25, "corpus shrank to {}", cases.len());
    for case in cases {
        let outcome = inspect_contained(&case.name, &case.bytes, &case.limits);
        // Invalid and NeedsDecision are distinct; the container layer must
        // never produce NeedsDecision.
        assert!(
            matches!(
                outcome,
                Inspection::Invalid(_) | Inspection::Unsupported { .. }
            ),
            "{}: expected Invalid or Unsupported, got {outcome:?}",
            case.name
        );
        assert!(
            satisfies(&outcome, &case.want),
            "{}: wrong refusal, got {outcome:?}",
            case.name
        );
    }
}

#[test]
fn unsupported_refusals_name_a_compatible_version() {
    let payload = fixture(SONG, "minimal.valid.json");
    let future = build_zip(
        &container(&manifest(SONG, 2, &payload, &[]), &payload, &[]),
        CompressionMethod::Deflated,
    );
    let unknown = build_zip(
        &container(&manifest("mystery-kind", 1, &payload, &[]), &payload, &[]),
        CompressionMethod::Deflated,
    );
    for (name, bytes) in [("future", future), ("unknown", unknown)] {
        match inspect_contained(name, &bytes, &InspectLimits::default()) {
            Inspection::Unsupported {
                kind,
                version,
                supported,
            } => {
                let message = unsupported_message(&kind, version, supported);
                assert!(
                    message.contains("requires an Ordo version supporting"),
                    "{name}: {message}"
                );
            }
            other => panic!("{name}: expected Unsupported, got {other:?}"),
        }
    }
}

#[test]
fn rejected_create_leaves_existing_destination_untouched() {
    let root = tempfile::tempdir().expect("temp dir");
    let dest = root.path().join("existing.ordo");
    fs::write(&dest, b"previous good bytes").expect("seed");
    let bad = json!({"not": "a song"});
    let error = create(&dest, ArtifactKind::SongImport, &bad, &[]).expect_err("must refuse");
    assert!(matches!(error, CreateError::Payload(_)), "{error}");
    assert_eq!(fs::read(&dest).unwrap(), b"previous good bytes");
    assert!(no_temp_siblings(root.path()));
}

#[test]
fn valid_create_replaces_existing_destination() {
    let root = tempfile::tempdir().expect("temp dir");
    let dest = root.path().join("existing.ordo");
    fs::write(&dest, b"previous bytes").expect("seed");
    let payload: Value =
        serde_json::from_slice(&fixture(SONG, "minimal.valid.json")).expect("fixture json");
    create(&dest, ArtifactKind::SongImport, &payload, &[]).expect("create over existing");
    assert_ne!(fs::read(&dest).unwrap(), b"previous bytes");
    let staging = root.path().join(STAGING);
    fs::create_dir_all(&staging).expect("staging");
    assert!(matches!(
        inspect(&dest, &staging, &InspectLimits::default()),
        Inspection::Valid(_)
    ));
    assert!(no_temp_siblings(root.path()));
}

fn no_temp_siblings(dir: &Path) -> bool {
    fs::read_dir(dir)
        .expect("read dir")
        .map(|entry| {
            entry
                .expect("entry")
                .file_name()
                .to_string_lossy()
                .into_owned()
        })
        .all(|name| !name.contains(".tmp-"))
}
