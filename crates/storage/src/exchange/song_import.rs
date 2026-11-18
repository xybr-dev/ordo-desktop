//! Decoding the song-import v2 payload into the domain's import document.
//!
//! One pass over the JSON collects every shape problem with an RFC 6901
//! pointer (so a reviewer sees all of them at once), then the semantic rules
//! (references, content hash), then the typed conversion and the domain's
//! whole-document validation.

use std::collections::BTreeSet;
use std::path::Path;

use domain::identity::{ArrangementId, SongId};
use domain::song_library::import::{
    DocumentProblem, ImportArrangement, ImportDocument, ImportSection, SequenceEntry,
    rendition_attribution, resolve_language, validate_document,
};
use domain::song_library::{Field, SongDraft};
use serde_json::{Map, Value, json};
use sha2::{Digest, Sha256};

use super::container::{InspectLimits, Opened, Refusal, open};
use super::findings::{Finding, FindingCode};

const KIND: &str = "song-import";
const VERSION: u64 = 2;

/// The outcome of reading one `.ordo` file as a Song import.
// A document is produced once per file read; boxing it would cost the callers
// a deref for no gain.
#[allow(clippy::large_enum_variant)]
#[derive(Clone, Debug, Eq, PartialEq)]
pub enum SongImportRead {
    /// Every check passed; the typed document is ready to classify.
    Valid(ImportDocument),
    /// The file is malformed, unsafe, tampered with, of another kind or breaks
    /// the contract; the document is excluded whole (Decision 137).
    Invalid(Vec<Finding>),
    /// The file is well formed but from a newer Ordo, or of an unknown kind.
    Unsupported {
        /// The compatible-version message.
        message: String,
    },
}

/// Reads one file: container rules, manifest, payload hash, kind song-import,
/// version 2, payload contract and semantics, then the domain's
/// [`validate_document`]. Reads the archive into memory (64 MiB cap by
/// default); writes nothing anywhere.
pub fn read_song_import(path: &Path, limits: &InspectLimits) -> SongImportRead {
    match std::fs::metadata(path) {
        Ok(metadata) if metadata.len() > limits.max_archive => {
            return SongImportRead::Invalid(vec![Finding::container(format!(
                "The file is {} bytes; the limit is {} bytes.",
                metadata.len(),
                limits.max_archive
            ))]);
        }
        Ok(_) => {}
        Err(error) => return unreadable(&error),
    }
    match std::fs::read(path) {
        Ok(bytes) => read_song_import_bytes(&bytes, limits),
        Err(error) => unreadable(&error),
    }
}

fn unreadable(error: &std::io::Error) -> SongImportRead {
    SongImportRead::Invalid(vec![Finding::container(format!(
        "The file could not be read: {error}"
    ))])
}

/// The bytes variant of [`read_song_import`]; the path variant reads the file
/// and calls this.
pub fn read_song_import_bytes(bytes: &[u8], limits: &InspectLimits) -> SongImportRead {
    match open(bytes, limits, KIND, VERSION) {
        Ok(Opened { payload }) => decode_payload(&payload),
        Err(Refusal::Invalid(findings)) => SongImportRead::Invalid(findings),
        Err(Refusal::Unsupported(message)) => SongImportRead::Unsupported { message },
    }
}

fn decode_payload(payload: &[u8]) -> SongImportRead {
    let value: Value = match serde_json::from_slice(payload) {
        Ok(value) => value,
        Err(error) => {
            return SongImportRead::Invalid(vec![Finding::new(
                FindingCode::Schema,
                "",
                format!("payload.json is not valid JSON: {error}"),
            )]);
        }
    };
    match decode_value(&value) {
        Ok(document) => SongImportRead::Valid(document),
        Err(findings) => SongImportRead::Invalid(findings),
    }
}

/// An Arrangement as the JSON carries it, before references and hash.
struct RawArrangement {
    id: String,
    title: Option<String>,
    languages: Vec<String>,
    choirs: Vec<String>,
    translator: Option<String>,
    adapter: Option<String>,
    occasions: Vec<String>,
    notes: Option<String>,
    warnings: Vec<String>,
    hash: String,
    sections: Vec<RawSection>,
    sequence: Vec<String>,
}

struct RawSection {
    id: String,
    label: Option<String>,
    slides: Vec<Vec<String>>,
}

#[derive(Clone, Copy)]
enum Pattern {
    Uuid,
    Text,
    Line,
    Sha256,
}

struct Reader {
    findings: Vec<Finding>,
    ids: BTreeSet<String>,
}

fn child(pointer: &str, key: &str) -> String {
    format!("{pointer}/{}", key.replace('~', "~0").replace('/', "~1"))
}

fn at(pointer: &str, position: usize) -> String {
    format!("{pointer}/{position}")
}

fn is_uuid_v4(text: &str) -> bool {
    let bytes = text.as_bytes();
    if bytes.len() != 36 {
        return false;
    }
    bytes
        .iter()
        .enumerate()
        .all(|(position, byte)| match position {
            8 | 13 | 18 | 23 => *byte == b'-',
            14 => *byte == b'4',
            19 => matches!(byte, b'8' | b'9' | b'a' | b'b'),
            _ => matches!(byte, b'0'..=b'9' | b'a'..=b'f'),
        })
}

impl Reader {
    fn schema(&mut self, pointer: &str, message: impl Into<String>) {
        self.findings
            .push(Finding::new(FindingCode::Schema, pointer, message));
    }

    fn object<'a>(&mut self, value: &'a Value, pointer: &str) -> Option<&'a Map<String, Value>> {
        let map = value.as_object();
        if map.is_none() {
            self.schema(pointer, "must be a JSON object");
        }
        map
    }

    fn reject_unknown(&mut self, map: &Map<String, Value>, allowed: &[&str], pointer: &str) {
        for key in map.keys() {
            if !allowed.contains(&key.as_str()) {
                self.schema(&child(pointer, key), "unknown field");
            }
        }
    }

    /// A string at `pointer` matching `pattern`, or a finding.
    fn string(&mut self, value: &Value, pointer: &str, pattern: Pattern) -> Option<String> {
        let Some(text) = value.as_str() else {
            self.schema(pointer, "must be a string");
            return None;
        };
        let (valid, rule) = match pattern {
            Pattern::Uuid => (is_uuid_v4(text), "must be a lowercase UUIDv4"),
            Pattern::Text => (
                text.chars().any(|c| !c.is_whitespace()),
                "must contain a non-whitespace character",
            ),
            Pattern::Line => (
                !text.contains(['\r', '\n']) && text.chars().any(|c| !c.is_whitespace()),
                "must be one non-blank line without a line break",
            ),
            Pattern::Sha256 => (
                text.len() == 64 && text.bytes().all(|b| matches!(b, b'0'..=b'9' | b'a'..=b'f')),
                "must be a lowercase hex SHA-256",
            ),
        };
        if !valid {
            self.schema(pointer, rule);
            return None;
        }
        Some(text.to_owned())
    }

    fn required(
        &mut self,
        map: &Map<String, Value>,
        key: &str,
        pointer: &str,
        pattern: Pattern,
    ) -> Option<String> {
        let here = child(pointer, key);
        match map.get(key) {
            Some(value) => self.string(value, &here, pattern),
            None => {
                self.schema(&here, "required field is missing");
                None
            }
        }
    }

    fn optional(
        &mut self,
        map: &Map<String, Value>,
        key: &str,
        pointer: &str,
        pattern: Pattern,
    ) -> Option<String> {
        let value = map.get(key)?;
        self.string(value, &child(pointer, key), pattern)
    }

    /// A list of strings; `min` is its least length.
    fn list(
        &mut self,
        map: &Map<String, Value>,
        key: &str,
        pointer: &str,
        pattern: Pattern,
        min: usize,
    ) -> Vec<String> {
        let here = child(pointer, key);
        let Some(value) = map.get(key) else {
            if min > 0 {
                self.schema(&here, "required field is missing");
            }
            return Vec::new();
        };
        let Some(items) = value.as_array() else {
            self.schema(&here, "must be an array");
            return Vec::new();
        };
        if items.len() < min {
            self.schema(&here, format!("must contain at least {min} item(s)"));
        }
        items
            .iter()
            .enumerate()
            .filter_map(|(position, item)| self.string(item, &at(&here, position), pattern))
            .collect()
    }

    /// An id that must also be new to the document.
    fn unique_id(&mut self, id: Option<String>, pointer: &str) -> Option<String> {
        let id = id?;
        if !self.ids.insert(id.clone()) {
            self.schema(pointer, format!("duplicate id {id}"));
        }
        Some(id)
    }
}

fn decode_value(value: &Value) -> Result<ImportDocument, Vec<Finding>> {
    let mut reader = Reader {
        findings: Vec::new(),
        ids: BTreeSet::new(),
    };
    let Some(map) = reader.object(value, "") else {
        return Err(reader.findings);
    };
    match map.get("schema_version") {
        None => {
            reader.schema("/schema_version", "required field is missing");
            return Err(reader.findings);
        }
        Some(version) => match version.as_u64() {
            Some(VERSION) => {}
            Some(found) => {
                reader.findings.push(Finding::new(
                    FindingCode::UnsupportedVersion,
                    "/schema_version",
                    format!("schema_version {found} is not supported by this build (v{VERSION})"),
                ));
                return Err(reader.findings);
            }
            None => {
                reader.schema("/schema_version", "must be an integer");
                return Err(reader.findings);
            }
        },
    }
    reader.reject_unknown(
        map,
        &[
            "schema_version",
            "id",
            "title",
            "aliases",
            "composer",
            "source",
            "rights",
            "liturgical_moments",
            "occasions",
            "arrangements",
        ],
        "",
    );
    let id = reader.required(map, "id", "", Pattern::Uuid);
    let id = reader.unique_id(id, "/id");
    let title = reader.required(map, "title", "", Pattern::Text);
    let aliases = reader.list(map, "aliases", "", Pattern::Text, 0);
    let composer = reader.optional(map, "composer", "", Pattern::Text);
    let source = reader.optional(map, "source", "", Pattern::Text);
    let rights = reader.optional(map, "rights", "", Pattern::Text);
    let moments = reader.list(map, "liturgical_moments", "", Pattern::Text, 0);
    let occasions = reader.list(map, "occasions", "", Pattern::Text, 0);
    let arrangements = read_arrangements(&mut reader, map);

    if !reader.findings.is_empty() {
        return Err(reader.findings);
    }
    semantics(&mut reader.findings, &arrangements);
    if !reader.findings.is_empty() {
        return Err(reader.findings);
    }
    let (Some(id), Some(title)) = (id, title) else {
        return Err(reader.findings);
    };
    let Ok(song_id) = SongId::parse(&id) else {
        return Err(vec![Finding::new(
            FindingCode::Schema,
            "/id",
            "must be a lowercase UUIDv4",
        )]);
    };
    let mut converted = Vec::with_capacity(arrangements.len());
    for (position, arrangement) in arrangements.iter().enumerate() {
        match convert(arrangement) {
            Some(arrangement) => converted.push(arrangement),
            None => {
                return Err(vec![Finding::new(
                    FindingCode::Schema,
                    at("/arrangements", position),
                    "has an id that is not a lowercase UUIDv4",
                )]);
            }
        }
    }
    let document = ImportDocument {
        id: song_id,
        song: SongDraft {
            title,
            aliases,
            original_attribution: composer,
            provenance: source,
            rights_notes: rights,
            liturgical_moments: moments,
            occasions,
        },
        arrangements: converted,
        warnings: Vec::new(),
    };
    let problems = validate_document(&document);
    if problems.is_empty() {
        Ok(document)
    } else {
        Err(problems.iter().map(problem_finding).collect())
    }
}

fn read_arrangements(reader: &mut Reader, map: &Map<String, Value>) -> Vec<RawArrangement> {
    let here = "/arrangements";
    let Some(value) = map.get("arrangements") else {
        reader.schema(here, "required field is missing");
        return Vec::new();
    };
    let Some(items) = value.as_array() else {
        reader.schema(here, "must be an array");
        return Vec::new();
    };
    if items.is_empty() {
        reader.schema(here, "must contain at least one Arrangement");
    }
    items
        .iter()
        .enumerate()
        .filter_map(|(position, item)| read_arrangement(reader, item, &at(here, position)))
        .collect()
}

fn read_arrangement(reader: &mut Reader, value: &Value, pointer: &str) -> Option<RawArrangement> {
    let map = reader.object(value, pointer)?;
    reader.reject_unknown(
        map,
        &[
            "id",
            "title",
            "languages",
            "choirs",
            "translator",
            "adapter",
            "occasions",
            "notes",
            "extraction_warnings",
            "content_sha256",
            "sections",
            "sequence",
        ],
        pointer,
    );
    let id = reader.required(map, "id", pointer, Pattern::Uuid);
    let id = reader.unique_id(id, &child(pointer, "id"));
    let title = reader.optional(map, "title", pointer, Pattern::Text);
    let languages = reader.list(map, "languages", pointer, Pattern::Text, 1);
    let choirs = reader.list(map, "choirs", pointer, Pattern::Text, 0);
    let translator = reader.optional(map, "translator", pointer, Pattern::Text);
    let adapter = reader.optional(map, "adapter", pointer, Pattern::Text);
    let occasions = reader.list(map, "occasions", pointer, Pattern::Text, 0);
    let notes = reader.optional(map, "notes", pointer, Pattern::Text);
    let warnings = reader.list(map, "extraction_warnings", pointer, Pattern::Text, 0);
    let hash = reader.required(map, "content_sha256", pointer, Pattern::Sha256);
    let sequence = reader.list(map, "sequence", pointer, Pattern::Uuid, 1);

    let sections_pointer = child(pointer, "sections");
    let mut sections = Vec::new();
    match map.get("sections") {
        None => reader.schema(&sections_pointer, "required field is missing"),
        Some(value) => match value.as_array() {
            None => reader.schema(&sections_pointer, "must be an array"),
            Some(items) => {
                if items.is_empty() {
                    reader.schema(&sections_pointer, "must contain at least one section");
                }
                for (position, item) in items.iter().enumerate() {
                    if let Some(section) =
                        read_section(reader, item, &at(&sections_pointer, position))
                    {
                        sections.push(section);
                    }
                }
            }
        },
    }
    Some(RawArrangement {
        id: id?,
        title,
        languages,
        choirs,
        translator,
        adapter,
        occasions,
        notes,
        warnings,
        hash: hash?,
        sections,
        sequence,
    })
}

fn read_section(reader: &mut Reader, value: &Value, pointer: &str) -> Option<RawSection> {
    let map = reader.object(value, pointer)?;
    reader.reject_unknown(map, &["id", "label", "slides"], pointer);
    let id = reader.required(map, "id", pointer, Pattern::Uuid);
    let id = reader.unique_id(id, &child(pointer, "id"));
    let label = reader.optional(map, "label", pointer, Pattern::Text);

    let slides_pointer = child(pointer, "slides");
    let mut slides = Vec::new();
    match map.get("slides") {
        None => reader.schema(&slides_pointer, "required field is missing"),
        Some(value) => match value.as_array() {
            None => reader.schema(&slides_pointer, "must be an array"),
            Some(items) => {
                if items.is_empty() {
                    reader.schema(&slides_pointer, "must contain at least one slide");
                }
                for (position, item) in items.iter().enumerate() {
                    let slide_pointer = at(&slides_pointer, position);
                    if let Some(slide) = reader.object(item, &slide_pointer) {
                        reader.reject_unknown(slide, &["lines"], &slide_pointer);
                        slides.push(reader.list(slide, "lines", &slide_pointer, Pattern::Line, 1));
                    }
                }
            }
        },
    }
    Some(RawSection {
        id: id?,
        label,
        slides,
    })
}

/// Every Sequence entry names a section of its Arrangement; the recorded
/// content hash matches the lines.
fn semantics(findings: &mut Vec<Finding>, arrangements: &[RawArrangement]) {
    for (position, arrangement) in arrangements.iter().enumerate() {
        let here = at("/arrangements", position);
        let known: BTreeSet<&str> = arrangement
            .sections
            .iter()
            .map(|section| section.id.as_str())
            .collect();
        for (entry, reference) in arrangement.sequence.iter().enumerate() {
            if !known.contains(reference.as_str()) {
                findings.push(Finding::new(
                    FindingCode::MissingReference,
                    at(&child(&here, "sequence"), entry),
                    format!("references unknown section {reference}"),
                ));
            }
        }
        let computed = content_hash(
            arrangement
                .sections
                .iter()
                .flat_map(|section| section.slides.iter().flatten().map(String::as_str)),
        );
        if computed != arrangement.hash {
            findings.push(Finding::new(
                FindingCode::HashMismatch,
                child(&here, "content_sha256"),
                format!("recorded {}, recomputed {computed}", arrangement.hash),
            ));
        }
    }
}

/// SHA-256 of every line, each followed by LF.
fn content_hash<'a>(lines: impl Iterator<Item = &'a str>) -> String {
    let mut hasher = Sha256::new();
    for line in lines {
        hasher.update(line.as_bytes());
        hasher.update(b"\n");
    }
    hasher
        .finalize()
        .iter()
        .map(|byte| format!("{byte:02x}"))
        .collect()
}

fn convert(raw: &RawArrangement) -> Option<ImportArrangement> {
    let sections: Vec<&str> = raw.sections.iter().map(|s| s.id.as_str()).collect();
    let sequence = raw
        .sequence
        .iter()
        .map(|reference| {
            sections
                .iter()
                .position(|id| id == reference)
                .map(SequenceEntry::Section)
        })
        .collect::<Option<Vec<_>>>()?;
    Some(ImportArrangement {
        id: ArrangementId::parse(&raw.id).ok()?,
        title: raw.title.clone(),
        languages: raw
            .languages
            .iter()
            .map(|language| {
                resolve_language(language).map_or_else(|| language.clone(), str::to_owned)
            })
            .collect(),
        choirs: raw.choirs.clone(),
        rendition_attribution: rendition_attribution(
            raw.translator.as_deref(),
            raw.adapter.as_deref(),
        ),
        occasions: raw.occasions.clone(),
        notes: raw.notes.clone(),
        sections: raw
            .sections
            .iter()
            .map(|section| ImportSection {
                label: section.label.clone(),
                lyrics: section
                    .slides
                    .iter()
                    .map(|slide| slide.join("\n"))
                    .collect::<Vec<_>>()
                    .join("\n\n"),
            })
            .collect(),
        sequence,
        warnings: raw.warnings.clone(),
    })
}

/// The pointer a Song or Arrangement field lives at in the payload.
fn field_pointer(arrangement: Option<usize>, field: Field) -> String {
    let member = match field {
        Field::OriginalAttribution => "composer",
        Field::Provenance => "source",
        Field::RightsNotes => "rights",
        Field::Aliases => "aliases",
        Field::LiturgicalMoment => "liturgical_moments",
        Field::Occasion => "occasions",
        Field::Language => "languages",
        Field::ArrangementTitle => "title",
        Field::RenditionAttribution => "translator",
        Field::ArrangementNotes => "notes",
        Field::Choir => "choirs",
        Field::SectionLabel => "sections",
        _ => "",
    };
    let base = arrangement.map_or_else(String::new, |index| at("/arrangements", index));
    if member.is_empty() {
        base
    } else {
        child(&base, member)
    }
}

fn problem_finding(problem: &DocumentProblem) -> Finding {
    let (pointer, message) = match problem {
        DocumentProblem::BlankTitle => ("/title".to_owned(), "the title is blank".to_owned()),
        DocumentProblem::UnlistedLanguage { arrangement, value } => (
            child(&at("/arrangements", *arrangement), "languages"),
            format!("{value:?} is not one of Ordo's languages"),
        ),
        DocumentProblem::NoLanguage { arrangement } => (
            child(&at("/arrangements", *arrangement), "languages"),
            "names no language".to_owned(),
        ),
        DocumentProblem::UnknownSection {
            arrangement,
            position,
        } => (
            at(
                &child(&at("/arrangements", *arrangement), "sequence"),
                *position,
            ),
            "names a section the Arrangement does not have".to_owned(),
        ),
        DocumentProblem::BlankField { arrangement, field } => (
            field_pointer(*arrangement, *field),
            format!("{field:?} is blank"),
        ),
        DocumentProblem::Duplicate { arrangement, field } => (
            field_pointer(*arrangement, *field),
            format!("{field:?} repeats a value"),
        ),
        DocumentProblem::DuplicateArrangement(id) => (
            "/arrangements".to_owned(),
            format!("Arrangement {id} appears twice"),
        ),
    };
    Finding::new(FindingCode::Schema, pointer, message)
}

/// The canonical v2 payload of `document`, for the round-trip gate test.
///
/// Section ids are derived from the Arrangement id and the section's position
/// (a machine document's section ids are never kept locally), so the same
/// document always encodes to the same JSON. Optional members are omitted; an
/// unresolved Markdown repeat has no machine form and is left out of the
/// Sequence.
pub fn encode_song_import(document: &ImportDocument) -> Value {
    let mut root = Map::new();
    root.insert("schema_version".to_owned(), json!(VERSION));
    root.insert("id".to_owned(), json!(document.id.to_string()));
    root.insert("title".to_owned(), json!(document.song.title));
    insert_list(&mut root, "aliases", &document.song.aliases);
    insert_text(
        &mut root,
        "composer",
        document.song.original_attribution.as_ref(),
    );
    insert_text(&mut root, "source", document.song.provenance.as_ref());
    insert_text(&mut root, "rights", document.song.rights_notes.as_ref());
    insert_list(
        &mut root,
        "liturgical_moments",
        &document.song.liturgical_moments,
    );
    insert_list(&mut root, "occasions", &document.song.occasions);
    let arrangements: Vec<Value> = document
        .arrangements
        .iter()
        .map(encode_arrangement)
        .collect();
    root.insert("arrangements".to_owned(), Value::Array(arrangements));
    Value::Object(root)
}

fn insert_text(map: &mut Map<String, Value>, key: &str, value: Option<&String>) {
    if let Some(value) = value {
        map.insert(key.to_owned(), json!(value));
    }
}

fn insert_list(map: &mut Map<String, Value>, key: &str, values: &[String]) {
    if !values.is_empty() {
        map.insert(key.to_owned(), json!(values));
    }
}

fn encode_arrangement(arrangement: &ImportArrangement) -> Value {
    let section_ids: Vec<String> = (0..arrangement.sections.len())
        .map(|index| derived_section_id(arrangement.id, index))
        .collect();
    let slides_of = |lyrics: &str| -> Vec<Vec<String>> {
        lyrics
            .split("\n\n")
            .map(|slide| slide.split('\n').map(str::to_owned).collect())
            .collect()
    };
    let mut lines = Vec::new();
    let sections: Vec<Value> = arrangement
        .sections
        .iter()
        .zip(&section_ids)
        .map(|(section, id)| {
            let slides = slides_of(&section.lyrics);
            lines.extend(slides.iter().flatten().cloned());
            let mut object = Map::new();
            object.insert("id".to_owned(), json!(id));
            insert_text(&mut object, "label", section.label.as_ref());
            object.insert(
                "slides".to_owned(),
                Value::Array(
                    slides
                        .iter()
                        .map(|slide| json!({ "lines": slide }))
                        .collect(),
                ),
            );
            Value::Object(object)
        })
        .collect();
    let sequence: Vec<&String> = arrangement
        .sequence
        .iter()
        .filter_map(|entry| match entry {
            SequenceEntry::Section(index) => section_ids.get(*index),
            SequenceEntry::Unresolved { .. } => None,
        })
        .collect();
    let mut object = Map::new();
    object.insert("id".to_owned(), json!(arrangement.id.to_string()));
    insert_text(&mut object, "title", arrangement.title.as_ref());
    object.insert("languages".to_owned(), json!(arrangement.languages));
    insert_list(&mut object, "choirs", &arrangement.choirs);
    insert_text(
        &mut object,
        "translator",
        arrangement.rendition_attribution.as_ref(),
    );
    insert_list(&mut object, "occasions", &arrangement.occasions);
    insert_text(&mut object, "notes", arrangement.notes.as_ref());
    insert_list(&mut object, "extraction_warnings", &arrangement.warnings);
    object.insert(
        "content_sha256".to_owned(),
        json!(content_hash(lines.iter().map(String::as_str))),
    );
    object.insert("sections".to_owned(), Value::Array(sections));
    object.insert("sequence".to_owned(), json!(sequence));
    Value::Object(object)
}

/// A stable lowercase UUIDv4-shaped id for section `index` of `arrangement`.
fn derived_section_id(arrangement: ArrangementId, index: usize) -> String {
    let digest = Sha256::digest(format!("{arrangement}:{index}").as_bytes());
    let mut bytes = [0_u8; 16];
    bytes.copy_from_slice(&digest[..16]);
    bytes[6] = (bytes[6] & 0x0f) | 0x40;
    bytes[8] = (bytes[8] & 0x3f) | 0x80;
    let hex: String = bytes.iter().map(|byte| format!("{byte:02x}")).collect();
    format!(
        "{}-{}-{}-{}-{}",
        &hex[0..8],
        &hex[8..12],
        &hex[12..16],
        &hex[16..20],
        &hex[20..32]
    )
}
