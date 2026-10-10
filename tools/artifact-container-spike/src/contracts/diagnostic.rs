//! `diagnostic-report`: deliberately exported, sanitized troubleshooting
//! evidence (decision 205). Opening one is always read-only.
//!
//! Rules: `read_only` must be `true`; `assets_fingerprint` is the sorted-values
//! fingerprint of every `assets[].sha256` (`hash-mismatch`); each log's
//! `asset_id` must name one of the report's assets (`missing-reference`).

use super::support::{
    AssetRef, Errors, check_asset_ref, check_hash, check_ref, check_sha256, check_text,
    check_timestamp, check_unique, check_uuid, child, id_set, index, object, schema,
    sorted_fingerprint, version_gate,
};
use serde::Deserialize;
use serde_json::Value;

pub(super) const REQUIRED: &[&str] = &[
    "app",
    "assets",
    "assets_fingerprint",
    "checks",
    "environment",
    "generated_at",
    "id",
    "logs",
    "read_only",
    "schema_version",
];
pub(super) const OPTIONAL: &[&str] = &[];

#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct App {
    build: String,
    version: String,
}

#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct Environment {
    os: String,
    arch: String,
}

#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct Check {
    id: String,
    code: String,
    status: String,
    detail: Option<String>,
}

#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct Log {
    id: String,
    asset_id: String,
    code: Option<String>,
    timestamp: String,
}

struct Report {
    logs: Vec<Log>,
    assets: Vec<AssetRef>,
    assets_fingerprint: String,
}

pub(super) fn check(value: &Value, errors: &mut Errors) {
    let before = errors.len();
    if let Some(report) = parse(value, "", errors)
        && errors.len() == before
    {
        semantics(&report, "", errors);
    }
}

fn parse(value: &Value, base: &str, errors: &mut Errors) -> Option<Report> {
    if !version_gate("diagnostic-report", value, base, errors) {
        return None;
    }
    let obj = object(value, base, errors)?;
    obj.reject_unknown(&[REQUIRED, OPTIONAL].concat(), errors);
    let id = obj.required::<String>("id", errors);
    let read_only = obj.required::<bool>("read_only", errors);
    let generated_at = obj.required::<String>("generated_at", errors);
    let app = obj.required::<App>("app", errors);
    let environment = obj.required::<Environment>("environment", errors);
    let checks = obj.required_list::<Check>("checks", errors);
    let logs = obj.required_list::<Log>("logs", errors);
    let assets = obj.required_list::<AssetRef>("assets", errors);
    let assets_fingerprint = obj.required::<String>("assets_fingerprint", errors);
    let id = id?;
    let read_only = read_only?;
    let generated_at = generated_at?;
    let app = app?;
    let environment = environment?;
    let checks = checks?;
    let logs = logs?;
    let assets = assets?;
    let assets_fingerprint = assets_fingerprint?;

    check_uuid(&id, &child(base, "id"), errors);
    if !read_only {
        schema(errors, &child(base, "read_only"), "must be true");
    }
    check_timestamp(&generated_at, &child(base, "generated_at"), errors);
    let app_path = child(base, "app");
    check_text(&app.build, &child(&app_path, "build"), errors);
    check_text(&app.version, &child(&app_path, "version"), errors);
    let environment_path = child(base, "environment");
    check_text(&environment.os, &child(&environment_path, "os"), errors);
    check_text(&environment.arch, &child(&environment_path, "arch"), errors);

    let checks_path = child(base, "checks");
    for (position, entry) in checks.iter().enumerate() {
        let path = index(&checks_path, position);
        check_uuid(&entry.id, &child(&path, "id"), errors);
        check_text(&entry.code, &child(&path, "code"), errors);
        if !matches!(entry.status.as_str(), "pass" | "warn" | "fail") {
            schema(
                errors,
                &child(&path, "status"),
                "must be pass, warn, or fail",
            );
        }
        if let Some(detail) = &entry.detail {
            check_text(detail, &child(&path, "detail"), errors);
        }
    }

    let logs_path = child(base, "logs");
    for (position, entry) in logs.iter().enumerate() {
        let path = index(&logs_path, position);
        check_uuid(&entry.id, &child(&path, "id"), errors);
        check_uuid(&entry.asset_id, &child(&path, "asset_id"), errors);
        if let Some(code) = &entry.code {
            check_text(code, &child(&path, "code"), errors);
        }
        check_timestamp(&entry.timestamp, &child(&path, "timestamp"), errors);
    }

    let assets_path = child(base, "assets");
    for (position, asset) in assets.iter().enumerate() {
        check_asset_ref(asset, &index(&assets_path, position), errors);
    }
    check_unique(
        assets
            .iter()
            .enumerate()
            .map(|(position, asset)| {
                (
                    asset.id.as_str(),
                    child(&index(&assets_path, position), "id"),
                )
            })
            .chain(checks.iter().enumerate().map(|(position, entry)| {
                (
                    entry.id.as_str(),
                    child(&index(&checks_path, position), "id"),
                )
            }))
            .chain(logs.iter().enumerate().map(|(position, entry)| {
                (entry.id.as_str(), child(&index(&logs_path, position), "id"))
            })),
        errors,
    );
    check_sha256(
        &assets_fingerprint,
        &child(base, "assets_fingerprint"),
        errors,
    );

    Some(Report {
        logs,
        assets,
        assets_fingerprint,
    })
}

fn semantics(report: &Report, base: &str, errors: &mut Errors) {
    let asset_ids = id_set(report.assets.iter().map(|asset| asset.id.as_str()));
    let logs_path = child(base, "logs");
    for (position, entry) in report.logs.iter().enumerate() {
        check_ref(
            &asset_ids,
            &entry.asset_id,
            &child(&index(&logs_path, position), "asset_id"),
            errors,
        );
    }

    let hashes: Vec<&str> = report
        .assets
        .iter()
        .map(|asset| asset.sha256.as_str())
        .collect();
    check_hash(
        &report.assets_fingerprint,
        &sorted_fingerprint(&hashes),
        &child(base, "assets_fingerprint"),
        errors,
    );
}
