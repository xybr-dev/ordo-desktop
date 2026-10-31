//! `parish-backup`: a complete portable snapshot of one Parish scope
//! (MVP-PLAN 9.3).
//!
//! Fingerprint rule: `assets_fingerprint` is the sorted-values fingerprint of
//! `database.sha256` followed by every `assets[].sha256` (`hash-mismatch`).
//! The database bytes themselves are checked by the container layer against
//! `database.sha256`. Semantic rule: `parish.logo_asset_id` must name an asset
//! (`missing-reference`).

use super::support::{
    AssetRef, BACKUP_DATABASE_CEILING, Errors, check_asset_ref, check_hash, check_ref,
    check_relative, check_sha256, check_text, check_timestamp, check_unique, check_uuid, child,
    id_set, index, object, schema, sorted_fingerprint, version_gate,
};
use serde::Deserialize;
use serde_json::Value;

pub(super) const REQUIRED: &[&str] = &[
    "assets",
    "assets_fingerprint",
    "created_at",
    "database",
    "id",
    "parish",
    "schema_version",
];
pub(super) const OPTIONAL: &[&str] = &["package_provenance"];

#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct Parish {
    id: String,
    name: String,
    logo_asset_id: Option<String>,
}

#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct Database {
    path: String,
    sha256: String,
    size: u64,
    schema_version: u64,
}

#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct Provenance {
    package_version_id: String,
    fingerprint: String,
}

struct Backup {
    logo_asset_id: Option<String>,
    database: Database,
    assets: Vec<AssetRef>,
    assets_fingerprint: String,
}

pub(super) fn check(value: &Value, errors: &mut Errors) {
    let before = errors.len();
    if let Some(backup) = parse(value, "", errors)
        && errors.len() == before
    {
        semantics(&backup, "", errors);
    }
}

fn parse(value: &Value, base: &str, errors: &mut Errors) -> Option<Backup> {
    if !version_gate("parish-backup", value, base, errors) {
        return None;
    }
    let obj = object(value, base, errors)?;
    obj.reject_unknown(&[REQUIRED, OPTIONAL].concat(), errors);
    let id = obj.required::<String>("id", errors);
    let created_at = obj.required::<String>("created_at", errors);
    let parish = obj.required::<Parish>("parish", errors);
    let database = obj.required::<Database>("database", errors);
    let assets = obj.required_list::<AssetRef>("assets", errors);
    let assets_fingerprint = obj.required::<String>("assets_fingerprint", errors);
    let provenance = obj.optional_list::<Provenance>("package_provenance", errors);
    let id = id?;
    let created_at = created_at?;
    let parish = parish?;
    let database = database?;
    let assets = assets?;
    let assets_fingerprint = assets_fingerprint?;

    check_uuid(&id, &child(base, "id"), errors);
    check_timestamp(&created_at, &child(base, "created_at"), errors);

    let parish_path = child(base, "parish");
    check_uuid(&parish.id, &child(&parish_path, "id"), errors);
    check_text(&parish.name, &child(&parish_path, "name"), errors);
    if let Some(logo) = &parish.logo_asset_id {
        check_uuid(logo, &child(&parish_path, "logo_asset_id"), errors);
    }

    let database_path = child(base, "database");
    check_relative(
        &database.path,
        "database/",
        &child(&database_path, "path"),
        errors,
    );
    check_sha256(&database.sha256, &child(&database_path, "sha256"), errors);
    if database.size > BACKUP_DATABASE_CEILING {
        schema(
            errors,
            &child(&database_path, "size"),
            "exceeds the 4 GiB backup ceiling",
        );
    }
    if database.schema_version < 1 {
        schema(
            errors,
            &child(&database_path, "schema_version"),
            "must be at least 1",
        );
    }

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
    check_sha256(
        &assets_fingerprint,
        &child(base, "assets_fingerprint"),
        errors,
    );

    let provenance_path = child(base, "package_provenance");
    for (position, entry) in provenance.iter().enumerate() {
        let path = index(&provenance_path, position);
        check_uuid(
            &entry.package_version_id,
            &child(&path, "package_version_id"),
            errors,
        );
        check_sha256(&entry.fingerprint, &child(&path, "fingerprint"), errors);
    }

    Some(Backup {
        logo_asset_id: parish.logo_asset_id,
        database,
        assets,
        assets_fingerprint,
    })
}

fn semantics(backup: &Backup, base: &str, errors: &mut Errors) {
    if let Some(logo) = &backup.logo_asset_id {
        let asset_ids = id_set(backup.assets.iter().map(|asset| asset.id.as_str()));
        check_ref(
            &asset_ids,
            logo,
            &child(&child(base, "parish"), "logo_asset_id"),
            errors,
        );
    }

    let mut hashes = vec![backup.database.sha256.as_str()];
    hashes.extend(backup.assets.iter().map(|asset| asset.sha256.as_str()));
    check_hash(
        &backup.assets_fingerprint,
        &sorted_fingerprint(&hashes),
        &child(base, "assets_fingerprint"),
        errors,
    );
}
