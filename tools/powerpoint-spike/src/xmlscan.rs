//! Flattens XML into start, end and text events for validation queries.

use quick_xml::{
    Reader,
    events::{BytesStart, Event},
};

#[derive(Debug, Clone)]
pub(crate) enum Ev {
    Start(String, Vec<(String, String)>),
    End(String),
    Text(String),
}

type Attrs = [(String, String)];

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
                let decoded = text.decode().map_err(|error| error.to_string())?;
                let unescaped =
                    quick_xml::escape::unescape(&decoded).map_err(|error| error.to_string())?;
                out.push(Ev::Text(unescaped.into_owned()));
            }
            Event::Eof => break,
            _ => {}
        }
    }
    Ok(out)
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
