//! `cargo xtask inspect-pptx <file>`: reads a generated deck, prints its slide
//! order with section, kind and notes, and fails if the structural rules of
//! the deck shape (`docs/plans/T0.9-implementation-plan.md`) are broken.
//!
//! This reads OOXML directly and deliberately shares no code with the
//! generator in `tools/powerpoint-spike`, so a generator bug cannot hide
//! itself from the inspector.

use std::{
    collections::HashMap,
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
const IMAGE_REL: &str = "/image";
const NOTES_REL: &str = "/notesSlide";

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
    structure_problems(slides, raw).unwrap_or_else(|error| vec![error])
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
    let mut file = zip
        .by_name(name)
        .map_err(|error| format!("{name}: {error}"))?;
    let mut bytes = Vec::new();
    file.read_to_end(&mut bytes)
        .map_err(|error| format!("{name}: {error}"))?;
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

fn element(start: &BytesStart<'_>) -> Element {
    Element {
        name: String::from_utf8_lossy(start.local_name().as_ref()).into_owned(),
        attrs: start
            .attributes()
            .filter_map(Result::ok)
            .map(|attribute| {
                let key = String::from_utf8_lossy(attribute.key.as_ref()).into_owned();
                let raw = String::from_utf8_lossy(&attribute.value);
                let value = unescape(&raw).map_or_else(|_| raw.to_string(), |v| v.into_owned());
                (key, value)
            })
            .collect(),
    }
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
            Event::Start(start) => visit(Node::Start(element(&start)))?,
            Event::Empty(start) => {
                let element = element(&start);
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
                    presentation.sections.push(Section {
                        name: element.attr("name").unwrap_or("").to_owned(),
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
                .ok_or_else(|| format!("{part}: Relationship without Id"))?;
            rels.insert(
                id.to_owned(),
                Relationship {
                    kind: element.attr("Type").unwrap_or("").to_owned(),
                    target: resolve(base, element.attr("Target").unwrap_or("")),
                },
            );
        }
        Ok(())
    })?;
    Ok(rels)
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
}
