//! `cargo xtask inspect-pptx <file>`: reads a generated deck, prints its slide
//! order with section, kind and notes, and fails if the structural rules of
//! the deck shape (`docs/plans/T0.9-implementation-plan.md`) are broken.
//!
//! This reads OOXML directly and deliberately shares no code with the
//! generator in `tools/powerpoint-spike`, so a generator bug cannot hide
//! itself from the inspector.

use std::{
    collections::{HashMap, HashSet},
    io::{Cursor, Read},
    path::Path,
};

use quick_xml::{
    Reader,
    escape::unescape,
    events::{BytesStart, Event},
};
use sha2::{Digest, Sha256};
use zip::ZipArchive;

/// One slide of a deck, numbered from 1 in presentation order.
#[derive(Debug, Clone, PartialEq)]
pub struct SlideInfo {
    pub index: usize,
    /// Name of the first section listing the slide; empty if none does.
    pub section: String,
    pub kind: SlideKind,
    pub notes: Option<String>,
}

#[derive(Debug, Clone, PartialEq)]
pub enum SlideKind {
    Image {
        width: u32,
        height: u32,
        sha256: String,
    },
    Black,
}

const PRESENTATION: &str = "ppt/presentation.xml";
const PRESENTATION_RELS: &str = "ppt/_rels/presentation.xml.rels";
const SLIDE_WIDTH: u64 = 12_192_000;
const SLIDE_HEIGHT: u64 = 6_858_000;
const IMAGE_WIDTH: u32 = 1920;
const IMAGE_HEIGHT: u32 = 1080;
const SAFETY_SECTION: &str = "Safety";
const BLACK: &str = "000000";
const CONTENT_TYPES: &str = "[Content_Types].xml";
const RELS_TYPE: &str = "application/vnd.openxmlformats-package.relationships+xml";
const IMAGE_REL: &str = "/image";
const NOTES_REL: &str = "/notesSlide";
/// Largest package part the inspector will read; a larger part is refused so
/// a zip bomb cannot exhaust memory.
const MAX_PART_BYTES: u64 = 64 * 1024 * 1024;

type Parsed<T> = Result<T, String>;

/// Reads a deck's slides in presentation order.
pub fn read_deck(bytes: &[u8]) -> Result<Vec<SlideInfo>, String> {
    let mut zip = open_zip(bytes)?;
    let presentation = parse_presentation(&read_part(&mut zip, PRESENTATION)?)?;
    let rels = parse_rels(
        &read_part(&mut zip, PRESENTATION_RELS)?,
        PRESENTATION_RELS,
        "ppt",
    )?;

    let mut slides = Vec::with_capacity(presentation.slides.len());
    for (position, (slide_id, rid)) in presentation.slides.iter().enumerate() {
        let index = position + 1;
        let slide_part = rels
            .get(rid)
            .ok_or_else(|| format!("slide {index}: relationship {rid} not found"))?
            .target
            .clone();
        let slide = analyze_slide(&read_part(&mut zip, &slide_part)?, &slide_part)?;
        let slide_rels_part = rels_path(&slide_part);
        let slide_rels = parse_rels(
            &read_part(&mut zip, &slide_rels_part)?,
            &slide_rels_part,
            dir_of(&slide_part),
        )?;

        let section = presentation
            .sections
            .iter()
            .find(|section| section.slide_ids.contains(slide_id))
            .map_or_else(String::new, |section| section.name.clone());
        let kind = slide_kind(&mut zip, &slide, &slide_rels, index)?;
        let notes = match slide_rels
            .values()
            .find(|rel| rel.kind.ends_with(NOTES_REL))
        {
            Some(rel) => notes_text(&read_part(&mut zip, &rel.target)?, &rel.target)?,
            None => None,
        };
        slides.push(SlideInfo {
            index,
            section,
            kind,
            notes,
        });
    }
    Ok(slides)
}

/// Returns one message per broken rule; an empty list means the deck passes.
/// `raw` is the same deck the slides were read from.
pub fn check_rules(slides: &[SlideInfo], raw: &[u8]) -> Vec<String> {
    let mut problems = structure_problems(slides, raw).unwrap_or_else(|error| vec![error]);
    problems.extend(content_type_problems(raw).unwrap_or_else(|error| vec![error]));
    problems
}

fn structure_problems(slides: &[SlideInfo], raw: &[u8]) -> Parsed<Vec<String>> {
    let mut zip = open_zip(raw)?;
    let presentation = parse_presentation(&read_part(&mut zip, PRESENTATION)?)?;
    let rels = parse_rels(
        &read_part(&mut zip, PRESENTATION_RELS)?,
        PRESENTATION_RELS,
        "ppt",
    )?;
    let mut problems = Vec::new();

    if presentation.slide_size != Some((SLIDE_WIDTH, SLIDE_HEIGHT)) {
        problems.push(format!(
            "slide size is {:?}, expected {SLIDE_WIDTH}x{SLIDE_HEIGHT} EMU (16:9)",
            presentation.slide_size
        ));
    }
    if presentation.slides.len() != slides.len() {
        problems.push(format!(
            "deck has {} slides but {} were inspected",
            presentation.slides.len(),
            slides.len()
        ));
        return Ok(problems);
    }

    for (info, (slide_id, rid)) in slides.iter().zip(&presentation.slides) {
        let index = info.index;
        let memberships = presentation
            .sections
            .iter()
            .filter(|section| section.slide_ids.contains(slide_id))
            .count();
        if memberships != 1 {
            problems.push(format!(
                "slide {index}: in {memberships} sections, expected exactly one"
            ));
        }
        let Some(rel) = rels.get(rid) else {
            problems.push(format!("slide {index}: relationship {rid} not found"));
            continue;
        };
        let slide = analyze_slide(&read_part(&mut zip, &rel.target)?, &rel.target)?;

        match &info.kind {
            SlideKind::Image { width, height, .. } => {
                if (*width, *height) != (IMAGE_WIDTH, IMAGE_HEIGHT) {
                    problems.push(format!(
                        "slide {index}: image is {width}x{height}, expected {IMAGE_WIDTH}x{IMAGE_HEIGHT}"
                    ));
                }
                match slide.pictures.as_slice() {
                    [picture] => {
                        if picture.offset != Some((0, 0))
                            || picture.extent != Some((SLIDE_WIDTH, SLIDE_HEIGHT))
                        {
                            problems.push(format!(
                                "slide {index}: picture is not full bleed (offset 0,0, extent = slide size)"
                            ));
                        }
                    }
                    pictures => problems.push(format!(
                        "slide {index}: image slide has {} pictures, expected one",
                        pictures.len()
                    )),
                }
                if slide.has_shape {
                    problems.push(format!("slide {index}: image slide has a shape (p:sp)"));
                }
                if slide.has_text {
                    problems.push(format!("slide {index}: image slide has text (a:t)"));
                }
                if info.notes.is_some() {
                    problems.push(format!("slide {index}: image slide has notes"));
                }
            }
            SlideKind::Black => {
                if !slide.black_background || !slide.pictures.is_empty() {
                    problems.push(format!(
                        "slide {index}: black slide must have a black background and no picture"
                    ));
                }
                if info
                    .notes
                    .as_deref()
                    .is_none_or(|notes| notes.trim().is_empty())
                {
                    problems.push(format!("slide {index}: black slide has no notes"));
                }
            }
        }
    }

    if let (Some(first), Some(last)) = (slides.first(), slides.last()) {
        for (slide, role) in [(first, "lead-in"), (last, "closing")] {
            if !matches!(slide.kind, SlideKind::Black) || slide.section != SAFETY_SECTION {
                problems.push(format!(
                    "slide {}: the {role} must be a black slide in section \"{SAFETY_SECTION}\"",
                    slide.index
                ));
            }
        }
    } else {
        problems.push("deck has no slides".to_owned());
    }
    Ok(problems)
}

/// The outline printed by the inspector: one line per slide.
pub fn outline(slides: &[SlideInfo]) -> String {
    slides
        .iter()
        .map(|slide| match &slide.kind {
            SlideKind::Image {
                width,
                height,
                sha256,
            } => format!(
                "{:>3} [{}] image {width}x{height} {}",
                slide.index,
                slide.section,
                sha256.get(..12).unwrap_or(sha256)
            ),
            SlideKind::Black => format!(
                "{:>3} [{}] black — {}",
                slide.index,
                slide.section,
                slide.notes.as_deref().unwrap_or("").replace('\n', " ")
            ),
        })
        .collect::<Vec<_>>()
        .join("\n")
}

/// Prints the outline, then fails if any deck rule is broken.
pub fn inspect_pptx(path: &Path) -> Result<(), Box<dyn std::error::Error>> {
    let bytes = std::fs::read(path)?;
    let slides = read_deck(&bytes)?;
    println!("{}", outline(&slides));
    let problems = check_rules(&slides, &bytes);
    if problems.is_empty() {
        println!("ok: {} slides pass the deck rules", slides.len());
        return Ok(());
    }
    for problem in &problems {
        eprintln!("rule failed: {problem}");
    }
    Err(format!("{} deck rule(s) failed", problems.len()).into())
}

// --- package and XML helpers -------------------------------------------------

fn open_zip(bytes: &[u8]) -> Parsed<ZipArchive<Cursor<&[u8]>>> {
    ZipArchive::new(Cursor::new(bytes)).map_err(|error| format!("not a ZIP package: {error}"))
}

fn read_part(zip: &mut ZipArchive<Cursor<&[u8]>>, name: &str) -> Parsed<Vec<u8>> {
    read_part_limited(zip, name, MAX_PART_BYTES)
}

/// Reads at most `limit` bytes of a part and fails if it holds more.
fn read_part_limited(
    zip: &mut ZipArchive<Cursor<&[u8]>>,
    name: &str,
    limit: u64,
) -> Parsed<Vec<u8>> {
    let file = zip
        .by_name(name)
        .map_err(|error| format!("{name}: {error}"))?;
    let mut bytes = Vec::new();
    file.take(limit + 1)
        .read_to_end(&mut bytes)
        .map_err(|error| format!("{name}: {error}"))?;
    if bytes.len() as u64 > limit {
        return Err(format!("{name}: larger than {limit} bytes"));
    }
    Ok(bytes)
}

/// `ppt/slides/slide1.xml` -> `ppt/slides/_rels/slide1.xml.rels`.
fn rels_path(part: &str) -> String {
    match part.rsplit_once('/') {
        Some((dir, file)) => format!("{dir}/_rels/{file}.rels"),
        None => format!("_rels/{part}.rels"),
    }
}

fn dir_of(part: &str) -> &str {
    part.rsplit_once('/').map_or("", |(dir, _)| dir)
}

/// Resolves a relationship target against the directory of its source part.
fn resolve(base: &str, target: &str) -> String {
    let joined = match target.strip_prefix('/') {
        Some(absolute) => absolute.to_owned(),
        None => format!("{base}/{target}"),
    };
    let mut segments: Vec<&str> = Vec::new();
    for segment in joined.split('/') {
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

#[derive(Debug)]
struct Element {
    name: String,
    attrs: Vec<(String, String)>,
}

impl Element {
    fn attr(&self, key: &str) -> Option<&str> {
        self.attrs
            .iter()
            .find(|(name, _)| name.as_str() == key)
            .map(|(_, value)| value.as_str())
    }

    fn number(&self, key: &str) -> Option<u64> {
        self.attr(key)?.parse().ok()
    }
}

enum Node {
    Start(Element),
    End(String),
    Text(String),
}

fn element(start: &BytesStart<'_>, part: &str) -> Parsed<Element> {
    let attrs = start
        .attributes()
        .map(|attribute| {
            let attribute =
                attribute.map_err(|error| format!("{part}: malformed attribute: {error}"))?;
            let key = String::from_utf8_lossy(attribute.key.as_ref()).into_owned();
            let raw = String::from_utf8_lossy(&attribute.value);
            let value = unescape(&raw)
                .map_err(|error| format!("{part}: attribute {key}: {error}"))?
                .into_owned();
            Ok((key, value))
        })
        .collect::<Parsed<Vec<_>>>()?;
    Ok(Element {
        name: String::from_utf8_lossy(start.local_name().as_ref()).into_owned(),
        attrs,
    })
}

/// Streams the elements of one part in document order. Attributes keep their
/// qualified names (`r:id`); element names are local names (`sldId`).
fn walk(xml: &[u8], part: &str, mut visit: impl FnMut(Node) -> Parsed<()>) -> Parsed<()> {
    let text = std::str::from_utf8(xml).map_err(|error| format!("{part}: not UTF-8: {error}"))?;
    let mut reader = Reader::from_str(text);
    loop {
        match reader
            .read_event()
            .map_err(|error| format!("{part}: malformed XML: {error}"))?
        {
            Event::Start(start) => visit(Node::Start(element(&start, part)?))?,
            Event::Empty(start) => {
                let element = element(&start, part)?;
                let name = element.name.clone();
                visit(Node::Start(element))?;
                visit(Node::End(name))?;
            }
            Event::End(end) => visit(Node::End(
                String::from_utf8_lossy(end.local_name().as_ref()).into_owned(),
            ))?,
            Event::Text(text) => {
                let decoded = text.decode().map_err(|error| format!("{part}: {error}"))?;
                visit(Node::Text(decoded.into_owned()))?;
            }
            Event::GeneralRef(reference) => {
                let name = reference
                    .decode()
                    .map_err(|error| format!("{part}: {error}"))?;
                let reference = format!("&{name};");
                let decoded = unescape(&reference).map_err(|error| format!("{part}: {error}"))?;
                visit(Node::Text(decoded.into_owned()))?;
            }
            Event::Eof => return Ok(()),
            _ => {}
        }
    }
}

// --- presentation.xml ----------------------------------------------------------

#[derive(Default)]
struct Presentation {
    slide_size: Option<(u64, u64)>,
    /// `(sldId id, r:id)` in presentation order.
    slides: Vec<(u32, String)>,
    sections: Vec<Section>,
}

struct Section {
    name: String,
    slide_ids: Vec<u32>,
}

fn slide_id(element: &Element) -> Parsed<u32> {
    element
        .attr("id")
        .and_then(|value| value.parse().ok())
        .ok_or_else(|| "sldId without a numeric id".to_owned())
}

fn parse_presentation(xml: &[u8]) -> Parsed<Presentation> {
    let mut presentation = Presentation::default();
    let mut in_section = false;
    walk(xml, PRESENTATION, |node| {
        match node {
            Node::Start(element) => match element.name.as_str() {
                "sldSz" => {
                    if let (Some(cx), Some(cy)) = (element.number("cx"), element.number("cy")) {
                        presentation.slide_size = Some((cx, cy));
                    }
                }
                "section" => {
                    let name = element
                        .attr("name")
                        .ok_or_else(|| format!("{PRESENTATION}: section without name attribute"))?;
                    presentation.sections.push(Section {
                        name: name.to_owned(),
                        slide_ids: Vec::new(),
                    });
                    in_section = true;
                }
                "sldId" if in_section => {
                    let id = slide_id(&element)?;
                    if let Some(section) = presentation.sections.last_mut() {
                        section.slide_ids.push(id);
                    }
                }
                "sldId" => {
                    let rid = element
                        .attr("r:id")
                        .ok_or_else(|| "sldId without r:id".to_owned())?
                        .to_owned();
                    presentation.slides.push((slide_id(&element)?, rid));
                }
                _ => {}
            },
            Node::End(name) => {
                if name == "section" {
                    in_section = false;
                }
            }
            Node::Text(_) => {}
        }
        Ok(())
    })?;
    Ok(presentation)
}

// --- relationships -------------------------------------------------------------

struct Relationship {
    kind: String,
    /// Package path of the target, with `..` resolved.
    target: String,
}

fn parse_rels(xml: &[u8], part: &str, base: &str) -> Parsed<HashMap<String, Relationship>> {
    let mut rels = HashMap::new();
    walk(xml, part, |node| {
        if let Node::Start(element) = node
            && element.name == "Relationship"
        {
            let id = element
                .attr("Id")
                .ok_or_else(|| format!("{part}: Relationship without Id attribute"))?;
            let kind = element
                .attr("Type")
                .ok_or_else(|| format!("{part}: Relationship {id} without Type attribute"))?;
            let target = element
                .attr("Target")
                .ok_or_else(|| format!("{part}: Relationship {id} without Target attribute"))?;
            rels.insert(
                id.to_owned(),
                Relationship {
                    kind: kind.to_owned(),
                    target: resolve(base, target),
                },
            );
        }
        Ok(())
    })?;
    Ok(rels)
}

// --- content types --------------------------------------------------------------

/// The content type a relationship type requires of its target, by the last
/// segment of the type URI. Unknown types carry no expectation.
fn required_type(relationship_type: &str) -> Option<&'static str> {
    Some(
        match relationship_type.rsplit('/').next().unwrap_or_default() {
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
            "presProps" => {
                "application/vnd.openxmlformats-officedocument.presentationml.presProps+xml"
            }
            "viewProps" => {
                "application/vnd.openxmlformats-officedocument.presentationml.viewProps+xml"
            }
            "tableStyles" => {
                "application/vnd.openxmlformats-officedocument.presentationml.tableStyles+xml"
            }
            "core-properties" => "application/vnd.openxmlformats-package.core-properties+xml",
            "extended-properties" => {
                "application/vnd.openxmlformats-officedocument.extended-properties+xml"
            }
            "image" => "image/png",
            _ => return None,
        },
    )
}

#[derive(Default)]
struct ContentTypes {
    /// Lower-cased extension -> type.
    defaults: HashMap<String, String>,
    /// Lower-cased part name without the leading `/` -> type.
    overrides: HashMap<String, String>,
}

impl ContentTypes {
    /// Override for the part name, else Default for its extension.
    fn effective(&self, part: &str) -> Option<&str> {
        self.overrides
            .get(&part.to_ascii_lowercase())
            .or_else(|| {
                let (_, extension) = part.rsplit_once('.')?;
                self.defaults.get(&extension.to_ascii_lowercase())
            })
            .map(String::as_str)
    }
}

fn parse_content_types(xml: &[u8]) -> Parsed<ContentTypes> {
    let mut types = ContentTypes::default();
    walk(xml, CONTENT_TYPES, |node| {
        if let Node::Start(element) = node {
            let content_type = |element: &Element| {
                element
                    .attr("ContentType")
                    .map(str::to_owned)
                    .ok_or_else(|| {
                        format!(
                            "{CONTENT_TYPES}: {} without ContentType attribute",
                            element.name
                        )
                    })
            };
            match element.name.as_str() {
                "Default" => {
                    let extension = element.attr("Extension").ok_or_else(|| {
                        format!("{CONTENT_TYPES}: Default without Extension attribute")
                    })?;
                    let extension = extension.to_ascii_lowercase();
                    if types
                        .defaults
                        .insert(extension.clone(), content_type(&element)?)
                        .is_some()
                    {
                        return Err(format!(
                            "{CONTENT_TYPES}: duplicate Default for extension {extension}"
                        ));
                    }
                }
                "Override" => {
                    let name = element.attr("PartName").ok_or_else(|| {
                        format!("{CONTENT_TYPES}: Override without PartName attribute")
                    })?;
                    let name = name.strip_prefix('/').unwrap_or(name).to_ascii_lowercase();
                    if types
                        .overrides
                        .insert(name.clone(), content_type(&element)?)
                        .is_some()
                    {
                        return Err(format!("{CONTENT_TYPES}: duplicate Override for /{name}"));
                    }
                }
                _ => {}
            }
        }
        Ok(())
    })?;
    Ok(types)
}

/// Checks `[Content_Types].xml` against the role of each part: relationship
/// parts, and the targets of internal relationships, must have the content
/// type their role requires, and no Override may name a missing part.
fn content_type_problems(raw: &[u8]) -> Parsed<Vec<String>> {
    let mut zip = open_zip(raw)?;
    let types = parse_content_types(&read_part(&mut zip, CONTENT_TYPES)?)?;
    let parts: Vec<String> = zip
        .file_names()
        .filter(|name| !name.ends_with('/'))
        .map(str::to_owned)
        .collect();
    // Part names are compared case-insensitively, as OPC requires.
    let part_names: HashSet<String> = parts.iter().map(|name| name.to_ascii_lowercase()).collect();
    let mut problems = Vec::new();

    let mut overridden: Vec<&String> = types.overrides.keys().collect();
    overridden.sort();
    for name in overridden {
        if !part_names.contains(name.as_str()) {
            problems.push(format!(
                "{CONTENT_TYPES}: Override names /{name}, which is not in the package"
            ));
        }
    }

    for part in &parts {
        if !part.ends_with(".rels") {
            continue;
        }
        let found = types.effective(part);
        if found != Some(RELS_TYPE) {
            problems.push(format!(
                "{part}: content type is {}, expected {RELS_TYPE}",
                found.unwrap_or("none")
            ));
        }
        // `dir/_rels/name.rels` belongs to `dir/name`; `_rels/.rels` to the package.
        let rels_dir = dir_of(part);
        let base = rels_dir
            .strip_suffix("_rels")
            .map_or("", |dir| dir.trim_end_matches('/'));
        let xml = read_part(&mut zip, part)?;
        let mut targets = Vec::new();
        walk(&xml, part, |node| {
            if let Node::Start(element) = node
                && element.name == "Relationship"
                && element.attr("TargetMode") != Some("External")
                && let (Some(kind), Some(target)) = (element.attr("Type"), element.attr("Target"))
            {
                targets.push((kind.to_owned(), resolve(base, target)));
            }
            Ok(())
        })?;
        for (kind, target) in targets {
            if !part_names.contains(&target.to_ascii_lowercase()) {
                problems.push(format!(
                    "{target} (relationship {kind} in {part}) is missing"
                ));
                continue;
            }
            let Some(expected) = required_type(&kind) else {
                continue;
            };
            let found = types.effective(&target);
            if found != Some(expected) {
                problems.push(format!(
                    "{target}: content type is {}, expected {expected} (relationship {kind} in {part})",
                    found.unwrap_or("none")
                ));
            }
        }
    }
    Ok(problems)
}

// --- slides, pictures and notes --------------------------------------------------

#[derive(Default)]
struct SlideXml {
    pictures: Vec<Picture>,
    has_shape: bool,
    has_text: bool,
    black_background: bool,
}

#[derive(Default)]
struct Picture {
    offset: Option<(u64, u64)>,
    extent: Option<(u64, u64)>,
    embed: Option<String>,
}

fn analyze_slide(xml: &[u8], part: &str) -> Parsed<SlideXml> {
    let mut slide = SlideXml::default();
    let mut in_picture = false;
    let mut in_background = false;
    walk(xml, part, |node| {
        match node {
            Node::Start(element) => match element.name.as_str() {
                "pic" => {
                    slide.pictures.push(Picture::default());
                    in_picture = true;
                }
                "off" | "ext" if in_picture => {
                    if let Some(picture) = slide.pictures.last_mut() {
                        if element.name == "off" {
                            if let (Some(x), Some(y)) = (element.number("x"), element.number("y")) {
                                picture.offset.get_or_insert((x, y));
                            }
                        } else if let (Some(cx), Some(cy)) =
                            (element.number("cx"), element.number("cy"))
                        {
                            picture.extent.get_or_insert((cx, cy));
                        }
                    }
                }
                "blip" if in_picture => {
                    if let Some(picture) = slide.pictures.last_mut() {
                        picture.embed = element.attr("r:embed").map(str::to_owned);
                    }
                }
                "bg" => in_background = true,
                "srgbClr" if in_background && element.attr("val") == Some(BLACK) => {
                    slide.black_background = true;
                }
                "sp" => slide.has_shape = true,
                "t" => slide.has_text = true,
                _ => {}
            },
            Node::End(name) => match name.as_str() {
                "pic" => in_picture = false,
                "bg" => in_background = false,
                _ => {}
            },
            Node::Text(_) => {}
        }
        Ok(())
    })?;
    Ok(slide)
}

/// The text of a notes slide's body placeholder, paragraphs separated by
/// newlines. `None` when there is no body text.
fn notes_text(xml: &[u8], part: &str) -> Parsed<Option<String>> {
    let mut notes = String::new();
    let mut shape = String::new();
    let mut in_shape = false;
    let mut is_body = false;
    let mut in_text = false;
    walk(xml, part, |node| {
        match node {
            Node::Start(element) => match element.name.as_str() {
                "sp" => {
                    in_shape = true;
                    is_body = false;
                    shape.clear();
                }
                "ph" if in_shape && element.attr("type") == Some("body") => is_body = true,
                "t" => in_text = true,
                _ => {}
            },
            Node::End(name) => match name.as_str() {
                "sp" => {
                    if is_body {
                        notes.push_str(&shape);
                    }
                    in_shape = false;
                }
                "p" if in_shape => shape.push('\n'),
                "t" => in_text = false,
                _ => {}
            },
            Node::Text(text) => {
                if in_text && in_shape {
                    shape.push_str(&text);
                }
            }
        }
        Ok(())
    })?;
    let notes = notes.trim_end_matches('\n').to_owned();
    Ok((!notes.is_empty()).then_some(notes))
}

fn slide_kind(
    zip: &mut ZipArchive<Cursor<&[u8]>>,
    slide: &SlideXml,
    rels: &HashMap<String, Relationship>,
    index: usize,
) -> Parsed<SlideKind> {
    match slide.pictures.as_slice() {
        [] if slide.black_background => Ok(SlideKind::Black),
        [] => Err(format!("slide {index}: no picture and no black background")),
        [picture] => {
            let embed = picture
                .embed
                .as_deref()
                .ok_or_else(|| format!("slide {index}: picture has no r:embed"))?;
            let rel = rels
                .get(embed)
                .ok_or_else(|| format!("slide {index}: relationship {embed} not found"))?;
            if !rel.kind.ends_with(IMAGE_REL) {
                return Err(format!("slide {index}: picture is not an image"));
            }
            let bytes = read_part(zip, &rel.target)?;
            let (width, height) =
                png_size(&bytes).map_err(|error| format!("slide {index}: {error}"))?;
            Ok(SlideKind::Image {
                width,
                height,
                sha256: hex_sha256(&bytes),
            })
        }
        pictures => Err(format!(
            "slide {index}: {} pictures, expected one",
            pictures.len()
        )),
    }
}

/// Reads the dimensions from the PNG header only.
fn png_size(bytes: &[u8]) -> Parsed<(u32, u32)> {
    let reader = png::Decoder::new(Cursor::new(bytes))
        .read_info()
        .map_err(|error| format!("not a readable PNG: {error}"))?;
    let info = reader.info();
    Ok((info.width, info.height))
}

fn hex_sha256(bytes: &[u8]) -> String {
    Sha256::digest(bytes)
        .iter()
        .map(|byte| format!("{byte:02x}"))
        .collect()
}

#[cfg(test)]
mod tests {
    use std::io::Write;

    use super::*;

    const NS: &str = r#"xmlns:a="http://schemas.openxmlformats.org/drawingml/2006/main" xmlns:r="http://schemas.openxmlformats.org/officeDocument/2006/relationships" xmlns:p="http://schemas.openxmlformats.org/presentationml/2006/main""#;
    const SLIDE_TYPE_IMAGE: &str =
        "http://schemas.openxmlformats.org/officeDocument/2006/relationships/image";
    const SLIDE_TYPE_NOTES: &str =
        "http://schemas.openxmlformats.org/officeDocument/2006/relationships/notesSlide";
    const LEAD_IN: &str = "Safety lead-in: black slide before the first Mass item.";
    const CLOSING: &str = "Safety closing: black slide after the final Mass item.";

    fn png(width: u32, height: u32) -> Vec<u8> {
        let mut bytes = Vec::new();
        let mut encoder = png::Encoder::new(&mut bytes, width, height);
        encoder.set_color(png::ColorType::Rgb);
        encoder.set_depth(png::BitDepth::Eight);
        let mut writer = encoder.write_header().unwrap();
        writer
            .write_image_data(&vec![0u8; (width * height * 3) as usize])
            .unwrap();
        writer.finish().unwrap();
        bytes
    }

    fn rels(entries: &[(&str, &str, &str)]) -> String {
        let body: String = entries
            .iter()
            .map(|(id, kind, target)| {
                format!(r#"<Relationship Id="{id}" Type="{kind}" Target="{target}"/>"#)
            })
            .collect();
        format!(
            r#"<?xml version="1.0" encoding="UTF-8" standalone="yes"?><Relationships xmlns="http://schemas.openxmlformats.org/package/2006/relationships">{body}</Relationships>"#
        )
    }

    fn black_slide() -> String {
        format!(
            r#"<?xml version="1.0" encoding="UTF-8" standalone="yes"?><p:sld {NS}><p:cSld><p:bg><p:bgPr><a:solidFill><a:srgbClr val="000000"/></a:solidFill><a:effectLst/></p:bgPr></p:bg><p:spTree/></p:cSld></p:sld>"#
        )
    }

    fn image_slide() -> String {
        format!(
            r#"<?xml version="1.0" encoding="UTF-8" standalone="yes"?><p:sld {NS}><p:cSld><p:spTree><p:pic><p:nvPicPr><p:cNvPr id="2" name="Picture 1"/><p:cNvPicPr><a:picLocks noChangeAspect="1"/></p:cNvPicPr><p:nvPr/></p:nvPicPr><p:blipFill><a:blip r:embed="rId1"/><a:stretch><a:fillRect/></a:stretch></p:blipFill><p:spPr><a:xfrm><a:off x="0" y="0"/><a:ext cx="{SLIDE_WIDTH}" cy="{SLIDE_HEIGHT}"/></a:xfrm><a:prstGeom prst="rect"><a:avLst/></a:prstGeom></p:spPr></p:pic></p:spTree></p:cSld></p:sld>"#
        )
    }

    fn notes_slide(text: &str) -> String {
        format!(
            r#"<?xml version="1.0" encoding="UTF-8" standalone="yes"?><p:notes {NS}><p:cSld><p:spTree><p:sp><p:nvSpPr><p:cNvPr id="2" name="Slide Image"/><p:cNvSpPr/><p:nvPr><p:ph type="sldImg"/></p:nvPr></p:nvSpPr><p:spPr/></p:sp><p:sp><p:nvSpPr><p:cNvPr id="3" name="Notes"/><p:cNvSpPr/><p:nvPr><p:ph type="body" idx="1"/></p:nvPr></p:nvSpPr><p:spPr/><p:txBody><a:bodyPr/><a:lstStyle/><a:p><a:r><a:rPr lang="en-US"/><a:t>{text}</a:t></a:r></a:p></p:txBody></p:sp></p:spTree></p:cSld></p:notes>"#
        )
    }

    /// A three-slide deck: black lead-in, one image, black closing.
    /// `unsectioned_image` leaves the image's slide out of every section;
    /// `image_notes` gives the image slide a notes slide.
    fn deck(image: (u32, u32), image_notes: bool, unsectioned_image: bool) -> Vec<u8> {
        deck_with_overrides(image, image_notes, unsectioned_image, &[])
    }

    /// `[Content_Types].xml` for the fixture deck: Defaults for rels, xml and
    /// png, plus the Overrides the deck's parts need, then `extra`.
    fn content_types(image_notes: bool, extra: &[(&str, &str)]) -> String {
        const PML: &str = "application/vnd.openxmlformats-officedocument.presentationml";
        let mut overrides = vec![
            (
                "/ppt/presentation.xml".to_owned(),
                format!("{PML}.presentation.main+xml"),
            ),
            (
                "/ppt/slides/slide1.xml".to_owned(),
                format!("{PML}.slide+xml"),
            ),
            (
                "/ppt/slides/slide2.xml".to_owned(),
                format!("{PML}.slide+xml"),
            ),
            (
                "/ppt/slides/slide3.xml".to_owned(),
                format!("{PML}.slide+xml"),
            ),
            (
                "/ppt/notesSlides/notesSlide1.xml".to_owned(),
                format!("{PML}.notesSlide+xml"),
            ),
            (
                "/ppt/notesSlides/notesSlide3.xml".to_owned(),
                format!("{PML}.notesSlide+xml"),
            ),
        ];
        if image_notes {
            overrides.push((
                "/ppt/notesSlides/notesSlide2.xml".to_owned(),
                format!("{PML}.notesSlide+xml"),
            ));
        }
        for (part, kind) in extra {
            overrides.retain(|(name, _)| name != part);
            overrides.push(((*part).to_owned(), (*kind).to_owned()));
        }
        let body: String = overrides
            .iter()
            .map(|(part, kind)| format!(r#"<Override PartName="{part}" ContentType="{kind}"/>"#))
            .collect();
        format!(
            r#"<?xml version="1.0" encoding="UTF-8" standalone="yes"?><Types xmlns="http://schemas.openxmlformats.org/package/2006/content-types"><Default Extension="rels" ContentType="{RELS_TYPE}"/><Default Extension="xml" ContentType="application/xml"/><Default Extension="png" ContentType="image/png"/>{body}</Types>"#
        )
    }

    fn deck_with_overrides(
        image: (u32, u32),
        image_notes: bool,
        unsectioned_image: bool,
        extra: &[(&str, &str)],
    ) -> Vec<u8> {
        let presentation = format!(
            r#"<?xml version="1.0" encoding="UTF-8" standalone="yes"?><p:presentation {NS}><p:sldIdLst><p:sldId id="256" r:id="rId2"/><p:sldId id="257" r:id="rId3"/><p:sldId id="258" r:id="rId4"/></p:sldIdLst><p:sldSz cx="{SLIDE_WIDTH}" cy="{SLIDE_HEIGHT}"/><p:extLst><p:ext uri="{{521415D9-36F7-43E2-AB2F-B90AF26B5E84}}"><p14:sectionLst xmlns:p14="http://schemas.microsoft.com/office/powerpoint/2010/main"><p14:section name="Safety" id="{{00000000-0000-0000-0000-000000000001}}"><p14:sldIdLst><p14:sldId id="256"/></p14:sldIdLst></p14:section><p14:section name="Item one" id="{{00000000-0000-0000-0000-000000000002}}">{item}</p14:section><p14:section name="Safety" id="{{00000000-0000-0000-0000-000000000003}}"><p14:sldIdLst><p14:sldId id="258"/></p14:sldIdLst></p14:section></p14:sectionLst></p:ext></p:extLst></p:presentation>"#,
            item = if unsectioned_image {
                String::new()
            } else {
                r#"<p14:sldIdLst><p14:sldId id="257"/></p14:sldIdLst>"#.to_owned()
            }
        );
        let presentation_rels = rels(&[
            (
                "rId2",
                "http://schemas.openxmlformats.org/officeDocument/2006/relationships/slide",
                "slides/slide1.xml",
            ),
            (
                "rId3",
                "http://schemas.openxmlformats.org/officeDocument/2006/relationships/slide",
                "slides/slide2.xml",
            ),
            (
                "rId4",
                "http://schemas.openxmlformats.org/officeDocument/2006/relationships/slide",
                "slides/slide3.xml",
            ),
        ]);
        let image_rels = if image_notes {
            rels(&[
                ("rId1", SLIDE_TYPE_IMAGE, "../media/image1.png"),
                ("rId5", SLIDE_TYPE_NOTES, "../notesSlides/notesSlide2.xml"),
            ])
        } else {
            rels(&[("rId1", SLIDE_TYPE_IMAGE, "../media/image1.png")])
        };
        let mut parts: Vec<(&str, Vec<u8>)> = vec![
            (
                "[Content_Types].xml",
                content_types(image_notes, extra).into_bytes(),
            ),
            ("ppt/presentation.xml", presentation.into_bytes()),
            (
                "ppt/_rels/presentation.xml.rels",
                presentation_rels.into_bytes(),
            ),
            ("ppt/slides/slide1.xml", black_slide().into_bytes()),
            (
                "ppt/slides/_rels/slide1.xml.rels",
                rels(&[("rId1", SLIDE_TYPE_NOTES, "../notesSlides/notesSlide1.xml")]).into_bytes(),
            ),
            ("ppt/slides/slide2.xml", image_slide().into_bytes()),
            ("ppt/slides/_rels/slide2.xml.rels", image_rels.into_bytes()),
            ("ppt/slides/slide3.xml", black_slide().into_bytes()),
            (
                "ppt/slides/_rels/slide3.xml.rels",
                rels(&[("rId1", SLIDE_TYPE_NOTES, "../notesSlides/notesSlide3.xml")]).into_bytes(),
            ),
            (
                "ppt/notesSlides/notesSlide1.xml",
                notes_slide(LEAD_IN).into_bytes(),
            ),
            (
                "ppt/notesSlides/notesSlide3.xml",
                notes_slide(CLOSING).into_bytes(),
            ),
            ("ppt/media/image1.png", png(image.0, image.1)),
        ];
        if image_notes {
            parts.push((
                "ppt/notesSlides/notesSlide2.xml",
                notes_slide("Hold").into_bytes(),
            ));
        }

        let mut writer = zip::ZipWriter::new(Cursor::new(Vec::new()));
        for (name, bytes) in parts {
            writer
                .start_file(name, zip::write::SimpleFileOptions::default())
                .unwrap();
            writer.write_all(&bytes).unwrap();
        }
        writer.finish().unwrap().into_inner()
    }

    #[test]
    fn compliant_deck_reads_checks_and_prints() {
        let bytes = deck((IMAGE_WIDTH, IMAGE_HEIGHT), false, false);
        let slides = read_deck(&bytes).expect("deck reads");

        assert_eq!(slides.len(), 3);
        assert_eq!(slides[0].kind, SlideKind::Black);
        assert_eq!(slides[0].section, "Safety");
        assert_eq!(slides[0].notes.as_deref(), Some(LEAD_IN));
        assert_eq!(slides[1].section, "Item one");
        assert_eq!(slides[1].notes, None);
        assert_eq!(slides[2].section, "Safety");
        assert_eq!(slides[2].notes.as_deref(), Some(CLOSING));
        let sha256 = hex_sha256(&png(IMAGE_WIDTH, IMAGE_HEIGHT));
        assert_eq!(
            slides[1].kind,
            SlideKind::Image {
                width: IMAGE_WIDTH,
                height: IMAGE_HEIGHT,
                sha256: sha256.clone(),
            }
        );

        assert_eq!(check_rules(&slides, &bytes), Vec::<String>::new());
        assert_eq!(
            outline(&slides),
            format!(
                "  1 [Safety] black — {LEAD_IN}\n  2 [Item one] image 1920x1080 {}\n  3 [Safety] black — {CLOSING}",
                &sha256[..12]
            )
        );
    }

    const SLIDE_CT: &str = "application/vnd.openxmlformats-officedocument.presentationml.slide+xml";

    /// Rewrites `[Content_Types].xml` of a built deck.
    fn rewrite_content_types(bytes: &[u8], edit: impl Fn(String) -> String) -> Vec<u8> {
        let mut source = ZipArchive::new(Cursor::new(bytes)).unwrap();
        let mut writer = zip::ZipWriter::new(Cursor::new(Vec::new()));
        for index in 0..source.len() {
            let mut file = source.by_index(index).unwrap();
            let mut data = Vec::new();
            file.read_to_end(&mut data).unwrap();
            if file.name() == CONTENT_TYPES {
                data = edit(String::from_utf8(data).unwrap()).into_bytes();
            }
            writer
                .start_file(file.name(), zip::write::SimpleFileOptions::default())
                .unwrap();
            writer.write_all(&data).unwrap();
        }
        writer.finish().unwrap().into_inner()
    }

    fn content_type_problems_of(extra: &[(&str, &str)]) -> Vec<String> {
        let bytes = deck_with_overrides((IMAGE_WIDTH, IMAGE_HEIGHT), false, false, extra);
        let slides = read_deck(&bytes).expect("deck reads");
        check_rules(&slides, &bytes)
    }

    #[test]
    fn override_on_a_relationship_part_breaks_the_rules() {
        let problems = content_type_problems_of(&[("/ppt/slides/_rels/slide1.xml.rels", SLIDE_CT)]);
        assert!(
            problems.iter().any(
                |problem| problem.contains("ppt/slides/_rels/slide1.xml.rels")
                    && problem.contains(SLIDE_CT)
                    && problem.contains(RELS_TYPE)
            ),
            "{problems:?}"
        );
    }

    #[test]
    fn slide_with_the_wrong_content_type_breaks_the_rules() {
        let problems = content_type_problems_of(&[(
            "/ppt/slides/slide2.xml",
            "application/vnd.openxmlformats-officedocument.presentationml.notesSlide+xml",
        )]);
        assert!(
            problems
                .iter()
                .any(|problem| problem.starts_with("ppt/slides/slide2.xml")
                    && problem.contains(SLIDE_CT)),
            "{problems:?}"
        );
    }

    #[test]
    fn override_for_a_missing_part_breaks_the_rules() {
        let problems = content_type_problems_of(&[("/ppt/slides/slide9.xml", SLIDE_CT)]);
        assert!(
            problems
                .iter()
                .any(|problem| problem.contains("/ppt/slides/slide9.xml")
                    && problem.contains("not in the package")),
            "{problems:?}"
        );
    }

    #[test]
    fn upper_case_extension_and_part_name_are_accepted() {
        let bytes = deck((IMAGE_WIDTH, IMAGE_HEIGHT), false, false);
        let upper = rewrite_content_types(&bytes, |text| {
            text.replace("Extension=\"png\"", "Extension=\"PNG\"")
                .replace("/ppt/slides/slide1.xml", "/PPT/Slides/Slide1.xml")
        });
        let problems = check_rules(&read_deck(&upper).unwrap(), &upper);
        assert_eq!(problems, Vec::<String>::new());
    }

    #[test]
    fn duplicate_override_is_an_error() {
        let bytes = deck((IMAGE_WIDTH, IMAGE_HEIGHT), false, false);
        let doubled = rewrite_content_types(&bytes, |text| {
            text.replace(
                "</Types>",
                &format!(
                    "<Override PartName=\"/PPT/slides/slide1.xml\" ContentType=\"{SLIDE_CT}\"/></Types>"
                ),
            )
        });
        let problems = check_rules(&read_deck(&doubled).unwrap(), &doubled);
        assert!(
            problems
                .iter()
                .any(|problem| problem.contains("duplicate Override")),
            "{problems:?}"
        );
    }

    #[test]
    fn duplicate_default_is_an_error() {
        let bytes = deck((IMAGE_WIDTH, IMAGE_HEIGHT), false, false);
        let doubled = rewrite_content_types(&bytes, |text| {
            text.replace(
                "</Types>",
                "<Default Extension=\"PNG\" ContentType=\"image/png\"/></Types>",
            )
        });
        let problems = check_rules(&read_deck(&doubled).unwrap(), &doubled);
        assert!(
            problems
                .iter()
                .any(|problem| problem.contains("duplicate Default")),
            "{problems:?}"
        );
    }

    #[test]
    fn missing_relationship_target_is_reported() {
        let bytes = deck((IMAGE_WIDTH, IMAGE_HEIGHT), false, false);
        let mut source = ZipArchive::new(Cursor::new(bytes.as_slice())).unwrap();
        let mut writer = zip::ZipWriter::new(Cursor::new(Vec::new()));
        for index in 0..source.len() {
            let mut file = source.by_index(index).unwrap();
            if file.name() == "ppt/media/image1.png" {
                continue;
            }
            let mut data = Vec::new();
            file.read_to_end(&mut data).unwrap();
            writer
                .start_file(file.name(), zip::write::SimpleFileOptions::default())
                .unwrap();
            writer.write_all(&data).unwrap();
        }
        let stripped = writer.finish().unwrap().into_inner();
        let problems = content_type_problems(&stripped).unwrap();
        assert!(
            problems.iter().any(
                |problem| problem.contains("ppt/media/image1.png (relationship")
                    && problem.contains("in ppt/slides/_rels/slide2.xml.rels")
                    && problem.ends_with("is missing")
            ),
            "{problems:?}"
        );
    }

    #[test]
    fn deck_without_content_types_breaks_the_rules() {
        let bytes = deck((IMAGE_WIDTH, IMAGE_HEIGHT), false, false);
        let mut source = ZipArchive::new(Cursor::new(bytes.as_slice())).unwrap();
        let mut writer = zip::ZipWriter::new(Cursor::new(Vec::new()));
        for index in 0..source.len() {
            let mut file = source.by_index(index).unwrap();
            if file.name() == CONTENT_TYPES {
                continue;
            }
            let mut data = Vec::new();
            file.read_to_end(&mut data).unwrap();
            writer
                .start_file(file.name(), zip::write::SimpleFileOptions::default())
                .unwrap();
            writer.write_all(&data).unwrap();
        }
        let stripped = writer.finish().unwrap().into_inner();
        let problems = check_rules(&read_deck(&stripped).unwrap(), &stripped);
        assert!(
            problems
                .iter()
                .any(|problem| problem.contains(CONTENT_TYPES)),
            "{problems:?}"
        );
    }

    #[test]
    fn wrong_image_size_breaks_the_rules() {
        let bytes = deck((1280, 720), false, false);
        let slides = read_deck(&bytes).expect("deck reads");
        let problems = check_rules(&slides, &bytes);
        assert!(
            problems.iter().any(|problem| problem.contains("1280x720")),
            "{problems:?}"
        );
    }

    #[test]
    fn slide_outside_every_section_breaks_the_rules() {
        let bytes = deck((IMAGE_WIDTH, IMAGE_HEIGHT), false, true);
        let slides = read_deck(&bytes).expect("deck reads");
        let problems = check_rules(&slides, &bytes);
        assert!(
            problems
                .iter()
                .any(|problem| problem.contains("slide 2: in 0 sections")),
            "{problems:?}"
        );
    }

    #[test]
    fn notes_on_an_image_slide_break_the_rules() {
        let bytes = deck((IMAGE_WIDTH, IMAGE_HEIGHT), true, false);
        let slides = read_deck(&bytes).expect("deck reads");
        assert_eq!(slides[1].notes.as_deref(), Some("Hold"));
        let problems = check_rules(&slides, &bytes);
        assert!(
            problems
                .iter()
                .any(|problem| problem.contains("image slide has notes")),
            "{problems:?}"
        );
    }

    #[test]
    fn relationship_missing_target_or_type_is_an_error() {
        let no_target = format!(
            r#"<Relationships xmlns="x"><Relationship Id="rId1" Type="{SLIDE_TYPE_IMAGE}"/></Relationships>"#
        );
        let error = parse_rels(
            no_target.as_bytes(),
            "ppt/slides/_rels/slide1.xml.rels",
            "ppt/slides",
        )
        .err()
        .expect("missing Target is refused");
        assert!(
            error.contains("ppt/slides/_rels/slide1.xml.rels"),
            "{error}"
        );
        assert!(
            error.contains("rId1") && error.contains("Target"),
            "{error}"
        );

        let no_type =
            r#"<Relationships xmlns="x"><Relationship Id="rId2" Target="a.xml"/></Relationships>"#;
        let error = parse_rels(no_type.as_bytes(), "ppt/_rels/presentation.xml.rels", "ppt")
            .err()
            .expect("missing Type is refused");
        assert!(error.contains("rId2") && error.contains("Type"), "{error}");
    }

    #[test]
    fn section_without_name_is_an_error() {
        let xml = format!(
            r#"<p:presentation {NS}><p:sldIdLst/><p14:sectionLst xmlns:p14="http://schemas.microsoft.com/office/powerpoint/2010/main"><p14:section id="{{1}}"/></p14:sectionLst></p:presentation>"#
        );
        let error = parse_presentation(xml.as_bytes())
            .err()
            .expect("missing section name is refused");
        assert!(
            error.contains("section") && error.contains("name"),
            "{error}"
        );
    }

    #[test]
    fn malformed_attribute_is_an_error_not_dropped() {
        // Unquoted attribute value: quick-xml reports it; the old code dropped it.
        let xml = format!(
            r#"<p:presentation {NS}><p:sldIdLst><p:sldId id=256 r:id="rId2"/></p:sldIdLst></p:presentation>"#
        );
        let error = parse_presentation(xml.as_bytes())
            .err()
            .expect("malformed attribute is refused");
        assert!(error.contains(PRESENTATION), "{error}");
    }

    #[test]
    fn part_over_the_limit_is_refused() {
        let mut writer = zip::ZipWriter::new(Cursor::new(Vec::new()));
        writer
            .start_file("ppt/big.xml", zip::write::SimpleFileOptions::default())
            .unwrap();
        writer.write_all(&[b'x'; 100]).unwrap();
        let bytes = writer.finish().unwrap().into_inner();

        let mut zip = open_zip(&bytes).expect("zip opens");
        let error = read_part_limited(&mut zip, "ppt/big.xml", 10)
            .expect_err("part larger than the limit is refused");
        assert!(
            error.contains("ppt/big.xml") && error.contains("10"),
            "{error}"
        );
        let exact = read_part_limited(&mut zip, "ppt/big.xml", 100).expect("exact size reads");
        assert_eq!(exact.len(), 100);
    }

    #[test]
    fn notes_join_paragraphs_with_newlines() {
        let xml = r#"<p:notes xmlns:a="a" xmlns:p="p"><p:cSld><p:spTree><p:sp><p:nvSpPr><p:cNvPr id="3" name="Notes"/><p:cNvSpPr/><p:nvPr><p:ph type="body" idx="1"/></p:nvPr></p:nvSpPr><p:spPr/><p:txBody><a:bodyPr/><a:lstStyle/><a:p><a:r><a:t>First line</a:t></a:r></a:p><a:p><a:r><a:t>Second line</a:t></a:r></a:p></p:txBody></p:sp></p:spTree></p:cSld></p:notes>"#;
        let notes = notes_text(xml.as_bytes(), "ppt/notesSlides/notesSlide1.xml")
            .expect("notes read")
            .expect("notes present");
        assert_eq!(notes, "First line\nSecond line");

        let slide = SlideInfo {
            index: 1,
            section: "Safety".to_owned(),
            kind: SlideKind::Black,
            notes: Some(notes),
        };
        assert_eq!(
            outline(&[slide]),
            "  1 [Safety] black — First line Second line"
        );
    }
}
