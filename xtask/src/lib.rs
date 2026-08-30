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

const FORBIDDEN_DOMAIN_DEPENDENCIES: &[&str] = &[
    "gpui",
    "gpui_platform",
    "gpui-base",
    "rusqlite",
    "libsqlite3-sys",
    "sqlx",
    "tokio",
    "async-std",
    "windows",
    "winapi",
    "x11rb",
    "wayland-client",
];

const QUALIFIED_UI_DEPENDENCIES: &[&str] = &["gpui", "gpui-base"];

const LUCIDE_ICON_DIR: &str = "crates/ui/assets/icons/lucide";

const LUCIDE_ICONS: &[&str] = &[
    "activity.svg",
    "calendar-days.svg",
    "check.svg",
    "chevron-down.svg",
    "chevron-right.svg",
    "circle-alert.svg",
    "circle-question-mark.svg",
    "database-backup.svg",
    "ellipsis.svg",
    "grip-vertical.svg",
    "info.svg",
    "library.svg",
    "settings.svg",
    "swatch-book.svg",
    "triangle-alert.svg",
    "x.svg",
];

/// Bundled assets that are not icons; the Lucide subset is appended from
/// `LUCIDE_ICONS` so the two lists cannot drift apart.
const REQUIRED_UI_ASSETS: &[&str] = &[
    "crates/ui/assets/fonts/atkinson-hyperlegible-next/AtkinsonHyperlegibleNext[wght].ttf",
    "crates/ui/assets/fonts/noto-sans/NotoSans[wdth,wght].ttf",
    "crates/ui/assets/licenses/Atkinson-Hyperlegible-Next-OFL-1.1.txt",
    "crates/ui/assets/licenses/Lucide-ISC-MIT.txt",
    "crates/ui/assets/licenses/Noto-Sans-OFL-1.1.txt",
];

fn required_ui_assets() -> impl Iterator<Item = String> {
    REQUIRED_UI_ASSETS
        .iter()
        .map(|asset| (*asset).to_owned())
        .chain(
            LUCIDE_ICONS
                .iter()
                .map(|icon| format!("{LUCIDE_ICON_DIR}/{icon}")),
        )
}

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
    icon_family: String,
    bundled_resources: Vec<String>,
}

pub fn repo_root() -> PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR"))
        .parent()
        .expect("xtask must be inside the workspace")
        .to_path_buf()
}

pub fn verify() -> XtaskResult<()> {
    let root = repo_root();
    let metadata = metadata(&root)?;
    println!("[1/9] workspace contracts");
    verify_architecture_metadata(&metadata)?;
    println!("[2/9] design tokens");
    verify_design_tokens(&root)?;
    println!("[3/9] offline runtime contract");
    verify_offline_contract(&root)?;
    println!("[4/9] git pins");
    verify_git_pins_metadata(&root, &metadata)?;
    println!("[5/9] rustfmt");
    run(&root, "cargo", &["fmt", "--all", "--check"])?;
    println!("[6/9] clippy");
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
    println!("[7/9] shipping configuration");
    run(
        &root,
        "cargo",
        &["check", "-p", "ordo", "--release", "--locked"],
    )?;
    println!("[8/9] tests");
    run(&root, "cargo", &["test", "--workspace", "--locked"])?;
    println!("[9/9] dependencies, advisories, sources, and licenses");
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
    verify_architecture_metadata(&metadata)
}

fn verify_architecture_metadata(metadata: &Metadata) -> XtaskResult<()> {
    for name in ["domain", "presentation"] {
        let package = workspace_packages(metadata)
            .into_iter()
            .find(|package| package.name == name)
            .ok_or_else(|| format!("{name} package is missing"))?;
        validate_domain_dependencies(
            &package
                .dependencies
                .iter()
                .map(|dependency| dependency.name.as_str())
                .collect::<Vec<_>>(),
        )?;
    }
    let actual_names: BTreeSet<_> = workspace_packages(metadata)
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
        "native-runtime-tests",
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

    let edges: BTreeMap<_, _> = workspace_packages(metadata)
        .into_iter()
        .map(|package| {
            (
                package.name.as_str(),
                internal_dependencies(package, metadata)
                    .into_iter()
                    .collect::<BTreeSet<_>>(),
            )
        })
        .collect();
    validate_dependency_edges(&edges)?;

    let direct_dependencies = |include_development| {
        workspace_packages(metadata)
            .into_iter()
            .map(|package| {
                (
                    package.name.as_str(),
                    package
                        .dependencies
                        .iter()
                        .filter(|dependency| {
                            dependency.kind == DependencyKind::Normal
                                || (include_development
                                    && dependency.kind == DependencyKind::Development)
                        })
                        .map(|dependency| dependency.name.as_str())
                        .collect::<BTreeSet<_>>(),
                )
            })
            .collect::<BTreeMap<_, _>>()
    };
    validate_ui_dependency_policy(&direct_dependencies(false), &direct_dependencies(true))
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
        ("native-runtime-tests", BTreeSet::new()),
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
    let forbidden: Vec<_> = dependencies
        .iter()
        .filter(|dependency| FORBIDDEN_DOMAIN_DEPENDENCIES.contains(dependency))
        .collect();
    if forbidden.is_empty() {
        Ok(())
    } else {
        Err(format!("forbidden domain dependencies: {forbidden:?}").into())
    }
}

fn validate_ui_dependency_policy(
    normal_dependencies: &BTreeMap<&str, BTreeSet<&str>>,
    normal_and_dev_dependencies: &BTreeMap<&str, BTreeSet<&str>>,
) -> XtaskResult<()> {
    let ui_dependencies = normal_dependencies
        .get("ui")
        .ok_or("ui package is missing")?;
    let qualified: BTreeSet<_> = QUALIFIED_UI_DEPENDENCIES.iter().copied().collect();
    let unqualified: Vec<_> = ui_dependencies.difference(&qualified).collect();
    if !unqualified.is_empty() {
        return Err(format!("unqualified ui dependency: {unqualified:?}").into());
    }

    for (package, package_dependencies) in normal_and_dev_dependencies {
        if *package != "ui" && package_dependencies.contains("gpui-base") {
            return Err(format!(
                "gpui-base must remain private to ui, but {package} depends on it"
            )
            .into());
        }
    }
    Ok(())
}

/// Design-token validation is a named gate of its own, as T0.1 requires, but the
/// check itself lives beside the constants it guards: `ui::tokens` pins every
/// value to `DESIGN.md`'s frontmatter, which is something only Rust can read.
pub fn verify_design_tokens(root: &Path) -> XtaskResult<()> {
    run(
        root,
        "cargo",
        &["test", "-p", "ui", "--locked", "--lib", "tokens::"],
    )
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
    if manifest.icon_family != "Lucide" {
        return Err("Lucide is the only allowed operator icon family".into());
    }
    // Exact equality, not containment: the manifest is the committed declaration
    // of the bundled inventory, so an asset added to one side and not the other
    // is drift either way.
    let registered: BTreeSet<String> = manifest.bundled_resources.iter().cloned().collect();
    let required: BTreeSet<String> = required_ui_assets().collect();
    if registered != required {
        let unregistered: Vec<_> = required.difference(&registered).collect();
        let unexpected: Vec<_> = registered.difference(&required).collect();
        return Err(format!(
            "runtime manifest does not match the required UI assets: \
             missing {unregistered:?}, unexpected {unexpected:?}"
        )
        .into());
    }
    for resource in &manifest.bundled_resources {
        if !root.join(resource).is_file() {
            return Err(format!("bundled resource is missing: {resource}").into());
        }
    }
    validate_lucide_assets(&root.join("crates/ui/assets/icons"))?;
    Ok(())
}

fn validate_lucide_assets(icons_root: &Path) -> XtaskResult<()> {
    let families: BTreeSet<_> = fs::read_dir(icons_root)?
        .map(|entry| entry.map(|entry| entry.file_name()))
        .collect::<Result<_, _>>()?;
    if families != BTreeSet::from(["lucide".into()]) {
        return Err("crates/ui/assets/icons must contain only the Lucide family".into());
    }

    let lucide_root = icons_root.join("lucide");
    let actual: BTreeSet<_> = fs::read_dir(&lucide_root)?
        .map(|entry| entry.map(|entry| entry.file_name()))
        .collect::<Result<_, _>>()?;
    let expected: BTreeSet<_> = LUCIDE_ICONS
        .iter()
        .map(OsStr::new)
        .map(OsStr::to_owned)
        .collect();
    if actual != expected {
        return Err(format!(
            "bundled Lucide subset differs: expected {expected:?}, got {actual:?}"
        )
        .into());
    }

    for icon in LUCIDE_ICONS {
        let svg = fs::read_to_string(lucide_root.join(icon))?;
        if !svg.contains("viewBox=\"0 0 24 24\"")
            || !svg.contains("stroke=\"currentColor\"")
            || !svg.contains("stroke-width=\"2\"")
        {
            return Err(format!("Lucide icon has unexpected visual vocabulary: {icon}").into());
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
    let forbidden = [
        "reqwest::",
        "ureq::",
        "hyper::",
        "std::net::",
        "tokio::net::",
        "TcpStream",
        "TcpListener",
        "UdpSocket",
    ];
    if let Some(token) = forbidden.iter().find(|token| source.contains(**token)) {
        return Err(format!("runtime network token is forbidden: {token}").into());
    }
    Ok(())
}

pub fn verify_git_pins(root: &Path) -> XtaskResult<()> {
    let metadata = metadata(root)?;
    verify_git_pins_metadata(root, &metadata)
}

fn verify_git_pins_metadata(root: &Path, metadata: &Metadata) -> XtaskResult<()> {
    let pins = git_pins(metadata)?;
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
    fn rejects_runtime_network_source() {
        assert!(validate_offline_source("reqwest::get(\"example\")").is_err());
        assert!(validate_offline_source("std::net::TcpListener::bind(\"0.0.0.0:0\")").is_err());
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
        let mismatched_zed = format!(
            "source = \"git+https://github.com/zed-industries/zed#{}\"",
            "a".repeat(40)
        );
        assert!(validate_git_lock(&mismatched_zed, &pins).is_err());
    }

    #[test]
    fn rejects_inverted_domain_dependency() {
        let edges = BTreeMap::from([("domain", BTreeSet::from(["ui"])), ("ui", BTreeSet::new())]);
        assert!(validate_dependency_edges(&edges).is_err());
    }

    #[test]
    fn rejects_forbidden_domain_dependencies() {
        assert!(validate_domain_dependencies(&["gpui"]).is_err());
        assert!(validate_domain_dependencies(&["uuid", "serde"]).is_ok());
    }

    #[test]
    fn keeps_gpui_base_private_to_ui() {
        let valid = BTreeMap::from([
            ("ui", BTreeSet::from(["gpui", "gpui-base"])),
            ("ordo", BTreeSet::from(["gpui", "ui"])),
        ]);

        let with_test_helper = BTreeMap::from([
            ("ui", BTreeSet::from(["gpui", "gpui-base", "insta"])),
            ("ordo", BTreeSet::from(["gpui", "ui"])),
        ]);
        assert!(validate_ui_dependency_policy(&valid, &with_test_helper).is_ok());

        let removed = BTreeMap::from([
            ("ui", BTreeSet::from(["gpui"])),
            ("ordo", BTreeSet::from(["gpui", "ui"])),
        ]);
        assert!(validate_ui_dependency_policy(&removed, &removed).is_ok());

        let leaked_through_dev = BTreeMap::from([
            ("ui", BTreeSet::from(["gpui", "gpui-base"])),
            ("ordo", BTreeSet::from(["gpui-base", "ui"])),
        ]);
        assert!(validate_ui_dependency_policy(&valid, &leaked_through_dev).is_err());
    }

    #[test]
    fn rejects_unqualified_ui_dependencies() {
        let dependencies = BTreeMap::from([(
            "ui",
            BTreeSet::from(["gpui", "gpui-base", "other-ui-behavior"]),
        )]);
        assert!(validate_ui_dependency_policy(&dependencies, &dependencies).is_err());
    }

    #[test]
    fn finds_first_incomplete_milestone() {
        let milestones = "\n### T0.1 — Done\n- [x] check\n### T0.2 — Next\n- [ ] check\n";
        assert_eq!(find_active_milestone(milestones), "T0.2 — Next");
    }
}
