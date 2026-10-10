//! Fixture corpus and schema drift guards for the seven payload contracts, plus
//! the container manifest schema.

use super::{Contract, declared_keys, validate_contract};
use crate::{ArtifactKind, FORMAT, FileRef, Manifest, create};
use jsonschema::Validator;
use serde_json::Value;
use std::{
    collections::BTreeSet,
    fs::{self, File},
    io::Read,
    path::{Path, PathBuf},
};

const CONTRACTS: [Contract; 7] = [
    Contract::SongImport,
    Contract::LiturgyImport,
    Contract::SnapshotManifest,
    Contract::CelebrationPackage,
    Contract::ParishBackup,
    Contract::DiagnosticReport,
    Contract::RecoveryJournal,
];

const VALID: [&str; 3] = [
    "minimal.valid.json",
    "representative.valid.json",
    "boundary.valid.json",
];
const REJECT: [&str; 4] = [
    "invalid.reject.json",
    "unknown-version.reject.json",
    "missing-reference.reject.json",
    "tampered-hash.reject.json",
];
const CODES: [&str; 4] = [
    "schema",
    "unsupported-version",
    "missing-reference",
    "hash-mismatch",
];

fn repo_root() -> PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR")).join("../..")
}

fn fixtures_dir(contract: Contract) -> PathBuf {
    repo_root()
        .join("tests/fixtures/contracts")
        .join(contract.name())
}

fn read_json(path: &Path) -> Value {
    let text =
        fs::read_to_string(path).unwrap_or_else(|error| panic!("{}: {error}", path.display()));
    serde_json::from_str(&text).unwrap_or_else(|error| panic!("{}: {error}", path.display()))
}

fn strings(value: &Value) -> BTreeSet<String> {
    value
        .as_array()
        .expect("array")
        .iter()
        .map(|item| item.as_str().expect("string").to_owned())
        .collect()
}

#[test]
fn every_contract_has_all_seven_fixture_types() {
    for contract in CONTRACTS {
        let dir = fixtures_dir(contract);
        let present: BTreeSet<String> = fs::read_dir(&dir)
            .unwrap_or_else(|error| panic!("{}: {error}", dir.display()))
            .map(|entry| entry.unwrap().file_name().to_string_lossy().into_owned())
            .collect();
        let mut expected: BTreeSet<String> = VALID.iter().map(|name| (*name).to_owned()).collect();
        for name in REJECT {
            expected.insert(name.to_owned());
            expected.insert(name.replace(".json", ".expect"));
        }
        assert_eq!(present, expected, "fixture set for {}", contract.name());
    }
}

#[test]
fn valid_fixtures_pass_and_reject_fixtures_fail_with_their_expected_code() {
    for contract in CONTRACTS {
        let dir = fixtures_dir(contract);
        for name in VALID {
            let doc = read_json(&dir.join(name));
            if let Err(errors) = validate_contract(contract, &doc) {
                panic!("{} {name} should be valid: {errors:#?}", contract.name());
            }
        }
        for name in REJECT {
            let doc = read_json(&dir.join(name));
            let expect_path = dir.join(name.replace(".json", ".expect"));
            let expected = fs::read_to_string(&expect_path)
                .unwrap_or_else(|error| panic!("{}: {error}", expect_path.display()));
            assert!(
                !expected.contains(char::is_whitespace),
                "{} must contain exactly one code",
                expect_path.display()
            );
            assert!(
                CODES.contains(&expected.as_str()),
                "unknown code {expected}"
            );
            match validate_contract(contract, &doc) {
                Ok(()) => panic!("{} {name} should be rejected", contract.name()),
                Err(errors) => {
                    assert!(!errors.is_empty());
                    for error in &errors {
                        assert_eq!(
                            error.code,
                            expected,
                            "{} {name}: {error:?}",
                            contract.name()
                        );
                    }
                }
            }
        }
    }
}

#[test]
fn author_examples_match_the_minimal_fixture() {
    for contract in CONTRACTS {
        let example = read_json(
            &repo_root()
                .join("contracts")
                .join(contract.name())
                .join("examples/minimal.json"),
        );
        let fixture = read_json(&fixtures_dir(contract).join("minimal.valid.json"));
        assert_eq!(example, fixture, "{}", contract.name());
    }
}

#[test]
fn schemas_parse_and_match_the_validator_key_lists() {
    for contract in CONTRACTS {
        let name = contract.name();
        let schema = read_json(
            &repo_root()
                .join("contracts")
                .join(name)
                .join("v1.schema.json"),
        );
        assert_eq!(
            schema["$schema"],
            "https://json-schema.org/draft/2020-12/schema"
        );
        assert_eq!(
            schema["$id"],
            format!("https://ordo.invalid/contracts/{name}/v1.schema.json")
        );
        assert!(schema["title"].is_string(), "{name} title");
        assert!(schema["description"].is_string(), "{name} description");
        assert_eq!(schema["type"], "object");
        assert_eq!(schema["additionalProperties"], false, "{name}");
        assert_eq!(schema["properties"]["schema_version"]["const"], 1, "{name}");

        let (required, optional) = declared_keys(contract);
        let declared_required: BTreeSet<String> =
            required.iter().map(|key| (*key).to_owned()).collect();
        let declared_all: BTreeSet<String> = required
            .iter()
            .chain(optional)
            .map(|key| (*key).to_owned())
            .collect();
        assert_eq!(
            strings(&schema["required"]),
            declared_required,
            "{name} required"
        );
        let properties: BTreeSet<String> = schema["properties"]
            .as_object()
            .expect("properties")
            .keys()
            .cloned()
            .collect();
        assert_eq!(properties, declared_all, "{name} properties");
    }
}

#[test]
fn container_manifest_schema_matches_the_manifest_struct() {
    let schema = read_json(&repo_root().join("contracts/container/manifest.v1.schema.json"));
    assert_eq!(
        schema["$schema"],
        "https://json-schema.org/draft/2020-12/schema"
    );
    assert_eq!(
        schema["$id"],
        "https://ordo.invalid/contracts/container/manifest.v1.schema.json"
    );
    assert!(schema["title"].is_string());
    assert_eq!(schema["additionalProperties"], false);

    let manifest = Manifest {
        format: FORMAT.to_owned(),
        format_version: 1,
        kind: ArtifactKind::SongImport,
        schema_version: 1,
        artifact_id: String::new(),
        created_at: String::new(),
        payload: FileRef {
            path: String::new(),
            sha256: String::new(),
            size: 0,
        },
        assets: Vec::new(),
    };
    let serialized = serde_json::to_value(&manifest).expect("serialize");
    let struct_keys: BTreeSet<String> = serialized
        .as_object()
        .expect("object")
        .keys()
        .cloned()
        .collect();
    assert_eq!(strings(&schema["required"]), struct_keys, "required");
    let properties: BTreeSet<String> = schema["properties"]
        .as_object()
        .expect("properties")
        .keys()
        .cloned()
        .collect();
    assert_eq!(properties, struct_keys, "properties");
}

fn manifest_schema_path() -> PathBuf {
    repo_root().join("contracts/container/manifest.v1.schema.json")
}

/// Compiles a schema with the offline JSON Schema 2020-12 engine. Only local
/// `#/$defs` references are used; remote references cannot resolve.
fn load_validator(path: &Path) -> Validator {
    let schema = read_json(path);
    jsonschema::validator_for(&schema).unwrap_or_else(|error| panic!("{}: {error}", path.display()))
}

fn schema_errors(validator: &Validator, doc: &Value) -> Vec<String> {
    validator
        .iter_errors(doc)
        .map(|error| error.to_string())
        .collect()
}

#[test]
fn fixtures_and_examples_match_their_json_schemas() {
    for contract in CONTRACTS {
        let name = contract.name();
        let validator = load_validator(
            &repo_root()
                .join("contracts")
                .join(name)
                .join("v1.schema.json"),
        );
        let dir = fixtures_dir(contract);
        for file in VALID {
            let doc = read_json(&dir.join(file));
            let errors = schema_errors(&validator, &doc);
            assert!(
                errors.is_empty(),
                "{name} {file} must pass its schema: {errors:#?}"
            );
        }
        for file in ["invalid.reject.json", "unknown-version.reject.json"] {
            let doc = read_json(&dir.join(file));
            assert!(
                !validator.is_valid(&doc),
                "{name} {file} must fail its schema"
            );
        }
        // Semantic-only rejections: the schema cannot see references or hashes,
        // so these must pass the schema and fail only the Rust validator.
        for file in ["missing-reference.reject.json", "tampered-hash.reject.json"] {
            let doc = read_json(&dir.join(file));
            let errors = schema_errors(&validator, &doc);
            assert!(
                errors.is_empty(),
                "{name} {file} must pass its schema: {errors:#?}"
            );
            assert!(
                validate_contract(contract, &doc).is_err(),
                "{name} {file} must fail the Rust validator"
            );
        }
        let example = read_json(
            &repo_root()
                .join("contracts")
                .join(name)
                .join("examples/minimal.json"),
        );
        let errors = schema_errors(&validator, &example);
        assert!(errors.is_empty(), "{name} minimal example: {errors:#?}");
    }
}

fn read_zip_manifest(path: &Path) -> Value {
    let file = File::open(path).unwrap_or_else(|error| panic!("{}: {error}", path.display()));
    let mut archive = zip::ZipArchive::new(file).expect("valid zip");
    let mut entry = archive
        .by_name("manifest.json")
        .expect("manifest.json entry");
    let mut text = String::new();
    entry.read_to_string(&mut text).expect("utf-8 manifest");
    serde_json::from_str(&text).expect("manifest JSON")
}

#[test]
fn manifests_written_by_create_match_the_container_manifest_schema() {
    let validator = load_validator(&manifest_schema_path());
    let scratch = tempfile::tempdir().expect("tempdir");
    for kind in ArtifactKind::ALL {
        let payload = read_json(
            &repo_root()
                .join("contracts")
                .join(kind.as_str())
                .join("examples/minimal.json"),
        );
        let assets = vec![("assets/note.txt".to_owned(), b"note\n".to_vec())];
        let dest = scratch.path().join(format!("{}.ordo", kind.as_str()));
        create(&dest, kind, &payload, &assets)
            .unwrap_or_else(|error| panic!("{}: {error:?}", kind.as_str()));
        let manifest = read_zip_manifest(&dest);
        let errors = schema_errors(&validator, &manifest);
        assert!(errors.is_empty(), "{} manifest: {errors:#?}", kind.as_str());
    }
}

fn non_empty_text(value: &Value) -> bool {
    value.as_str().is_some_and(|text| !text.trim().is_empty())
}

/// AC4 evidence: every schema is documented at its root and at each top-level property.
#[test]
fn every_schema_documents_its_root_and_top_level_properties() {
    let mut schemas: Vec<PathBuf> = CONTRACTS
        .iter()
        .map(|contract| {
            repo_root()
                .join("contracts")
                .join(contract.name())
                .join("v1.schema.json")
        })
        .collect();
    schemas.push(manifest_schema_path());
    for path in schemas {
        let schema = read_json(&path);
        let label = path.display().to_string();
        assert!(
            non_empty_text(&schema["description"]),
            "{label}: root description"
        );
        for (key, property) in schema["properties"].as_object().expect("properties") {
            assert!(
                non_empty_text(&property["description"]),
                "{label}: top-level property `{key}` needs a description"
            );
        }
    }
}

/// AC4 evidence: every fixture and example is human-readable pretty JSON.
#[test]
fn every_fixture_and_example_is_pretty_printed() {
    let mut files = Vec::new();
    for contract in CONTRACTS {
        let dir = fixtures_dir(contract);
        for entry in fs::read_dir(&dir).unwrap_or_else(|error| panic!("{}: {error}", dir.display()))
        {
            let path = entry.expect("dir entry").path();
            if path.extension().is_some_and(|ext| ext == "json") {
                files.push(path);
            }
        }
        files.push(
            repo_root()
                .join("contracts")
                .join(contract.name())
                .join("examples/minimal.json"),
        );
    }
    for path in files {
        let text = fs::read_to_string(&path).expect("read fixture");
        assert!(
            text.contains('\n') && text.contains("\n  "),
            "{} must be pretty-printed with two-space indentation",
            path.display()
        );
        let value: Value = serde_json::from_str(&text).expect("fixture JSON");
        let pretty = serde_json::to_string_pretty(&value).expect("pretty JSON");
        let normalised = text.replace("\r\n", "\n");
        assert_eq!(
            normalised.trim_end_matches('\n'),
            pretty,
            "{} must round-trip through serde_json::to_string_pretty",
            path.display()
        );
    }
}
