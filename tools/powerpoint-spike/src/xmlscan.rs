//! Flattens XML into start, end and text events for validation queries.

use quick_xml::{
    Reader,
    events::{BytesRef, BytesStart, Event},
};

#[derive(Debug, Clone)]
pub(crate) enum Ev {
    Start(String, Vec<(String, String)>),
    End(String),
    Text(String),
}

pub(crate) type Attrs = [(String, String)];

pub(crate) fn events(xml: &[u8]) -> Result<Vec<Ev>, String> {
    let mut reader = Reader::from_reader(xml);
    let mut out = Vec::new();
    loop {
        let event = reader.read_event().map_err(|error| error.to_string())?;
        match event {
            Event::Start(start) => {
                let (name, attrs) = start_tag(&start)?;
                out.push(Ev::Start(name, attrs));
            }
            Event::Empty(start) => {
                let (name, attrs) = start_tag(&start)?;
                out.push(Ev::Start(name.clone(), attrs));
                out.push(Ev::End(name));
            }
            Event::End(end) => {
                out.push(Ev::End(
                    String::from_utf8_lossy(end.name().as_ref()).into_owned(),
                ));
            }
            Event::Text(text) => {
                // Raw text never contains `&`; references arrive as `GeneralRef`
                // events, so no unescaping is needed here.
                let decoded = text.decode().map_err(|error| error.to_string())?;
                push_text(&mut out, &decoded);
            }
            Event::GeneralRef(reference) => {
                let decoded = resolve_reference(&reference)?;
                push_text(&mut out, &decoded);
            }
            Event::Eof => break,
            _ => {}
        }
    }
    Ok(out)
}

/// Appends `text` to a trailing text event so one logical text node stays one
/// string even when quick-xml splits it around entity references.
fn push_text(out: &mut Vec<Ev>, text: &str) {
    if let Some(Ev::Text(previous)) = out.last_mut() {
        previous.push_str(text);
    } else {
        out.push(Ev::Text(text.to_owned()));
    }
}

/// Resolves `&name;`, `&#N;` or `&#xN;` to its text.
fn resolve_reference(reference: &BytesRef<'_>) -> Result<String, String> {
    if let Some(ch) = reference
        .resolve_char_ref()
        .map_err(|error| error.to_string())?
    {
        return Ok(ch.to_string());
    }
    let name = reference.decode().map_err(|error| error.to_string())?;
    quick_xml::escape::unescape(&format!("&{name};"))
        .map(|decoded| decoded.into_owned())
        .map_err(|error| error.to_string())
}

fn start_tag(start: &BytesStart<'_>) -> Result<(String, Vec<(String, String)>), String> {
    let name = String::from_utf8_lossy(start.name().as_ref()).into_owned();
    let mut attrs = Vec::new();
    for attr in start.attributes() {
        let attr = attr.map_err(|error| error.to_string())?;
        let key = String::from_utf8_lossy(attr.key.as_ref()).into_owned();
        let value = attr
            .normalized_value(quick_xml::XmlVersion::Implicit1_0)
            .map_err(|error| error.to_string())?
            .into_owned();
        attrs.push((key, value));
    }
    Ok((name, attrs))
}

pub(crate) fn attr<'a>(attrs: &'a Attrs, key: &str) -> Option<&'a str> {
    attrs
        .iter()
        .find(|(name, _)| name == key)
        .map(|(_, value)| value.as_str())
}

/// Attribute lists of every start tag named `name`, in document order.
pub(crate) fn starts<'a>(evs: &'a [Ev], name: &str) -> Vec<&'a Attrs> {
    evs.iter()
        .filter_map(|ev| match ev {
            Ev::Start(tag, attrs) if tag.as_str() == name => Some(attrs.as_slice()),
            _ => None,
        })
        .collect()
}

/// Start tags nested inside `container` (its first occurrence per span), as
/// `(name, attributes)`. Assumes `container` is not nested inside itself.
pub(crate) fn within<'a>(evs: &'a [Ev], container: &str) -> Vec<(&'a str, &'a Attrs)> {
    let mut out = Vec::new();
    let mut inside = false;
    for ev in evs {
        match ev {
            Ev::Start(tag, _) if tag.as_str() == container => inside = true,
            Ev::End(tag) if tag.as_str() == container => inside = false,
            Ev::Start(tag, attrs) if inside => out.push((tag.as_str(), attrs.as_slice())),
            _ => {}
        }
    }
    out
}

/// Concatenated text inside every `tag` element.
pub(crate) fn text_in(evs: &[Ev], tag: &str) -> String {
    let mut out = String::new();
    let mut inside = false;
    for ev in evs {
        match ev {
            Ev::Start(name, _) if name.as_str() == tag => inside = true,
            Ev::End(name) if name.as_str() == tag => inside = false,
            Ev::Text(text) if inside => out.push_str(text),
            _ => {}
        }
    }
    out
}

/// Text of every `a:p` paragraph (its `a:t` runs concatenated), joined with
/// `\n`. Notes use this so multi-line notes round-trip exactly.
pub(crate) fn paragraph_text(evs: &[Ev]) -> String {
    let mut paragraphs: Vec<String> = Vec::new();
    let mut in_paragraph = false;
    let mut in_run_text = false;
    for ev in evs {
        match ev {
            Ev::Start(name, _) if name.as_str() == "a:p" => {
                paragraphs.push(String::new());
                in_paragraph = true;
            }
            Ev::End(name) if name.as_str() == "a:p" => in_paragraph = false,
            Ev::Start(name, _) if name.as_str() == "a:t" && in_paragraph => in_run_text = true,
            Ev::End(name) if name.as_str() == "a:t" => in_run_text = false,
            Ev::Text(text) if in_run_text => {
                if let Some(current) = paragraphs.last_mut() {
                    current.push_str(text);
                }
            }
            _ => {}
        }
    }
    paragraphs.join("\n")
}

/// `(section name, slide ids)` for each `p14:section`, in order.
pub(crate) fn sections(evs: &[Ev]) -> Vec<(String, Vec<String>)> {
    let mut out: Vec<(String, Vec<String>)> = Vec::new();
    for ev in evs {
        if let Ev::Start(tag, attrs) = ev {
            match tag.as_str() {
                "p14:section" => {
                    out.push((
                        attr(attrs, "name").unwrap_or_default().to_owned(),
                        Vec::new(),
                    ));
                }
                "p14:sldId" => {
                    if let (Some(section), Some(id)) = (out.last_mut(), attr(attrs, "id")) {
                        section.1.push(id.to_owned());
                    }
                }
                _ => {}
            }
        }
    }
    out
}
