//! Hand-written PresentationML parts (ECMA-376 transitional).
//!
//! Every text value is XML-escaped before it is placed in a template.

use crate::plan::{Kind, Planned, Section};
use crate::{SLIDE_CX, SLIDE_CY};

const DECL: &str = "<?xml version=\"1.0\" encoding=\"UTF-8\" standalone=\"yes\"?>\n";
const NS_A: &str = "http://schemas.openxmlformats.org/drawingml/2006/main";
const NS_P14: &str = "http://schemas.microsoft.com/office/powerpoint/2010/main";
const NS: &str = "xmlns:a=\"http://schemas.openxmlformats.org/drawingml/2006/main\" xmlns:r=\"http://schemas.openxmlformats.org/officeDocument/2006/relationships\" xmlns:p=\"http://schemas.openxmlformats.org/presentationml/2006/main\"";
const REL_NS: &str = "http://schemas.openxmlformats.org/officeDocument/2006/relationships";
const SECTION_EXT: &str = "{521415D9-36F7-43E2-AB2F-B90AF26B5E84}";
const TABLE_STYLE: &str = "{5C22544A-7EE6-4342-B048-85BDC9FD1C3A}";
const GROUP_PROPS: &str = "<p:nvGrpSpPr><p:cNvPr id=\"1\" name=\"\"/><p:cNvGrpSpPr/><p:nvPr/></p:nvGrpSpPr><p:grpSpPr><a:xfrm><a:off x=\"0\" y=\"0\"/><a:ext cx=\"0\" cy=\"0\"/><a:chOff x=\"0\" y=\"0\"/><a:chExt cx=\"0\" cy=\"0\"/></a:xfrm></p:grpSpPr>";
const CLR_MAP: &str = "<p:clrMap bg1=\"lt1\" tx1=\"dk1\" bg2=\"lt2\" tx2=\"dk2\" accent1=\"accent1\" accent2=\"accent2\" accent3=\"accent3\" accent4=\"accent4\" accent5=\"accent5\" accent6=\"accent6\" hlink=\"hlink\" folHlink=\"folHlink\"/>";
const MASTER_CLR_OVR: &str = "<p:clrMapOvr><a:masterClrMapping/></p:clrMapOvr>";
const LAYOUT_REL: &str = "../slideLayouts/slideLayout1.xml";

/// Slide ids are 256 + zero-based slide index.
fn slide_id(index: usize) -> usize {
    256 + index
}

/// Escapes the five XML special characters.
pub(crate) fn esc(text: &str) -> String {
    let mut out = String::with_capacity(text.len());
    for ch in text.chars() {
        match ch {
            '&' => out.push_str("&amp;"),
            '<' => out.push_str("&lt;"),
            '>' => out.push_str("&gt;"),
            '"' => out.push_str("&quot;"),
            '\'' => out.push_str("&apos;"),
            other => out.push(other),
        }
    }
    out
}

fn rel(id: &str, ty: &str, target: &str) -> String {
    let full = if ty.starts_with("http") {
        ty.to_owned()
    } else {
        format!("{REL_NS}/{ty}")
    };
    format!(
        "<Relationship Id=\"{id}\" Type=\"{full}\" Target=\"{}\"/>",
        esc(target)
    )
}

fn rels_doc(items: &[String]) -> String {
    format!(
        "{DECL}<Relationships xmlns=\"http://schemas.openxmlformats.org/package/2006/relationships\">{}</Relationships>",
        items.concat()
    )
}

fn section_guid(ordinal: usize) -> String {
    format!("{{7A1C0000-0000-4000-8000-{ordinal:012X}}}")
}

fn content_type(name: &str) -> Option<&'static str> {
    Some(match name {
        "ppt/presentation.xml" => {
            "application/vnd.openxmlformats-officedocument.presentationml.presentation.main+xml"
        }
        "ppt/presProps.xml" => {
            "application/vnd.openxmlformats-officedocument.presentationml.presProps+xml"
        }
        "ppt/viewProps.xml" => {
            "application/vnd.openxmlformats-officedocument.presentationml.viewProps+xml"
        }
        "ppt/tableStyles.xml" => {
            "application/vnd.openxmlformats-officedocument.presentationml.tableStyles+xml"
        }
        "docProps/core.xml" => "application/vnd.openxmlformats-package.core-properties+xml",
        "docProps/app.xml" => {
            "application/vnd.openxmlformats-officedocument.extended-properties+xml"
        }
        _ if name.starts_with("ppt/slideMasters/") => {
            "application/vnd.openxmlformats-officedocument.presentationml.slideMaster+xml"
        }
        _ if name.starts_with("ppt/slideLayouts/") => {
            "application/vnd.openxmlformats-officedocument.presentationml.slideLayout+xml"
        }
        _ if name.starts_with("ppt/slides/") => {
            "application/vnd.openxmlformats-officedocument.presentationml.slide+xml"
        }
        _ if name.starts_with("ppt/notesMasters/") => {
            "application/vnd.openxmlformats-officedocument.presentationml.notesMaster+xml"
        }
        _ if name.starts_with("ppt/notesSlides/") => {
            "application/vnd.openxmlformats-officedocument.presentationml.notesSlide+xml"
        }
        _ if name.starts_with("ppt/theme/") => {
            "application/vnd.openxmlformats-officedocument.theme+xml"
        }
        _ => return None,
    })
}

/// Returns every package part as `(name, bytes)`, `[Content_Types].xml` first.
pub(crate) fn package(
    slides: &[Planned<'_>],
    sections: &[Section],
    title: &str,
) -> Vec<(String, Vec<u8>)> {
    let mut xml: Vec<(String, String)> = Vec::new();
    let mut media: Vec<(String, Vec<u8>)> = Vec::new();
    let mut image_no = 0usize;
    let mut notes_count = 0usize;

    for (index, slide) in slides.iter().enumerate() {
        let number = index + 1;
        let (body, rels) = match slide.kind {
            Kind::Image(png) => {
                image_no += 1;
                media.push((format!("ppt/media/image{image_no}.png"), png.to_vec()));
                (
                    image_slide(number),
                    rels_doc(&[
                        rel("rId1", "slideLayout", LAYOUT_REL),
                        rel("rId2", "image", &format!("../media/image{image_no}.png")),
                    ]),
                )
            }
            Kind::Black(notes) => {
                notes_count += 1;
                xml.push((
                    format!("ppt/notesSlides/notesSlide{number}.xml"),
                    notes_slide(notes),
                ));
                xml.push((
                    format!("ppt/notesSlides/_rels/notesSlide{number}.xml.rels"),
                    rels_doc(&[
                        rel("rId1", "notesMaster", "../notesMasters/notesMaster1.xml"),
                        rel("rId2", "slide", &format!("../slides/slide{number}.xml")),
                    ]),
                ));
                (
                    black_slide(),
                    rels_doc(&[
                        rel("rId1", "slideLayout", LAYOUT_REL),
                        rel(
                            "rId2",
                            "notesSlide",
                            &format!("../notesSlides/notesSlide{number}.xml"),
                        ),
                    ]),
                )
            }
        };
        xml.push((format!("ppt/slides/slide{number}.xml"), body));
        xml.push((format!("ppt/slides/_rels/slide{number}.xml.rels"), rels));
    }

    // presentation.xml and its relationships.
    let sld_ids: String = (0..slides.len())
        .map(|i| format!("<p:sldId id=\"{}\" r:id=\"rId{}\"/>", slide_id(i), 6 + i))
        .collect();
    let section_list: String = sections
        .iter()
        .enumerate()
        .map(|(ordinal, (name, indices))| {
            let ids: String = indices
                .iter()
                .map(|i| format!("<p14:sldId id=\"{}\"/>", slide_id(*i)))
                .collect();
            format!(
                "<p14:section name=\"{}\" id=\"{}\"><p14:sldIdLst>{ids}</p14:sldIdLst></p14:section>",
                esc(name),
                section_guid(ordinal + 1)
            )
        })
        .collect();
    xml.push((
        "ppt/presentation.xml".to_owned(),
        format!(
            "{DECL}<p:presentation {NS}><p:sldMasterIdLst><p:sldMasterId id=\"2147483648\" r:id=\"rId1\"/></p:sldMasterIdLst><p:notesMasterIdLst><p:notesMasterId r:id=\"rId2\"/></p:notesMasterIdLst><p:sldIdLst>{sld_ids}</p:sldIdLst><p:sldSz cx=\"{SLIDE_CX}\" cy=\"{SLIDE_CY}\"/><p:notesSz cx=\"6858000\" cy=\"9144000\"/><p:extLst><p:ext uri=\"{SECTION_EXT}\"><p14:sectionLst xmlns:p14=\"{NS_P14}\">{section_list}</p14:sectionLst></p:ext></p:extLst></p:presentation>"
        ),
    ));
    let mut pres_rels = vec![
        rel("rId1", "slideMaster", "slideMasters/slideMaster1.xml"),
        rel("rId2", "notesMaster", "notesMasters/notesMaster1.xml"),
        rel("rId3", "presProps", "presProps.xml"),
        rel("rId4", "viewProps", "viewProps.xml"),
        rel("rId5", "tableStyles", "tableStyles.xml"),
    ];
    for i in 0..slides.len() {
        pres_rels.push(rel(
            &format!("rId{}", 6 + i),
            "slide",
            &format!("slides/slide{}.xml", i + 1),
        ));
    }
    xml.push((
        "ppt/_rels/presentation.xml.rels".to_owned(),
        rels_doc(&pres_rels),
    ));

    // Support parts.
    xml.push((
        "ppt/presProps.xml".to_owned(),
        format!("{DECL}<p:presentationPr {NS}/>"),
    ));
    xml.push((
        "ppt/viewProps.xml".to_owned(),
        format!("{DECL}<p:viewPr {NS}><p:gridSpacing cx=\"76200\" cy=\"76200\"/></p:viewPr>"),
    ));
    xml.push((
        "ppt/tableStyles.xml".to_owned(),
        format!("{DECL}<a:tblStyleLst xmlns:a=\"{NS_A}\" def=\"{TABLE_STYLE}\"/>"),
    ));

    // Master, layout, and themes.
    xml.push((
        "ppt/slideMasters/slideMaster1.xml".to_owned(),
        format!(
            "{DECL}<p:sldMaster {NS}><p:cSld><p:bg><p:bgRef idx=\"1001\"><a:schemeClr val=\"bg1\"/></p:bgRef></p:bg><p:spTree>{GROUP_PROPS}</p:spTree></p:cSld>{CLR_MAP}<p:sldLayoutIdLst><p:sldLayoutId id=\"2147483649\" r:id=\"rId1\"/></p:sldLayoutIdLst><p:txStyles><p:titleStyle><a:lvl1pPr/></p:titleStyle><p:bodyStyle><a:lvl1pPr/></p:bodyStyle><p:otherStyle><a:defPPr/></p:otherStyle></p:txStyles></p:sldMaster>"
        ),
    ));
    xml.push((
        "ppt/slideMasters/_rels/slideMaster1.xml.rels".to_owned(),
        rels_doc(&[
            rel("rId1", "slideLayout", LAYOUT_REL),
            rel("rId2", "theme", "../theme/theme1.xml"),
        ]),
    ));
    xml.push((
        "ppt/slideLayouts/slideLayout1.xml".to_owned(),
        format!(
            "{DECL}<p:sldLayout {NS} type=\"blank\" preserve=\"1\"><p:cSld name=\"Blank\"><p:spTree>{GROUP_PROPS}</p:spTree></p:cSld>{MASTER_CLR_OVR}</p:sldLayout>"
        ),
    ));
    xml.push((
        "ppt/slideLayouts/_rels/slideLayout1.xml.rels".to_owned(),
        rels_doc(&[rel(
            "rId1",
            "slideMaster",
            "../slideMasters/slideMaster1.xml",
        )]),
    ));
    xml.push(("ppt/theme/theme1.xml".to_owned(), theme()));
    xml.push(("ppt/theme/theme2.xml".to_owned(), theme()));

    // Notes master.
    xml.push((
        "ppt/notesMasters/notesMaster1.xml".to_owned(),
        format!(
            "{DECL}<p:notesMaster {NS}><p:cSld><p:spTree>{GROUP_PROPS}<p:sp><p:nvSpPr><p:cNvPr id=\"2\" name=\"Notes Placeholder 1\"/><p:cNvSpPr><a:spLocks noGrp=\"1\"/></p:cNvSpPr><p:nvPr><p:ph type=\"body\" sz=\"quarter\" idx=\"1\"/></p:nvPr></p:nvSpPr><p:spPr><a:xfrm><a:off x=\"685800\" y=\"4400550\"/><a:ext cx=\"5486400\" cy=\"3600450\"/></a:xfrm><a:prstGeom prst=\"rect\"><a:avLst/></a:prstGeom></p:spPr><p:txBody><a:bodyPr vert=\"horz\" lIns=\"91440\" tIns=\"45720\" rIns=\"91440\" bIns=\"45720\" rtlCol=\"0\"/><a:lstStyle/><a:p><a:pPr lvl=\"0\"/><a:r><a:rPr lang=\"en-US\"/><a:t>Notes</a:t></a:r></a:p></p:txBody></p:sp></p:spTree></p:cSld>{CLR_MAP}<p:notesStyle><a:lvl1pPr marL=\"0\" algn=\"l\" defTabSz=\"914400\" rtl=\"0\" eaLnBrk=\"1\" latinLnBrk=\"0\" hangingPunct=\"1\"><a:defRPr sz=\"1200\" kern=\"1200\"><a:solidFill><a:schemeClr val=\"tx1\"/></a:solidFill><a:latin typeface=\"+mn-lt\"/><a:ea typeface=\"+mn-ea\"/><a:cs typeface=\"+mn-cs\"/></a:defRPr></a:lvl1pPr></p:notesStyle></p:notesMaster>"
        ),
    ));
    xml.push((
        "ppt/notesMasters/_rels/notesMaster1.xml.rels".to_owned(),
        rels_doc(&[rel("rId1", "theme", "../theme/theme2.xml")]),
    ));

    // Document properties and package root.
    xml.push((
        "docProps/core.xml".to_owned(),
        format!(
            "{DECL}<cp:coreProperties xmlns:cp=\"http://schemas.openxmlformats.org/package/2006/metadata/core-properties\" xmlns:dc=\"http://purl.org/dc/elements/1.1/\" xmlns:dcterms=\"http://purl.org/dc/terms/\" xmlns:xsi=\"http://www.w3.org/2001/XMLSchema-instance\"><dc:title>{}</dc:title><dc:creator>Ordo</dc:creator><dcterms:created xsi:type=\"dcterms:W3CDTF\">2026-01-01T00:00:00Z</dcterms:created></cp:coreProperties>",
            esc(title)
        ),
    ));
    xml.push((
        "docProps/app.xml".to_owned(),
        format!(
            "{DECL}<Properties xmlns=\"http://schemas.openxmlformats.org/officeDocument/2006/extended-properties\"><Application>Ordo powerpoint-spike</Application><Slides>{}</Slides><Notes>{notes_count}</Notes></Properties>",
            slides.len()
        ),
    ));
    xml.push((
        "_rels/.rels".to_owned(),
        rels_doc(&[
            rel("rId1", "officeDocument", "ppt/presentation.xml"),
            rel(
                "rId2",
                "http://schemas.openxmlformats.org/package/2006/relationships/metadata/core-properties",
                "docProps/core.xml",
            ),
            rel("rId3", "extended-properties", "docProps/app.xml"),
        ]),
    ));

    // [Content_Types].xml is generated from the parts above.
    let overrides: String = xml
        .iter()
        .filter_map(|(name, _)| {
            content_type(name)
                .map(|ct| format!("<Override PartName=\"/{name}\" ContentType=\"{ct}\"/>"))
        })
        .collect();
    let content_types = format!(
        "{DECL}<Types xmlns=\"http://schemas.openxmlformats.org/package/2006/content-types\"><Default Extension=\"rels\" ContentType=\"application/vnd.openxmlformats-package.relationships+xml\"/><Default Extension=\"xml\" ContentType=\"application/xml\"/><Default Extension=\"png\" ContentType=\"image/png\"/>{overrides}</Types>"
    );

    let mut parts = Vec::with_capacity(xml.len() + media.len() + 1);
    parts.push(("[Content_Types].xml".to_owned(), content_types.into_bytes()));
    parts.extend(
        xml.into_iter()
            .map(|(name, body)| (name, body.into_bytes())),
    );
    parts.extend(media);
    parts
}

fn image_slide(number: usize) -> String {
    format!(
        "{DECL}<p:sld {NS}><p:cSld><p:spTree>{GROUP_PROPS}<p:pic><p:nvPicPr><p:cNvPr id=\"2\" name=\"Frame {number}\"/><p:cNvPicPr><a:picLocks noChangeAspect=\"1\"/></p:cNvPicPr><p:nvPr/></p:nvPicPr><p:blipFill><a:blip r:embed=\"rId2\"/><a:stretch><a:fillRect/></a:stretch></p:blipFill><p:spPr><a:xfrm><a:off x=\"0\" y=\"0\"/><a:ext cx=\"{SLIDE_CX}\" cy=\"{SLIDE_CY}\"/></a:xfrm><a:prstGeom prst=\"rect\"><a:avLst/></a:prstGeom></p:spPr></p:pic></p:spTree></p:cSld>{MASTER_CLR_OVR}</p:sld>"
    )
}

fn black_slide() -> String {
    format!(
        "{DECL}<p:sld {NS}><p:cSld><p:bg><p:bgPr><a:solidFill><a:srgbClr val=\"000000\"/></a:solidFill><a:effectLst/></p:bgPr></p:bg><p:spTree>{GROUP_PROPS}</p:spTree></p:cSld>{MASTER_CLR_OVR}</p:sld>"
    )
}

fn notes_slide(notes: &str) -> String {
    format!(
        "{DECL}<p:notes {NS}><p:cSld><p:spTree>{GROUP_PROPS}<p:sp><p:nvSpPr><p:cNvPr id=\"2\" name=\"Notes Placeholder 1\"/><p:cNvSpPr><a:spLocks noGrp=\"1\"/></p:cNvSpPr><p:nvPr><p:ph type=\"body\" idx=\"1\"/></p:nvPr></p:nvSpPr><p:spPr/><p:txBody><a:bodyPr/><a:lstStyle/><a:p><a:r><a:rPr lang=\"en-US\"/><a:t>{}</a:t></a:r></a:p></p:txBody></p:sp></p:spTree></p:cSld>{MASTER_CLR_OVR}</p:notes>",
        esc(notes)
    )
}

/// A minimal complete theme; master, notes master and tables reference it.
fn theme() -> String {
    const COLOURS: [(&str, &str); 12] = [
        ("dk1", "000000"),
        ("lt1", "FFFFFF"),
        ("dk2", "1F2937"),
        ("lt2", "E5E7EB"),
        ("accent1", "2563EB"),
        ("accent2", "DC2626"),
        ("accent3", "16A34A"),
        ("accent4", "D97706"),
        ("accent5", "7C3AED"),
        ("accent6", "0891B2"),
        ("hlink", "2563EB"),
        ("folHlink", "7C3AED"),
    ];
    let colours: String = COLOURS
        .iter()
        .map(|(name, hex)| format!("<a:{name}><a:srgbClr val=\"{hex}\"/></a:{name}>"))
        .collect();
    let font = "<a:latin typeface=\"Calibri\"/><a:ea typeface=\"\"/><a:cs typeface=\"\"/>";
    let fill = "<a:solidFill><a:schemeClr val=\"phClr\"/></a:solidFill>";
    let line = "<a:ln w=\"6350\"><a:solidFill><a:schemeClr val=\"phClr\"/></a:solidFill></a:ln>";
    let effect = "<a:effectStyle><a:effectLst/></a:effectStyle>";
    format!(
        "{DECL}<a:theme xmlns:a=\"{NS_A}\" name=\"Ordo\"><a:themeElements><a:clrScheme name=\"Ordo\">{colours}</a:clrScheme><a:fontScheme name=\"Ordo\"><a:majorFont>{font}</a:majorFont><a:minorFont>{font}</a:minorFont></a:fontScheme><a:fmtScheme name=\"Ordo\"><a:fillStyleLst>{}</a:fillStyleLst><a:lnStyleLst>{}</a:lnStyleLst><a:effectStyleLst>{}</a:effectStyleLst><a:bgFillStyleLst>{}</a:bgFillStyleLst></a:fmtScheme></a:themeElements><a:objectDefaults/><a:extraClrSchemeLst/></a:theme>",
        fill.repeat(3),
        line.repeat(3),
        effect.repeat(3),
        fill.repeat(3)
    )
}
