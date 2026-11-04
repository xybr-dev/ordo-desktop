//! The committed Normal Mass and Recovery Drill snapshots, read through the
//! public snapshot reader.
//!
//! The expected values below are hard-coded on purpose: a fixture edit that
//! forgets to update this file, or a reader change that silently reorders or
//! reclassifies Cues, fails here instead of passing by agreement with itself.

use std::collections::BTreeMap;
use std::fs;
use std::path::{Path, PathBuf};

use domain::presentation::{ContentHash, CueBehavior, CueId, FrameId, ProgramEffect, ThemeKind};
use presentation::snapshot_reader::{
    Dependency, DependencyId, DirectorySource, LoadedSnapshot, MAX_MANIFEST_BYTES,
    SnapshotLoadError, read_snapshot,
};
use rendering::{TargetDensity, Theme, render_template};

const SECTIONS: [&str; 6] = [
    "Before Mass",
    "Introductory Rites",
    "Liturgy of the Word",
    "Liturgy of the Eucharist",
    "Concluding Rites",
    "After Mass",
];

const NORMAL_MASS_FINGERPRINT: &str =
    "349081bb45f7adf2c66ad22cf3071fe3afcf598e67d99cf422cf4bcc14c1c233";
const RECOVERY_DRILL_FINGERPRINT: &str =
    "0a8b2474c81b7b466a321916c7af0058b0244ad7907ba5ee2fb82aa780b39cc4";

// The fingerprint covers frame and asset bytes only. The manifest hash is the
// SHA-256 of the exact `snapshot.json` bytes, so these pin every identity, title,
// Cue behavior, and the order of sections, items, and Cues as well.
const NORMAL_MASS_MANIFEST_HASH: &str =
    "522dc9e5a53d6327c66e4763393a6f317d1d392e3dda870d29a6bf15ec6a88ec";
const RECOVERY_DRILL_MANIFEST_HASH: &str =
    "79124c7bd4ae9766478d8aebabff93234e037d8e181f95313df508afe03d5c8b";

fn fixture_dir(name: &str) -> PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("../fixtures/presentations")
        .join(name)
}

fn load_from(dir: &Path) -> Result<LoadedSnapshot, SnapshotLoadError> {
    read_snapshot(&DirectorySource::new(dir))
}

fn load(name: &str) -> LoadedSnapshot {
    load_from(&fixture_dir(name)).unwrap_or_else(|error| panic!("{name} must load: {error}"))
}

fn cue_id(text: &str) -> CueId {
    text.parse().unwrap()
}

fn frame_id(text: &str) -> FrameId {
    text.parse().unwrap()
}

fn hash(text: &str) -> ContentHash {
    ContentHash::parse(text).unwrap()
}

/// The classification of a Cue's Program effect, without the frame identity.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
enum Effect {
    Frame,
    Black,
    Unchanged,
    NotPresentable,
}

fn effect_of(effect: ProgramEffect) -> Effect {
    match effect {
        ProgramEffect::Frame(_) => Effect::Frame,
        ProgramEffect::Black => Effect::Black,
        ProgramEffect::Unchanged => Effect::Unchanged,
        ProgramEffect::NotPresentable => Effect::NotPresentable,
    }
}

fn section_titles(loaded: &LoadedSnapshot) -> Vec<&str> {
    loaded
        .snapshot()
        .sections()
        .iter()
        .map(|section| section.title())
        .collect()
}

fn behaviors(loaded: &LoadedSnapshot) -> Vec<&CueBehavior> {
    let snapshot = loaded.snapshot();
    (0..snapshot.cue_count())
        .map(|flat| snapshot.cue(flat).unwrap().behavior())
        .collect()
}

/// How many Cues show each frame.
fn frame_uses(loaded: &LoadedSnapshot) -> BTreeMap<FrameId, usize> {
    let mut uses = BTreeMap::new();
    for behavior in behaviors(loaded) {
        if let CueBehavior::Project { frame, .. } = behavior {
            *uses.entry(*frame).or_insert(0) += 1;
        }
    }
    uses
}

fn assert_both_themes_and_an_override(loaded: &LoadedSnapshot) {
    let kinds: Vec<ThemeKind> = loaded.snapshot().themes().iter().map(|t| t.kind).collect();
    assert!(kinds.contains(&ThemeKind::Custom));
    assert!(kinds.contains(&ThemeKind::Fallback));
    assert!(
        loaded
            .snapshot()
            .sections()
            .iter()
            .flat_map(|section| section.items())
            .any(|item| item.theme_override().is_some()),
        "at least one item overrides the Mass Theme"
    );
}

/// flat, section, item-in-section, Cue identity, Program effect.
const RECOVERY_DRILL_ORDER: [(usize, usize, usize, &str, Effect); 18] = [
    (
        0,
        0,
        0,
        "b4000000-0000-4000-8000-000000000001",
        Effect::Frame,
    ),
    (
        1,
        0,
        0,
        "b4000000-0000-4000-8000-000000000002",
        Effect::Frame,
    ),
    (
        2,
        0,
        0,
        "b4000000-0000-4000-8000-000000000003",
        Effect::NotPresentable,
    ),
    (
        3,
        0,
        0,
        "b4000000-0000-4000-8000-000000000004",
        Effect::Frame,
    ),
    (
        4,
        1,
        0,
        "b4000000-0000-4000-8000-000000000005",
        Effect::Black,
    ),
    (
        5,
        1,
        0,
        "b4000000-0000-4000-8000-000000000006",
        Effect::Frame,
    ),
    (
        6,
        1,
        1,
        "b4000000-0000-4000-8000-000000000007",
        Effect::Black,
    ),
    (
        7,
        1,
        1,
        "b4000000-0000-4000-8000-000000000008",
        Effect::Black,
    ),
    (
        8,
        2,
        0,
        "b4000000-0000-4000-8000-000000000009",
        Effect::Frame,
    ),
    (
        9,
        2,
        0,
        "b4000000-0000-4000-8000-00000000000a",
        Effect::Unchanged,
    ),
    (
        10,
        2,
        0,
        "b4000000-0000-4000-8000-00000000000b",
        Effect::Frame,
    ),
    (
        11,
        3,
        0,
        "b4000000-0000-4000-8000-00000000000c",
        Effect::Frame,
    ),
    (
        12,
        3,
        0,
        "b4000000-0000-4000-8000-00000000000d",
        Effect::Frame,
    ),
    (
        13,
        4,
        0,
        "b4000000-0000-4000-8000-00000000000e",
        Effect::Frame,
    ),
    (
        14,
        4,
        0,
        "b4000000-0000-4000-8000-00000000000f",
        Effect::Frame,
    ),
    (
        15,
        5,
        0,
        "b4000000-0000-4000-8000-000000000010",
        Effect::Frame,
    ),
    (
        16,
        5,
        0,
        "b4000000-0000-4000-8000-000000000011",
        Effect::Frame,
    ),
    (
        17,
        5,
        0,
        "b4000000-0000-4000-8000-000000000012",
        Effect::Frame,
    ),
];

#[test]
fn recovery_drill_flattens_in_the_documented_order() {
    let loaded = load("recovery-drill");
    let snapshot = loaded.snapshot();

    assert_eq!(section_titles(&loaded), SECTIONS);
    assert_eq!(snapshot.item_count(), 7);
    assert_eq!(snapshot.cue_count(), 18);
    assert_eq!(snapshot.positions().len(), RECOVERY_DRILL_ORDER.len());

    for (position, expected) in snapshot.positions().iter().zip(RECOVERY_DRILL_ORDER) {
        let (flat, section, item, cue, effect) = expected;
        assert_eq!(position.flat, flat);
        assert_eq!(position.section, section, "section of Cue {flat}");
        assert_eq!(position.item, item, "item of Cue {flat}");
        assert_eq!(position.cue_id, cue_id(cue), "identity of Cue {flat}");
        assert_eq!(
            effect_of(snapshot.cue(flat).unwrap().program_effect()),
            effect,
            "effect of Cue {flat}"
        );
    }
}

#[test]
fn recovery_drill_covers_every_cue_behavior_and_its_adversarial_controls() {
    let loaded = load("recovery-drill");
    let snapshot = loaded.snapshot();
    let all = behaviors(&loaded);

    assert!(all.iter().any(|b| matches!(b, CueBehavior::Project { .. })));
    assert!(
        all.iter()
            .any(|b| matches!(b, CueBehavior::OperatorTrigger))
    );
    assert!(all.iter().any(|b| matches!(b, CueBehavior::Hold)));
    assert!(all.iter().any(|b| matches!(b, CueBehavior::Wait)));
    assert!(
        all.iter()
            .any(|b| matches!(b, CueBehavior::Unavailable { .. }))
    );

    assert_both_themes_and_an_override(&loaded);
    assert!(frame_uses(&loaded).values().any(|uses| *uses >= 2));

    // The one unavailable Cue sits between two projectable Cues, so Freeze
    // protection has a valid Cue on each side to navigate to.
    let unavailable: Vec<usize> = (0..snapshot.cue_count())
        .filter(|flat| !snapshot.cue(*flat).unwrap().is_presentable())
        .collect();
    assert_eq!(unavailable, [2]);
    for neighbour in [1, 3] {
        assert!(matches!(
            snapshot.cue(neighbour).unwrap().behavior(),
            CueBehavior::Project { .. }
        ));
    }

    let items = || {
        snapshot
            .sections()
            .iter()
            .flat_map(|section| section.items())
    };
    // One item has only operator-only Cues (trigger and hold).
    assert!(items().any(|item| {
        item.cues().len() == 2
            && matches!(item.cues()[0].behavior(), CueBehavior::OperatorTrigger)
            && matches!(item.cues()[1].behavior(), CueBehavior::Hold)
    }));
    // An auto-play item with at least two Cues.
    assert!(items().any(|item| item.auto_play().is_some() && item.cues().len() >= 2));
}

#[test]
fn recovery_drill_has_a_stable_fingerprint_and_hashes() {
    let loaded = load("recovery-drill");
    assert_eq!(loaded.fingerprint(), hash(RECOVERY_DRILL_FINGERPRINT));
    assert_eq!(loaded.manifest_hash(), hash(RECOVERY_DRILL_MANIFEST_HASH));
    assert_eq!(
        loaded.frame_hash(frame_id("b5000000-0000-4000-8000-000000000001")),
        Some(hash(
            "9462c3c6e8279f87886c4d9a50f0646d99f27b51f4a46d09f50758bf0909574f"
        ))
    );
    assert_eq!(
        loaded.frame_hash(frame_id("b5000000-0000-4000-8000-000000000004")),
        Some(hash(
            "a670036fc80f0eed08e9be2c954bece1e026f9df71c08891d44591a2bf3dc4f6"
        ))
    );
    assert_eq!(
        loaded.asset_hash("b6000000-0000-4000-8000-000000000001".parse().unwrap()),
        Some(hash(
            "877a3c4794dfc0253c6116a4b8f4cd463b2c88927af61e76f440456551ea1907"
        ))
    );
}

#[test]
fn normal_mass_has_the_sunday_structure() {
    let loaded = load("normal-mass");
    let snapshot = loaded.snapshot();

    assert_eq!(section_titles(&loaded), SECTIONS);
    assert_eq!(snapshot.item_count(), 15);
    assert_eq!(snapshot.cue_count(), 33);

    let mut per_section = [0usize; 6];
    for position in snapshot.positions() {
        per_section[position.section] += 1;
    }
    assert_eq!(per_section, [4, 9, 7, 7, 4, 2]);

    // Selected positions: a trigger, the Hold, the Wait, and the last Cue.
    let cue = |flat: usize| snapshot.cue(flat).unwrap();
    assert_eq!(cue(0).label(), "Celebration title");
    assert_eq!(cue(4).label(), "Song identification");
    assert_eq!(cue(10).label(), "Presider greets the assembly");
    assert!(matches!(cue(10).behavior(), CueBehavior::OperatorTrigger));
    assert!(matches!(cue(12).behavior(), CueBehavior::Hold));
    assert!(matches!(cue(17).behavior(), CueBehavior::Wait));
    assert_eq!(
        cue(17).private_context(),
        Some("Cantor sings the verse unprojected.")
    );
    assert_eq!(cue(32).label(), "Next gathering");

    assert!(
        behaviors(&loaded)
            .iter()
            .all(|b| !matches!(b, CueBehavior::Unavailable { .. })),
        "normal-mass has no unavailable Cue"
    );
    assert_both_themes_and_an_override(&loaded);
    // The repeated chorus is one frame shown by three Cues.
    assert_eq!(
        frame_uses(&loaded).get(&frame_id("a5000000-0000-4000-8000-000000000006")),
        Some(&3)
    );
}

#[test]
fn normal_mass_has_a_stable_fingerprint_and_hashes() {
    let loaded = load("normal-mass");
    assert_eq!(loaded.fingerprint(), hash(NORMAL_MASS_FINGERPRINT));
    assert_eq!(loaded.manifest_hash(), hash(NORMAL_MASS_MANIFEST_HASH));
    assert_eq!(
        loaded.frame_hash(frame_id("a5000000-0000-4000-8000-000000000001")),
        Some(hash(
            "2c86d94edde06f4c7bca782611c82e5c8d91f4ab6bdcfc07e3a0bb393d311ef5"
        ))
    );
    assert_eq!(
        loaded.asset_hash("a6000000-0000-4000-8000-000000000001".parse().unwrap()),
        Some(hash(
            "010772f21685d07137dc4d24b5334af28ac5795852325d99253073aa396b79de"
        ))
    );
}

#[test]
fn the_recovery_drill_command_script_uses_only_known_commands() {
    const KNOWN: [&str; 18] = [
        "next",
        "previous",
        "next-item",
        "previous-item",
        "first",
        "last",
        "take",
        "freeze",
        "blackout",
        "enable-output",
        "disable-output",
        "disconnect",
        "display-back",
        "display-back-other",
        "autoplay",
        "loop-section",
        "stop-autoplay",
        "recover",
    ];
    // `tick <milliseconds>` lets time pass instead of issuing a command.
    let known = |line: &str| {
        KNOWN.contains(&line)
            || line
                .strip_prefix("tick ")
                .is_some_and(|ms| !ms.is_empty() && ms.bytes().all(|b| b.is_ascii_digit()))
    };
    let script = fs::read_to_string(fixture_dir("recovery-drill").join("commands.txt")).unwrap();
    assert!(!script.is_empty());
    assert!(script.ends_with('\n') && !script.contains('\r'));
    // `#` lines are comments; every other line is one command.
    let commands: Vec<&str> = script
        .lines()
        .filter(|line| !line.starts_with('#'))
        .collect();
    for (index, line) in script.lines().enumerate() {
        assert!(
            line.starts_with('#') || known(line),
            "commands.txt line {} is not a known command: {line:?}",
            index + 1
        );
    }
    for required in [
        "enable-output",
        "take",
        "freeze",
        "blackout",
        "autoplay",
        "disconnect",
        "last",
        "first",
    ] {
        assert!(commands.contains(&required), "{required}");
    }
}

/// Every long frame must compose to more rendered lines than it has source
/// lines (it wraps) and must do so without a ceiling warning, so the wrapping
/// controls really exercise the wrapping path.
fn assert_wraps_without_warnings(loaded: &LoadedSnapshot, frame: &str, name: &str) {
    let content = loaded.frame(frame_id(frame)).expect(name);
    let (composed, diagnostics) =
        render_template(content, &Theme::default(), TargetDensity::SCALE_100)
            .unwrap_or_else(|error| panic!("{name} must compose: {error}"));
    let source_lines = content.body.lines().count();
    assert!(
        composed.body_lines().len() > source_lines,
        "{name}: {} rendered lines from {source_lines} source lines",
        composed.body_lines().len()
    );
    assert!(
        diagnostics.warnings.is_empty(),
        "{name}: {:?}",
        diagnostics.warnings
    );
}

#[test]
fn the_long_frames_wrap_without_warnings() {
    let drill = load("recovery-drill");
    assert_wraps_without_warnings(
        &drill,
        "b5000000-0000-4000-8000-000000000006",
        "recovery-drill long lyric",
    );
    let mass = load("normal-mass");
    assert_wraps_without_warnings(
        &mass,
        "a5000000-0000-4000-8000-00000000000b",
        "normal-mass reading passage",
    );
    assert_wraps_without_warnings(
        &mass,
        "a5000000-0000-4000-8000-000000000011",
        "normal-mass communion verse",
    );
}

#[test]
fn operator_only_cues_carry_their_private_context() {
    let loaded = load("recovery-drill");
    let snapshot = loaded.snapshot();
    // Flat 4 and 6 are triggers and 7 is the Hold: all Black on the Program
    // output, and each must give the operator something to read.
    for flat in [4, 6, 7] {
        let cue = snapshot.cue(flat).unwrap();
        assert_eq!(cue.program_effect(), ProgramEffect::Black, "Cue {flat}");
        assert!(cue.private_context().is_some(), "Cue {flat}");
    }
    // The Wait leaves the Program output unchanged and also explains itself.
    assert!(snapshot.cue(9).unwrap().private_context().is_some());
}

#[test]
fn no_fixture_text_has_carriage_returns_or_a_missing_final_lf() {
    for name in ["normal-mass", "recovery-drill"] {
        let dir = fixture_dir(name);
        let mut files = vec![dir.join("snapshot.json")];
        for entry in fs::read_dir(dir.join("frames")).unwrap() {
            files.push(entry.unwrap().path());
        }
        for file in files {
            let bytes = fs::read(&file).unwrap();
            assert!(!bytes.contains(&b'\r'), "{}", file.display());
            assert!(bytes.ends_with(b"\n"), "{}", file.display());
        }
    }
}

#[test]
fn a_symbolic_link_is_refused_not_followed() {
    let copy = copy_fixture("recovery-drill");
    let frame = copy.path().join("frames/title.txt");
    let moved = copy.path().join("frames/title-real.txt");
    fs::rename(&frame, &moved).unwrap();
    #[cfg(unix)]
    let linked = std::os::unix::fs::symlink(&moved, &frame);
    #[cfg(windows)]
    let linked = std::os::windows::fs::symlink_file(&moved, &frame);
    if linked.is_err() {
        // Creating a symbolic link needs a privilege some Windows accounts lack.
        eprintln!("skipped: cannot create a symbolic link here");
        return;
    }
    let error = load_from(copy.path()).unwrap_err();
    assert!(
        matches!(
            error,
            SnapshotLoadError::RefusedDependency {
                dependency: Dependency {
                    id: DependencyId::Frame(_),
                    ..
                },
                ..
            }
        ),
        "{error:?}"
    );
}

/// A scratch copy of one fixture, so a test can damage it freely.
fn copy_fixture(name: &str) -> tempfile::TempDir {
    fn copy_dir(from: &Path, to: &Path) {
        fs::create_dir_all(to).unwrap();
        for entry in fs::read_dir(from).unwrap() {
            let entry = entry.unwrap();
            let target = to.join(entry.file_name());
            if entry.file_type().unwrap().is_dir() {
                copy_dir(&entry.path(), &target);
            } else {
                fs::copy(entry.path(), target).unwrap();
            }
        }
    }
    let copy = tempfile::tempdir().unwrap();
    copy_dir(&fixture_dir(name), copy.path());
    copy
}

fn flip_first_byte(path: &Path) {
    let mut bytes = fs::read(path).unwrap();
    bytes[0] ^= 0x01;
    fs::write(path, bytes).unwrap();
}

#[test]
fn an_unmodified_copy_still_loads() {
    for name in ["normal-mass", "recovery-drill"] {
        let copy = copy_fixture(name);
        assert!(load_from(copy.path()).is_ok(), "{name}");
    }
}

#[test]
fn a_deleted_frame_file_is_rejected_as_missing() {
    for (name, first_frame) in [
        ("normal-mass", "frames/celebration-title.txt"),
        ("recovery-drill", "frames/title.txt"),
    ] {
        let copy = copy_fixture(name);
        fs::remove_file(copy.path().join(first_frame)).unwrap();
        let error = load_from(copy.path()).unwrap_err();
        assert!(
            matches!(
                error,
                SnapshotLoadError::MissingDependency {
                    dependency: Dependency {
                        id: DependencyId::Frame(_),
                        ..
                    }
                }
            ),
            "{name}: {error:?}"
        );
    }
}

#[test]
fn a_changed_frame_byte_is_rejected_as_a_hash_mismatch() {
    let copy = copy_fixture("recovery-drill");
    flip_first_byte(&copy.path().join("frames/text-a.txt"));
    let error = load_from(copy.path()).unwrap_err();
    assert!(
        matches!(
            error,
            SnapshotLoadError::HashMismatch {
                dependency: Dependency {
                    id: DependencyId::Frame(_),
                    ..
                },
                ..
            }
        ),
        "{error:?}"
    );
}

#[test]
fn a_changed_asset_byte_is_rejected_as_a_hash_mismatch() {
    let copy = copy_fixture("recovery-drill");
    flip_first_byte(&copy.path().join("assets/drill-image.png"));
    let error = load_from(copy.path()).unwrap_err();
    assert!(
        matches!(
            error,
            SnapshotLoadError::HashMismatch {
                dependency: Dependency {
                    id: DependencyId::Asset(_),
                    ..
                },
                ..
            }
        ),
        "{error:?}"
    );
}

#[test]
fn an_oversized_manifest_is_refused_by_the_directory_source() {
    let copy = copy_fixture("recovery-drill");
    let manifest = copy.path().join("snapshot.json");
    // One byte over the cap: refused from its length, whatever it holds.
    fs::write(&manifest, vec![b' '; MAX_MANIFEST_BYTES + 1]).unwrap();
    let error = load_from(copy.path()).unwrap_err();
    assert!(
        matches!(error, SnapshotLoadError::RefusedManifest { .. }),
        "{error:?}"
    );

    // Exactly at the cap it is read and then judged as the bad JSON it is.
    fs::write(&manifest, vec![b'\n'; MAX_MANIFEST_BYTES]).unwrap();
    let error = load_from(copy.path()).unwrap_err();
    assert!(
        matches!(error, SnapshotLoadError::Manifest { .. }),
        "{error:?}"
    );
}
