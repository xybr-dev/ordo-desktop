use std::collections::BTreeMap;
use std::fmt::Write as _;

use domain::presentation::{
    AutoPlay, CueBehavior, CueId, MassItemId, MassItemKind, MassSectionId, SnapshotId, ThemeId,
    ThemeKind, ThemeRef,
};

use super::*;

fn uuid(n: u16) -> String {
    format!("00000000-0000-4000-8000-{n:012x}")
}

fn snapshot_id() -> SnapshotId {
    uuid(1).parse().unwrap()
}

fn custom_theme() -> ThemeId {
    uuid(0x101).parse().unwrap()
}

fn fallback_theme() -> ThemeId {
    uuid(0x102).parse().unwrap()
}

fn frame_id() -> FrameId {
    uuid(0x201).parse().unwrap()
}

fn image_frame_id() -> FrameId {
    uuid(0x202).parse().unwrap()
}

fn asset_id() -> AssetId {
    uuid(0x301).parse().unwrap()
}

fn section_id() -> MassSectionId {
    uuid(0x401).parse().unwrap()
}

fn item_id() -> MassItemId {
    uuid(0x501).parse().unwrap()
}

fn cue_id() -> CueId {
    uuid(0x601).parse().unwrap()
}

fn text_frame(body: &str) -> String {
    format!(
        "ordo-fixture/1\nkind:GeneralText\nheader-absent\nbody-lines:1\n{body}\nfooter-absent\nimage-absent\n"
    )
}

fn image_frame(asset: &str) -> String {
    format!(
        "ordo-fixture/1\nkind:GeneralImage\nheader:Image header\nbody-lines:0\nfooter-lines:1\nFixture caption\nimage:{asset}\nimage-size:2x2\n"
    )
}

/// An in-memory source: these tests never touch the filesystem.
#[derive(Default)]
struct MemorySource {
    files: BTreeMap<String, Vec<u8>>,
}

impl MemorySource {
    fn insert(&mut self, path: &str, bytes: impl Into<Vec<u8>>) {
        self.files.insert(path.to_owned(), bytes.into());
    }
}

impl SnapshotSource for MemorySource {
    fn read(&self, path: &str) -> Result<Vec<u8>, SourceError> {
        self.files.get(path).cloned().ok_or(SourceError::NotFound)
    }
}

#[derive(Clone)]
struct FrameSpec {
    id: FrameId,
    path: String,
    text: String,
}

#[derive(Clone)]
struct AssetSpec {
    id: AssetId,
    path: String,
    bytes: Vec<u8>,
}

#[derive(Clone)]
struct CueSpec {
    id: CueId,
    label: String,
    private_context: Option<String>,
    behavior: CueBehavior,
}

/// A Cue with private context, valid for every behavior.
fn operator_cue(n: u16, label: &str, behavior: CueBehavior) -> CueSpec {
    CueSpec {
        id: uuid(n).parse().unwrap(),
        label: label.to_owned(),
        private_context: Some("Operator note".to_owned()),
        behavior,
    }
}

#[derive(Clone)]
struct ItemSpec {
    id: MassItemId,
    title: String,
    kind: MassItemKind,
    theme: Option<ThemeId>,
    auto_play: Option<u32>,
    cues: Vec<CueSpec>,
}

#[derive(Clone)]
struct SectionSpec {
    id: MassSectionId,
    title: String,
    items: Vec<ItemSpec>,
}

/// A whole manifest as a value, so each test mutates the one fact it is
/// about and the reader still sees a self-consistent snapshot. Hashes and
/// the fingerprint are derived, never typed by hand.
#[derive(Clone)]
struct Spec {
    snapshot: SnapshotId,
    title: String,
    themes: Vec<ThemeRef>,
    mass_theme: ThemeId,
    frames: Vec<FrameSpec>,
    assets: Vec<AssetSpec>,
    sections: Vec<SectionSpec>,
}

impl Spec {
    /// Two items covering every Cue behavior, a text frame, and an image
    /// frame that uses the one declared asset.
    fn minimal() -> Self {
        Self {
            snapshot: snapshot_id(),
            title: "Minimal".to_owned(),
            themes: vec![
                ThemeRef {
                    id: custom_theme(),
                    name: "Parish Paschal".to_owned(),
                    kind: ThemeKind::Custom,
                },
                ThemeRef {
                    id: fallback_theme(),
                    name: "Ordo fallback".to_owned(),
                    kind: ThemeKind::Fallback,
                },
            ],
            mass_theme: custom_theme(),
            frames: vec![
                FrameSpec {
                    id: frame_id(),
                    path: "frames/one.txt".to_owned(),
                    text: text_frame("Hello from the fixture"),
                },
                FrameSpec {
                    id: image_frame_id(),
                    path: "frames/image.txt".to_owned(),
                    text: image_frame(&asset_id().to_string()),
                },
            ],
            assets: vec![AssetSpec {
                id: asset_id(),
                path: "assets/pixel.png".to_owned(),
                bytes: vec![0x89, b'P', b'N', b'G'],
            }],
            sections: vec![SectionSpec {
                id: section_id(),
                title: "Before Mass".to_owned(),
                items: vec![
                    ItemSpec {
                        id: item_id(),
                        title: "Announcements".to_owned(),
                        kind: MassItemKind::GeneralPresentation,
                        theme: None,
                        auto_play: Some(8),
                        cues: vec![
                            CueSpec {
                                id: cue_id(),
                                label: "Slide one".to_owned(),
                                private_context: Some("Said only to the operator".to_owned()),
                                behavior: CueBehavior::Project {
                                    frame: frame_id(),
                                    duration_seconds: Some(3),
                                },
                            },
                            operator_cue(
                                0x602,
                                "Image slide",
                                CueBehavior::Project {
                                    frame: image_frame_id(),
                                    duration_seconds: None,
                                },
                            ),
                        ],
                    },
                    ItemSpec {
                        id: uuid(0x502).parse().unwrap(),
                        title: "Rite".to_owned(),
                        kind: MassItemKind::Rite,
                        theme: Some(fallback_theme()),
                        auto_play: None,
                        cues: vec![
                            operator_cue(0x603, "Trigger", CueBehavior::OperatorTrigger),
                            operator_cue(0x604, "Hold", CueBehavior::Hold),
                            operator_cue(0x605, "Wait", CueBehavior::Wait),
                            operator_cue(
                                0x606,
                                "Missing",
                                CueBehavior::Unavailable {
                                    reason: "Image could not be captured".to_owned(),
                                },
                            ),
                        ],
                    },
                ],
            }],
        }
    }

    fn hashes(&self) -> impl Iterator<Item = ContentHash> + '_ {
        let frames = self
            .frames
            .iter()
            .map(|frame| hash_bytes(frame.text.as_bytes()));
        let assets = self.assets.iter().map(|asset| hash_bytes(&asset.bytes));
        frames.chain(assets)
    }

    fn fingerprint(&self) -> ContentHash {
        fingerprint_of(self.hashes())
    }

    fn manifest(&self) -> String {
        let mut out = String::new();
        writeln!(out, "{MANIFEST_FORMAT}").unwrap();
        writeln!(out, "snapshot {}", self.snapshot).unwrap();
        writeln!(out, "title {}", self.title).unwrap();
        writeln!(out, "fingerprint {}", self.fingerprint()).unwrap();
        for theme in &self.themes {
            writeln!(out, "theme {}", theme.id).unwrap();
            writeln!(out, "theme-name {}", theme.name).unwrap();
            let kind = match theme.kind {
                ThemeKind::Custom => "Custom",
                ThemeKind::Fallback => "Fallback",
            };
            writeln!(out, "theme-kind {kind}").unwrap();
        }
        writeln!(out, "mass-theme {}", self.mass_theme).unwrap();
        for frame in &self.frames {
            writeln!(out, "frame {}", frame.id).unwrap();
            writeln!(out, "frame-path {}", frame.path).unwrap();
            writeln!(out, "frame-sha256 {}", hash_bytes(frame.text.as_bytes())).unwrap();
        }
        for asset in &self.assets {
            writeln!(out, "asset {}", asset.id).unwrap();
            writeln!(out, "asset-path {}", asset.path).unwrap();
            writeln!(out, "asset-size {}", asset.bytes.len()).unwrap();
            writeln!(out, "asset-sha256 {}", hash_bytes(&asset.bytes)).unwrap();
        }
        for section in &self.sections {
            writeln!(out, "section {}", section.id).unwrap();
            writeln!(out, "section-title {}", section.title).unwrap();
            for item in &section.items {
                writeln!(out, "item {}", item.id).unwrap();
                writeln!(out, "item-title {}", item.title).unwrap();
                let kind = match item.kind {
                    MassItemKind::Song => "Song",
                    MassItemKind::Reading => "Reading",
                    MassItemKind::Psalm => "Psalm",
                    MassItemKind::ResponseSequence => "ResponseSequence",
                    MassItemKind::GeneralPresentation => "GeneralPresentation",
                    MassItemKind::Rite => "Rite",
                };
                writeln!(out, "item-kind {kind}").unwrap();
                if let Some(theme) = item.theme {
                    writeln!(out, "item-theme {theme}").unwrap();
                }
                if let Some(seconds) = item.auto_play {
                    writeln!(out, "item-auto-play {seconds}").unwrap();
                }
                for cue in &item.cues {
                    writeln!(out, "cue {}", cue.id).unwrap();
                    writeln!(out, "cue-label {}", cue.label).unwrap();
                    if let Some(context) = &cue.private_context {
                        writeln!(out, "cue-private-context {context}").unwrap();
                    }
                    match &cue.behavior {
                        CueBehavior::Project {
                            frame,
                            duration_seconds,
                        } => {
                            writeln!(out, "cue-behavior Project").unwrap();
                            writeln!(out, "cue-frame {frame}").unwrap();
                            if let Some(seconds) = duration_seconds {
                                writeln!(out, "cue-duration {seconds}").unwrap();
                            }
                        }
                        CueBehavior::OperatorTrigger => {
                            writeln!(out, "cue-behavior OperatorTrigger").unwrap();
                        }
                        CueBehavior::Hold => writeln!(out, "cue-behavior Hold").unwrap(),
                        CueBehavior::Wait => writeln!(out, "cue-behavior Wait").unwrap(),
                        CueBehavior::Unavailable { reason } => {
                            writeln!(out, "cue-behavior Unavailable").unwrap();
                            writeln!(out, "cue-reason {reason}").unwrap();
                        }
                    }
                }
            }
        }
        out
    }

    fn source(&self) -> MemorySource {
        let mut source = MemorySource::default();
        source.insert(MANIFEST_PATH, self.manifest());
        for frame in &self.frames {
            source.insert(&frame.path, frame.text.clone());
        }
        for asset in &self.assets {
            source.insert(&asset.path, asset.bytes.clone());
        }
        source
    }

    fn load(&self) -> Result<LoadedSnapshot, SnapshotLoadError> {
        read_snapshot(&self.source())
    }
}

#[test]
fn a_minimal_snapshot_loads_every_verified_value() {
    let spec = Spec::minimal();
    let loaded = spec.load().unwrap();

    let snapshot = loaded.snapshot();
    assert_eq!(snapshot.id(), snapshot_id());
    assert_eq!(snapshot.title(), "Minimal");
    assert_eq!(snapshot.item_count(), 2);
    assert_eq!(snapshot.cue_count(), 6);
    assert_eq!(snapshot.sections().len(), 1);
    assert_eq!(snapshot.sections()[0].title(), "Before Mass");
    assert_eq!(snapshot.mass_theme().id, custom_theme());

    let item = &snapshot.sections()[0].items()[0];
    assert_eq!(item.kind(), MassItemKind::GeneralPresentation);
    assert_eq!(item.auto_play(), Some(AutoPlay { default_seconds: 8 }));
    assert_eq!(snapshot.effective_theme(0).unwrap().id, custom_theme());

    let cue = snapshot.cue(0).unwrap();
    assert_eq!(cue.label(), "Slide one");
    assert_eq!(cue.private_context(), Some("Said only to the operator"));
    assert_eq!(
        cue.program_effect(),
        domain::presentation::ProgramEffect::Frame(frame_id())
    );
    assert_eq!(
        cue.behavior(),
        &CueBehavior::Project {
            frame: frame_id(),
            duration_seconds: Some(3),
        }
    );

    assert_eq!(
        loaded.frame(frame_id()).unwrap().body,
        "Hello from the fixture"
    );
    assert_eq!(
        loaded.frame_hash(frame_id()),
        Some(hash_bytes(spec.frames[0].text.as_bytes()))
    );
    assert_eq!(
        loaded
            .snapshot()
            .frame_ids()
            .into_iter()
            .collect::<Vec<_>>(),
        vec![frame_id(), image_frame_id()]
    );
    assert_eq!(
        loaded.manifest_hash(),
        hash_bytes(spec.manifest().as_bytes())
    );
    assert_eq!(
        loaded.asset(asset_id()),
        Some(spec.assets[0].bytes.as_slice())
    );
    assert_eq!(
        loaded.asset_hash(asset_id()),
        Some(hash_bytes(&spec.assets[0].bytes))
    );
    assert_eq!(loaded.asset_ids().collect::<Vec<_>>(), vec![asset_id()]);
    assert_eq!(loaded.fingerprint(), spec.fingerprint());
}

#[test]
fn the_fingerprint_follows_the_contract_rule() {
    // No dependencies: the SHA-256 of an empty digest stream.
    assert_eq!(
        fingerprint_of([]).to_string(),
        "e3b0c44298fc1c149afbf4c8996fb92427ae41e4649b934ca495991b7852b855"
    );

    let one = hash_bytes(b"one");
    let two = hash_bytes(b"two");
    let mut sorted = vec![one.to_string(), two.to_string()];
    sorted.sort_unstable();
    let mut expected = Sha256::new();
    for hex in &sorted {
        expected.update(hex.as_bytes());
        expected.update(b"\n");
    }
    assert_eq!(
        fingerprint_of([two, one]),
        fingerprint_of([one, two]),
        "declaration order never changes the fingerprint"
    );
    assert_eq!(
        fingerprint_of([one, two]),
        digest_bytes(expected.finalize())
    );
    assert_ne!(
        fingerprint_of([one, one]),
        fingerprint_of([one]),
        "a second declaration of the same bytes is still a second line"
    );
}

#[test]
fn the_manifest_must_start_with_its_format_line() {
    let spec = Spec::minimal();
    let manifest = spec
        .manifest()
        .replacen(MANIFEST_FORMAT, "ordo-fixture/1", 1);
    let mut source = spec.source();
    source.insert(MANIFEST_PATH, manifest);

    assert_eq!(
        read_snapshot(&source).unwrap_err(),
        SnapshotLoadError::Manifest {
            line: 1,
            reason: ManifestReason::UnknownFormat {
                found: "ordo-fixture/1".to_owned(),
            },
        }
    );
}

#[test]
fn out_of_order_records_name_their_line() {
    let spec = Spec::minimal();
    let manifest = spec
        .manifest()
        .replace("title Minimal", "themes-are-not-here");
    let mut source = spec.source();
    source.insert(MANIFEST_PATH, manifest);

    assert_eq!(
        read_snapshot(&source).unwrap_err(),
        SnapshotLoadError::Manifest {
            line: 3,
            reason: ManifestReason::Unexpected {
                expected: "title",
                found: "themes-are-not-here".to_owned(),
            },
        }
    );
}

#[test]
fn non_canonical_identities_and_digests_are_rejected() {
    let spec = Spec::minimal();
    let manifest =
        spec.manifest()
            .replacen(&format!("snapshot {}", snapshot_id()), "snapshot 0", 1);
    let mut source = spec.source();
    source.insert(MANIFEST_PATH, manifest);
    assert!(matches!(
        read_snapshot(&source).unwrap_err(),
        SnapshotLoadError::Manifest {
            line: 2,
            reason: ManifestReason::InvalidId { .. },
        }
    ));

    let spec = Spec::minimal();
    let manifest = spec.manifest().replacen(
        &spec.fingerprint().to_string(),
        &spec.fingerprint().to_string().to_uppercase(),
        1,
    );
    let mut source = spec.source();
    source.insert(MANIFEST_PATH, manifest);
    assert!(matches!(
        read_snapshot(&source).unwrap_err(),
        SnapshotLoadError::Manifest {
            line: 4,
            reason: ManifestReason::InvalidHash {
                error: ContentHashError::NotLowercaseHex,
                ..
            },
        }
    ));
}

#[test]
fn blank_text_is_rejected_rather_than_forwarded() {
    let spec = Spec::minimal();
    let manifest = spec.manifest().replace("title Minimal", "title   ");
    let mut source = spec.source();
    source.insert(MANIFEST_PATH, manifest);

    assert_eq!(
        read_snapshot(&source).unwrap_err(),
        SnapshotLoadError::Manifest {
            line: 3,
            reason: ManifestReason::Blank { field: "title" },
        }
    );
}

#[test]
fn dependency_paths_cannot_leave_the_snapshot_root() {
    for unsafe_path in [
        "../escape.txt",
        "/absolute.txt",
        "frames\\windows.txt",
        "C:/drive.txt",
        "frames//double.txt",
        "frames/./dot.txt",
        "frames/",
        "assets/not-a-frame.txt",
        "other/one.txt",
        "frames/con.txt",
        "frames/NUL",
        "frames/Aux.tar.gz",
        "frames/com1.txt",
        "frames/LPT9.png",
        "frames/conin$.txt",
        "frames/ends-with-dot.",
        "frames/what?.txt",
        "frames/star*.txt",
        "con/one.txt",
    ] {
        let mut spec = Spec::minimal();
        spec.frames[0].path = unsafe_path.to_owned();
        assert!(
            matches!(
                spec.load().unwrap_err(),
                SnapshotLoadError::Manifest {
                    reason: ManifestReason::UnsafePath { .. },
                    ..
                }
            ),
            "{unsafe_path:?} must be refused"
        );
    }
}

#[test]
fn duplicate_frame_ids_and_shared_paths_are_rejected() {
    let mut duplicate_id = Spec::minimal();
    duplicate_id.frames.push(FrameSpec {
        id: frame_id(),
        path: "frames/two.txt".to_owned(),
        text: text_frame("Another"),
    });
    assert!(matches!(
        duplicate_id.load().unwrap_err(),
        SnapshotLoadError::Manifest {
            reason: ManifestReason::Duplicate { field: "frame", .. },
            ..
        }
    ));

    let mut shared_path = Spec::minimal();
    shared_path.frames.push(FrameSpec {
        id: uuid(0x2f0).parse().unwrap(),
        path: "frames/one.txt".to_owned(),
        text: text_frame("Another"),
    });
    assert!(matches!(
        shared_path.load().unwrap_err(),
        SnapshotLoadError::Manifest {
            reason: ManifestReason::DuplicatePath { .. },
            ..
        }
    ));
}

#[test]
fn a_missing_dependency_names_the_identity_and_path() {
    let spec = Spec::minimal();
    let mut source = spec.source();
    source.files.remove("frames/one.txt");

    assert_eq!(
        read_snapshot(&source).unwrap_err(),
        SnapshotLoadError::MissingDependency {
            dependency: Dependency {
                id: DependencyId::Frame(frame_id()),
                path: "frames/one.txt".to_owned(),
            },
        }
    );
}

#[test]
fn a_mismatched_dependency_names_both_digests() {
    let spec = Spec::minimal();
    let tampered = text_frame("Tampered");
    let mut source = spec.source();
    source.insert("frames/one.txt", tampered.clone());

    assert_eq!(
        read_snapshot(&source).unwrap_err(),
        SnapshotLoadError::HashMismatch {
            dependency: Dependency {
                id: DependencyId::Frame(frame_id()),
                path: "frames/one.txt".to_owned(),
            },
            expected: hash_bytes(spec.frames[0].text.as_bytes()),
            actual: hash_bytes(tampered.as_bytes()),
        }
    );
}

#[test]
fn a_tampered_asset_is_rejected_before_any_reference_check() {
    let spec = Spec::minimal();
    let mut source = spec.source();
    source.insert("assets/pixel.png", vec![0xff; 4]);

    assert!(matches!(
        read_snapshot(&source).unwrap_err(),
        SnapshotLoadError::HashMismatch {
            dependency: Dependency {
                id: DependencyId::Asset(_),
                ..
            },
            ..
        }
    ));
}

#[test]
fn a_manifest_without_a_fingerprint_match_is_rejected() {
    let spec = Spec::minimal();
    let other = hash_bytes(b"other");
    let manifest = spec
        .manifest()
        .replacen(&spec.fingerprint().to_string(), &other.to_string(), 1);
    let mut source = spec.source();
    source.insert(MANIFEST_PATH, manifest);

    assert_eq!(
        read_snapshot(&source).unwrap_err(),
        SnapshotLoadError::FingerprintMismatch {
            declared: other,
            actual: spec.fingerprint(),
        }
    );
}

#[test]
fn a_frame_that_is_not_canonical_names_the_frame_and_line() {
    let mut spec = Spec::minimal();
    spec.frames[0].text =
        "ordo-fixture/1\nkind:NotAKind\nheader-absent\nbody-lines:0\nfooter-absent\nimage-absent\n"
            .to_owned();

    assert_eq!(
        spec.load().unwrap_err(),
        SnapshotLoadError::InvalidFrame {
            frame: frame_id(),
            path: "frames/one.txt".to_owned(),
            error: FrameParseError::Line {
                line: 2,
                reason: FrameReason::InvalidKind {
                    value: "NotAKind".to_owned(),
                },
            },
        }
    );
}

#[test]
fn a_frame_that_cannot_compose_is_rejected_at_load() {
    let mut spec = Spec::minimal();
    // A General Image frame without a foreground element parses but cannot
    // compose; the reader must catch it here, not on the display path.
    spec.frames[0].text =
        "ordo-fixture/1\nkind:GeneralImage\nheader-absent\nbody-lines:0\nfooter-absent\nimage-absent\n"
            .to_owned();

    assert_eq!(
        spec.load().unwrap_err(),
        SnapshotLoadError::UnrenderableFrame {
            frame: frame_id(),
            path: "frames/one.txt".to_owned(),
            error: RenderError::MissingForegroundImage,
        }
    );
}

#[test]
fn a_project_cue_may_not_name_an_undeclared_frame() {
    let mut spec = Spec::minimal();
    let ghost: FrameId = uuid(0x299).parse().unwrap();
    spec.sections[0].items[0].cues[0].behavior = CueBehavior::Project {
        frame: ghost,
        duration_seconds: None,
    };

    assert_eq!(
        spec.load().unwrap_err(),
        SnapshotLoadError::UndeclaredFrame {
            cue: cue_id(),
            frame: ghost,
        }
    );
}

#[test]
fn a_declared_frame_no_cue_shows_is_rejected() {
    let mut spec = Spec::minimal();
    let ghost: FrameId = uuid(0x299).parse().unwrap();
    spec.frames.push(FrameSpec {
        id: ghost,
        path: "frames/two.txt".to_owned(),
        text: text_frame("Unused"),
    });

    assert_eq!(
        spec.load().unwrap_err(),
        SnapshotLoadError::UnusedFrame { frame: ghost }
    );
}

#[test]
fn a_declared_asset_no_frame_uses_is_rejected() {
    let mut spec = Spec::minimal();
    spec.frames[1].text = text_frame("No image any more");

    assert_eq!(
        spec.load().unwrap_err(),
        SnapshotLoadError::UnusedAsset { asset: asset_id() }
    );
}

#[test]
fn an_asset_must_have_its_declared_size() {
    let spec = Spec::minimal();
    let manifest = spec.manifest().replace("asset-size 4", "asset-size 5");
    let mut source = spec.source();
    source.insert(MANIFEST_PATH, manifest);

    assert_eq!(
        read_snapshot(&source).unwrap_err(),
        SnapshotLoadError::WrongAssetSize {
            asset: asset_id(),
            path: "assets/pixel.png".to_owned(),
            declared: 5,
            actual: 4,
        }
    );
}

#[test]
fn an_image_frame_must_name_a_declared_asset() {
    let mut spec = Spec::minimal();
    let undeclared = uuid(0x399);
    spec.frames[1].text = image_frame(&undeclared);

    assert_eq!(
        spec.load().unwrap_err(),
        SnapshotLoadError::UndeclaredAsset {
            frame: image_frame_id(),
            asset: undeclared,
        }
    );
}

#[test]
fn domain_rejections_surface_as_the_snapshot_error() {
    let mut spec = Spec::minimal();
    // An operator trigger without the private phrase is uninspectable, so
    // the domain rejects it. The reader must forward that rule rather than
    // inventing its own.
    spec.frames.clear();
    spec.sections[0].items[0].cues[0] = CueSpec {
        id: cue_id(),
        label: "The Lord be with you".to_owned(),
        private_context: None,
        behavior: CueBehavior::OperatorTrigger,
    };

    assert_eq!(
        spec.load().unwrap_err(),
        SnapshotLoadError::Snapshot(SnapshotError::MissingPrivateContext { cue: cue_id() })
    );
}

#[test]
fn the_source_sees_the_manifest_first_and_only_declared_paths() {
    struct RecordingSource {
        inner: MemorySource,
        read: std::cell::RefCell<Vec<String>>,
    }

    impl SnapshotSource for RecordingSource {
        fn read(&self, path: &str) -> Result<Vec<u8>, SourceError> {
            self.read.borrow_mut().push(path.to_owned());
            self.inner.read(path)
        }
    }

    let spec = Spec::minimal();
    let source = RecordingSource {
        inner: spec.source(),
        read: std::cell::RefCell::new(Vec::new()),
    };
    read_snapshot(&source).unwrap();
    assert_eq!(
        source.read.borrow().as_slice(),
        [
            MANIFEST_PATH,
            "frames/one.txt",
            "frames/image.txt",
            "assets/pixel.png"
        ]
    );
}

#[test]
fn the_frame_parser_accepts_every_committed_golden_fixture() {
    // The golden corpus is the authority for the canonical frame format;
    // if this parser stops accepting all of it, a snapshot could declare a
    // frame the renderer's own harness still treats as valid.
    let dir =
        std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("../../tests/fixtures/rendering");
    let mut checked = 0;
    for entry in std::fs::read_dir(&dir).expect("golden fixtures must exist") {
        let path = entry.unwrap().path();
        if path.extension().is_none_or(|extension| extension != "txt") {
            continue;
        }
        let mut text = std::fs::read_to_string(&path).unwrap();
        // Two golden fixtures end without a final LF (the harness reads with
        // `str::lines`); the snapshot reader requires one, so the grammar
        // comparison supplies it rather than editing the renderer's corpus.
        if !text.ends_with('\n') {
            text.push('\n');
        }
        parse_frame(&text).unwrap_or_else(|error| {
            panic!("{} must parse: {error}", path.display());
        });
        checked += 1;
    }
    assert!(
        checked >= 20,
        "expected the full golden corpus, got {checked}"
    );
}

#[test]
fn the_frame_parser_rejects_miscounts_and_trailing_content() {
    let miscount = "ordo-fixture/1\nkind:Lyrics\nheader-absent\nbody-lines:2\none line\nfooter-absent\nimage-absent\n";
    // A body count that runs past the real body consumes the footer line,
    // so the failure names the record that no longer matches, not the
    // count: a line format cannot know which line the author meant.
    assert!(matches!(
        parse_frame(miscount),
        Err(FrameParseError::Line { line: 7, .. })
    ));

    let trailing = "ordo-fixture/1\nkind:Black\nheader-absent\nbody-lines:0\nfooter-absent\nimage-absent\nextra\n";
    assert_eq!(
        parse_frame(trailing),
        Err(FrameParseError::Line {
            line: 7,
            reason: FrameReason::Trailing,
        })
    );

    let leading_zero = "ordo-fixture/1\nkind:Lyrics\nheader-absent\nbody-lines:01\nx\nfooter-absent\nimage-absent\n";
    assert_eq!(
        parse_frame(leading_zero),
        Err(FrameParseError::Line {
            line: 4,
            reason: FrameReason::InvalidCount {
                value: "01".to_owned(),
            },
        })
    );

    let bad_size = "ordo-fixture/1\nkind:GeneralImage\nheader-absent\nbody-lines:0\nfooter-absent\nimage:some-asset\nimage-size:2x\n";
    assert_eq!(
        parse_frame(bad_size),
        Err(FrameParseError::Line {
            line: 7,
            reason: FrameReason::InvalidSize {
                value: "2x".to_owned(),
            },
        })
    );
}

#[test]
fn loaded_snapshots_are_send_and_sync_for_worker_ownership() {
    const fn assert_send_sync<T: Send + Sync>() {}
    assert_send_sync::<LoadedSnapshot>();
    assert_send_sync::<DirectorySource>();
}

#[test]
fn segments_ending_in_a_space_are_not_portable() {
    assert!(!is_snapshot_relative("frames/ends-with-space "));
    assert!(!is_snapshot_relative("frames /one.txt"));
}

#[test]
fn ordinary_names_that_only_resemble_devices_are_accepted() {
    for path in [
        "frames/console.txt",
        "frames/com10.txt",
        "frames/com0.txt",
        "frames/lpt.txt",
        "frames/nullable.txt",
        "frames/auxiliary.txt",
    ] {
        assert!(is_snapshot_relative(path), "{path:?} is an ordinary name");
    }
}

#[test]
fn paths_that_differ_only_in_ascii_case_are_one_file() {
    let mut spec = Spec::minimal();
    spec.frames.push(FrameSpec {
        id: uuid(0x2f1).parse().unwrap(),
        path: "frames/ONE.txt".to_owned(),
        text: text_frame("Another"),
    });
    assert!(matches!(
        spec.load().unwrap_err(),
        SnapshotLoadError::Manifest {
            reason: ManifestReason::DuplicatePath { .. },
            ..
        }
    ));
}

#[test]
fn a_wrong_sized_asset_is_reported_before_its_digest_is_checked() {
    let spec = Spec::minimal();
    let mut source = spec.source();
    // Wrong length and wrong digest at once: the cheap length check wins.
    source.insert("assets/pixel.png", vec![0xff]);
    assert_eq!(
        read_snapshot(&source).unwrap_err(),
        SnapshotLoadError::WrongAssetSize {
            asset: asset_id(),
            path: "assets/pixel.png".to_owned(),
            declared: 4,
            actual: 1,
        }
    );
}

#[test]
fn a_hostile_line_count_never_sizes_an_allocation() {
    let hostile_body = "ordo-fixture/1\nkind:Lyrics\nheader-absent\nbody-lines:4000000000\nline\nfooter-absent\nimage-absent\n";
    let hostile_footer = "ordo-fixture/1\nkind:Lyrics\nheader-absent\nbody-lines:0\nfooter-lines:4000000000\nline\nimage-absent\n";
    for text in [hostile_body, hostile_footer] {
        assert!(
            matches!(
                parse_frame(text),
                Err(FrameParseError::Line {
                    reason: FrameReason::Missing { .. },
                    ..
                })
            ),
            "{text:?}"
        );
    }

    let mut spec = Spec::minimal();
    spec.frames[0].text = hostile_body.to_owned();
    assert!(matches!(
        spec.load().unwrap_err(),
        SnapshotLoadError::InvalidFrame { frame, .. } if frame == frame_id()
    ));
}

#[test]
fn a_crlf_manifest_is_rejected_at_its_first_carriage_return() {
    let spec = Spec::minimal();
    let mut source = spec.source();
    source.insert(MANIFEST_PATH, spec.manifest().replace('\n', "\r\n"));
    assert_eq!(
        read_snapshot(&source).unwrap_err(),
        SnapshotLoadError::Manifest {
            line: 1,
            reason: ManifestReason::CarriageReturn,
        }
    );

    // A single stray carriage return in the middle names its own line.
    let mut source = spec.source();
    source.insert(
        MANIFEST_PATH,
        spec.manifest()
            .replacen("title Minimal", "title Minimal\r", 1),
    );
    assert_eq!(
        read_snapshot(&source).unwrap_err(),
        SnapshotLoadError::Manifest {
            line: 3,
            reason: ManifestReason::CarriageReturn,
        }
    );
}

#[test]
fn a_crlf_frame_is_rejected_rather_than_normalized() {
    let mut spec = Spec::minimal();
    spec.frames[0].text = spec.frames[0].text.replace('\n', "\r\n");
    assert_eq!(
        spec.load().unwrap_err(),
        SnapshotLoadError::InvalidFrame {
            frame: frame_id(),
            path: "frames/one.txt".to_owned(),
            error: FrameParseError::Line {
                line: 1,
                reason: FrameReason::CarriageReturn,
            },
        }
    );
}

#[test]
fn a_missing_final_lf_is_rejected_in_the_manifest_and_in_frames() {
    let spec = Spec::minimal();
    let manifest = spec.manifest();
    let mut source = spec.source();
    source.insert(MANIFEST_PATH, manifest.trim_end_matches('\n'));
    assert_eq!(
        read_snapshot(&source).unwrap_err(),
        SnapshotLoadError::Manifest {
            line: manifest.lines().count(),
            reason: ManifestReason::MissingFinalNewline,
        }
    );

    let mut spec = Spec::minimal();
    let text = spec.frames[0].text.clone();
    spec.frames[0].text = text.trim_end_matches('\n').to_owned();
    assert_eq!(
        spec.load().unwrap_err(),
        SnapshotLoadError::InvalidFrame {
            frame: frame_id(),
            path: "frames/one.txt".to_owned(),
            error: FrameParseError::Line {
                line: text.lines().count(),
                reason: FrameReason::MissingFinalNewline,
            },
        }
    );
}

#[test]
fn a_byte_order_mark_is_not_part_of_the_format_line() {
    let spec = Spec::minimal();
    let mut source = spec.source();
    source.insert(MANIFEST_PATH, format!("\u{feff}{}", spec.manifest()));
    assert!(matches!(
        read_snapshot(&source).unwrap_err(),
        SnapshotLoadError::Manifest {
            line: 1,
            reason: ManifestReason::UnknownFormat { .. },
        }
    ));

    let mut spec = Spec::minimal();
    spec.frames[0].text = format!("\u{feff}{}", spec.frames[0].text);
    assert!(matches!(
        spec.load().unwrap_err(),
        SnapshotLoadError::InvalidFrame {
            error: FrameParseError::Line {
                line: 1,
                reason: FrameReason::UnknownFormat { .. },
            },
            ..
        }
    ));
}

#[test]
fn trailing_whitespace_in_a_text_value_is_rejected() {
    for suffix in [" ", "\t", "\u{a0}"] {
        let spec = Spec::minimal();
        let mut source = spec.source();
        source.insert(
            MANIFEST_PATH,
            spec.manifest()
                .replacen("title Minimal", &format!("title Minimal{suffix}"), 1),
        );
        assert_eq!(
            read_snapshot(&source).unwrap_err(),
            SnapshotLoadError::Manifest {
                line: 3,
                reason: ManifestReason::TrailingWhitespace { field: "title" },
            },
            "{suffix:?}"
        );
    }
}

#[test]
fn an_image_record_is_only_allowed_on_a_general_image_frame() {
    let mut spec = Spec::minimal();
    spec.frames[0].text = format!(
        "ordo-fixture/1\nkind:GeneralText\nheader-absent\nbody-lines:1\nHello\nfooter-absent\nimage:{}\nimage-size:2x2\n",
        asset_id()
    );
    assert_eq!(
        spec.load().unwrap_err(),
        SnapshotLoadError::InvalidFrame {
            frame: frame_id(),
            path: "frames/one.txt".to_owned(),
            error: FrameParseError::Line {
                line: 7,
                reason: FrameReason::ImageOnNonImageFrame,
            },
        }
    );
}

#[test]
fn a_loaded_snapshot_pins_its_manifest_bytes() {
    let before = Spec::minimal().load().unwrap();
    let mut renamed = Spec::minimal();
    renamed.title = "Renamed".to_owned();
    let after = renamed.load().unwrap();
    assert_eq!(
        before.fingerprint(),
        after.fingerprint(),
        "the fingerprint covers dependencies only"
    );
    assert_ne!(before.manifest_hash(), after.manifest_hash());
}
