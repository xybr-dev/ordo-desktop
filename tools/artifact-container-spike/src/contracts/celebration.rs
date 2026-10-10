//! `celebration-package`: one complete Celebration with an independent snapshot
//! for every included Mass plan, plus the deduplicated offline assets.
//!
//! Rules:
//! - `package_version.fingerprint` is the sorted-values fingerprint of the
//!   snapshot `id`s (`hash-mismatch`).
//! - Every snapshot asset must appear in the package `assets` list with the same
//!   id, path, size, and sha256. A missing id is `missing-reference`; any
//!   difference is `hash-mismatch`.
//! - Each embedded snapshot follows the `snapshot-manifest` rules in full.

use super::snapshot::{self, Snapshot};
use super::support::{
    AssetRef, Errors, check_asset_ref, check_date, check_hash, check_sha256, check_text,
    check_timestamp, check_unique, check_uuid, child, index, object, push, schema,
    sorted_fingerprint, version_gate,
};
use serde::Deserialize;
use serde_json::Value;
use std::collections::BTreeMap;

pub(super) const REQUIRED: &[&str] = &[
    "assets",
    "celebration",
    "id",
    "package_version",
    "schema_version",
    "snapshots",
];
pub(super) const OPTIONAL: &[&str] = &["exported_at"];

#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct Celebration {
    id: String,
    date: String,
    title: String,
}

#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct PackageVersion {
    id: String,
    fingerprint: String,
}

struct Package {
    package_version: PackageVersion,
    snapshots: Vec<Snapshot>,
    assets: Vec<AssetRef>,
}

pub(super) fn check(value: &Value, errors: &mut Errors) {
    let before = errors.len();
    if let Some(package) = parse(value, "", errors)
        && errors.len() == before
    {
        semantics(&package, "", errors);
    }
}

fn parse(value: &Value, base: &str, errors: &mut Errors) -> Option<Package> {
    if !version_gate("celebration-package", value, base, errors) {
        return None;
    }
    let obj = object(value, base, errors)?;
    obj.reject_unknown(&[REQUIRED, OPTIONAL].concat(), errors);
    let id = obj.required::<String>("id", errors);
    let celebration = obj.required::<Celebration>("celebration", errors);
    let package_version = obj.required::<PackageVersion>("package_version", errors);
    let snapshots = obj.required_list::<Value>("snapshots", errors);
    let assets = obj.required_list::<AssetRef>("assets", errors);
    let exported_at = obj.optional::<String>("exported_at", errors);
    let id = id?;
    let celebration = celebration?;
    let package_version = package_version?;
    let raw_snapshots = snapshots?;
    let assets = assets?;

    check_uuid(&id, &child(base, "id"), errors);
    let celebration_path = child(base, "celebration");
    check_uuid(&celebration.id, &child(&celebration_path, "id"), errors);
    check_text(
        &celebration.title,
        &child(&celebration_path, "title"),
        errors,
    );
    check_date(&celebration.date, &child(&celebration_path, "date"), errors);
    let version_path = child(base, "package_version");
    check_uuid(&package_version.id, &child(&version_path, "id"), errors);
    check_sha256(
        &package_version.fingerprint,
        &child(&version_path, "fingerprint"),
        errors,
    );
    if let Some(exported_at) = &exported_at {
        check_timestamp(exported_at, &child(base, "exported_at"), errors);
    }

    let snapshots_path = child(base, "snapshots");
    if raw_snapshots.is_empty() {
        schema(
            errors,
            &snapshots_path,
            "must contain at least one snapshot",
        );
    }
    let snapshots: Vec<Snapshot> = raw_snapshots
        .iter()
        .enumerate()
        .filter_map(|(position, raw)| {
            snapshot::parse(raw, &index(&snapshots_path, position), errors)
        })
        .collect();
    check_unique(
        snapshots.iter().enumerate().map(|(position, snapshot)| {
            (
                snapshot.id.as_str(),
                child(&index(&snapshots_path, position), "id"),
            )
        }),
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

    Some(Package {
        package_version,
        snapshots,
        assets,
    })
}

fn semantics(package: &Package, base: &str, errors: &mut Errors) {
    let snapshots_path = child(base, "snapshots");
    let listed: BTreeMap<&str, &AssetRef> = package
        .assets
        .iter()
        .map(|asset| (asset.id.as_str(), asset))
        .collect();

    for (position, snapshot) in package.snapshots.iter().enumerate() {
        let path = index(&snapshots_path, position);
        snapshot::semantics(snapshot, &path, errors);

        let assets_path = child(&path, "assets");
        for (asset_position, asset) in snapshot.assets.iter().enumerate() {
            let asset_path = index(&assets_path, asset_position);
            match listed.get(asset.id.as_str()) {
                None => push(
                    errors,
                    "missing-reference",
                    &asset_path,
                    format!("asset {} is not listed in the package assets", asset.id),
                ),
                Some(listed_asset) => {
                    if listed_asset.path != asset.path
                        || listed_asset.size != asset.size
                        || listed_asset.sha256 != asset.sha256
                    {
                        push(
                            errors,
                            "hash-mismatch",
                            &asset_path,
                            format!("asset {} differs from the package asset list", asset.id),
                        );
                    }
                }
            }
        }
    }

    let snapshot_ids: Vec<&str> = package
        .snapshots
        .iter()
        .map(|snapshot| snapshot.id.as_str())
        .collect();
    check_hash(
        &package.package_version.fingerprint,
        &sorted_fingerprint(&snapshot_ids),
        &child(&child(base, "package_version"), "fingerprint"),
        errors,
    );
}
