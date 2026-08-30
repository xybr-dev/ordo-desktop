use cargo_metadata::{DependencyKind, Metadata, MetadataCommand, Package};
use serde::Deserialize;
use std::{
    collections::{BTreeMap, BTreeSet},
    error::Error,
    ffi::OsStr,
    fs,
    path::{Path, PathBuf},
    process::{Command, Stdio},
};

pub type XtaskResult<T> = Result<T, Box<dyn Error>>;

#[derive(Debug, Deserialize)]
struct GitPins {
    zed: String,
    #[serde(rename = "gpui-base")]
    gpui_base: String,
}

#[derive(Debug, Deserialize)]
struct RuntimeManifest {
    schema_version: u32,
    network_required: bool,
    bundled_resources: Vec<String>,
}

#[derive(Debug, Deserialize)]
struct DesignTokens {
    schema_version: u32,
    canonical_slide: CanonicalSlide,
    appearance: Appearance,
}

#[derive(Debug, Deserialize)]
struct CanonicalSlide {
    width: u32,
    height: u32,
    aspect_ratio: String,
}

#[derive(Debug, Deserialize)]
struct Appearance {
    dark_background: String,
    light_background: String,
    focus_ring: String,
}

pub fn repo_root() -> PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR"))
        .parent()
        .expect("xtask must be inside the workspace")
        .to_path_buf()
}

pub fn verify() -> XtaskResult<()> {
    let root = repo_root();
    println!("[1/8] workspace contracts");
    verify_architecture(&root)?;
    println!("[2/8] design tokens");
    verify_design_tokens(&root)?;
    println!("[3/8] offline runtime contract");
    verify_offline_contract(&root)?;
    println!("[4/8] git pins");
    verify_git_pins(&root)?;
    println!("[5/8] rustfmt");
    run(&root, "cargo", &["fmt", "--all", "--check"])?;
    println!("[6/8] clippy");
    run(
        &root,
        "cargo",
        &[
            "clippy",
            "--workspace",
            "--all-targets",
            "--locked",
            "--",
            "-D",
            "warnings",
        ],
    )?;
    println!("[7/8] tests");
    run(&root, "cargo", &["test", "--workspace", "--locked"])?;
    println!("[8/8] dependencies, advisories, sources, and licenses");
    run(&root, "cargo", &["deny", "check"])?;
    println!("Ordo verification passed.");
    Ok(())
}

pub fn context() -> XtaskResult<()> {
    let root = repo_root();
    let metadata = metadata(&root)?;
    let pins = git_pins(&metadata)?;
    println!("Ordo workspace: {}", root.display());
    println!("Zed/GPUI pin: {}", pins.zed);
    println!("gpui-base pin: {}", pins.gpui_base);
    println!("Runtime: Offline; network not required");
    println!("Workspace dependencies:");
    for package in workspace_packages(&metadata) {
        let mut internal = internal_dependencies(package, &metadata);
        internal.sort_unstable();
        let dependencies = if internal.is_empty() {
            "(none)".to_owned()
        } else {
            internal.join(", ")
        };
        println!("  {} -> {dependencies}", package.name);
    }
    println!("Active milestone: {}", active_milestone(&root)?);
    Ok(())
}

pub fn verify_architecture(root: &Path) -> XtaskResult<()> {
    let metadata = metadata(root)?;
    let domain = workspace_packages(&metadata)
        .into_iter()
        .find(|package| package.name == "domain")
        .ok_or("domain package is missing")?;
    validate_domain_dependencies(
        &domain
            .dependencies
            .iter()
            .map(|dependency| dependency.name.as_str())
            .collect::<Vec<_>>(),
    )?;
    let actual_names: BTreeSet<_> = workspace_packages(&metadata)
        .into_iter()
        .map(|package| package.name.as_str())
        .collect();
    let expected_names: BTreeSet<_> = [
        "domain",
        "storage",
        "rendering",
        "presentation",
        "ui",
        "platform",
        "ordo",
        "ui-lab",
        "xtask",
    ]
    .into_iter()
    .collect();
    if actual_names != expected_names {
        return Err(format!(
            "workspace members differ: expected {expected_names:?}, got {actual_names:?}"
        )
        .into());
    }

    let edges: BTreeMap<_, _> = workspace_packages(&metadata)
        .into_iter()
        .map(|package| {
            (
                package.name.as_str(),
                internal_dependencies(package, &metadata)
                    .into_iter()
                    .collect::<BTreeSet<_>>(),
            )
        })
        .collect();
    validate_dependency_edges(&edges)
}

fn validate_dependency_edges(edges: &BTreeMap<&str, BTreeSet<&str>>) -> XtaskResult<()> {
    let allowed: BTreeMap<&str, BTreeSet<&str>> = [
        ("domain", BTreeSet::new()),
        ("storage", BTreeSet::from(["domain"])),
        ("rendering", BTreeSet::from(["domain"])),
        ("presentation", BTreeSet::from(["domain", "rendering"])),
        ("ui", BTreeSet::new()),
        ("platform", BTreeSet::new()),
        (
            "ordo",
            BTreeSet::from([
                "domain",
                "storage",
                "rendering",
                "presentation",
                "ui",
                "platform",
            ]),
        ),
        ("ui-lab", BTreeSet::from(["ui"])),
        ("xtask", BTreeSet::new()),
    ]
    .into_iter()
    .collect();

    for (package, actual) in edges {
        let expected = allowed
            .get(package)
            .ok_or_else(|| format!("no dependency contract for {package}"))?;
        if !actual.is_subset(expected) {
            let forbidden: Vec<_> = actual.difference(expected).collect();
            return Err(
                format!("forbidden internal dependencies for {package}: {forbidden:?}").into(),
            );
        }
    }
    Ok(())
}

fn validate_domain_dependencies(dependencies: &[&str]) -> XtaskResult<()> {
    if dependencies.is_empty() {
        Ok(())
    } else {
        Err(format!("domain must remain dependency-free, found {dependencies:?}").into())
    }
}

pub fn verify_design_tokens(root: &Path) -> XtaskResult<()> {
    let contents = fs::read_to_string(root.join("crates/ui/design-tokens.json"))?;
    validate_design_tokens(&contents)
}

fn validate_design_tokens(contents: &str) -> XtaskResult<()> {
    let tokens: DesignTokens = serde_json::from_str(contents)?;
    if tokens.schema_version != 1 {
        return Err("unsupported design-token schema".into());
    }
    if tokens.canonical_slide.width != 1920
        || tokens.canonical_slide.height != 1080
        || tokens.canonical_slide.aspect_ratio != "16:9"
    {
        return Err("canonical slide must remain 1920x1080 at 16:9".into());
    }
    for (name, color) in [
        ("dark_background", tokens.appearance.dark_background),
        ("light_background", tokens.appearance.light_background),
        ("focus_ring", tokens.appearance.focus_ring),
    ] {
        if !is_hex_color(&color) {
            return Err(format!("{name} must be an opaque #RRGGBB color").into());
        }
    }
    Ok(())
}

fn is_hex_color(value: &str) -> bool {
    value.len() == 7
        && value.starts_with('#')
        && value[1..].bytes().all(|byte| byte.is_ascii_hexdigit())
}

pub fn verify_offline_contract(root: &Path) -> XtaskResult<()> {
    let contents = fs::read_to_string(root.join("resources/runtime-manifest.json"))?;
    let manifest: RuntimeManifest = serde_json::from_str(&contents)?;
    validate_runtime_manifest(&manifest, root)?;
    for relative in ["crates", "tools/ui-lab"] {
        scan_runtime_source(&root.join(relative))?;
    }
    Ok(())
}

fn validate_runtime_manifest(manifest: &RuntimeManifest, root: &Path) -> XtaskResult<()> {
    if manifest.schema_version != 1 {
        return Err("unsupported runtime-manifest schema".into());
    }
    if manifest.network_required {
        return Err("Ordo runtime must not require a network".into());
    }
    for resource in &manifest.bundled_resources {
        if !root.join(resource).is_file() {
            return Err(format!("bundled resource is missing: {resource}").into());
        }
    }
    Ok(())
}

fn scan_runtime_source(path: &Path) -> XtaskResult<()> {
    for entry in fs::read_dir(path)? {
        let entry = entry?;
        let path = entry.path();
        if path.is_dir() {
            scan_runtime_source(&path)?;
        } else if path.extension() == Some(OsStr::new("rs")) {
            let source = fs::read_to_string(&path)?;
            validate_offline_source(&source)
                .map_err(|error| format!("{}: {error}", path.display()))?;
        }
    }
    Ok(())
}

fn validate_offline_source(source: &str) -> XtaskResult<()> {
    let forbidden = ["reqwest::", "TcpStream", "UdpSocket"];
    if let Some(token) = forbidden.iter().find(|token| source.contains(**token)) {
        return Err(format!("runtime network token is forbidden: {token}").into());
    }
    Ok(())
}

pub fn verify_git_pins(root: &Path) -> XtaskResult<()> {
    let metadata = metadata(root)?;
    let pins = git_pins(&metadata)?;
    let lock = fs::read_to_string(root.join("Cargo.lock"))?;
    validate_git_lock(&lock, &pins)
}

fn validate_git_lock(lock: &str, pins: &GitPins) -> XtaskResult<()> {
    let mut saw_zed = false;
    let mut saw_gpui_base = false;
    for line in lock.lines().filter(|line| line.contains("git+")) {
        let revision = line
            .rsplit_once('#')
            .map(|(_, revision)| revision.trim_end_matches('"'))
            .ok_or_else(|| format!("git source lacks an immutable revision: {line}"))?;
        if revision.len() != 40 || !revision.bytes().all(|byte| byte.is_ascii_hexdigit()) {
            return Err(format!("git source lacks a full immutable revision: {line}").into());
        }
        if line.contains("github.com/zed-industries/zed#") {
            saw_zed = true;
            if revision != pins.zed {
                return Err(format!(
                    "Zed/GPUI lock revision is {revision}, expected {}",
                    pins.zed
                )
                .into());
            }
        }
        if line.contains("github.com/longbridge/gpui-component?rev=") {
            saw_gpui_base = true;
            if revision != pins.gpui_base {
                return Err(format!(
                    "gpui-base lock revision is {revision}, expected {}",
                    pins.gpui_base
                )
                .into());
            }
        }
    }
    if !saw_zed || !saw_gpui_base {
        return Err("Cargo.lock must contain the pinned GPUI and gpui-base sources".into());
    }
    Ok(())
}

fn git_pins(metadata: &Metadata) -> XtaskResult<GitPins> {
    let pins = metadata
        .workspace_metadata
        .get("ordo")
        .and_then(|ordo| ordo.get("git-pins"))
        .ok_or("workspace.metadata.ordo.git-pins is missing")?;
    Ok(serde_json::from_value(pins.clone())?)
}

fn metadata(root: &Path) -> XtaskResult<Metadata> {
    Ok(MetadataCommand::new()
        .current_dir(root)
        .no_deps()
        .other_options(vec!["--locked".to_owned()])
        .exec()?)
}

fn workspace_packages(metadata: &Metadata) -> Vec<&Package> {
    metadata.workspace_packages()
}

fn internal_dependencies<'a>(package: &'a Package, metadata: &'a Metadata) -> Vec<&'a str> {
    let workspace_names: BTreeSet<_> = workspace_packages(metadata)
        .into_iter()
        .map(|package| package.name.as_str())
        .collect();
    package
        .dependencies
        .iter()
        .filter(|dependency| dependency.kind == DependencyKind::Normal)
        .filter_map(|dependency| {
            let name = dependency.name.as_str();
            workspace_names.contains(name).then_some(name)
        })
        .collect()
}

fn active_milestone(root: &Path) -> XtaskResult<String> {
    let milestones = fs::read_to_string(root.join("MILESTONES.md"))?;
    Ok(find_active_milestone(&milestones).to_owned())
}

fn find_active_milestone(milestones: &str) -> &str {
    milestones
        .split("\n### ")
        .skip(1)
        .find_map(|section| {
            let title = section.lines().next()?;
            (title.starts_with('T') && section.lines().any(|line| line.starts_with("- [ ]")))
                .then_some(title)
        })
        .unwrap_or("No incomplete milestone found")
}

fn run(root: &Path, program: &str, args: &[&str]) -> XtaskResult<()> {
    let status = Command::new(program)
        .args(args)
        .current_dir(root)
        .stdin(Stdio::inherit())
        .stdout(Stdio::inherit())
        .stderr(Stdio::inherit())
        .status()?;
    if !status.success() {
        return Err(format!("command failed: {program} {}", args.join(" ")).into());
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn rejects_noncanonical_slide_tokens() {
        let invalid = r##"{
            "schema_version": 1,
            "canonical_slide": {"width": 1366, "height": 768, "aspect_ratio": "16:9"},
            "appearance": {
                "dark_background": "#11100F",
                "light_background": "#F7F3EB",
                "focus_ring": "#2F6FED"
            }
        }"##;
        assert!(validate_design_tokens(invalid).is_err());
    }

    #[test]
    fn rejects_runtime_network_source() {
        assert!(validate_offline_source("reqwest::get(\"example\")").is_err());
        assert!(validate_offline_source("//! Ref: https://example.invalid/spec").is_ok());
    }

    #[test]
    fn rejects_floating_git_lock_source() {
        let lock = "source = \"git+https://example.invalid/repository\"";
        let pins = GitPins {
            zed: "0".repeat(40),
            gpui_base: "1".repeat(40),
        };
        assert!(validate_git_lock(lock, &pins).is_err());
    }

    #[test]
    fn rejects_inverted_domain_dependency() {
        let edges = BTreeMap::from([("domain", BTreeSet::from(["ui"])), ("ui", BTreeSet::new())]);
        assert!(validate_dependency_edges(&edges).is_err());
    }

    #[test]
    fn rejects_domain_dependencies() {
        assert!(validate_domain_dependencies(&["gpui"]).is_err());
    }

    #[test]
    fn finds_first_incomplete_milestone() {
        let milestones = "\n### T0.1 — Done\n- [x] check\n### T0.2 — Next\n- [ ] check\n";
        assert_eq!(find_active_milestone(milestones), "T0.2 — Next");
    }
}
