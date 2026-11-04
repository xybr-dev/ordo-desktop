use std::collections::BTreeMap;

use domain::presentation::{
    AutoPlay, CueBehavior, CueId, MassItemId, MassItemKind, MassSectionId, SnapshotId, ThemeId,
    ThemeKind, ThemeRef,
};

use serde_json::{Value, json};

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

    /// The manifest as a JSON value. Keys serialize in sorted order, which is
    /// deterministic; the committed fixtures use the authored order instead.
    fn manifest_json(&self) -> Value {
        let themes: Vec<Value> = self
            .themes
            .iter()
            .map(|theme| {
                json!({
                    "id": theme.id.to_string(),
                    "name": theme.name,
                    "kind": match theme.kind {
                        ThemeKind::Custom => "custom",
                        ThemeKind::Fallback => "fallback",
                    },
                })
            })
            .collect();
        let frames: Vec<Value> = self
            .frames
            .iter()
            .map(|frame| {
                json!({
                    "id": frame.id.to_string(),
                    "path": frame.path,
                    "sha256": hash_bytes(frame.text.as_bytes()).to_string(),
                })
            })
            .collect();
        let assets: Vec<Value> = self
            .assets
            .iter()
            .map(|asset| {
                json!({
                    "id": asset.id.to_string(),
                    "path": asset.path,
                    "size": asset.bytes.len(),
                    "sha256": hash_bytes(&asset.bytes).to_string(),
                })
            })
            .collect();
        let sections: Vec<Value> = self
            .sections
            .iter()
            .map(|section| {
                let items: Vec<Value> = section.items.iter().map(item_json).collect();
                json!({
                    "id": section.id.to_string(),
                    "title": section.title,
                    "items": items,
                })
            })
            .collect();
        json!({
            "schema_version": 2,
            "id": self.snapshot.to_string(),
            "mass_plan_id": uuid(0x701),
            "title": self.title,
            "themes": themes,
            "mass_theme_id": self.mass_theme.to_string(),
            "sections": sections,
            "frames": frames,
            "assets": assets,
            "dependencies_fingerprint": self.fingerprint().to_string(),
        })
    }

    fn manifest(&self) -> String {
        serialize(&self.manifest_json())
    }

    /// The source with the manifest edited as JSON, so a test changes the one
    /// fact it is about.
    fn source_with(&self, edit: impl FnOnce(&mut Value)) -> MemorySource {
        let mut value = self.manifest_json();
        edit(&mut value);
        let mut source = self.source();
        source.insert(MANIFEST_PATH, serialize(&value));
        source
    }

    /// The source with raw manifest text, for bytes JSON cannot express.
    fn source_with_text(&self, text: impl Into<Vec<u8>>) -> MemorySource {
        let mut source = self.source();
        source.insert(MANIFEST_PATH, text);
        source
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

fn serialize(value: &Value) -> String {
    let mut text = serde_json::to_string_pretty(value).unwrap();
    text.push('\n');
    text
}

fn item_json(item: &ItemSpec) -> Value {
    let cues: Vec<Value> = item
        .cues
        .iter()
        .map(|cue| {
            let behavior = match &cue.behavior {
                CueBehavior::Project {
                    frame,
                    duration_seconds,
                } => {
                    let mut behavior = json!({"type": "project", "frame_id": frame.to_string()});
                    if let Some(seconds) = duration_seconds {
                        behavior["duration_seconds"] = json!(seconds);
                    }
                    behavior
                }
                CueBehavior::OperatorTrigger => json!({"type": "operator_trigger"}),
                CueBehavior::Hold => json!({"type": "hold"}),
                CueBehavior::Wait => json!({"type": "wait"}),
                CueBehavior::Unavailable { reason } => {
                    json!({"type": "unavailable", "reason": reason})
                }
            };
            let mut value = json!({
                "id": cue.id.to_string(),
                "label": cue.label,
                "behavior": behavior,
            });
            if let Some(context) = &cue.private_context {
                value["private_context"] = json!(context);
            }
            value
        })
        .collect();
    let mut value = json!({
        "id": item.id.to_string(),
        "title": item.title,
        "kind": match item.kind {
            MassItemKind::Song => "song",
            MassItemKind::Reading => "reading",
            MassItemKind::Psalm => "psalm",
            MassItemKind::ResponseSequence => "response_sequence",
            MassItemKind::GeneralPresentation => "general_presentation",
            MassItemKind::Rite => "rite",
        },
        "cues": cues,
    });
    if let Some(theme) = item.theme {
        value["theme_id"] = json!(theme.to_string());
    }
    if let Some(seconds) = item.auto_play {
        value["auto_play_seconds"] = json!(seconds);
    }
    value
}

/// The `Manifest` reason of a failed read, for tests that assert on it.
fn manifest_reason(error: SnapshotLoadError) -> ManifestReason {
    match error {
        SnapshotLoadError::Manifest { reason } => reason,
        other => panic!("expected a manifest error, got {other:?}"),
    }
}

/// The cause text of a `Malformed` manifest error.
fn malformed_message(error: SnapshotLoadError) -> String {
    match manifest_reason(error) {
        ManifestReason::Malformed { message, .. } => message,
        other => panic!("expected a malformed manifest, got {other:?}"),
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
fn the_manifest_must_be_the_v2_json_document() {
    // A frame file where the manifest belongs is not JSON.
    let spec = Spec::minimal();
    let source = spec.source_with_text("ordo-fixture/1\n");
    assert!(matches!(
        manifest_reason(read_snapshot(&source).unwrap_err()),
        ManifestReason::Malformed { line: 1, .. }
    ));

    // The v1 line manifest is not JSON either.
    let source = spec.source_with_text("ordo-presentation-snapshot/1\nsnapshot x\n");
    assert!(matches!(
        manifest_reason(read_snapshot(&source).unwrap_err()),
        ManifestReason::Malformed { .. }
    ));

    // An empty manifest has no final LF to find.
    assert_eq!(
        manifest_reason(read_snapshot(&spec.source_with_text("")).unwrap_err()),
        ManifestReason::MissingFinalNewline
    );
}

#[test]
fn only_schema_version_2_is_read() {
    let spec = Spec::minimal();
    for (version, found) in [(json!(1), 1), (json!(3), 3), (json!(0), 0), (json!(99), 99)] {
        let source = spec.source_with(|value| value["schema_version"] = version.clone());
        let error = read_snapshot(&source).unwrap_err();
        assert_eq!(
            error,
            SnapshotLoadError::Manifest {
                reason: ManifestReason::UnsupportedVersion { found },
            }
        );
        assert_eq!(
            error.to_string(),
            format!(
                "snapshot.json: requires an Ordo version supporting snapshot-manifest v{found}; this build supports v2"
            )
        );
    }
}

#[test]
fn a_v1_manifest_is_refused_by_version_before_its_shape_is_judged() {
    // A genuine v1 document: flat `items` and `assets_fingerprint`, none of
    // v2's members. It must be refused as v1, not as a pile of unknown fields.
    let spec = Spec::minimal();
    let v1 = format!(
        "{{\"schema_version\":1,\"id\":\"{}\",\"mass_plan_id\":\"{}\",\"items\":[],\"assets\":[],\"assets_fingerprint\":\"{}\"}}\n",
        uuid(1),
        uuid(2),
        hash_bytes(b"")
    );
    assert_eq!(
        read_snapshot(&spec.source_with_text(v1)).unwrap_err(),
        SnapshotLoadError::Manifest {
            reason: ManifestReason::UnsupportedVersion { found: 1 },
        }
    );
}

#[test]
fn a_missing_or_mistyped_schema_version_is_malformed() {
    let spec = Spec::minimal();
    let source = spec.source_with(|value| {
        value.as_object_mut().unwrap().remove("schema_version");
    });
    assert!(
        malformed_message(read_snapshot(&source).unwrap_err())
            .contains("missing field `schema_version`")
    );

    for bad in [json!("2"), json!(2.5), json!(-2), json!(null)] {
        let source = spec.source_with(|value| value["schema_version"] = bad.clone());
        assert!(
            matches!(
                manifest_reason(read_snapshot(&source).unwrap_err()),
                ManifestReason::Malformed { .. }
            ),
            "{bad}"
        );
    }
}

#[test]
fn a_missing_required_member_is_named() {
    let spec = Spec::minimal();
    for member in [
        "id",
        "mass_plan_id",
        "title",
        "themes",
        "mass_theme_id",
        "sections",
        "frames",
        "assets",
        "dependencies_fingerprint",
    ] {
        let source = spec.source_with(|value| {
            value.as_object_mut().unwrap().remove(member);
        });
        let message = malformed_message(read_snapshot(&source).unwrap_err());
        assert!(
            message.contains(&format!("missing field `{member}`")),
            "{member}: {message}"
        );
    }

    // Nested required members too.
    let source = spec.source_with(|value| {
        value["sections"][0]["items"][0]["cues"][0]
            .as_object_mut()
            .unwrap()
            .remove("label");
    });
    assert!(
        malformed_message(read_snapshot(&source).unwrap_err()).contains("missing field `label`")
    );
}

#[test]
fn an_unknown_member_is_rejected_at_every_level() {
    let spec = Spec::minimal();
    let paths: [&[&str]; 8] = [
        &[],
        &["themes", "0"],
        &["sections", "0"],
        &["sections", "0", "items", "0"],
        &["sections", "0", "items", "0", "cues", "0"],
        &["sections", "0", "items", "0", "cues", "0", "behavior"],
        &["frames", "0"],
        &["assets", "0"],
    ];
    for path in paths {
        let source = spec.source_with(|value| {
            let mut at = value;
            for step in path {
                at = match step.parse::<usize>() {
                    Ok(index) => &mut at[index],
                    Err(_) => &mut at[*step],
                };
            }
            at["surprise"] = json!(true);
        });
        let message = malformed_message(read_snapshot(&source).unwrap_err());
        assert!(
            message.contains("unknown field `surprise`"),
            "{path:?}: {message}"
        );
    }
}

#[test]
fn an_empty_behavior_variant_still_rejects_unknown_members() {
    let spec = Spec::minimal();
    // Item 1 holds the trigger (Cue 0), Hold (1), Wait (2) and Unavailable (3).
    for cue in 0..4 {
        let source = spec.source_with(|value| {
            value["sections"][0]["items"][1]["cues"][cue]["behavior"]["surprise"] = json!(1);
        });
        let message = malformed_message(read_snapshot(&source).unwrap_err());
        assert!(
            message.contains("unknown field `surprise`"),
            "{cue}: {message}"
        );
    }
}

#[test]
fn an_unknown_variant_or_kind_is_rejected() {
    let spec = Spec::minimal();
    let source = spec.source_with(|value| {
        value["sections"][0]["items"][1]["cues"][0]["behavior"]["type"] = json!("teleport");
    });
    assert!(malformed_message(read_snapshot(&source).unwrap_err()).contains("unknown variant"));

    // v1 spelled kinds in PascalCase; v2 is snake_case only.
    let source = spec.source_with(|value| {
        value["sections"][0]["items"][0]["kind"] = json!("GeneralPresentation");
    });
    assert!(malformed_message(read_snapshot(&source).unwrap_err()).contains("unknown variant"));
    let source = spec.source_with(|value| value["themes"][0]["kind"] = json!("Custom"));
    assert!(malformed_message(read_snapshot(&source).unwrap_err()).contains("unknown variant"));
}

#[test]
fn a_duplicate_key_is_rejected_not_last_one_wins() {
    let spec = Spec::minimal();
    let manifest = spec.manifest();

    // Top level.
    let duplicated = manifest.replacen("{\n", "{\n  \"title\": \"Other\",\n", 1);
    assert!(
        malformed_message(read_snapshot(&spec.source_with_text(duplicated)).unwrap_err())
            .contains("duplicate field `title`")
    );

    // Nested in a Cue.
    let duplicated = manifest.replacen(
        "\"label\": \"Slide one\"",
        "\"label\": \"Slide one\",\n          \"label\": \"Slide two\"",
        1,
    );
    assert_ne!(duplicated, manifest);
    assert!(
        malformed_message(read_snapshot(&spec.source_with_text(duplicated)).unwrap_err())
            .contains("duplicate field `label`")
    );

    // Inside the tagged behavior, the tag itself and a payload member.
    let duplicated = manifest.replacen(
        "\"type\": \"operator_trigger\"",
        "\"type\": \"operator_trigger\",\n            \"type\": \"hold\"",
        1,
    );
    assert_ne!(duplicated, manifest);
    assert!(
        matches!(
            manifest_reason(read_snapshot(&spec.source_with_text(duplicated)).unwrap_err()),
            ManifestReason::Malformed { .. }
        ),
        "a repeated type tag must not pick one"
    );
    let duplicated = manifest.replacen(
        "\"reason\": \"Image could not be captured\"",
        "\"reason\": \"Image could not be captured\",\n            \"reason\": \"Other\"",
        1,
    );
    assert_ne!(duplicated, manifest);
    assert!(
        malformed_message(read_snapshot(&spec.source_with_text(duplicated)).unwrap_err())
            .contains("duplicate field `reason`")
    );

    // The version probe also refuses a repeated `schema_version`.
    let duplicated = manifest.replacen("{\n", "{\n  \"schema_version\": 1,\n", 1);
    assert!(matches!(
        manifest_reason(read_snapshot(&spec.source_with_text(duplicated)).unwrap_err()),
        ManifestReason::Malformed { .. }
    ));
}

#[test]
fn null_is_not_a_way_to_spell_an_absent_optional_member() {
    let spec = Spec::minimal();
    for (path, member) in [
        (vec!["sections", "0", "items", "0"], "theme_id"),
        (vec!["sections", "0", "items", "0"], "auto_play_seconds"),
        (
            vec!["sections", "0", "items", "0", "cues", "0"],
            "private_context",
        ),
        (
            vec!["sections", "0", "items", "0", "cues", "0", "behavior"],
            "duration_seconds",
        ),
        (vec![], "celebration_id"),
        (vec![], "captured_at"),
    ] {
        let source = spec.source_with(|value| {
            let mut at = value;
            for step in &path {
                at = match step.parse::<usize>() {
                    Ok(index) => &mut at[index],
                    Err(_) => &mut at[*step],
                };
            }
            at[member] = Value::Null;
        });
        assert!(
            matches!(
                manifest_reason(read_snapshot(&source).unwrap_err()),
                ManifestReason::Malformed { .. }
            ),
            "{member}: null must be refused"
        );
    }
}

#[test]
fn optional_provenance_members_are_validated_and_accepted() {
    let spec = Spec::minimal();
    let source = spec.source_with(|value| {
        value["celebration_id"] = json!(uuid(0x702));
        value["captured_at"] = json!("2026-11-04T09:30:00Z");
    });
    assert!(read_snapshot(&source).is_ok());

    let source = spec.source_with(|value| value["mass_plan_id"] = json!("nope"));
    assert!(matches!(
        manifest_reason(read_snapshot(&source).unwrap_err()),
        ManifestReason::InvalidId { at, .. } if at == "/mass_plan_id"
    ));
    let source =
        spec.source_with(|value| value["celebration_id"] = json!(uuid(0x7ab).to_uppercase()));
    assert!(matches!(
        manifest_reason(read_snapshot(&source).unwrap_err()),
        ManifestReason::InvalidId { at, .. } if at == "/celebration_id"
    ));
    for bad in [
        "2026-11-04",
        "2026-11-04T09:30:00",
        "2026-11-04 09:30:00Z",
        "2026-11-04T09:30:00.5Z",
    ] {
        let source = spec.source_with(|value| value["captured_at"] = json!(bad));
        assert!(
            matches!(
                manifest_reason(read_snapshot(&source).unwrap_err()),
                ManifestReason::InvalidTimestamp { .. }
            ),
            "{bad}"
        );
    }
}

#[test]
fn non_canonical_identities_and_digests_are_rejected() {
    let spec = Spec::minimal();
    let source = spec.source_with(|value| value["id"] = json!("0"));
    assert!(matches!(
        manifest_reason(read_snapshot(&source).unwrap_err()),
        ManifestReason::InvalidId { at, .. } if at == "/id"
    ));

    // Identities nested deep name their JSON Pointer.
    let source = spec.source_with(|value| {
        value["sections"][0]["items"][0]["cues"][1]["behavior"]["frame_id"] =
            json!(uuid(0xabc).to_uppercase());
    });
    assert!(matches!(
        manifest_reason(read_snapshot(&source).unwrap_err()),
        ManifestReason::InvalidId { at, .. } if at == "/sections/0/items/0/cues/1/behavior/frame_id"
    ));

    let source = spec.source_with(|value| {
        value["dependencies_fingerprint"] = json!(spec.fingerprint().to_string().to_uppercase());
    });
    assert!(matches!(
        manifest_reason(read_snapshot(&source).unwrap_err()),
        ManifestReason::InvalidHash {
            at,
            error: ContentHashError::NotLowercaseHex,
            ..
        } if at == "/dependencies_fingerprint"
    ));
}

#[test]
fn blank_text_is_rejected_rather_than_forwarded() {
    let spec = Spec::minimal();
    let source = spec.source_with(|value| value["title"] = json!("   "));
    assert_eq!(
        read_snapshot(&source).unwrap_err(),
        SnapshotLoadError::Manifest {
            reason: ManifestReason::Blank {
                at: "/title".to_owned(),
            },
        }
    );

    // Every text member is checked, not only the title.
    let cases: [(&[&str], &str); 6] = [
        (&["themes", "0", "name"], "/themes/0/name"),
        (&["sections", "0", "title"], "/sections/0/title"),
        (
            &["sections", "0", "items", "0", "title"],
            "/sections/0/items/0/title",
        ),
        (
            &["sections", "0", "items", "0", "cues", "0", "label"],
            "/sections/0/items/0/cues/0/label",
        ),
        (
            &[
                "sections",
                "0",
                "items",
                "0",
                "cues",
                "0",
                "private_context",
            ],
            "/sections/0/items/0/cues/0/private_context",
        ),
        (
            &[
                "sections", "0", "items", "1", "cues", "3", "behavior", "reason",
            ],
            "/sections/0/items/1/cues/3/behavior/reason",
        ),
    ];
    for (path, pointer) in cases {
        let source = spec.source_with(|value| {
            let mut at = value;
            for step in path {
                at = match step.parse::<usize>() {
                    Ok(index) => &mut at[index],
                    Err(_) => &mut at[*step],
                };
            }
            *at = json!("\t ");
        });
        assert_eq!(
            read_snapshot(&source).unwrap_err(),
            SnapshotLoadError::Manifest {
                reason: ManifestReason::Blank {
                    at: pointer.to_owned(),
                },
            },
            "{pointer}"
        );
    }
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
        manifest_reason(duplicate_id.load().unwrap_err()),
        ManifestReason::Duplicate {
            field: "frames",
            ..
        }
    ));

    let mut duplicate_asset = Spec::minimal();
    duplicate_asset.assets.push(AssetSpec {
        id: asset_id(),
        path: "assets/other.png".to_owned(),
        bytes: vec![1, 2, 3],
    });
    assert!(matches!(
        manifest_reason(duplicate_asset.load().unwrap_err()),
        ManifestReason::Duplicate {
            field: "assets",
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
        manifest_reason(shared_path.load().unwrap_err()),
        ManifestReason::DuplicatePath { .. }
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
    let source = spec.source_with(|value| value["assets"][0]["size"] = json!(5));

    assert_eq!(
        read_snapshot(&source).unwrap_err(),
        SnapshotLoadError::WrongAssetSize {
            asset: asset_id(),
            path: "assets/pixel.png".to_owned(),
            declared: 5,
            actual: 4,
        }
    );

    // A size that is not a plain unsigned integer is malformed, not coerced.
    for bad in [json!(-1), json!(4.0), json!("4"), json!(4_294_967_296_u64)] {
        let source = spec.source_with(|value| value["assets"][0]["size"] = bad.clone());
        assert!(
            matches!(
                manifest_reason(read_snapshot(&source).unwrap_err()),
                ManifestReason::Malformed { .. }
            ),
            "{bad}"
        );
    }
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
    let source = spec.source_with_text(spec.manifest().replace('\n', "\r\n"));
    assert_eq!(
        read_snapshot(&source).unwrap_err(),
        SnapshotLoadError::Manifest {
            reason: ManifestReason::CarriageReturn { line: 1 },
        }
    );

    // A single stray carriage return in the middle names its own line, even
    // where JSON would accept it as whitespace.
    let manifest = spec.manifest();
    let line = manifest
        .lines()
        .position(|line| line.contains("\"title\": \"Minimal\""))
        .unwrap()
        + 1;
    let source = spec.source_with_text(manifest.replacen(
        "\"title\": \"Minimal\"",
        "\"title\": \"Minimal\"\r",
        1,
    ));
    assert_eq!(
        read_snapshot(&source).unwrap_err(),
        SnapshotLoadError::Manifest {
            reason: ManifestReason::CarriageReturn { line },
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
    let source = spec.source_with_text(manifest.trim_end_matches('\n'));
    assert_eq!(
        read_snapshot(&source).unwrap_err(),
        SnapshotLoadError::Manifest {
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
fn a_byte_order_mark_is_rejected_in_the_manifest_and_in_frames() {
    let spec = Spec::minimal();
    let source = spec.source_with_text(format!("\u{feff}{}", spec.manifest()));
    assert_eq!(
        read_snapshot(&source).unwrap_err(),
        SnapshotLoadError::Manifest {
            reason: ManifestReason::ByteOrderMark,
        }
    );

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
fn trailing_whitespace_and_line_breaks_in_a_text_value_are_rejected() {
    let spec = Spec::minimal();
    for suffix in [" ", "\t", "\u{a0}"] {
        let source = spec.source_with(|value| value["title"] = json!(format!("Minimal{suffix}")));
        assert_eq!(
            read_snapshot(&source).unwrap_err(),
            SnapshotLoadError::Manifest {
                reason: ManifestReason::TrailingWhitespace {
                    at: "/title".to_owned(),
                },
            },
            "{suffix:?}"
        );
    }

    // JSON can spell a line break inside a string; the manifest texts are one
    // line, as the old line format guaranteed.
    for title in ["Two\nlines", "Two\rlines"] {
        let source = spec.source_with(|value| value["title"] = json!(title));
        assert_eq!(
            read_snapshot(&source).unwrap_err(),
            SnapshotLoadError::Manifest {
                reason: ManifestReason::LineBreak {
                    at: "/title".to_owned(),
                },
            },
            "{title:?}"
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

#[test]
fn a_manifest_over_the_size_cap_is_refused_before_parsing() {
    let spec = Spec::minimal();
    let source = spec.source_with_text(vec![b' '; MAX_MANIFEST_BYTES + 1]);
    assert_eq!(
        read_snapshot(&source).unwrap_err(),
        SnapshotLoadError::Manifest {
            reason: ManifestReason::TooLarge {
                size: MAX_MANIFEST_BYTES + 1,
                limit: MAX_MANIFEST_BYTES,
            },
        }
    );

    // The cap is checked before UTF-8: bytes that could never be text are
    // still reported as too large, not decoded.
    let source = spec.source_with_text(vec![0xff; MAX_MANIFEST_BYTES + 1]);
    assert!(matches!(
        manifest_reason(read_snapshot(&source).unwrap_err()),
        ManifestReason::TooLarge { .. }
    ));

    // Exactly at the cap is parsed (and here fails as the bad JSON it is).
    let source = spec.source_with_text(vec![b'\n'; MAX_MANIFEST_BYTES]);
    assert!(matches!(
        manifest_reason(read_snapshot(&source).unwrap_err()),
        ManifestReason::Malformed { .. }
    ));
}

#[test]
fn a_manifest_that_is_not_utf8_is_refused() {
    let spec = Spec::minimal();
    let mut bytes = spec.manifest().into_bytes();
    bytes[10] = 0xff;
    assert_eq!(
        read_snapshot(&spec.source_with_text(bytes)).unwrap_err(),
        SnapshotLoadError::ManifestNotUtf8
    );
}

#[test]
fn a_hostile_manifest_never_sizes_an_allocation_from_its_content() {
    // A wall of nested arrays and a huge declared size: the parser reads what
    // is present and fails; nothing reserves capacity from a number.
    let spec = Spec::minimal();
    let nested = format!("{}\n", "[".repeat(10_000));
    assert!(matches!(
        manifest_reason(read_snapshot(&spec.source_with_text(nested)).unwrap_err()),
        ManifestReason::Malformed { .. }
    ));
    let source = spec.source_with(|value| value["assets"][0]["size"] = json!(u32::MAX));
    assert!(matches!(
        read_snapshot(&source).unwrap_err(),
        SnapshotLoadError::WrongAssetSize { .. }
    ));
}

#[test]
fn the_committed_schema_pins_the_same_version_and_members() {
    // The schema is the contract for other tools; this keeps it from drifting
    // from the constants the reader enforces.
    let schema: Value = serde_json::from_str(
        &std::fs::read_to_string(
            std::path::Path::new(env!("CARGO_MANIFEST_DIR"))
                .join("../../contracts/snapshot-manifest/v2.schema.json"),
        )
        .unwrap(),
    )
    .unwrap();
    assert_eq!(schema["properties"]["schema_version"]["const"], json!(2));
    assert_eq!(schema["additionalProperties"], json!(false));
    let mut required: Vec<&str> = schema["required"]
        .as_array()
        .unwrap()
        .iter()
        .map(|member| member.as_str().unwrap())
        .collect();
    required.sort_unstable();
    let manifest = Spec::minimal().manifest_json();
    let mut members: Vec<&str> = manifest
        .as_object()
        .unwrap()
        .keys()
        .map(String::as_str)
        .collect();
    members.sort_unstable();
    assert_eq!(required, members, "every member the tests emit is required");
}
