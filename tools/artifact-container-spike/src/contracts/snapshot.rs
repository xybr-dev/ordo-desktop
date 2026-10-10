//! `snapshot-manifest`: an immutable Presentation snapshot of one Mass plan
//! (CONTEXT: Presentation snapshot). Embedded by `celebration-package`.
//!
//! Fingerprint rule: `assets_fingerprint` is the sorted-values fingerprint of
//! every `assets[].sha256`. Semantic rule: every item `asset_ids` entry must name
//! an asset of this snapshot (`missing-reference`).

use super::support::{
    AssetRef, Errors, check_asset_ref, check_hash, check_list_uuid, check_ref, check_sha256,
    check_text, check_timestamp, check_unique, check_uuid, child, id_set, index, object, schema,
    sorted_fingerprint, version_gate,
};
use serde::Deserialize;
use serde_json::Value;

pub(super) const REQUIRED: &[&str] = &[
    "assets",
    "assets_fingerprint",
    "id",
    "items",
    "mass_plan_id",
    "schema_version",
];
pub(super) const OPTIONAL: &[&str] = &["captured_at", "celebration_id"];

#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
pub(super) struct Item {
    pub(super) id: String,
    pub(super) title: String,
    pub(super) asset_ids: Vec<String>,
}

pub(super) struct Snapshot {
    pub(super) id: String,
    pub(super) items: Vec<Item>,
    pub(super) assets: Vec<AssetRef>,
    pub(super) assets_fingerprint: String,
}

pub(super) fn check(value: &Value, errors: &mut Errors) {
    let before = errors.len();
    if let Some(snapshot) = parse(value, "", errors)
        && errors.len() == before
    {
        semantics(&snapshot, "", errors);
    }
}

pub(super) fn parse(value: &Value, base: &str, errors: &mut Errors) -> Option<Snapshot> {
    if !version_gate("snapshot-manifest", value, base, errors) {
        return None;
    }
    let obj = object(value, base, errors)?;
    obj.reject_unknown(&[REQUIRED, OPTIONAL].concat(), errors);
    let id = obj.required::<String>("id", errors);
    let mass_plan_id = obj.required::<String>("mass_plan_id", errors);
    let items = obj.required_list::<Item>("items", errors);
    let assets = obj.required_list::<AssetRef>("assets", errors);
    let assets_fingerprint = obj.required::<String>("assets_fingerprint", errors);
    let celebration_id = obj.optional::<String>("celebration_id", errors);
    let captured_at = obj.optional::<String>("captured_at", errors);
    let id = id?;
    let mass_plan_id = mass_plan_id?;
    let items = items?;
    let assets = assets?;
    let assets_fingerprint = assets_fingerprint?;

    check_uuid(&id, &child(base, "id"), errors);
    check_uuid(&mass_plan_id, &child(base, "mass_plan_id"), errors);
    if let Some(celebration_id) = &celebration_id {
        check_uuid(celebration_id, &child(base, "celebration_id"), errors);
    }
    if let Some(captured_at) = &captured_at {
        check_timestamp(captured_at, &child(base, "captured_at"), errors);
    }
    check_sha256(
        &assets_fingerprint,
        &child(base, "assets_fingerprint"),
        errors,
    );

    let items_path = child(base, "items");
    if items.is_empty() {
        schema(errors, &items_path, "must contain at least one item");
    }
    for (position, item) in items.iter().enumerate() {
        let path = index(&items_path, position);
        check_uuid(&item.id, &child(&path, "id"), errors);
        check_text(&item.title, &child(&path, "title"), errors);
        check_list_uuid(&item.asset_ids, &child(&path, "asset_ids"), errors);
    }
    check_unique(
        items
            .iter()
            .enumerate()
            .map(|(position, item)| (item.id.as_str(), child(&index(&items_path, position), "id"))),
        errors,
    );

    let assets_path = child(base, "assets");
    for (position, asset) in assets.iter().enumerate() {
        check_asset_ref(asset, &index(&assets_path, position), errors);
    }
    check_unique(
        assets.iter().enumerate().map(|(position, asset)| {
            (
                asset.id.as_str(),
                child(&index(&assets_path, position), "id"),
            )
        }),
        errors,
    );

    Some(Snapshot {
        id,
        items,
        assets,
        assets_fingerprint,
    })
}

pub(super) fn semantics(snapshot: &Snapshot, base: &str, errors: &mut Errors) {
    let asset_ids = id_set(snapshot.assets.iter().map(|asset| asset.id.as_str()));
    let items_path = child(base, "items");
    for (position, item) in snapshot.items.iter().enumerate() {
        let references_path = child(&index(&items_path, position), "asset_ids");
        for (reference_position, reference) in item.asset_ids.iter().enumerate() {
            check_ref(
                &asset_ids,
                reference,
                &index(&references_path, reference_position),
                errors,
            );
        }
    }

    let hashes: Vec<&str> = snapshot
        .assets
        .iter()
        .map(|asset| asset.sha256.as_str())
        .collect();
    check_hash(
        &snapshot.assets_fingerprint,
        &sorted_fingerprint(&hashes),
        &child(base, "assets_fingerprint"),
        errors,
    );
}
