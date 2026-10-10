//! Command-line driver for the container spike.
//!
//! `create <kind> <payload.json> <out.ordo>` writes a container with no assets.
//! `inspect <file.ordo>` prints the outcome and stages into a scratch directory
//! that is removed on exit. Exit status is 0 only for a valid container.

use artifact_container_spike::{
    ArtifactKind, InspectLimits, Inspection, ScratchDir, create, inspect, unsupported_message,
};
use std::{fs, path::Path, process::ExitCode, str::FromStr};

const USAGE: &str = "usage:\n  artifact-container-spike create <kind> <payload.json> <out.ordo>\n  artifact-container-spike inspect <file.ordo>";

fn main() -> ExitCode {
    let args: Vec<String> = std::env::args().skip(1).collect();
    let refs: Vec<&str> = args.iter().map(String::as_str).collect();
    let outcome = match refs.as_slice() {
        ["create", kind, payload, out] => create_command(kind, payload, out),
        ["inspect", archive] => inspect_command(archive),
        _ => Err(USAGE.to_owned()),
    };
    match outcome {
        Ok(code) => code,
        Err(message) => {
            eprintln!("{message}");
            ExitCode::from(2)
        }
    }
}

fn create_command(kind: &str, payload_path: &str, out: &str) -> Result<ExitCode, String> {
    let kind = ArtifactKind::from_str(kind)?;
    let text =
        fs::read_to_string(payload_path).map_err(|e| format!("cannot read {payload_path}: {e}"))?;
    let payload: serde_json::Value =
        serde_json::from_str(&text).map_err(|e| format!("{payload_path} is not JSON: {e}"))?;
    match create(Path::new(out), kind, &payload, &[]) {
        Ok(manifest) => {
            println!("created {out}");
            println!(
                "{}",
                serde_json::to_string_pretty(&manifest).map_err(|e| e.to_string())?
            );
            Ok(ExitCode::SUCCESS)
        }
        Err(error) => {
            println!("not created: {error}");
            Ok(ExitCode::FAILURE)
        }
    }
}

fn inspect_command(archive: &str) -> Result<ExitCode, String> {
    let scratch = ScratchDir::new("inspect-cli").map_err(|e| e.to_string())?;
    match inspect(
        Path::new(archive),
        scratch.path(),
        &InspectLimits::default(),
    ) {
        Inspection::Valid(routed) => {
            println!(
                "valid: {} v{} (artifact {}, created {})",
                routed.kind.as_str(),
                routed.manifest.schema_version,
                routed.manifest.artifact_id,
                routed.manifest.created_at
            );
            println!("staged {} asset(s)", routed.staged_assets.len());
            Ok(ExitCode::SUCCESS)
        }
        Inspection::Invalid(failures) => {
            println!("invalid:");
            for failure in &failures {
                println!("  {failure:?}");
            }
            Ok(ExitCode::FAILURE)
        }
        Inspection::Unsupported {
            kind,
            version,
            supported,
        } => {
            println!(
                "unsupported: {}",
                unsupported_message(&kind, version, supported)
            );
            Ok(ExitCode::FAILURE)
        }
        Inspection::NeedsDecision { reasons } => {
            println!("needs decision: {reasons:?}");
            Ok(ExitCode::FAILURE)
        }
    }
}
