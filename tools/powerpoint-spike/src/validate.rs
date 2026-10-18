//! Reopens a written deck and checks it against a [`DeckPlan`].
//!
//! [`outline`] extracts what the archive says (order, sections, shapes, notes,
//! media hashes); [`validate`] compares that with what the plan expects. The
//! two are separate so tests can inspect an outline directly.

use crate::plan::{Kind, Planned, plan_slides};
use crate::xmlscan::{Attrs, Ev, attr, events, paragraph_text, sections, starts, text_in, within};
use crate::{DeckPlan, FRAME_HEIGHT, FRAME_WIDTH, SLIDE_CX, SLIDE_CY, png_dims, sha256_hex};
use std::{
    collections::{HashMap, HashSet},
    io::{Cursor, Read},
};

/// Largest part accepted from the archive; anything bigger is an error.
const MAX_PART_BYTES: u64 = 64 * 1024 * 1024;

/// What one slide contains, as read from the archive.
#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) struct OutlineSlide {
    pub pictures: usize,
    pub shapes: usize,
    pub text: String,
    /// Background `srgbClr` value, if the slide has a `p:bg`.
    pub background: Option<String>,
    /// Picture offset and extent `(x, y, cx, cy)`, if there is one picture.
    pub picture_frame: Option<(String, String, String, String)>,
    pub no_change_aspect: bool,
    /// SHA-256 of the embedded media bytes.
    pub media_sha256: Option<String>,
    /// Decoded size of the embedded media.
    pub media_size: Option<(u32, u32)>,
    /// Notes text, if the slide has a notes slide.
    pub notes: Option<String>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) struct Outline {
    pub slide_size: (String, String),
    pub slides: Vec<OutlineSlide>,
    /// Section names with zero-based slide indices.
    pub sections: Vec<(String, Vec<usize>)>,
}

/// A relationship with its target resolved to a package part name.
struct Rel {
    id: String,
    ty: String,
    target: String,
    external: bool,
}

/// Validates `pptx` against `plan`. Returns every failure found.
pub fn validate(pptx: &[u8], plan: &DeckPlan) -> Result<(), Vec<String>> {
    let found = outline(pptx).map_err(|error| vec![error])?;
    let (slides, sections) = plan_slides(plan);
    let mut errors = Vec::new();

    if found.slide_size != (SLIDE_CX.to_string(), SLIDE_CY.to_string()) {
        errors.push(format!(
            "slide size is {}x{}, expected {SLIDE_CX}x{SLIDE_CY} (16:9)",
            found.slide_size.0, found.slide_size.1
        ));
    }
    if found.slides.len() != slides.len() {
        errors.push(format!(
            "expected {} slides, found {}",
            slides.len(),
            found.slides.len()
        ));
        return Err(errors);
    }
    if found.sections != sections {
        errors.push(format!(
            "sections differ: expected {sections:?}, found {:?}",
            found.sections
        ));
    }
    for (index, (planned, actual)) in slides.iter().zip(&found.slides).enumerate() {
        check_slide(index, planned, actual, &mut errors);
    }
    if errors.is_empty() {
        Ok(())
    } else {
        Err(errors)
    }
}

fn check_slide(
    index: usize,
    planned: &Planned<'_>,
    actual: &OutlineSlide,
    errors: &mut Vec<String>,
) {
    let number = index + 1;
    let label = &planned.label;
    if actual.shapes != 0 {
        errors.push(format!(
            "slide {number} ({label}): has {} shape(s), expected none",
            actual.shapes
        ));
    }
    if !actual.text.is_empty() {
        errors.push(format!(
            "slide {number} ({label}): has slide text {:?}",
            actual.text
        ));
    }
    match planned.kind {
        Kind::Image(png) => {
            if actual.pictures != 1 {
                errors.push(format!(
                    "slide {number} ({label}): {} pictures, expected 1",
                    actual.pictures
                ));
            }
            if actual.background.is_some() {
                errors.push(format!(
                    "slide {number} ({label}): image slide has a background override"
                ));
            }
            let full_bleed = (
                "0".to_owned(),
                "0".to_owned(),
                SLIDE_CX.to_string(),
                SLIDE_CY.to_string(),
            );
            if actual.picture_frame.as_ref() != Some(&full_bleed) {
                errors.push(format!(
                    "slide {number} ({label}): picture frame is {:?}, expected full bleed {full_bleed:?}",
                    actual.picture_frame
                ));
            }
            if !actual.no_change_aspect {
                errors.push(format!(
                    "slide {number} ({label}): picture lacks noChangeAspect"
                ));
            }
            if actual.notes.is_some() {
                errors.push(format!(
                    "slide {number} ({label}): congregation slide has notes"
                ));
            }
            if actual.media_sha256.as_deref() != Some(sha256_hex(png).as_str()) {
                errors.push(format!(
                    "slide {number} ({label}): image bytes differ from the frame"
                ));
            }
            let expected_size = Some((FRAME_WIDTH, FRAME_HEIGHT));
            if actual.media_size != expected_size {
                errors.push(format!(
                    "slide {number} ({label}): image is {:?}, expected {FRAME_WIDTH}x{FRAME_HEIGHT}",
                    actual.media_size
                ));
            }
        }
        Kind::Black(notes) => {
            if actual.pictures != 0 {
                errors.push(format!(
                    "slide {number} ({label}): operator slide has {} pictures",
                    actual.pictures
                ));
            }
            if actual.background.as_deref() != Some("000000") {
                errors.push(format!("slide {number} ({label}): background is not black"));
            }
            if actual.notes.as_deref() != Some(notes) {
                errors.push(format!(
                    "slide {number} ({label}): notes are {:?}, expected {notes:?}",
                    actual.notes
                ));
            }
        }
    }
}

/// Reads the archive into an [`Outline`]. Structural problems are errors.
pub(crate) fn outline(pptx: &[u8]) -> Result<Outline, String> {
    let parts = read_parts(pptx)?;
    for required in ["[Content_Types].xml", "_rels/.rels", "ppt/presentation.xml"] {
        if !parts.contains_key(required) {
            return Err(format!("missing required part {required}"));
        }
    }
    let integrity = package_integrity(&parts);
    if !integrity.is_empty() {
        return Err(integrity.join("; "));
    }
    let presentation = events_of(&parts, "ppt/presentation.xml")?;
    let size = starts(&presentation, "p:sldSz")
        .first()
        .map(|attrs| -> Result<(String, String), String> {
            Ok((
                required(attrs, "cx")?.to_owned(),
                required(attrs, "cy")?.to_owned(),
            ))
        })
        .ok_or_else(|| "presentation has no sldSz".to_owned())??;

    let pres_rels = rels_of(&parts, "ppt/presentation.xml")?;
    let mut slide_ids = Vec::new();
    let mut slides = Vec::new();
    for attrs in starts(&presentation, "p:sldId") {
        let id = required(attrs, "id")?.to_owned();
        let rid = required(attrs, "r:id")?;
        let target = find_rel(&pres_rels, rid)?;
        slide_ids.push(id);
        slides.push(read_slide(&parts, &target)?);
    }

    let mut found_sections = Vec::new();
    for (name, ids) in sections(&presentation) {
        let mut indices = Vec::with_capacity(ids.len());
        for id in &ids {
            let index = slide_ids
                .iter()
                .position(|known| known == id)
                .ok_or_else(|| format!("section {name} references unknown slide id {id}"))?;
            indices.push(index);
        }
        found_sections.push((name, indices));
    }

    Ok(Outline {
        slide_size: size,
        slides,
        sections: found_sections,
    })
}

fn read_slide(parts: &HashMap<String, Vec<u8>>, part: &str) -> Result<OutlineSlide, String> {
    let evs = events_of(parts, part)?;
    let rels = rels_of(parts, part)?;

    let pictures = starts(&evs, "p:pic").len();
    let shapes = ["p:sp", "p:grpSp", "p:graphicFrame", "p:cxnSp"]
        .iter()
        .map(|tag| starts(&evs, tag).len())
        .sum();
    let background = within(&evs, "p:bg")
        .into_iter()
        .find(|(tag, _)| *tag == "a:srgbClr")
        .and_then(|(_, attrs)| attr(attrs, "val").map(str::to_owned));

    let pic = within(&evs, "p:pic");
    let mut offset = None;
    let mut extent = None;
    for (tag, attrs) in &pic {
        match *tag {
            "a:off" => {
                offset = Some((
                    attr(attrs, "x").unwrap_or_default().to_owned(),
                    attr(attrs, "y").unwrap_or_default().to_owned(),
                ));
            }
            "a:ext" => {
                extent = Some((
                    attr(attrs, "cx").unwrap_or_default().to_owned(),
                    attr(attrs, "cy").unwrap_or_default().to_owned(),
                ));
            }
            _ => {}
        }
    }
    let picture_frame = match (offset, extent) {
        (Some((x, y)), Some((cx, cy))) => Some((x, y, cx, cy)),
        _ => None,
    };
    let no_change_aspect = pic
        .iter()
        .any(|(tag, attrs)| *tag == "a:picLocks" && attr(attrs, "noChangeAspect") == Some("1"));
    let embed = pic
        .iter()
        .find(|(tag, _)| *tag == "a:blip")
        .and_then(|(_, attrs)| attr(attrs, "r:embed"));

    let mut media_sha256 = None;
    let mut media_size = None;
    if let Some(rid) = embed {
        let target = find_rel(&rels, rid)?;
        let bytes = parts
            .get(&target)
            .ok_or_else(|| format!("{part}: missing media part {target}"))?;
        media_sha256 = Some(sha256_hex(bytes));
        media_size = Some(png_dims(bytes).map_err(|error| format!("{target}: {error}"))?);
    }

    let notes = match rels.iter().find(|rel| rel.ty.ends_with("/notesSlide")) {
        Some(rel) => Some(paragraph_text(&events_of(parts, &rel.target)?)),
        None => None,
    };

    Ok(OutlineSlide {
        pictures,
        shapes,
        text: text_in(&evs, "a:t"),
        background,
        picture_frame,
        no_change_aspect,
        media_sha256,
        media_size,
        notes,
    })
}

fn read_parts(pptx: &[u8]) -> Result<HashMap<String, Vec<u8>>, String> {
    let mut archive = zip::ZipArchive::new(Cursor::new(pptx))
        .map_err(|error| format!("not a readable zip: {error}"))?;
    let mut parts = HashMap::new();
    for index in 0..archive.len() {
        let mut file = archive
            .by_index(index)
            .map_err(|error| format!("unreadable zip entry {index}: {error}"))?;
        let name = file.name().to_owned();
        let mut bytes = Vec::new();
        file.by_ref()
            .take(MAX_PART_BYTES + 1)
            .read_to_end(&mut bytes)
            .map_err(|error| format!("cannot read {name}: {error}"))?;
        if bytes.len() as u64 > MAX_PART_BYTES {
            return Err(format!("{name} is larger than {MAX_PART_BYTES} bytes"));
        }
        if parts.insert(name.clone(), bytes).is_some() {
            return Err(format!("duplicate zip entry {name}"));
        }
    }
    Ok(parts)
}

/// Package-level checks that do not depend on the plan: content types, rels
/// targets, id uniqueness and notes wiring. Returns every failure found.
fn package_integrity(parts: &HashMap<String, Vec<u8>>) -> Vec<String> {
    let mut errors = Vec::new();
    let content_types = match events_of(parts, "[Content_Types].xml") {
        Ok(evs) => evs,
        Err(error) => return vec![error],
    };
    let defaults = content_types_of(&content_types, "Default", "Extension");
    let overrides: HashMap<String, String> =
        content_types_of(&content_types, "Override", "PartName")
            .into_iter()
            .map(|(name, ty)| (name.trim_start_matches('/').to_owned(), ty))
            .collect();
    for name in overrides.keys() {
        if !parts.contains_key(name) {
            errors.push(format!(
                "Override names part {name}, which is not in the package"
            ));
        }
    }
    let effective = |name: &str| -> Option<&String> {
        let extension = name
            .rsplit_once('.')
            .map(|(_, ext)| ext)
            .unwrap_or_default();
        overrides.get(name).or_else(|| defaults.get(extension))
    };
    for name in parts.keys() {
        if name == "[Content_Types].xml" {
            continue;
        }
        // Every XML part here has a specific type, so a generic `xml` Default
        // (application/xml) is not enough: those parts need an Override.
        let extension = name
            .rsplit_once('.')
            .map(|(_, ext)| ext)
            .unwrap_or_default();
        let covered = if extension == "xml" {
            overrides.contains_key(name)
        } else {
            overrides.contains_key(name) || defaults.contains_key(extension)
        };
        if !covered {
            errors.push(format!("part {name} has no content type"));
        }
    }

    for name in parts.keys().filter(|name| name.ends_with(".rels")) {
        match effective(name) {
            Some(ty) if ty == RELS_TYPE => {}
            found => errors.push(format!(
                "{name}: content type {} is not {RELS_TYPE}",
                found.map_or("(none)", String::as_str)
            )),
        }
        let base = rels_base_dir(name);
        let rels = match parse_rels(parts, name, &base) {
            Ok(rels) => rels,
            Err(error) => {
                errors.push(error);
                continue;
            }
        };
        let mut ids = HashSet::new();
        for rel in &rels {
            if !ids.insert(rel.id.as_str()) {
                errors.push(format!("{name}: duplicate relationship id {}", rel.id));
            }
            if rel.external {
                continue;
            }
            if !parts.contains_key(&rel.target) {
                errors.push(format!("{name}: target {} is missing", rel.target));
                continue;
            }
            let role = rel.ty.rsplit('/').next().unwrap_or_default();
            if let Some(expected) = required_type(role) {
                let found = effective(&rel.target).map_or("(none)", String::as_str);
                if found != expected {
                    errors.push(format!(
                        "{}: content type {found} but {name} relationship {} ({role}) requires {expected}",
                        rel.target, rel.id
                    ));
                }
            }
        }
    }

    match events_of(parts, "ppt/presentation.xml") {
        Ok(presentation) => {
            let mut seen = HashSet::new();
            for attrs in starts(&presentation, "p:sldId") {
                match attr(attrs, "id").and_then(|id| id.parse::<u32>().ok()) {
                    Some(id) if id >= 256 => {
                        if !seen.insert(id) {
                            errors.push(format!("duplicate sldId {id}"));
                        }
                    }
                    _ => errors.push("sldId is missing, not a number, or below 256".to_owned()),
                }
            }
        }
        Err(error) => errors.push(error),
    }

    for name in parts.keys() {
        let Some(number) = name
            .strip_prefix("ppt/notesSlides/notesSlide")
            .and_then(|rest| rest.strip_suffix(".xml"))
        else {
            continue;
        };
        let rels_name = format!("ppt/notesSlides/_rels/notesSlide{number}.xml.rels");
        match parse_rels(parts, &rels_name, "ppt/notesSlides") {
            Ok(rels) => {
                let slide = format!("ppt/slides/slide{number}.xml");
                let points_to_slide = rels
                    .iter()
                    .any(|rel| rel.ty.ends_with("/slide") && rel.target == slide);
                let points_to_master = rels.iter().any(|rel| {
                    rel.ty.ends_with("/notesMaster")
                        && rel.target == "ppt/notesMasters/notesMaster1.xml"
                });
                if !points_to_slide {
                    errors.push(format!("{name} does not point to {slide}"));
                }
                if !points_to_master {
                    errors.push(format!("{name} does not point to notesMaster1"));
                }
            }
            Err(error) => errors.push(error),
        }
    }
    errors
}

const RELS_TYPE: &str = "application/vnd.openxmlformats-package.relationships+xml";

/// `(key attribute, ContentType)` of every `tag` element in `[Content_Types].xml`.
fn content_types_of(evs: &[Ev], tag: &str, key: &str) -> HashMap<String, String> {
    starts(evs, tag)
        .into_iter()
        .filter_map(|attrs| {
            Some((
                attr(attrs, key)?.to_owned(),
                attr(attrs, "ContentType")?.to_owned(),
            ))
        })
        .collect()
}

/// Content type a relationship's target must have, keyed by the last segment
/// of the relationship type. Written independently of the generator.
fn required_type(role: &str) -> Option<&'static str> {
    Some(match role {
        "officeDocument" => {
            "application/vnd.openxmlformats-officedocument.presentationml.presentation.main+xml"
        }
        "slide" => "application/vnd.openxmlformats-officedocument.presentationml.slide+xml",
        "slideLayout" => {
            "application/vnd.openxmlformats-officedocument.presentationml.slideLayout+xml"
        }
        "slideMaster" => {
            "application/vnd.openxmlformats-officedocument.presentationml.slideMaster+xml"
        }
        "notesSlide" => {
            "application/vnd.openxmlformats-officedocument.presentationml.notesSlide+xml"
        }
        "notesMaster" => {
            "application/vnd.openxmlformats-officedocument.presentationml.notesMaster+xml"
        }
        "theme" => "application/vnd.openxmlformats-officedocument.theme+xml",
        "presProps" => "application/vnd.openxmlformats-officedocument.presentationml.presProps+xml",
        "viewProps" => "application/vnd.openxmlformats-officedocument.presentationml.viewProps+xml",
        "tableStyles" => {
            "application/vnd.openxmlformats-officedocument.presentationml.tableStyles+xml"
        }
        "core-properties" => "application/vnd.openxmlformats-package.core-properties+xml",
        "extended-properties" => {
            "application/vnd.openxmlformats-officedocument.extended-properties+xml"
        }
        "image" => "image/png",
        _ => return None,
    })
}

/// Directory that the targets of a `.rels` part are relative to.
fn rels_base_dir(rels_name: &str) -> String {
    let (dir, _) = rels_name.rsplit_once('/').unwrap_or(("", rels_name));
    match dir.strip_suffix("_rels") {
        Some(base) => base.trim_end_matches('/').to_owned(),
        None => String::new(),
    }
}

/// A required attribute; a missing one is a validation error, never "".
fn required<'a>(attrs: &'a Attrs, key: &str) -> Result<&'a str, String> {
    attr(attrs, key).ok_or_else(|| format!("element is missing attribute {key}"))
}

fn events_of(parts: &HashMap<String, Vec<u8>>, name: &str) -> Result<Vec<Ev>, String> {
    let bytes = parts
        .get(name)
        .ok_or_else(|| format!("missing part {name}"))?;
    events(bytes).map_err(|error| format!("{name}: {error}"))
}

/// Relationships of `source`, with targets resolved to package part names.
fn rels_of(parts: &HashMap<String, Vec<u8>>, source: &str) -> Result<Vec<Rel>, String> {
    let (dir, file) = source.rsplit_once('/').unwrap_or(("", source));
    let rels_name = if dir.is_empty() {
        format!("_rels/{file}.rels")
    } else {
        format!("{dir}/_rels/{file}.rels")
    };
    parse_rels(parts, &rels_name, dir)
}

/// Parses the `.rels` part `rels_name`, resolving targets against `base`.
fn parse_rels(
    parts: &HashMap<String, Vec<u8>>,
    rels_name: &str,
    base: &str,
) -> Result<Vec<Rel>, String> {
    let evs = events_of(parts, rels_name)?;
    starts(&evs, "Relationship")
        .into_iter()
        .map(|attrs| {
            Ok(Rel {
                id: required(attrs, "Id")?.to_owned(),
                ty: required(attrs, "Type")?.to_owned(),
                target: resolve(base, required(attrs, "Target")?),
                external: attr(attrs, "TargetMode") == Some("External"),
            })
        })
        .collect()
}

fn find_rel(rels: &[Rel], id: &str) -> Result<String, String> {
    rels.iter()
        .find(|rel| rel.id == id)
        .map(|rel| rel.target.clone())
        .ok_or_else(|| format!("relationship {id} not found"))
}

/// Resolves a relationship target against the directory of its source part.
fn resolve(base: &str, target: &str) -> String {
    let mut segments: Vec<&str> = if target.starts_with('/') {
        Vec::new()
    } else {
        base.split('/')
            .filter(|segment| !segment.is_empty())
            .collect()
    };
    for segment in target.trim_start_matches('/').split('/') {
        match segment {
            "" | "." => {}
            ".." => {
                segments.pop();
            }
            other => segments.push(other),
        }
    }
    segments.join("/")
}
