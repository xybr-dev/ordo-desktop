//! `liturgy-import`: externally prepared readings and responses for one
//! Celebration (CONTEXT: Liturgy Import Document).
//!
//! Semantic rules: a response `follows` must name a reading (`missing-reference`),
//! and each `text_sha256` is the SHA-256 of that text's exact UTF-8 bytes
//! (`hash-mismatch`).

use super::support::{
    Errors, check_date, check_hash, check_opt_text, check_ref, check_sha256, check_text,
    check_unique, check_uuid, child, id_set, index, object, schema, sha256_text, version_gate,
};
use serde::Deserialize;
use serde_json::Value;

pub(super) const REQUIRED: &[&str] = &[
    "celebration",
    "id",
    "readings",
    "responses",
    "schema_version",
];
pub(super) const OPTIONAL: &[&str] = &["source"];

#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct Celebration {
    id: String,
    date: String,
    title: String,
}

#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct Reading {
    id: String,
    role: String,
    language: Option<String>,
    citation: Option<String>,
    text: String,
    text_sha256: String,
}

#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct Response {
    id: String,
    role: String,
    language: Option<String>,
    follows: Option<String>,
    text: String,
    text_sha256: String,
}

struct Liturgy {
    readings: Vec<Reading>,
    responses: Vec<Response>,
}

pub(super) fn check(value: &Value, errors: &mut Errors) {
    let before = errors.len();
    if let Some(liturgy) = parse(value, "", errors)
        && errors.len() == before
    {
        semantics(&liturgy, "", errors);
    }
}

fn parse(value: &Value, base: &str, errors: &mut Errors) -> Option<Liturgy> {
    if !version_gate("liturgy-import", value, base, errors) {
        return None;
    }
    let obj = object(value, base, errors)?;
    obj.reject_unknown(&[REQUIRED, OPTIONAL].concat(), errors);
    let id = obj.required::<String>("id", errors);
    let celebration = obj.required::<Celebration>("celebration", errors);
    let readings = obj.required_list::<Reading>("readings", errors);
    let responses = obj.required_list::<Response>("responses", errors);
    let source = obj.optional::<String>("source", errors);
    let id = id?;
    let celebration = celebration?;
    let readings = readings?;
    let responses = responses?;

    check_uuid(&id, &child(base, "id"), errors);
    let celebration_path = child(base, "celebration");
    check_uuid(&celebration.id, &child(&celebration_path, "id"), errors);
    check_date(&celebration.date, &child(&celebration_path, "date"), errors);
    check_text(
        &celebration.title,
        &child(&celebration_path, "title"),
        errors,
    );
    check_opt_text(&source, &child(base, "source"), errors);

    let readings_path = child(base, "readings");
    if readings.is_empty() {
        schema(errors, &readings_path, "must contain at least one reading");
    }
    for (position, reading) in readings.iter().enumerate() {
        let path = index(&readings_path, position);
        check_uuid(&reading.id, &child(&path, "id"), errors);
        check_text(&reading.role, &child(&path, "role"), errors);
        check_opt_text(&reading.language, &child(&path, "language"), errors);
        check_opt_text(&reading.citation, &child(&path, "citation"), errors);
        check_text(&reading.text, &child(&path, "text"), errors);
        check_sha256(&reading.text_sha256, &child(&path, "text_sha256"), errors);
    }

    let responses_path = child(base, "responses");
    for (position, response) in responses.iter().enumerate() {
        let path = index(&responses_path, position);
        check_uuid(&response.id, &child(&path, "id"), errors);
        check_text(&response.role, &child(&path, "role"), errors);
        check_opt_text(&response.language, &child(&path, "language"), errors);
        if let Some(follows) = &response.follows {
            check_uuid(follows, &child(&path, "follows"), errors);
        }
        check_text(&response.text, &child(&path, "text"), errors);
        check_sha256(&response.text_sha256, &child(&path, "text_sha256"), errors);
    }

    check_unique(
        readings
            .iter()
            .enumerate()
            .map(|(position, reading)| {
                (
                    reading.id.as_str(),
                    child(&index(&readings_path, position), "id"),
                )
            })
            .chain(responses.iter().enumerate().map(|(position, response)| {
                (
                    response.id.as_str(),
                    child(&index(&responses_path, position), "id"),
                )
            })),
        errors,
    );
    Some(Liturgy {
        readings,
        responses,
    })
}

fn semantics(liturgy: &Liturgy, base: &str, errors: &mut Errors) {
    let readings_path = child(base, "readings");
    for (position, reading) in liturgy.readings.iter().enumerate() {
        check_hash(
            &reading.text_sha256,
            &sha256_text(&reading.text),
            &child(&index(&readings_path, position), "text_sha256"),
            errors,
        );
    }

    let reading_ids = id_set(liturgy.readings.iter().map(|reading| reading.id.as_str()));
    let responses_path = child(base, "responses");
    for (position, response) in liturgy.responses.iter().enumerate() {
        let path = index(&responses_path, position);
        if let Some(follows) = &response.follows {
            check_ref(&reading_ids, follows, &child(&path, "follows"), errors);
        }
        check_hash(
            &response.text_sha256,
            &sha256_text(&response.text),
            &child(&path, "text_sha256"),
            errors,
        );
    }
}
