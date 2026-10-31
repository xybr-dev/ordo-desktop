//! `song-import`: one Song work with its Arrangements (decision 40).
//!
//! Semantic rules: every Arrangement `sequence` entry names a slide of the same
//! Arrangement (`missing-reference`), and `content_sha256` is the SHA-256 of
//! every slide line in slide order, each followed by LF (`hash-mismatch`).

use super::support::{
    Errors, check_hash, check_list_text, check_list_uuid, check_opt_text, check_ref, check_sha256,
    check_text, check_unique, check_uuid, child, id_set, index, object, schema, sha256_text,
    version_gate,
};
use serde::Deserialize;
use serde_json::Value;

pub(super) const REQUIRED: &[&str] = &["arrangements", "id", "schema_version", "title"];
pub(super) const OPTIONAL: &[&str] = &["aliases", "composer", "language", "source"];

#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct Slide {
    id: String,
    label: Option<String>,
    lines: Vec<String>,
}

#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct Arrangement {
    id: String,
    name: String,
    language: Option<String>,
    #[serde(default)]
    choirs: Vec<String>,
    #[serde(default)]
    occasions: Vec<String>,
    content_sha256: String,
    slides: Vec<Slide>,
    sequence: Vec<String>,
}

pub(super) fn check(value: &Value, errors: &mut Errors) {
    let before = errors.len();
    if let Some(arrangements) = parse(value, "", errors)
        && errors.len() == before
    {
        semantics(&arrangements, "", errors);
    }
}

fn parse(value: &Value, base: &str, errors: &mut Errors) -> Option<Vec<Arrangement>> {
    if !version_gate("song-import", value, base, errors) {
        return None;
    }
    let obj = object(value, base, errors)?;
    obj.reject_unknown(&[REQUIRED, OPTIONAL].concat(), errors);
    let id = obj.required::<String>("id", errors);
    let title = obj.required::<String>("title", errors);
    let arrangements = obj.required_list::<Arrangement>("arrangements", errors);
    let aliases = obj.optional_list::<String>("aliases", errors);
    let composer = obj.optional::<String>("composer", errors);
    let source = obj.optional::<String>("source", errors);
    let language = obj.optional::<String>("language", errors);
    let id = id?;
    let title = title?;
    let arrangements = arrangements?;

    check_uuid(&id, &child(base, "id"), errors);
    check_text(&title, &child(base, "title"), errors);
    check_opt_text(&composer, &child(base, "composer"), errors);
    check_opt_text(&source, &child(base, "source"), errors);
    check_opt_text(&language, &child(base, "language"), errors);
    check_list_text(&aliases, &child(base, "aliases"), errors);

    let arrangements_path = child(base, "arrangements");
    if arrangements.is_empty() {
        schema(
            errors,
            &arrangements_path,
            "must contain at least one Arrangement",
        );
    }
    check_unique(
        arrangements
            .iter()
            .enumerate()
            .map(|(position, arrangement)| {
                (
                    arrangement.id.as_str(),
                    child(&index(&arrangements_path, position), "id"),
                )
            }),
        errors,
    );

    for (position, arrangement) in arrangements.iter().enumerate() {
        let path = index(&arrangements_path, position);
        check_uuid(&arrangement.id, &child(&path, "id"), errors);
        check_text(&arrangement.name, &child(&path, "name"), errors);
        check_opt_text(&arrangement.language, &child(&path, "language"), errors);
        check_list_text(&arrangement.choirs, &child(&path, "choirs"), errors);
        check_list_text(&arrangement.occasions, &child(&path, "occasions"), errors);
        check_sha256(
            &arrangement.content_sha256,
            &child(&path, "content_sha256"),
            errors,
        );

        let slides_path = child(&path, "slides");
        if arrangement.slides.is_empty() {
            schema(errors, &slides_path, "must contain at least one slide");
        }
        check_unique(
            arrangement
                .slides
                .iter()
                .enumerate()
                .map(|(slide_position, slide)| {
                    (
                        slide.id.as_str(),
                        child(&index(&slides_path, slide_position), "id"),
                    )
                }),
            errors,
        );
        for (slide_position, slide) in arrangement.slides.iter().enumerate() {
            let slide_path = index(&slides_path, slide_position);
            check_uuid(&slide.id, &child(&slide_path, "id"), errors);
            check_opt_text(&slide.label, &child(&slide_path, "label"), errors);
            let lines_path = child(&slide_path, "lines");
            if slide.lines.is_empty() {
                schema(errors, &lines_path, "must contain at least one line");
            }
            check_list_text(&slide.lines, &lines_path, errors);
        }

        let sequence_path = child(&path, "sequence");
        if arrangement.sequence.is_empty() {
            schema(
                errors,
                &sequence_path,
                "must contain at least one slide reference",
            );
        }
        check_list_uuid(&arrangement.sequence, &sequence_path, errors);
    }
    Some(arrangements)
}

fn semantics(arrangements: &[Arrangement], base: &str, errors: &mut Errors) {
    let arrangements_path = child(base, "arrangements");
    for (position, arrangement) in arrangements.iter().enumerate() {
        let path = index(&arrangements_path, position);
        let slide_ids = id_set(arrangement.slides.iter().map(|slide| slide.id.as_str()));
        let sequence_path = child(&path, "sequence");
        for (sequence_position, reference) in arrangement.sequence.iter().enumerate() {
            check_ref(
                &slide_ids,
                reference,
                &index(&sequence_path, sequence_position),
                errors,
            );
        }

        let mut text = String::new();
        for slide in &arrangement.slides {
            for line in &slide.lines {
                text.push_str(line);
                text.push('\n');
            }
        }
        check_hash(
            &arrangement.content_sha256,
            &sha256_text(&text),
            &child(&path, "content_sha256"),
            errors,
        );
    }
}
