//! `recovery-journal`: the local crash-recovery record of one Presentation
//! session (MVP-PLAN 9.2). It is a local file, not a portable artifact.
//!
//! Rules: `seq` runs 1..n without gaps (`schema`); `cue_position.last_entry_seq`
//! must be the last entry's `seq`, or 0 when there are none (`missing-reference`);
//! `entries_fingerprint` is the SHA-256 of one line per entry, in order, of the
//! form `<seq> TAB <kind> TAB <cue_index> LF` (`hash-mismatch`).

use super::support::{
    Errors, check_hash, check_sha256, check_timestamp, check_uuid, child, index, object, push,
    schema, sha256_text, version_gate,
};
use serde::Deserialize;
use serde_json::Value;

pub(super) const REQUIRED: &[&str] = &[
    "cue_position",
    "entries",
    "entries_fingerprint",
    "id",
    "schema_version",
    "session_id",
    "snapshot_id",
];
pub(super) const OPTIONAL: &[&str] = &["updated_at"];

const KINDS: &[&str] = &[
    "cue",
    "freeze",
    "unfreeze",
    "blackout",
    "clear-blackout",
    "session-end",
];

#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct CuePosition {
    // Typed for the schema; the rules use only `last_entry_seq`.
    #[allow(dead_code)]
    cue_index: u64,
    last_entry_seq: u64,
}

#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct Entry {
    seq: u64,
    at: String,
    kind: String,
    cue_index: u64,
}

struct Journal {
    cue_position: CuePosition,
    entries: Vec<Entry>,
    entries_fingerprint: String,
}

pub(super) fn check(value: &Value, errors: &mut Errors) {
    let before = errors.len();
    if let Some(journal) = parse(value, "", errors)
        && errors.len() == before
    {
        semantics(&journal, "", errors);
    }
}

fn parse(value: &Value, base: &str, errors: &mut Errors) -> Option<Journal> {
    if !version_gate("recovery-journal", value, base, errors) {
        return None;
    }
    let obj = object(value, base, errors)?;
    obj.reject_unknown(&[REQUIRED, OPTIONAL].concat(), errors);
    let id = obj.required::<String>("id", errors);
    let session_id = obj.required::<String>("session_id", errors);
    let snapshot_id = obj.required::<String>("snapshot_id", errors);
    let cue_position = obj.required::<CuePosition>("cue_position", errors);
    let entries = obj.required_list::<Entry>("entries", errors);
    let entries_fingerprint = obj.required::<String>("entries_fingerprint", errors);
    let updated_at = obj.optional::<String>("updated_at", errors);
    let id = id?;
    let session_id = session_id?;
    let snapshot_id = snapshot_id?;
    let cue_position = cue_position?;
    let entries = entries?;
    let entries_fingerprint = entries_fingerprint?;

    check_uuid(&id, &child(base, "id"), errors);
    check_uuid(&session_id, &child(base, "session_id"), errors);
    check_uuid(&snapshot_id, &child(base, "snapshot_id"), errors);
    if let Some(updated_at) = &updated_at {
        check_timestamp(updated_at, &child(base, "updated_at"), errors);
    }
    let entries_path = child(base, "entries");
    for (position, entry) in entries.iter().enumerate() {
        let path = index(&entries_path, position);
        let expected = position as u64 + 1;
        if entry.seq != expected {
            schema(
                errors,
                &child(&path, "seq"),
                format!("must be {expected} (sequence runs 1..n without gaps)"),
            );
        }
        check_timestamp(&entry.at, &child(&path, "at"), errors);
        if !KINDS.contains(&entry.kind.as_str()) {
            schema(
                errors,
                &child(&path, "kind"),
                "is not a known journal entry kind",
            );
        }
    }
    check_sha256(
        &entries_fingerprint,
        &child(base, "entries_fingerprint"),
        errors,
    );

    Some(Journal {
        cue_position,
        entries,
        entries_fingerprint,
    })
}

fn semantics(journal: &Journal, base: &str, errors: &mut Errors) {
    let last_seq = journal.entries.last().map_or(0, |entry| entry.seq);
    if journal.cue_position.last_entry_seq != last_seq {
        push(
            errors,
            "missing-reference",
            &child(&child(base, "cue_position"), "last_entry_seq"),
            format!(
                "references entry {} but the last entry is {last_seq}",
                journal.cue_position.last_entry_seq
            ),
        );
    }

    let mut text = String::new();
    for entry in &journal.entries {
        text.push_str(&format!(
            "{}\t{}\t{}\n",
            entry.seq, entry.kind, entry.cue_index
        ));
    }
    check_hash(
        &journal.entries_fingerprint,
        &sha256_text(&text),
        &child(base, "entries_fingerprint"),
        errors,
    );
}
