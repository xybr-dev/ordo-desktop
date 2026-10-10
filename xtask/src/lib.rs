pub mod inspect_pptx;
pub mod measure_presenter;

use cargo_metadata::{DependencyKind, Metadata, MetadataCommand, Package, semver::VersionReq};
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

/// Non-GPUI crates `domain` and `presentation` may not use. Every GPUI crate is
/// forbidden there too, by `is_gpui_crate` rather than by listing, so a renamed
/// or newly published GPUI family cannot slip past an outdated list.
const FAULT_INJECTION_FEATURE: &str = "fault-injection";
const FORBIDDEN_DOMAIN_DEPENDENCIES: &[&str] = &[
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

/// Package names, not the `gpui`/`gpui_platform` names the manifests rename
/// them to: Cargo metadata reports a dependency by the package it resolves.
const QUALIFIED_UI_DEPENDENCIES: &[&str] = &[GPUI_PACKAGE, GPUI_BASE_PACKAGE];

/// Canonical shaping and raster stack, qualified in
/// `docs/qualification/RENDERING-T0.4.md`. External crates only; internal
/// edges such as `rendering -> domain` stay owned by the crate-graph contract.
const QUALIFIED_RENDERING_DEPENDENCIES: &[&str] =
    &["rustybuzz", "swash", "tiny-skia", "ttf-parser"];

/// The one GPUI crate family Ordo builds against: Longbridge's `gpui-pre`
/// crates.io snapshot of Zed (`docs/qualification/GPUI-BASE-T0.4a.md`).
const GPUI_PACKAGE: &str = "gpui-pre";
const GPUI_FAMILY_PREFIX: &str = "gpui-pre-";
const GPUI_BASE_PACKAGE: &str = "gpui-base";
const CRATES_IO: &str = "registry+https://github.com/rust-lang/crates.io-index";

/// Every GPUI-family name, current or foreign, so isolation policies hold
/// whichever family a dependency would come from.
fn is_gpui_crate(name: &str) -> bool {
    name == "gpui" || name.starts_with("gpui-") || name.starts_with("gpui_")
}

/// Whether `name` is the pinned `gpui-pre` snapshot or one of its crates.
fn is_gpui_snapshot_package(name: &str) -> bool {
    name == GPUI_PACKAGE || name.starts_with(GPUI_FAMILY_PREFIX)
}

const LUCIDE_ICON_DIR: &str = "crates/ui/assets/icons/lucide";

const LUCIDE_ICONS: &[&str] = &[
    "activity.svg",
    "calendar-days.svg",
    "check.svg",
    "chevron-down.svg",
    "chevron-left.svg",
    "chevron-right.svg",
    "circle-alert.svg",
    "circle-question-mark.svg",
    "database-backup.svg",
    "ellipsis.svg",
    "grip-vertical.svg",
    "info.svg",
    "library.svg",
    "plus.svg",
    "settings.svg",
    "swatch-book.svg",
    "triangle-alert.svg",
    "x.svg",
];

/// Bundled assets that are not icons; the Lucide subset is appended from
/// `LUCIDE_ICONS` so the two lists cannot drift apart.
const REQUIRED_UI_ASSETS: &[&str] = &[
    "assets/fonts/atkinson-hyperlegible-next/AtkinsonHyperlegibleNext[wght].ttf",
    "assets/fonts/noto-sans/NotoSans[wdth,wght].ttf",
    "assets/licenses/Atkinson-Hyperlegible-Next-OFL-1.1.txt",
    "crates/ui/assets/licenses/Lucide-ISC-MIT.txt",
    "assets/licenses/Noto-Sans-OFL-1.1.txt",
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

/// `workspace.metadata.ordo.gpui-supply`: the exact snapshot and release the
/// lock must resolve. `zed` records the commit the snapshot reproduces; it is
/// informational, since `gpui-pre`'s own metadata carries the authoritative
/// `zed-rev` and the locked checksum already fixes the snapshot's bytes.
#[derive(Debug, Deserialize)]
struct GpuiSupply {
    #[serde(rename = "gpui-pre")]
    gpui_pre: String,
    zed: String,
    #[serde(rename = "gpui-base")]
    gpui_base: String,
}

impl GpuiSupply {
    /// The version `name` must resolve to: `Some` for the snapshot's crates and
    /// `gpui-base`, `None` for any crate outside the GPUI supply.
    fn pinned_version(&self, name: &str) -> Option<&str> {
        if is_gpui_snapshot_package(name) {
            Some(&self.gpui_pre)
        } else if name == GPUI_BASE_PACKAGE {
            Some(&self.gpui_base)
        } else {
            None
        }
    }
}

#[derive(Debug, Deserialize)]
struct RuntimeManifest {
    schema_version: u32,
    network_required: bool,
    icon_family: String,
    bundled_resources: Vec<String>,
    font_hashes: BTreeMap<String, String>,
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
    println!("[1/10] workspace contracts");
    verify_architecture_metadata(&metadata)?;
    println!("[2/10] design tokens");
    verify_design_tokens(&root)?;
    println!("[3/10] decision records");
    verify_decision_records(&root)?;
    println!("[4/10] offline runtime contract");
    verify_offline_contract(&root)?;
    println!("[5/10] GPUI supply pins");
    verify_gpui_supply_metadata(&root, &metadata)?;
    println!("[6/10] rustfmt");
    run(&root, "cargo", &["fmt", "--all", "--check"])?;
    println!("[7/10] clippy");
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
    println!("[8/10] shipping configuration");
    run(
        &root,
        "cargo",
        &["check", "-p", "ordo", "--release", "--locked"],
    )?;
    println!("[9/10] tests");
    run(&root, "cargo", &["test", "--workspace", "--locked"])?;
    // The canonical-slide lab lives in an example, which the workspace suite
    // builds but never runs; drive its headless key test explicitly so the
    // review tool's shortcuts cannot rot silently.
    run(
        &root,
        "cargo",
        &[
            "test",
            "-p",
            "ordo",
            "--example",
            "canonical-slide-lab",
            "--locked",
        ],
    )?;
    println!("[10/10] dependencies, advisories, sources, and licenses");
    run(&root, "cargo", &["deny", "check"])?;
    println!("Ordo verification passed.");
    Ok(())
}

pub fn context() -> XtaskResult<()> {
    let root = repo_root();
    let metadata = metadata(&root)?;
    let supply = gpui_supply(&metadata)?;
    println!("Ordo workspace: {}", root.display());
    println!(
        "GPUI pin: gpui-pre {} (Zed {})",
        supply.gpui_pre, supply.zed
    );
    println!("gpui-base pin: {}", supply.gpui_base);
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
        "artifact-container-spike",
        "powerpoint-spike",
        "artifact-container-tests",
        "powerpoint-tests",
        "native-runtime-tests",
        "display-tests",
        "presentation-tests",
        "storage-tests",
        "performance-tests",
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
    validate_ui_dependency_policy(&direct_dependencies(false), &direct_dependencies(true))?;
    let workspace_names: BTreeSet<_> = workspace_packages(metadata)
        .into_iter()
        .map(|package| package.name.as_str())
        .collect();
    validate_rendering_dependency_policy(
        &direct_dependencies(false),
        &direct_dependencies(true),
        &workspace_names,
    )?;
    validate_sqlite_ownership(&direct_dependencies(false))?;
    validate_spike_reuse_policy(metadata)?;
    validate_fault_injection_policy(metadata)
}

/// Name suffix that marks a disposable prototype package (T0.10 AC5).
const SPIKE_SUFFIX: &str = "-spike";

/// No package outside `tests/` may depend on a `*-spike` package, at any
/// dependency kind. Test packages may reach a spike only as a dev-dependency.
fn validate_spike_reuse_policy(metadata: &Metadata) -> XtaskResult<()> {
    let owned: Vec<(String, String, String, DependencyKind)> = workspace_packages(metadata)
        .into_iter()
        .flat_map(|package| {
            let directory = package
                .manifest_path
                .parent()
                .and_then(|dir| dir.strip_prefix(&metadata.workspace_root).ok())
                .map_or_else(String::new, |dir| dir.as_str().to_owned());
            package
                .dependencies
                .iter()
                .filter(|dependency| dependency.name.ends_with(SPIKE_SUFFIX))
                .map(move |dependency| {
                    (
                        package.name.to_string(),
                        directory.clone(),
                        dependency.name.clone(),
                        dependency.kind,
                    )
                })
                .collect::<Vec<_>>()
        })
        .collect();
    let edges: Vec<_> = owned
        .iter()
        .map(|(package, directory, dependency, kind)| {
            (
                package.as_str(),
                directory.as_str(),
                dependency.as_str(),
                *kind,
            )
        })
        .collect();
    validate_spike_reuse(&edges)
}

/// Pure form of the spike reuse rule. Each edge is
/// `(package, manifest_dir_relative_to_workspace, dependency_name, kind)`.
fn validate_spike_reuse(edges: &[(&str, &str, &str, DependencyKind)]) -> XtaskResult<()> {
    let offending: Vec<String> = edges
        .iter()
        .filter(|(_, _, dependency, _)| dependency.ends_with(SPIKE_SUFFIX))
        .filter_map(|(package, directory, dependency, kind)| {
            let normalized = directory.replace('\\', "/");
            let is_test_package = normalized.starts_with("tests/");
            let allowed = is_test_package && *kind == DependencyKind::Development;
            (!allowed).then(|| {
                let kind = match kind {
                    DependencyKind::Normal => "normal",
                    DependencyKind::Development => "dev",
                    DependencyKind::Build => "build",
                    _ => "unknown",
                };
                format!(
                    "{package} must not depend on spike package {dependency} ({kind}); \
                     spikes are disposable prototypes (T0.10 AC5)"
                )
            })
        })
        .collect();
    if offending.is_empty() {
        Ok(())
    } else {
        Err(offending.join("\n").into())
    }
}

/// SQLite driver crates that only the Storage Module may link (T0.7 AC1).
const SQLITE_DRIVERS: &[&str] = &["rusqlite", "libsqlite3-sys"];

/// Only `storage` may name a SQLite driver as a normal dependency, so no other
/// crate can open its own connection. Test crates may use one as a
/// dev-dependency to plant fixtures.
fn validate_sqlite_ownership(
    normal_dependencies: &BTreeMap<&str, BTreeSet<&str>>,
) -> XtaskResult<()> {
    let offending: Vec<_> = normal_dependencies
        .iter()
        .filter(|(package, _)| **package != "storage")
        .flat_map(|(package, dependencies)| {
            dependencies
                .iter()
                .filter(|dependency| SQLITE_DRIVERS.contains(dependency))
                .map(move |dependency| format!("{package} -> {dependency}"))
        })
        .collect();
    if offending.is_empty() {
        Ok(())
    } else {
        Err(format!("only storage may depend on a SQLite driver: {offending:?}").into())
    }
}

/// Fault hooks are a test-only seam. No normal (non-dev) dependency edge between
/// workspace packages may enable `storage`'s `fault-injection` feature.
fn validate_fault_injection_policy(metadata: &Metadata) -> XtaskResult<()> {
    let workspace_names: BTreeSet<_> = workspace_packages(metadata)
        .into_iter()
        .map(|package| package.name.as_str())
        .collect();
    let edges: Vec<_> = workspace_packages(metadata)
        .into_iter()
        .flat_map(|package| {
            package.dependencies.iter().map(move |dependency| {
                (
                    package.name.as_str(),
                    dependency.name.as_str(),
                    dependency.kind,
                    dependency.features.as_slice(),
                )
            })
        })
        .collect();
    validate_fault_injection_edges(&edges, &workspace_names)
}

fn validate_fault_injection_edges(
    edges: &[(&str, &str, DependencyKind, &[String])],
    workspace_names: &BTreeSet<&str>,
) -> XtaskResult<()> {
    let offending: Vec<_> = edges
        .iter()
        .filter(|(_, dependency, kind, features)| {
            *kind == DependencyKind::Normal
                && workspace_names.contains(dependency)
                && features
                    .iter()
                    .any(|feature| feature == FAULT_INJECTION_FEATURE)
        })
        .map(|(package, dependency, _, _)| format!("{package} -> {dependency}"))
        .collect();
    if offending.is_empty() {
        Ok(())
    } else {
        Err(format!("fault-injection enabled in normal dependencies: {offending:?}").into())
    }
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
        ("artifact-container-spike", BTreeSet::new()),
        ("powerpoint-spike", BTreeSet::new()),
        ("artifact-container-tests", BTreeSet::new()),
        ("powerpoint-tests", BTreeSet::new()),
        ("native-runtime-tests", BTreeSet::new()),
        ("display-tests", BTreeSet::new()),
        ("presentation-tests", BTreeSet::new()),
        ("storage-tests", BTreeSet::new()),
        ("performance-tests", BTreeSet::new()),
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
        .filter(|dependency| {
            is_gpui_crate(dependency) || FORBIDDEN_DOMAIN_DEPENDENCIES.contains(dependency)
        })
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
        if *package != "ui" && package_dependencies.contains(GPUI_BASE_PACKAGE) {
            return Err(format!(
                "gpui-base must remain private to ui, but {package} depends on it"
            )
            .into());
        }
    }
    Ok(())
}

fn validate_rendering_dependency_policy(
    normal_dependencies: &BTreeMap<&str, BTreeSet<&str>>,
    normal_and_dev_dependencies: &BTreeMap<&str, BTreeSet<&str>>,
    workspace_names: &BTreeSet<&str>,
) -> XtaskResult<()> {
    let rendering_dependencies = normal_dependencies
        .get("rendering")
        .ok_or("rendering package is missing")?;
    let qualified: BTreeSet<_> = QUALIFIED_RENDERING_DEPENDENCIES.iter().copied().collect();
    let unqualified: Vec<_> = rendering_dependencies
        .difference(&qualified)
        .filter(|dependency| !workspace_names.contains(*dependency))
        .collect();
    if !unqualified.is_empty() {
        return Err(format!("unqualified rendering dependency: {unqualified:?}").into());
    }

    let rendering_all = normal_and_dev_dependencies
        .get("rendering")
        .ok_or("rendering package is missing")?;
    let leaked: Vec<_> = rendering_all
        .iter()
        .filter(|dependency| is_gpui_crate(dependency))
        .collect();
    if !leaked.is_empty() {
        return Err(format!("no GPUI crate may reach rendering, found: {leaked:?}").into());
    }
    Ok(())
}

/// Design-token validation is a named gate of its own, as T0.1 requires, but the
/// check itself lives beside the constants it guards: `ui::tokens` pins every
/// value to `DESIGN.md`'s frontmatter, which is something only Rust can read.
///
/// Deliberately unfiltered. A name filter that matches nothing exits zero, so
/// `--lib tokens::` would keep printing this gate while asserting nothing the
/// moment that module is renamed or moved. The cost is that this runs the whole
/// `ui` lib suite, so the gate's label is narrower than what it executes; read
/// it as the step that must not pass without the token checks, not as a promise
/// that only token checks run here.
pub fn verify_design_tokens(root: &Path) -> XtaskResult<()> {
    run(root, "cargo", &["test", "-p", "ui", "--locked", "--lib"])
}

/// `docs/decisions/adr.py check`: every record's frontmatter, both directions of
/// every supersede and amend link, and the generated index. The checker is the
/// one shared with Ordo's sibling repositories, so it stays Python rather than
/// being ported here; this only finds an interpreter to run it.
pub fn verify_decision_records(root: &Path) -> XtaskResult<()> {
    let python = ["python", "python3", "py"]
        .into_iter()
        .find(|program| {
            Command::new(program)
                .arg("--version")
                .stdout(Stdio::null())
                .stderr(Stdio::null())
                .status()
                .is_ok_and(|status| status.success())
        })
        .ok_or("decision records need Python 3 on the path (python, python3, or py)")?;
    run(root, python, &["docs/decisions/adr.py", "check"])
}

pub fn verify_offline_contract(root: &Path) -> XtaskResult<()> {
    let contents = fs::read_to_string(root.join("resources/runtime-manifest.json"))?;
    let manifest: RuntimeManifest = serde_json::from_str(&contents)?;
    validate_runtime_manifest(&manifest, root)?;
    for relative in ["crates", "tools/ui-lab"] {
        scan_runtime_source(&root.join(relative))?;
    }
    for relative in ["crates", "tools", "xtask"] {
        scan_unsafe_opt_outs(root, &root.join(relative))?;
    }
    Ok(())
}

/// The one source file allowed to opt out of the workspace `unsafe_code`
/// lint (Decision 269). The lint is `deny` rather than `forbid` only so this
/// file can; any other opt-out fails verification.
const UNSAFE_EXCEPTION: &str = "crates/platform/src/process_memory.rs";

fn scan_unsafe_opt_outs(root: &Path, path: &Path) -> XtaskResult<()> {
    for entry in fs::read_dir(path)? {
        let path = entry?.path();
        if path.is_dir() {
            if path.file_name() != Some(OsStr::new("target")) {
                scan_unsafe_opt_outs(root, &path)?;
            }
        } else if path.extension() == Some(OsStr::new("rs")) {
            let relative = path
                .strip_prefix(root)?
                .to_string_lossy()
                .replace('\\', "/");
            validate_unsafe_opt_out(&relative, &fs::read_to_string(&path)?)?;
        }
    }
    Ok(())
}

fn validate_unsafe_opt_out(relative: &str, source: &str) -> XtaskResult<()> {
    // The lint name is split so this file does not match its own scan.
    let lint = concat!("(unsafe", "_code)");
    let opts_out = ["allow", "expect", "warn"]
        .iter()
        .any(|level| source.contains(&format!("{level}{lint}")));
    if opts_out && relative != UNSAFE_EXCEPTION {
        return Err(format!(
            "{relative}: only {UNSAFE_EXCEPTION} may opt out of unsafe_code (Decision 269)"
        )
        .into());
    }
    Ok(())
}

/// The manifest is kept because T0.1 requires runtime resources to be declared
/// in a committed, schema-validated offline manifest, and because it states the
/// offline and icon-family contracts as data rather than as the absence of code.
/// Nothing loads assets through these paths — they are `include_bytes!`/
/// `include_str!` into the binary — so when T6.x generates its own manifest from
/// the packaged inventory, ask again whether this committed one still earns its
/// place rather than letting it survive on inertia.
fn validate_runtime_manifest(manifest: &RuntimeManifest, root: &Path) -> XtaskResult<()> {
    if manifest.schema_version != 2 {
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
    validate_font_hashes(&manifest.font_hashes, &manifest.bundled_resources, root)?;
    validate_lucide_assets(&root.join("crates/ui/assets/icons"))?;
    Ok(())
}

/// A corrupted or replaced font file fails verification: every bundled font
/// carries a pinned SHA-256 in the manifest, and every pinned hash names a
/// bundled resource whose bytes must match.
fn validate_font_hashes(
    hashes: &BTreeMap<String, String>,
    resources: &[String],
    root: &Path,
) -> XtaskResult<()> {
    for resource in resources
        .iter()
        .filter(|resource| resource.ends_with(".ttf"))
    {
        if !hashes.contains_key(resource) {
            return Err(format!("bundled font lacks a pinned hash: {resource}").into());
        }
    }
    for (path, expected) in hashes {
        if !resources.iter().any(|resource| resource == path) {
            return Err(format!("pinned font hash names an unregistered resource: {path}").into());
        }
        let bytes = fs::read(root.join(path))?;
        validate_font_digest(path, expected, &bytes)?;
    }
    Ok(())
}

fn validate_font_digest(path: &str, expected: &str, bytes: &[u8]) -> XtaskResult<()> {
    use sha2::Digest as _;
    let actual = format!("{:x}", sha2::Sha256::digest(bytes));
    if actual != expected {
        return Err(format!("bundled font hash mismatch: {path}").into());
    }
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

pub fn verify_gpui_supply(root: &Path) -> XtaskResult<()> {
    let metadata = metadata(root)?;
    verify_gpui_supply_metadata(root, &metadata)
}

fn verify_gpui_supply_metadata(root: &Path, metadata: &Metadata) -> XtaskResult<()> {
    let supply = gpui_supply(metadata)?;
    for package in workspace_packages(metadata) {
        for dependency in &package.dependencies {
            validate_gpui_requirement(
                &package.name,
                &dependency.name,
                &dependency.req,
                dependency
                    .source
                    .as_ref()
                    .map(|source| source.repr.as_str()),
                &supply,
            )?;
        }
    }
    let lock = fs::read_to_string(root.join("Cargo.lock"))?;
    validate_gpui_lock(&lock, &supply)
}

/// A workspace requirement on the GPUI snapshot or `gpui-base` must name one
/// exact crates.io version: a caret range lets Cargo resolve a second snapshot,
/// and a git or path source escapes the recorded supply. Any other GPUI-family
/// crate is a second type universe and is rejected outright.
fn validate_gpui_requirement(
    package: &str,
    dependency: &str,
    requirement: &VersionReq,
    source: Option<&str>,
    supply: &GpuiSupply,
) -> XtaskResult<()> {
    let Some(expected) = supply.pinned_version(dependency) else {
        if is_gpui_crate(dependency) {
            return Err(
                format!("{package} depends on {dependency}, outside the GPUI supply").into(),
            );
        }
        return Ok(());
    };
    if source != Some(CRATES_IO) {
        return Err(format!(
            "{package} must take {dependency} from crates.io, not {}",
            source.unwrap_or("a path")
        )
        .into());
    }
    // `VersionReq` renders one exact `=x.y.z` comparator as exactly this
    // string; a caret, partial, range, or pre-release requirement cannot.
    if requirement.to_string() != format!("={expected}") {
        return Err(format!(
            "{package} requires {dependency} {requirement}, expected exactly ={expected}"
        )
        .into());
    }
    Ok(())
}

/// One `[[package]]` entry of `Cargo.lock`.
#[derive(Debug, Default)]
struct LockPackage {
    name: String,
    version: String,
    source: Option<String>,
    dependencies: Vec<String>,
}

/// Reads the fields this gate needs from `Cargo.lock`'s line-oriented layout.
fn parse_lock_packages(lock: &str) -> Vec<LockPackage> {
    let unquote = |value: &str| value.trim().trim_matches('"').to_owned();
    let mut packages = Vec::new();
    let mut current: Option<LockPackage> = None;
    let mut in_dependencies = false;
    for line in lock.lines() {
        if line.starts_with('[') {
            packages.extend(current.take());
            in_dependencies = false;
            if line == "[[package]]" {
                current = Some(LockPackage::default());
            }
            continue;
        }
        let Some(package) = current.as_mut() else {
            continue;
        };
        if in_dependencies {
            if line.trim() == "]" {
                in_dependencies = false;
            } else {
                package
                    .dependencies
                    .push(unquote(line.trim().trim_end_matches(',')));
            }
        } else if let Some(value) = line.strip_prefix("name = ") {
            package.name = unquote(value);
        } else if let Some(value) = line.strip_prefix("version = ") {
            package.version = unquote(value);
        } else if let Some(value) = line.strip_prefix("source = ") {
            package.source = Some(unquote(value));
        } else if line == "dependencies = [" {
            in_dependencies = true;
        }
    }
    packages.extend(current);
    packages
}

/// The locked graph holds exactly one GPUI crate family, at the recorded
/// `gpui-pre` snapshot, and the one `gpui-base` release built against it, all
/// from crates.io. This is an allow-list: any other GPUI-family name (Zed's
/// `gpui`, gpui-fast, a GPUI Kit layer, an unknown fork) fails. A lock entry,
/// unlike a manifest requirement, also reflects `[patch]` and `[replace]`, so a
/// path or git override of the snapshot fails here. Any non-GPUI git source
/// must name a full immutable revision.
fn validate_gpui_lock(lock: &str, supply: &GpuiSupply) -> XtaskResult<()> {
    let packages = parse_lock_packages(lock);
    for package in &packages {
        if let Some(source) = package
            .source
            .as_deref()
            .and_then(|source| source.strip_prefix("git+"))
        {
            if is_gpui_crate(&package.name)
                || source.contains("github.com/zed-industries/zed")
                || source.contains("github.com/longbridge/")
            {
                return Err(format!(
                    "{} comes from git ({source}); GPUI must come from the gpui-pre snapshot",
                    package.name
                )
                .into());
            }
            let revision = source
                .rsplit_once('#')
                .map(|(_, revision)| revision)
                .unwrap_or_default();
            if revision.len() != 40 || !revision.bytes().all(|byte| byte.is_ascii_hexdigit()) {
                return Err(format!("git source lacks a full immutable revision: {source}").into());
            }
        }
        let Some(expected) = supply.pinned_version(&package.name) else {
            if is_gpui_crate(&package.name) {
                return Err(format!(
                    "Cargo.lock contains {} {}, outside the GPUI supply",
                    package.name, package.version
                )
                .into());
            }
            continue;
        };
        if package.source.as_deref() != Some(CRATES_IO) {
            return Err(format!(
                "Cargo.lock takes {} from {}, not crates.io",
                package.name,
                package.source.as_deref().unwrap_or("a path")
            )
            .into());
        }
        if package.version != expected {
            return Err(format!(
                "Cargo.lock resolves {} {}, expected {expected}",
                package.name, package.version
            )
            .into());
        }
    }
    let count = |name: &str| {
        packages
            .iter()
            .filter(|package| package.name == name)
            .count()
    };
    if count(GPUI_PACKAGE) != 1 || count(GPUI_BASE_PACKAGE) != 1 {
        return Err("Cargo.lock must resolve exactly one gpui-pre and one gpui-base".into());
    }
    let base_uses_snapshot = packages
        .iter()
        .find(|package| package.name == GPUI_BASE_PACKAGE)
        .is_some_and(|base| {
            base.dependencies
                .iter()
                .any(|dependency| dependency == GPUI_PACKAGE)
        });
    if !base_uses_snapshot {
        return Err("gpui-base does not resolve against the pinned gpui-pre snapshot".into());
    }
    Ok(())
}

fn gpui_supply(metadata: &Metadata) -> XtaskResult<GpuiSupply> {
    let supply = metadata
        .workspace_metadata
        .get("ordo")
        .and_then(|ordo| ordo.get("gpui-supply"))
        .ok_or("workspace.metadata.ordo.gpui-supply is missing")?;
    Ok(GpuiSupply::deserialize(supply)?)
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

/// Regenerate `tests/golden-frames/` from `tests/fixtures/rendering/`.
///
/// This is the corpus's only write path: the golden test never updates
/// itself, so ordinary `cargo test` and `cargo xtask verify` fail on drift.
/// Review the resulting `git diff` before committing — a regenerated golden
/// is the point of the gate, and a silent update would prove nothing.
pub fn bless_golden_frames(root: &Path) -> XtaskResult<()> {
    let status = Command::new("cargo")
        .args(["test", "-p", "rendering", "--test", "golden", "--locked"])
        .current_dir(root)
        .env("ORDO_UPDATE_FIXTURES", "1")
        .stdin(Stdio::inherit())
        .stdout(Stdio::inherit())
        .stderr(Stdio::inherit())
        .status()?;
    if !status.success() {
        return Err("command failed: cargo test -p rendering --test golden".into());
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn only_the_audited_module_may_opt_out_of_unsafe_code() {
        let opt_out = concat!("#[allow(unsafe", "_code)]\nmod imp {}");
        let expect = concat!("#![expect(unsafe", "_code)]");
        assert!(validate_unsafe_opt_out(UNSAFE_EXCEPTION, opt_out).is_ok());
        assert!(validate_unsafe_opt_out("crates/ordo/src/lib.rs", opt_out).is_err());
        assert!(validate_unsafe_opt_out("crates/ui/src/lib.rs", expect).is_err());
        assert!(validate_unsafe_opt_out("crates/ui/src/lib.rs", "fn safe() {}").is_ok());
    }

    #[test]
    fn rejects_fault_injection_in_normal_dependencies() {
        let workspace = BTreeSet::from(["storage", "ordo", "storage-tests"]);
        let on = vec![FAULT_INJECTION_FEATURE.to_owned()];
        let off: Vec<String> = Vec::new();

        // A normal edge from ordo to storage that enables the hook is rejected.
        let leaked = [("ordo", "storage", DependencyKind::Normal, on.as_slice())];
        assert!(validate_fault_injection_edges(&leaked, &workspace).is_err());

        // The same feature on a dev edge (the storage-tests harness) is allowed.
        let dev = [(
            "storage-tests",
            "storage",
            DependencyKind::Development,
            on.as_slice(),
        )];
        assert!(validate_fault_injection_edges(&dev, &workspace).is_ok());

        // A normal edge without the feature is allowed.
        let clean = [("ordo", "storage", DependencyKind::Normal, off.as_slice())];
        assert!(validate_fault_injection_edges(&clean, &workspace).is_ok());

        // Non-workspace crates are outside this policy.
        let external = [("ordo", "other", DependencyKind::Normal, on.as_slice())];
        assert!(validate_fault_injection_edges(&external, &workspace).is_ok());
    }

    #[test]
    fn only_storage_may_depend_on_a_sqlite_driver() {
        let owned = BTreeMap::from([
            ("storage", BTreeSet::from(["rusqlite"])),
            ("ordo", BTreeSet::from(["storage"])),
        ]);
        assert!(validate_sqlite_ownership(&owned).is_ok());
        let leaked = BTreeMap::from([
            ("storage", BTreeSet::from(["rusqlite"])),
            ("ordo", BTreeSet::from(["rusqlite"])),
        ]);
        assert!(validate_sqlite_ownership(&leaked).is_err());
    }

    #[test]
    fn spike_reuse_follows_the_t0_10_rule() {
        use DependencyKind::{Build, Development, Normal};

        // The real graph: test packages reach spikes only as dev-dependencies.
        let real = [
            (
                "artifact-container-tests",
                "tests/artifact-container",
                "artifact-container-spike",
                Development,
            ),
            (
                "powerpoint-tests",
                "tests/powerpoint",
                "powerpoint-spike",
                Development,
            ),
            // Non-spike edges are outside this rule.
            ("ordo", "crates/ordo", "storage", Normal),
        ];
        assert!(validate_spike_reuse(&real).is_ok());

        let ordo_normal = [("ordo", "crates/ordo", "powerpoint-spike", Normal)];
        assert!(validate_spike_reuse(&ordo_normal).is_err());

        let ui_lab_dev = [(
            "ui-lab",
            "tools/ui-lab",
            "artifact-container-spike",
            Development,
        )];
        assert!(validate_spike_reuse(&ui_lab_dev).is_err());

        let xtask_build = [("xtask", "xtask", "x-spike", Build)];
        assert!(validate_spike_reuse(&xtask_build).is_err());

        let test_normal = [("foo", "tests/foo", "x-spike", Normal)];
        assert!(validate_spike_reuse(&test_normal).is_err());

        // Windows-style separators are normalized before the test-directory check.
        let windows_dev = [("foo", "tests\\foo", "x-spike", Development)];
        assert!(validate_spike_reuse(&windows_dev).is_ok());
    }

    #[test]
    fn rejects_runtime_network_source() {
        assert!(validate_offline_source("reqwest::get(\"example\")").is_err());
        assert!(validate_offline_source("std::net::TcpListener::bind(\"0.0.0.0:0\")").is_err());
        assert!(validate_offline_source("//! Ref: https://example.invalid/spec").is_ok());
    }

    #[test]
    fn rejects_replaced_font_bytes() {
        let pinned = "ba7816bf8f01cfea414140de5dae2223b00361a396177a9cb410ff61f20015ad";
        assert!(validate_font_digest("font.ttf", pinned, b"abc").is_ok());
        assert!(validate_font_digest("font.ttf", pinned, b"abd").is_err());
    }

    #[test]
    fn rejects_corrupted_or_unpinned_bundled_font() {
        let dir = std::env::temp_dir().join(format!("ordo-xtask-font-{}", std::process::id()));
        std::fs::create_dir_all(&dir).expect("temp dir must be writable");
        std::fs::write(dir.join("font.ttf"), b"abd").expect("temp font must be writable");
        let pinned = "ba7816bf8f01cfea414140de5dae2223b00361a396177a9cb410ff61f20015ad";
        let resources = vec!["font.ttf".to_owned()];

        // Replaced bytes no longer match the pinned hash.
        let hashes = BTreeMap::from([("font.ttf".to_owned(), pinned.to_owned())]);
        assert!(validate_font_hashes(&hashes, &resources, &dir).is_err());

        // A font without any pinned hash is equally rejected.
        assert!(validate_font_hashes(&BTreeMap::new(), &resources, &dir).is_err());

        std::fs::remove_dir_all(&dir).ok();
    }

    fn supply() -> GpuiSupply {
        GpuiSupply {
            gpui_pre: "0.3.8".to_owned(),
            zed: "279fe070bb389b79652e52065b2f001edcc0b11b".to_owned(),
            gpui_base: "0.7.1".to_owned(),
        }
    }

    fn lock_entry(name: &str, version: &str, source: &str, dependencies: &[&str]) -> String {
        let mut entry = format!(
            "[[package]]\nname = \"{name}\"\nversion = \"{version}\"\nsource = \"{source}\"\n"
        );
        if !dependencies.is_empty() {
            entry.push_str("dependencies = [\n");
            for dependency in dependencies {
                entry.push_str(&format!(" \"{dependency}\",\n"));
            }
            entry.push_str("]\n");
        }
        entry + "\n"
    }

    fn pinned_lock() -> String {
        [
            lock_entry(
                "gpui-base",
                "0.7.1",
                CRATES_IO,
                &["gpui-pre", "gpui-pre-macros"],
            ),
            lock_entry("gpui-pre", "0.3.8", CRATES_IO, &["gpui-pre-macros"]),
            lock_entry("gpui-pre-macros", "0.3.8", CRATES_IO, &[]),
            lock_entry("gpui-pre-platform", "0.3.8", CRATES_IO, &["gpui-pre"]),
        ]
        .concat()
    }

    #[test]
    fn accepts_the_pinned_gpui_supply() {
        validate_gpui_lock(&pinned_lock(), &supply()).expect("pinned lock must pass");
    }

    #[test]
    fn rejects_a_git_gpui_source() {
        let revision = "a".repeat(40);
        let patched = pinned_lock()
            + &lock_entry(
                "gpui-pre-util",
                "0.3.8",
                &format!("git+https://github.com/longbridge/gpui-fast?rev={revision}#{revision}"),
                &[],
            );
        assert!(validate_gpui_lock(&patched, &supply()).is_err());
        let zed = pinned_lock()
            + &lock_entry(
                "zlog",
                "0.1.0",
                &format!("git+https://github.com/zed-industries/zed#{revision}"),
                &[],
            );
        assert!(validate_gpui_lock(&zed, &supply()).is_err());
    }

    #[test]
    fn rejects_floating_git_lock_source() {
        let lock = pinned_lock()
            + &lock_entry(
                "other",
                "1.0.0",
                "git+https://example.invalid/repository",
                &[],
            );
        assert!(validate_gpui_lock(&lock, &supply()).is_err());
    }

    #[test]
    fn rejects_a_second_gpui_crate_family() {
        for (name, version) in [
            ("gpui", "0.2.2"),
            ("gpui_platform", "0.1.0"),
            ("gpui-fast", "0.1.2"),
            ("gpui-fast-platform", "0.1.2"),
            ("gpui-component", "0.7.1"),
            ("gpui-kit-assets", "0.7.1"),
            ("gpui-ce", "0.1.0"),
            ("gpui_wgpu", "0.1.0"),
        ] {
            let lock = pinned_lock() + &lock_entry(name, version, CRATES_IO, &[]);
            assert!(
                validate_gpui_lock(&lock, &supply()).is_err(),
                "{name} must be rejected"
            );
        }
        let second_snapshot = pinned_lock() + &lock_entry("gpui-pre", "0.3.7", CRATES_IO, &[]);
        assert!(validate_gpui_lock(&second_snapshot, &supply()).is_err());
    }

    #[test]
    fn rejects_a_path_override_of_the_snapshot() {
        // `[patch.crates-io] gpui-pre = { path = "..." }` keeps the version
        // and leaves the manifest requirement untouched; only the lock shows
        // the missing registry source.
        let patched = pinned_lock().replace(
            &format!("name = \"gpui-pre\"\nversion = \"0.3.8\"\nsource = \"{CRATES_IO}\"\n"),
            "name = \"gpui-pre\"\nversion = \"0.3.8\"\n",
        );
        assert_ne!(patched, pinned_lock(), "the fixture must drop the source");
        assert!(validate_gpui_lock(&patched, &supply()).is_err());
    }

    #[test]
    fn rejects_an_unrecorded_snapshot_or_release() {
        let moved_base = pinned_lock().replace("version = \"0.7.1\"", "version = \"0.7.2\"");
        assert!(validate_gpui_lock(&moved_base, &supply()).is_err());
        let moved_snapshot = pinned_lock().replace("version = \"0.3.8\"", "version = \"0.3.9\"");
        assert!(validate_gpui_lock(&moved_snapshot, &supply()).is_err());
        let base_elsewhere =
            pinned_lock().replace("dependencies = [\n \"gpui-pre\",\n", "dependencies = [\n");
        assert!(validate_gpui_lock(&base_elsewhere, &supply()).is_err());
    }

    #[test]
    fn requires_exact_gpui_requirements() {
        let check = |dependency: &str, requirement: &str, source: Option<&str>| {
            validate_gpui_requirement(
                "ui",
                dependency,
                &VersionReq::parse(requirement).expect("valid requirement"),
                source,
                &supply(),
            )
        };
        assert!(check("gpui-pre", "=0.3.8", Some(CRATES_IO)).is_ok());
        assert!(check("gpui-pre-platform", "=0.3.8", Some(CRATES_IO)).is_ok());
        assert!(check("gpui-base", "=0.7.1", Some(CRATES_IO)).is_ok());
        assert!(check("serde", "1", Some(CRATES_IO)).is_ok());

        assert!(check("gpui-pre", "0.3.8", Some(CRATES_IO)).is_err());
        assert!(check("gpui-pre", "=0.3", Some(CRATES_IO)).is_err());
        assert!(check("gpui-pre", "=0.3.7", Some(CRATES_IO)).is_err());
        assert!(check("gpui-base", "^0.7.1", Some(CRATES_IO)).is_err());
        assert!(check("gpui-base", ">=0.7.1, <0.8", Some(CRATES_IO)).is_err());
        assert!(
            check(
                "gpui-pre",
                "=0.3.8",
                Some("git+https://github.com/longbridge/gpui-fast")
            )
            .is_err()
        );
        assert!(check("gpui", "*", None).is_err());
        assert!(check("gpui-fast", "=0.1.2", Some(CRATES_IO)).is_err());
        assert!(check("gpui-ce", "=0.1.0", Some(CRATES_IO)).is_err());
        assert!(check("gpui-pre", "=0.3.8-alpha", Some(CRATES_IO)).is_err());
    }

    #[test]
    fn rejects_inverted_domain_dependency() {
        let edges = BTreeMap::from([("domain", BTreeSet::from(["ui"])), ("ui", BTreeSet::new())]);
        assert!(validate_dependency_edges(&edges).is_err());
    }

    #[test]
    fn rejects_forbidden_domain_dependencies() {
        assert!(validate_domain_dependencies(&["gpui"]).is_err());
        assert!(validate_domain_dependencies(&["gpui-pre"]).is_err());
        assert!(validate_domain_dependencies(&["gpui-pre-platform"]).is_err());
        assert!(validate_domain_dependencies(&["uuid", "serde"]).is_ok());
    }

    #[test]
    fn keeps_gpui_base_private_to_ui() {
        let valid = BTreeMap::from([
            ("ui", BTreeSet::from(["gpui-pre", "gpui-base"])),
            ("ordo", BTreeSet::from(["gpui-pre", "ui"])),
        ]);

        let with_test_helper = BTreeMap::from([
            ("ui", BTreeSet::from(["gpui-pre", "gpui-base", "insta"])),
            ("ordo", BTreeSet::from(["gpui-pre", "ui"])),
        ]);
        assert!(validate_ui_dependency_policy(&valid, &with_test_helper).is_ok());

        let removed = BTreeMap::from([
            ("ui", BTreeSet::from(["gpui-pre"])),
            ("ordo", BTreeSet::from(["gpui-pre", "ui"])),
        ]);
        assert!(validate_ui_dependency_policy(&removed, &removed).is_ok());

        let leaked_through_dev = BTreeMap::from([
            ("ui", BTreeSet::from(["gpui-pre", "gpui-base"])),
            ("ordo", BTreeSet::from(["gpui-base", "ui"])),
        ]);
        assert!(validate_ui_dependency_policy(&valid, &leaked_through_dev).is_err());
    }

    #[test]
    fn rejects_unqualified_ui_dependencies() {
        let dependencies = BTreeMap::from([(
            "ui",
            BTreeSet::from(["gpui-pre", "gpui-base", "other-ui-behavior"]),
        )]);
        assert!(validate_ui_dependency_policy(&dependencies, &dependencies).is_err());
    }

    #[test]
    fn qualifies_the_canonical_rendering_stack() {
        let workspace = BTreeSet::from(["domain", "rendering"]);
        let qualified = BTreeMap::from([(
            "rendering",
            BTreeSet::from(["domain", "rustybuzz", "swash", "tiny-skia", "ttf-parser"]),
        )]);
        assert!(validate_rendering_dependency_policy(&qualified, &qualified, &workspace).is_ok());

        let with_extra = BTreeMap::from([(
            "rendering",
            BTreeSet::from(["domain", "rustybuzz", "cosmic-text"]),
        )]);
        assert!(
            validate_rendering_dependency_policy(&with_extra, &with_extra, &workspace).is_err()
        );
    }

    #[test]
    fn keeps_gpui_crates_out_of_rendering() {
        let workspace = BTreeSet::from(["domain", "rendering"]);
        for gpui_crate in [
            "gpui",
            "gpui-base",
            "gpui_platform",
            "gpui-pre",
            "gpui-pre-platform",
            "gpui-fast",
        ] {
            let normal = BTreeMap::from([("rendering", BTreeSet::from(["domain"]))]);
            let with_leak = BTreeMap::from([("rendering", BTreeSet::from(["domain", gpui_crate]))]);
            assert!(
                validate_rendering_dependency_policy(&normal, &with_leak, &workspace).is_err(),
                "{gpui_crate} must not reach rendering even as a dev-dependency"
            );
        }
    }

    #[test]
    fn finds_first_incomplete_milestone() {
        let milestones = "\n### T0.1 — Done\n- [x] check\n### T0.2 — Next\n- [ ] check\n";
        assert_eq!(find_active_milestone(milestones), "T0.2 — Next");
    }
}
