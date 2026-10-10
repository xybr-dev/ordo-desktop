//! Helpers shared by the contract validators: JSON-pointer aware field access,
//! error collection, and the pattern, hash, and reference rules.

use super::ContractError;
use crate::sha256_hex;
use serde::{Deserialize, de::DeserializeOwned};
use serde_json::{Map, Value};
use std::collections::BTreeSet;

pub(super) type Errors = Vec<ContractError>;

/// Largest Parish Backup database the spike accepts (MVP-PLAN section 6.4).
pub(super) const BACKUP_DATABASE_CEILING: u64 = 4 * 1024 * 1024 * 1024;

pub(super) fn push(
    errors: &mut Errors,
    code: &'static str,
    path: &str,
    message: impl Into<String>,
) {
    errors.push(ContractError {
        code,
        path: path.to_owned(),
        message: message.into(),
    });
}

pub(super) fn schema(errors: &mut Errors, path: &str, message: impl Into<String>) {
    push(errors, "schema", path, message);
}

/// RFC 6901 pointer to member `key` of the object at `path`.
pub(super) fn child(path: &str, key: &str) -> String {
    format!("{path}/{}", key.replace('~', "~0").replace('/', "~1"))
}

/// RFC 6901 pointer to element `position` of the array at `path`.
pub(super) fn index(path: &str, position: usize) -> String {
    format!("{path}/{position}")
}

/// The version gate every contract runs first, both at the top level and for
/// embedded documents. Returns `false` when shape checks must not continue.
pub(super) fn version_gate(name: &str, value: &Value, path: &str, errors: &mut Errors) -> bool {
    if !value.is_object() {
        schema(
            errors,
            path,
            format!("{name} document must be a JSON object"),
        );
        return false;
    }
    let version_path = child(path, "schema_version");
    match value.get("schema_version") {
        None => {
            schema(
                errors,
                &version_path,
                format!("{name} requires schema_version"),
            );
            false
        }
        Some(version) => match version.as_u64() {
            Some(1) => true,
            Some(found) => {
                push(
                    errors,
                    "unsupported-version",
                    &version_path,
                    format!("{name} schema_version {found} is not supported by this build (v1)"),
                );
                false
            }
            None => {
                schema(
                    errors,
                    &version_path,
                    format!("{name} schema_version must be an integer"),
                );
                false
            }
        },
    }
}

/// A JSON object at a known pointer, with typed member extraction that keeps
/// going after the first failure so every problem is collected.
pub(super) struct Object<'a> {
    map: &'a Map<String, Value>,
    path: &'a str,
}

pub(super) fn object<'a>(
    value: &'a Value,
    path: &'a str,
    errors: &mut Errors,
) -> Option<Object<'a>> {
    match value.as_object() {
        Some(map) => Some(Object { map, path }),
        None => {
            schema(errors, path, "must be a JSON object");
            None
        }
    }
}

impl Object<'_> {
    pub(super) fn reject_unknown(&self, allowed: &[&str], errors: &mut Errors) {
        for key in self.map.keys() {
            if !allowed.contains(&key.as_str()) {
                schema(errors, &child(self.path, key), "unknown field");
            }
        }
    }

    pub(super) fn required<T: DeserializeOwned>(
        &self,
        key: &str,
        errors: &mut Errors,
    ) -> Option<T> {
        let path = child(self.path, key);
        match self.map.get(key) {
            Some(value) => decode(value, &path, errors),
            None => {
                schema(errors, &path, "required field is missing");
                None
            }
        }
    }

    pub(super) fn optional<T: DeserializeOwned>(
        &self,
        key: &str,
        errors: &mut Errors,
    ) -> Option<T> {
        let path = child(self.path, key);
        self.map
            .get(key)
            .and_then(|value| decode(value, &path, errors))
    }

    pub(super) fn required_list<T: DeserializeOwned>(
        &self,
        key: &str,
        errors: &mut Errors,
    ) -> Option<Vec<T>> {
        let path = child(self.path, key);
        match self.map.get(key) {
            Some(value) => decode_list(value, &path, errors),
            None => {
                schema(errors, &path, "required field is missing");
                None
            }
        }
    }

    pub(super) fn optional_list<T: DeserializeOwned>(
        &self,
        key: &str,
        errors: &mut Errors,
    ) -> Vec<T> {
        let path = child(self.path, key);
        match self.map.get(key) {
            Some(value) => decode_list(value, &path, errors).unwrap_or_default(),
            None => Vec::new(),
        }
    }
}

fn decode<T: DeserializeOwned>(value: &Value, path: &str, errors: &mut Errors) -> Option<T> {
    match T::deserialize(value) {
        Ok(decoded) => Some(decoded),
        Err(error) => {
            schema(errors, path, error.to_string());
            None
        }
    }
}

fn decode_list<T: DeserializeOwned>(
    value: &Value,
    path: &str,
    errors: &mut Errors,
) -> Option<Vec<T>> {
    let Some(items) = value.as_array() else {
        schema(errors, path, "must be an array");
        return None;
    };
    Some(
        items
            .iter()
            .enumerate()
            .filter_map(|(position, item)| decode(item, &index(path, position), errors))
            .collect(),
    )
}

/// An asset or database file reference inside a payload. Its bytes live in the
/// container (or its staging directory); the payload carries the expected hash.
#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
pub(super) struct AssetRef {
    pub(super) id: String,
    pub(super) path: String,
    pub(super) sha256: String,
    pub(super) size: u64,
}

pub(super) fn check_asset_ref(asset: &AssetRef, path: &str, errors: &mut Errors) {
    check_uuid(&asset.id, &child(path, "id"), errors);
    check_relative(&asset.path, "assets/", &child(path, "path"), errors);
    check_sha256(&asset.sha256, &child(path, "sha256"), errors);
}

/// UUIDv4 in canonical lowercase hyphenated form.
pub(super) fn check_uuid(value: &str, path: &str, errors: &mut Errors) {
    let valid = uuid::Uuid::parse_str(value)
        .is_ok_and(|id| id.get_version_num() == 4 && id.hyphenated().to_string() == value);
    if !valid {
        schema(errors, path, "must be a lowercase hyphenated UUIDv4");
    }
}

pub(super) fn check_list_uuid(values: &[String], path: &str, errors: &mut Errors) {
    for (position, value) in values.iter().enumerate() {
        check_uuid(value, &index(path, position), errors);
    }
}

/// Lowercase hex SHA-256 (64 characters).
pub(super) fn check_sha256(value: &str, path: &str, errors: &mut Errors) {
    let valid = value.len() == 64
        && value
            .bytes()
            .all(|b| b.is_ascii_digit() || (b'a'..=b'f').contains(&b));
    if !valid {
        schema(errors, path, "must be 64 lowercase hex characters");
    }
}

/// `YYYY-MM-DDTHH:MM:SSZ` (UTC, no fraction).
pub(super) fn check_timestamp(value: &str, path: &str, errors: &mut Errors) {
    let valid = value.len() == 20
        && value.char_indices().all(|(position, c)| match position {
            4 | 7 => c == '-',
            10 => c == 'T',
            13 | 16 => c == ':',
            19 => c == 'Z',
            _ => c.is_ascii_digit(),
        });
    if !valid {
        schema(
            errors,
            path,
            "must be an RFC 3339 UTC timestamp (YYYY-MM-DDTHH:MM:SSZ)",
        );
    }
}

/// `YYYY-MM-DD`.
pub(super) fn check_date(value: &str, path: &str, errors: &mut Errors) {
    let valid = value.len() == 10
        && value.char_indices().all(|(position, c)| match position {
            4 | 7 => c == '-',
            _ => c.is_ascii_digit(),
        });
    if !valid {
        schema(errors, path, "must be a calendar date (YYYY-MM-DD)");
    }
}

/// A non-empty string (JSON Schema `minLength: 1`).
pub(super) fn check_text(value: &str, path: &str, errors: &mut Errors) {
    if value.is_empty() {
        schema(errors, path, "must not be empty");
    }
}

pub(super) fn check_opt_text(value: &Option<String>, path: &str, errors: &mut Errors) {
    if let Some(text) = value {
        check_text(text, path, errors);
    }
}

pub(super) fn check_list_text(values: &[String], path: &str, errors: &mut Errors) {
    for (position, value) in values.iter().enumerate() {
        check_text(value, &index(path, position), errors);
    }
}

/// A relative path under `prefix` made of non-empty segments that are neither
/// `.` nor `..`, with no backslash or NUL.
pub(super) fn check_relative(value: &str, prefix: &str, path: &str, errors: &mut Errors) {
    let valid = value.strip_prefix(prefix).is_some_and(|rest| {
        !value.contains(['\\', '\0'])
            && rest
                .split('/')
                .all(|segment| !segment.is_empty() && segment != "." && segment != "..")
    });
    if !valid {
        schema(
            errors,
            path,
            format!("must be a relative path under {prefix}"),
        );
    }
}

/// Reports every repeated id. Each entry is `(id, path_of_the_id_member)`.
pub(super) fn check_unique<'a>(
    ids: impl IntoIterator<Item = (&'a str, String)>,
    errors: &mut Errors,
) {
    let mut seen = BTreeSet::new();
    for (id, path) in ids {
        if !seen.insert(id) {
            schema(errors, &path, format!("duplicate id {id}"));
        }
    }
}

pub(super) fn id_set<'a>(ids: impl IntoIterator<Item = &'a str>) -> BTreeSet<&'a str> {
    ids.into_iter().collect()
}

/// Reports a reference to an id the document does not define.
pub(super) fn check_ref(known: &BTreeSet<&str>, reference: &str, path: &str, errors: &mut Errors) {
    if !known.contains(reference) {
        push(
            errors,
            "missing-reference",
            path,
            format!("references unknown id {reference}"),
        );
    }
}

/// Reports a recorded hash that differs from the recomputed one.
pub(super) fn check_hash(recorded: &str, computed: &str, path: &str, errors: &mut Errors) {
    if recorded != computed {
        push(
            errors,
            "hash-mismatch",
            path,
            format!("recorded {recorded}, recomputed {computed}"),
        );
    }
}

/// The contract fingerprint rule: SHA-256 of the sorted values, each followed
/// by a single LF. The empty list hashes the empty string.
pub(super) fn sorted_fingerprint(values: &[&str]) -> String {
    let mut sorted = values.to_vec();
    sorted.sort_unstable();
    let mut text = String::new();
    for value in sorted {
        text.push_str(value);
        text.push('\n');
    }
    sha256_text(&text)
}

pub(super) fn sha256_text(text: &str) -> String {
    sha256_hex(text.as_bytes())
}
