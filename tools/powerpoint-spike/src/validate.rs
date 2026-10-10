//! Reopens a written deck and checks it against a [`DeckPlan`].
//!
//! [`outline`] extracts what the archive says (order, sections, shapes, notes,
//! media hashes); [`validate`] compares that with what the plan expects. The
//! two are separate so tests can inspect an outline directly.

use crate::plan::{Kind, Planned, plan_slides};
use crate::xmlscan::{Ev, attr, events, sections, starts, text_in, within};
use crate::{DeckPlan, FRAME_HEIGHT, FRAME_WIDTH, SLIDE_CX, SLIDE_CY, png_dims, sha256_hex};
use std::{collections::HashMap, io::Cursor, io::Read};

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
    let presentation = events_of(&parts, "ppt/presentation.xml")?;
    let size = starts(&presentation, "p:sldSz")
        .first()
        .map(|attrs| {
            (
                attr(attrs, "cx").unwrap_or_default().to_owned(),
                attr(attrs, "cy").unwrap_or_default().to_owned(),
            )
        })
        .ok_or_else(|| "presentation has no sldSz".to_owned())?;

    let pres_rels = rels_of(&parts, "ppt/presentation.xml")?;
    let mut slide_ids = Vec::new();
    let mut slides = Vec::new();
    for attrs in starts(&presentation, "p:sldId") {
        let id = attr(attrs, "id").unwrap_or_default().to_owned();
        let rid = attr(attrs, "r:id").unwrap_or_default();
        let target = find_rel(&pres_rels, rid)?;
        slide_ids.push(id);
        slides.push(read_slide(&parts, &target)?);
    }

    let found_sections = sections(&presentation)
        .into_iter()
        .map(|(name, ids)| {
            let indices = ids
                .iter()
                .map(|id| {
                    slide_ids
                        .iter()
                        .position(|known| known == id)
                        .unwrap_or(usize::MAX)
                })
                .collect();
            (name, indices)
        })
        .collect();

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
        Some(rel) => Some(text_in(&events_of(parts, &rel.target)?, "a:t")),
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
        file.read_to_end(&mut bytes)
            .map_err(|error| format!("cannot read {name}: {error}"))?;
        parts.insert(name, bytes);
    }
    Ok(parts)
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
    let evs = events_of(parts, &rels_name)?;
    Ok(starts(&evs, "Relationship")
        .into_iter()
        .map(|attrs| Rel {
            id: attr(attrs, "Id").unwrap_or_default().to_owned(),
            ty: attr(attrs, "Type").unwrap_or_default().to_owned(),
            target: resolve(dir, attr(attrs, "Target").unwrap_or_default()),
        })
        .collect())
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
