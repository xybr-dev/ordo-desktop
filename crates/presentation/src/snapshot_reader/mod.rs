//! Snapshot reader: the one validation boundary between captured files and a
//! usable Presenter input.
//!
//! A [`LoadedSnapshot`] is the only way a Presentation snapshot reaches the
//! Presenter, and it can only be produced by [`read_snapshot`] succeeding. That
//! function checks every dependency the snapshot declares — every frame file's
//! bytes, digest, canonical format, and composition, and every binary asset's
//! bytes and digest — before it returns, so no partial or unverified input can
//! escape. A caller therefore never has to ask "is this frame present?" while
//! navigating live.
//!
//! # The snapshot directory
//!
//! ```text
//! snapshot.json           the manifest: `snapshot-manifest` v2 (JSON, LF)
//! frames/<name>.txt       one canonical `ordo-fixture/1` frame per file
//! assets/<name>           binary assets declared by digest
//! ```
//!
//! The manifest is the `snapshot-manifest` v2 JSON contract
//! (`contracts/snapshot-manifest/v2.schema.json`, Decision 311): `schema_version`
//! 2, the snapshot identity, title, Themes, Mass sections with their items and
//! Cues, the declared frames and assets, and `dependencies_fingerprint`. A
//! Cue's behavior is an internally tagged object (`project`, `operator_trigger`,
//! `hold`, `wait`, `unavailable`). Frame files stay `ordo-fixture/1` text; only
//! the manifest is JSON. `manifest.rs` holds the grammar.
//!
//! The reader is strict, so one snapshot is one byte sequence and nothing is
//! guessed:
//!
//! - The manifest may not exceed [`MAX_MANIFEST_BYTES`]; the size is checked
//!   before any parsing, and no allocation is sized from a count in the
//!   input.
//! - A `schema_version` other than 2 (v1 is frozen and never read; a higher
//!   version is from a newer Ordo) is refused with the contract's
//!   compatible-version message, before the rest of the document is judged.
//! - Unknown members, a member that appears twice, a missing required member,
//!   and `null` in place of an absent optional member are all rejected, as is
//!   a JSON array where an object belongs (serde's positional form is
//!   refused, so the member guards cannot be bypassed) and a repeated identity
//!   among frames or assets.
//! - One identity names one thing: the snapshot, Mass plan, Celebration,
//!   Themes, sections, items, Cues, frames, and assets share one identity
//!   namespace, and any collision is rejected (`DuplicateIdentity`). A Theme
//!   *reference* (`mass_theme_id`, `theme_id`) or a frame reference
//!   (`frame_id`) is a use of an identity, not a claim on it.
//! - The manifest is LF-only: a carriage return is rejected (naming its line),
//!   the text must end with a final LF, a byte-order mark is rejected, and no
//!   text value may be blank, hold a line break, or end in whitespace.
//!   Every frame is LF-only text under the same rules.
//! - JSON has several spellings of the same content (escapes such as `\u0041`
//!   for `A`, member order, and whitespace between tokens), and each gives
//!   different bytes. The reader accepts them all, so equal content does not
//!   mean equal bytes: [`LoadedSnapshot::manifest_hash`] pins the exact bytes
//!   that were read, and the fingerprint does not.
//!
//! # Invariants this boundary enforces
//!
//! - Fingerprint: the SHA-256 of every declared frame and asset digest's hex,
//!   sorted, each followed by LF — the fingerprint rule of `snapshot-manifest`
//!   v1 (`assets_fingerprint`) and v2, declared as `dependencies_fingerprint`. It
//!   covers the *dependency bytes only*: it does not cover the manifest, so a
//!   changed title, Cue order, identity, or behavior leaves it unchanged.
//!   [`LoadedSnapshot::manifest_hash`] is the SHA-256 of the exact
//!   `snapshot.json` bytes, and a caller that must pin the structure (every
//!   identity and the order) pins that digest as well.
//! - Dependency paths are snapshot-relative: frames live under `frames/` and
//!   assets under `assets/` (file names are free), with no absolute path,
//!   drive prefix, backslash separator, empty segment, `.`, or `..`, so a
//!   source can never be asked to read outside its own root. Segments are also
//!   portable to Windows: no device name (`CON`, `NUL`, `COM1`, ... with or
//!   without an extension), no trailing `.` or space, and no reserved
//!   character, and two paths that differ only in ASCII case count as one
//!   file.
//! - Every asset's byte length equals its declared `size`, checked
//!   before the bytes are hashed.
//! - A frame names an image (`image:`) only when it is a General Image frame.
//! - Every declared frame is shown by at least one Project Cue, and every
//!   Project Cue's frame is declared, so the set of frames is exactly the set
//!   the Presenter can reach.
//! - Every General Image frame names a declared asset, and every declared
//!   asset is named by some frame, so a later raster pass never reaches for
//!   bytes this snapshot did not verify and no unreachable bytes ride along.
//!
//! Reading touches the filesystem through [`SnapshotSource`], never a concrete
//! path, so the later `.ordo` container can feed verified bytes instead. The
//! caller owns the worker thread: this module never spawns one, and
//! [`read_snapshot`] does file I/O, hashing, and a full composition of every
//! frame, so it must run off the GPUI thread.
//!
//! Composition during the read uses the default [`Theme`] at 100% density. It
//! proves each frame is structurally renderable, not that it renders under
//! every Theme: a later render with another Theme or density can still fail,
//! and the Engine (T1.3) maps such a failure to an Unavailable Cue rather than
//! treating the snapshot as unverified.

mod frame;
mod lines;
mod manifest;
#[cfg(test)]
mod tests;

use std::collections::{BTreeMap, BTreeSet};
use std::fmt;
use std::fs;
use std::io::{ErrorKind, Read as _};
use std::path::PathBuf;

use domain::presentation::{
    AssetId, ContentHash, ContentHashError, CueBehavior, CueId, FrameId, IdError, MassItem,
    MassSection, PresentationSnapshot, SnapshotError,
};
use rendering::{RenderError, TargetDensity, TemplateContent, Theme, render_template};
use sha2::{Digest, Sha256};

use frame::parse_frame;
use manifest::{Manifest, build_snapshot, parse_manifest};

/// The one manifest file name a snapshot directory or container must carry.
pub const MANIFEST_PATH: &str = "snapshot.json";

/// Where every declared frame file lives.
const FRAME_DIRECTORY: &str = "frames";

/// Where every declared asset lives.
const ASSET_DIRECTORY: &str = "assets";

/// The only `schema_version` this build reads (`snapshot-manifest` v2).
const MANIFEST_SCHEMA_VERSION: u64 = 2;

/// The largest manifest the reader will parse, checked before parsing. A real
/// Mass plan is a few hundred kilobytes at most; the cap only bounds what a
/// hostile or corrupt file can make the parser do.
pub const MAX_MANIFEST_BYTES: usize = 4 * 1024 * 1024;

/// The format line every frame file starts with, shared with the golden-frame
/// harness in `crates/rendering/tests/golden.rs`.
const FRAME_FORMAT: &str = "ordo-fixture/1";

/// Reads the bytes of one snapshot-relative file.
///
/// Implementations must return exactly the bytes stored under `path`; the
/// reader hashes them, so a normalized or re-encoded copy is a hash mismatch
/// rather than silently accepted content. The reader only passes paths that
/// already passed [`is_snapshot_relative`], but a source is still free to
/// refuse any path it does not serve.
pub trait SnapshotSource {
    fn read(&self, path: &str) -> Result<Vec<u8>, SourceError>;
}

/// Why a [`SnapshotSource`] could not return a file's bytes.
#[derive(Clone, Debug, Eq, PartialEq)]
pub enum SourceError {
    /// Nothing exists at the requested snapshot-relative path.
    NotFound,
    /// The path exists but its bytes could not be read; the operating-system
    /// message is carried for diagnostics.
    Unreadable { reason: String },
    /// The source deliberately does not serve this path, for example because
    /// it is a symbolic link or not a regular file; nothing was read.
    Refused { reason: String },
}

/// A [`SnapshotSource`] over one directory: the ordinary on-disk snapshot.
///
/// It serves regular files only: a symbolic link (or, on Windows, any reparse
/// point) at the file or at any directory on the way to it is refused with
/// [`SourceError::Refused`], so a snapshot can never read bytes from outside
/// its own directory through a link.
#[derive(Clone, Debug)]
pub struct DirectorySource {
    root: PathBuf,
}

impl DirectorySource {
    pub fn new(root: impl Into<PathBuf>) -> Self {
        Self { root: root.into() }
    }
}

impl SnapshotSource for DirectorySource {
    fn read(&self, path: &str) -> Result<Vec<u8>, SourceError> {
        // The reader validates before calling, but this type is public: a
        // direct caller must not be able to walk out of `root` with `..` or an
        // absolute path that replaces the root entirely.
        if !is_snapshot_relative(path) {
            return Err(SourceError::Unreadable {
                reason: format!("{path:?} is not a snapshot-relative path"),
            });
        }
        let mut current = self.root.clone();
        let mut segments = path.split('/').peekable();
        while let Some(segment) = segments.next() {
            current.push(segment);
            let metadata = fs::symlink_metadata(&current).map_err(io_error)?;
            if is_link(&metadata) {
                return Err(SourceError::Refused {
                    reason: format!("{path:?} passes through a symbolic link"),
                });
            }
            if segments.peek().is_none() && !metadata.is_file() {
                return Err(SourceError::Refused {
                    reason: format!("{path:?} is not a regular file"),
                });
            }
        }
        if path == MANIFEST_PATH {
            return read_manifest_capped(&current);
        }
        fs::read(&current).map_err(io_error)
    }
}

/// Read the manifest without ever holding more than [`MAX_MANIFEST_BYTES`] of
/// it: the length is checked first, and the read itself is bounded in case the
/// file grows after the check. An oversized manifest is refused, not read.
fn read_manifest_capped(path: &std::path::Path) -> Result<Vec<u8>, SourceError> {
    let too_large = || SourceError::Refused {
        reason: format!("{MANIFEST_PATH} is larger than {MAX_MANIFEST_BYTES} bytes"),
    };
    let file = fs::File::open(path).map_err(io_error)?;
    if file.metadata().map_err(io_error)?.len() > MAX_MANIFEST_BYTES as u64 {
        return Err(too_large());
    }
    let mut bytes = Vec::new();
    file.take(MAX_MANIFEST_BYTES as u64 + 1)
        .read_to_end(&mut bytes)
        .map_err(io_error)?;
    if bytes.len() > MAX_MANIFEST_BYTES {
        return Err(too_large());
    }
    Ok(bytes)
}

fn io_error(error: std::io::Error) -> SourceError {
    if error.kind() == ErrorKind::NotFound {
        SourceError::NotFound
    } else {
        SourceError::Unreadable {
            reason: error.to_string(),
        }
    }
}

/// Whether the entry is a symbolic link, or on Windows any reparse point
/// (junctions and mount points are not reported as symbolic links).
fn is_link(metadata: &fs::Metadata) -> bool {
    #[cfg(windows)]
    {
        use std::os::windows::fs::MetadataExt as _;
        const FILE_ATTRIBUTE_REPARSE_POINT: u32 = 0x400;
        if metadata.file_attributes() & FILE_ATTRIBUTE_REPARSE_POINT != 0 {
            return true;
        }
    }
    metadata.file_type().is_symlink()
}

/// Which kind of declared dependency an error is about.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum DependencyId {
    Frame(FrameId),
    Asset(AssetId),
}

impl fmt::Display for DependencyId {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Frame(id) => write!(f, "frame {id}"),
            Self::Asset(id) => write!(f, "asset {id}"),
        }
    }
}

/// One declared dependency as the manifest names it: identity plus relative
/// path, so a failure can name both the thing and the file to fix.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct Dependency {
    pub id: DependencyId,
    pub path: String,
}

/// Every verified frame and asset of one successfully read snapshot.
///
/// The domain snapshot travels with the files it referenced: after
/// [`read_snapshot`] returns, every Cue that can go live has a frame, and every
/// frame has bytes that matched its declared digest. Nothing here is fetched
/// lazily, so a later failure can only be a rendering or device failure, never
/// a missing dependency.
pub struct LoadedSnapshot {
    snapshot: PresentationSnapshot,
    frames: BTreeMap<FrameId, LoadedFrame>,
    assets: BTreeMap<AssetId, LoadedAsset>,
    fingerprint: ContentHash,
    manifest_hash: ContentHash,
}

struct LoadedFrame {
    content: TemplateContent,
    hash: ContentHash,
}

struct LoadedAsset {
    bytes: Vec<u8>,
    hash: ContentHash,
}

impl LoadedSnapshot {
    /// The domain model: sections, Mass items, Cues, and Themes.
    pub fn snapshot(&self) -> &PresentationSnapshot {
        &self.snapshot
    }

    /// The parsed frame file a Project Cue shows.
    ///
    /// Returns `None` only for an identity from another snapshot: every frame
    /// named by this snapshot's own Cues was loaded and checked, so the Engine
    /// can treat `None` as a programming error rather than a live failure.
    pub fn frame(&self, id: FrameId) -> Option<&TemplateContent> {
        self.frames.get(&id).map(|frame| &frame.content)
    }

    /// The verified SHA-256 of a frame's bytes.
    pub fn frame_hash(&self, id: FrameId) -> Option<ContentHash> {
        self.frames.get(&id).map(|frame| frame.hash)
    }

    /// The verified bytes of a binary asset a General Image frame names.
    pub fn asset(&self, id: AssetId) -> Option<&[u8]> {
        self.assets.get(&id).map(|asset| asset.bytes.as_slice())
    }

    /// The verified SHA-256 of an asset's bytes.
    pub fn asset_hash(&self, id: AssetId) -> Option<ContentHash> {
        self.assets.get(&id).map(|asset| asset.hash)
    }

    /// Every verified asset identity, in stable identity order.
    pub fn asset_ids(&self) -> impl Iterator<Item = AssetId> + '_ {
        self.assets.keys().copied()
    }

    /// The digest of every declared frame and asset digest under the
    /// `snapshot-manifest` fingerprint rule (`dependencies_fingerprint`).
    ///
    /// This covers the dependency bytes only. It does not cover the manifest:
    /// a changed title, identity, Cue order, or Cue behavior leaves it
    /// unchanged. Use [`LoadedSnapshot::manifest_hash`] to pin those.
    pub fn fingerprint(&self) -> ContentHash {
        self.fingerprint
    }

    /// The SHA-256 of the exact `snapshot.json` bytes that were read, so it
    /// pins every identity, title, and the order of sections, items, and Cues
    /// that the dependency-only [`LoadedSnapshot::fingerprint`] leaves free.
    pub fn manifest_hash(&self) -> ContentHash {
        self.manifest_hash
    }
}

impl fmt::Debug for LoadedSnapshot {
    /// Names and digests only: asset bytes never reach a log line or a
    /// diagnostic report by accident.
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("LoadedSnapshot")
            .field("snapshot", &self.snapshot)
            .field(
                "frames",
                &self.frames.keys().copied().collect::<Vec<FrameId>>(),
            )
            .field(
                "assets",
                &self.assets.keys().copied().collect::<Vec<AssetId>>(),
            )
            .field("fingerprint", &self.fingerprint)
            .field("manifest_hash", &self.manifest_hash)
            .finish()
    }
}

/// Read and verify one complete snapshot.
///
/// Load order is deliberate: structure first, so a malformed manifest fails
/// before any file is touched; then every dependency's bytes and digest; then
/// every frame's format and composition; then the fingerprint; and only then
/// the cross-references between Cues, frames, and assets. Every step is
/// fallible on its own, and the first failure returns a typed error naming the
/// line, identity, or path to fix. Nothing is retained from a failed load.
pub fn read_snapshot(source: &dyn SnapshotSource) -> Result<LoadedSnapshot, SnapshotLoadError> {
    let bytes = source.read(MANIFEST_PATH).map_err(|error| match error {
        SourceError::NotFound => SnapshotLoadError::MissingManifest,
        SourceError::Unreadable { reason } => SnapshotLoadError::UnreadableManifest { reason },
        SourceError::Refused { reason } => SnapshotLoadError::RefusedManifest { reason },
    })?;
    // The cap comes first so an oversized file is never hashed or parsed.
    if bytes.len() > MAX_MANIFEST_BYTES {
        return Err(SnapshotLoadError::Manifest {
            reason: ManifestReason::TooLarge {
                size: bytes.len(),
                limit: MAX_MANIFEST_BYTES,
            },
        });
    }
    let manifest_hash = hash_bytes(&bytes);
    let text = std::str::from_utf8(&bytes).map_err(|_| SnapshotLoadError::ManifestNotUtf8)?;

    // Structure before I/O: a manifest that cannot even describe a snapshot
    // must not make the reader open files.
    let Manifest {
        snapshot: snapshot_id,
        title,
        fingerprint: declared_fingerprint,
        themes,
        mass_theme,
        frames: declared_frames,
        assets: declared_assets,
        sections,
    } = parse_manifest(text).map_err(|reason| SnapshotLoadError::Manifest { reason })?;
    let snapshot = build_snapshot(snapshot_id, title, mass_theme, themes, sections)?;

    let mut frames = BTreeMap::new();
    for declared in &declared_frames {
        let (id, path) = (declared.id, &declared.path);
        let bytes = read_dependency(source, DependencyId::Frame(id), path)?;
        let hash = check_hash(DependencyId::Frame(id), path, &bytes, declared.hash)?;
        let text = std::str::from_utf8(&bytes).map_err(|_| SnapshotLoadError::InvalidFrame {
            frame: id,
            path: path.clone(),
            error: FrameParseError::NotUtf8,
        })?;
        let content = parse_frame(text).map_err(|error| SnapshotLoadError::InvalidFrame {
            frame: id,
            path: path.clone(),
            error,
        })?;
        // Composition is the last thing that can make an otherwise well-formed
        // frame unusable, so it runs here rather than on the display path.
        render_template(&content, &Theme::default(), TargetDensity::SCALE_100).map_err(
            |error| SnapshotLoadError::UnrenderableFrame {
                frame: id,
                path: path.clone(),
                error,
            },
        )?;
        frames.insert(id, LoadedFrame { content, hash });
    }

    let mut assets = BTreeMap::new();
    for declared in &declared_assets {
        let (id, path) = (declared.id, &declared.path);
        let bytes = read_dependency(source, DependencyId::Asset(id), path)?;
        // The cheap length check comes before hashing so a wrong-sized file is
        // reported as such and never costs a digest.
        if declared.size as usize != bytes.len() {
            return Err(SnapshotLoadError::WrongAssetSize {
                asset: id,
                path: path.clone(),
                declared: declared.size,
                actual: bytes.len(),
            });
        }
        let hash = check_hash(DependencyId::Asset(id), path, &bytes, declared.hash)?;
        assets.insert(id, LoadedAsset { bytes, hash });
    }

    let fingerprint = fingerprint_of(
        frames
            .values()
            .map(|frame| frame.hash)
            .chain(assets.values().map(|asset| asset.hash)),
    );
    if fingerprint != declared_fingerprint {
        return Err(SnapshotLoadError::FingerprintMismatch {
            declared: declared_fingerprint,
            actual: fingerprint,
        });
    }

    verify_references(&snapshot, &frames, &assets)?;

    Ok(LoadedSnapshot {
        snapshot,
        frames,
        assets,
        fingerprint,
        manifest_hash,
    })
}

/// Read one declared dependency's bytes, naming the identity and path on
/// failure.
fn read_dependency(
    source: &dyn SnapshotSource,
    id: DependencyId,
    path: &str,
) -> Result<Vec<u8>, SnapshotLoadError> {
    let dependency = || Dependency {
        id,
        path: path.to_owned(),
    };
    source.read(path).map_err(|error| match error {
        SourceError::NotFound => SnapshotLoadError::MissingDependency {
            dependency: dependency(),
        },
        SourceError::Unreadable { reason } => SnapshotLoadError::UnreadableDependency {
            dependency: dependency(),
            reason,
        },
        SourceError::Refused { reason } => SnapshotLoadError::RefusedDependency {
            dependency: dependency(),
            reason,
        },
    })
}

/// Hash `bytes` once and check them against the declared digest, returning the
/// digest so the caller never re-hashes.
fn check_hash(
    id: DependencyId,
    path: &str,
    bytes: &[u8],
    declared: ContentHash,
) -> Result<ContentHash, SnapshotLoadError> {
    let actual = hash_bytes(bytes);
    if actual != declared {
        return Err(SnapshotLoadError::HashMismatch {
            dependency: Dependency {
                id,
                path: path.to_owned(),
            },
            expected: declared,
            actual,
        });
    }
    Ok(actual)
}

/// Check that the frames and assets the manifest declares are exactly the ones
/// the Cues can reach, and that image frames name verified assets.
///
/// This runs after the fingerprint so a manifest edit that forgets to recompute
/// the fingerprint is reported as the edit it is, not as a broken reference.
fn verify_references(
    snapshot: &PresentationSnapshot,
    frames: &BTreeMap<FrameId, LoadedFrame>,
    assets: &BTreeMap<AssetId, LoadedAsset>,
) -> Result<(), SnapshotLoadError> {
    let mut used = BTreeSet::new();
    for cue in snapshot
        .sections()
        .iter()
        .flat_map(MassSection::items)
        .flat_map(MassItem::cues)
    {
        if let CueBehavior::Project { frame, .. } = cue.behavior() {
            if !frames.contains_key(frame) {
                return Err(SnapshotLoadError::UndeclaredFrame {
                    cue: cue.id(),
                    frame: *frame,
                });
            }
            used.insert(*frame);
        }
    }
    for frame in frames.keys() {
        if !used.contains(frame) {
            return Err(SnapshotLoadError::UnusedFrame { frame: *frame });
        }
    }
    let mut used_assets = BTreeSet::new();
    for (frame, loaded) in frames {
        if let Some(image) = &loaded.content.image {
            let declared = AssetId::parse(&image.id)
                .ok()
                .filter(|asset| assets.contains_key(asset));
            let Some(asset) = declared else {
                return Err(SnapshotLoadError::UndeclaredAsset {
                    frame: *frame,
                    asset: image.id.clone(),
                });
            };
            used_assets.insert(asset);
        }
    }
    if let Some(asset) = assets.keys().find(|asset| !used_assets.contains(asset)) {
        return Err(SnapshotLoadError::UnusedAsset { asset: *asset });
    }
    Ok(())
}

/// The `snapshot-manifest` fingerprint rule (v1 `assets_fingerprint`, v2
/// `dependencies_fingerprint`): every digest's lowercase hex,
/// sorted, each followed by LF, hashed once more.
///
/// Duplicates appear once per declaration rather than being deduplicated, so
/// two declarations of identical bytes still change the fingerprint and a
/// declaration can never be added or removed without the digest noticing.
fn fingerprint_of(hashes: impl IntoIterator<Item = ContentHash>) -> ContentHash {
    let mut hex: Vec<String> = hashes.into_iter().map(|hash| hash.to_string()).collect();
    hex.sort_unstable();
    let mut digest = Sha256::new();
    for entry in &hex {
        digest.update(entry.as_bytes());
        digest.update(b"\n");
    }
    digest_bytes(digest.finalize())
}

fn hash_bytes(bytes: &[u8]) -> ContentHash {
    digest_bytes(Sha256::digest(bytes))
}

fn digest_bytes(digest: impl AsRef<[u8]>) -> ContentHash {
    let mut raw = [0u8; 32];
    raw.copy_from_slice(digest.as_ref());
    ContentHash::from_bytes(raw)
}

/// Whether `path` is a safe snapshot-relative path.
///
/// The separator is always `/`, because a manifest is a portable artifact and
/// `\` on Windows would otherwise silently accept absolute drive paths and
/// parent traversals. `.` and `..` segments are refused rather than normalized:
/// normalizing would let two manifest lines name the same file under different
/// spellings. Every segment must also be a legal Windows file name, because a
/// snapshot written on Linux must still open on a church PC: no device name
/// (`CON`, `NUL`, `COM1`, ...) with or without an extension, no trailing `.`
/// or space (Windows silently strips them, so two spellings would name one
/// file), and no reserved character or control character.
fn is_snapshot_relative(path: &str) -> bool {
    !path.is_empty()
        && !path.starts_with('/')
        && !path.contains('\\')
        && !path.contains(':')
        && path.split('/').all(is_portable_segment)
}

fn is_portable_segment(segment: &str) -> bool {
    !segment.is_empty()
        && segment != "."
        && segment != ".."
        && !segment.ends_with(['.', ' '])
        && !segment
            .chars()
            .any(|c| c.is_control() || matches!(c, '<' | '>' | '"' | '|' | '?' | '*'))
        && !is_windows_device_name(segment)
}

/// A reserved Windows device name, ignoring case and any extension
/// (`aux.txt` is still the AUX device).
fn is_windows_device_name(segment: &str) -> bool {
    let stem = segment.split('.').next().unwrap_or(segment);
    let upper = stem.to_ascii_uppercase();
    if matches!(
        upper.as_str(),
        "CON" | "PRN" | "AUX" | "NUL" | "CONIN$" | "CONOUT$"
    ) {
        return true;
    }
    ["COM", "LPT"].iter().any(|prefix| {
        upper.strip_prefix(prefix).is_some_and(|rest| {
            let mut digits = rest.chars();
            matches!(
                (digits.next(), digits.next()),
                (Some('1'..='9' | '\u{b9}' | '\u{b2}' | '\u{b3}'), None)
            )
        })
    })
}

/// Why one snapshot could not become usable input.
///
/// Every variant either names the manifest line, the dependency identity and
/// path, or the Cue involved, so the operator-facing message can point at the
/// file to repair instead of reporting that loading failed.
#[derive(Clone, Debug, Eq, PartialEq)]
pub enum SnapshotLoadError {
    /// The source has no `snapshot.json`.
    MissingManifest,
    /// `snapshot.json` exists but its bytes could not be read.
    UnreadableManifest { reason: String },
    /// The source refused to serve `snapshot.json` (for example a symbolic
    /// link); nothing was read.
    RefusedManifest { reason: String },
    /// `snapshot.json` is not UTF-8 text, so no JSON can be read.
    ManifestNotUtf8,
    /// The manifest is not a valid `snapshot-manifest` v2 document.
    Manifest { reason: ManifestReason },
    /// The assembled snapshot violates a domain invariant.
    Snapshot(SnapshotError),
    /// A declared frame or asset is absent from the source.
    MissingDependency { dependency: Dependency },
    /// A declared frame or asset exists but its bytes could not be read.
    UnreadableDependency {
        dependency: Dependency,
        reason: String,
    },
    /// The source refused to serve a declared frame or asset (for example a
    /// symbolic link); nothing was read.
    RefusedDependency {
        dependency: Dependency,
        reason: String,
    },
    /// A dependency's bytes do not match the digest the manifest declares.
    HashMismatch {
        dependency: Dependency,
        expected: ContentHash,
        actual: ContentHash,
    },
    /// A frame file is not canonical `ordo-fixture/1` content.
    InvalidFrame {
        frame: FrameId,
        path: String,
        error: FrameParseError,
    },
    /// A frame file parses but cannot compose into one complete frame.
    UnrenderableFrame {
        frame: FrameId,
        path: String,
        error: RenderError,
    },
    /// The declared fingerprint does not match the dependency digests.
    FingerprintMismatch {
        declared: ContentHash,
        actual: ContentHash,
    },
    /// A Project Cue shows a frame no record declares.
    UndeclaredFrame { cue: CueId, frame: FrameId },
    /// A declared frame is not shown by any Project Cue.
    UnusedFrame { frame: FrameId },
    /// A General Image frame names an asset no record declares.
    UndeclaredAsset { frame: FrameId, asset: String },
    /// A declared asset is not named by any frame.
    UnusedAsset { asset: AssetId },
    /// An asset's byte length is not its declared `asset-size`.
    WrongAssetSize {
        asset: AssetId,
        path: String,
        declared: u32,
        actual: usize,
    },
}

impl fmt::Display for SnapshotLoadError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::MissingManifest => write!(f, "the snapshot has no {MANIFEST_PATH} manifest"),
            Self::UnreadableManifest { reason } => {
                write!(f, "{MANIFEST_PATH} cannot be read: {reason}")
            }
            Self::RefusedManifest { reason } => {
                write!(f, "{MANIFEST_PATH} was refused: {reason}")
            }
            Self::ManifestNotUtf8 => write!(f, "{MANIFEST_PATH} is not UTF-8 text"),
            Self::Manifest { reason } => write!(f, "{MANIFEST_PATH}: {reason}"),
            Self::Snapshot(error) => write!(f, "the snapshot is not usable: {error}"),
            Self::MissingDependency { dependency } => {
                write!(f, "{} at {:?} is missing", dependency.id, dependency.path)
            }
            Self::UnreadableDependency { dependency, reason } => write!(
                f,
                "{} at {:?} cannot be read: {reason}",
                dependency.id, dependency.path
            ),
            Self::RefusedDependency { dependency, reason } => write!(
                f,
                "{} at {:?} was refused: {reason}",
                dependency.id, dependency.path
            ),
            Self::HashMismatch {
                dependency,
                expected,
                actual,
            } => write!(
                f,
                "{} at {:?} hashes to {actual}, not the declared {expected}",
                dependency.id, dependency.path
            ),
            Self::InvalidFrame { frame, path, error } => {
                write!(f, "frame {frame} at {path:?}: {error}")
            }
            Self::UnrenderableFrame { frame, path, error } => {
                write!(f, "frame {frame} at {path:?} cannot compose: {error}")
            }
            Self::FingerprintMismatch { declared, actual } => write!(
                f,
                "declared fingerprint {declared} does not match the dependency digests {actual}"
            ),
            Self::UndeclaredFrame { cue, frame } => write!(
                f,
                "Cue {cue} shows frame {frame}, which the manifest does not declare"
            ),
            Self::UnusedFrame { frame } => {
                write!(f, "frame {frame} is declared but no Project Cue shows it")
            }
            Self::UndeclaredAsset { frame, asset } => write!(
                f,
                "frame {frame} uses asset {asset:?}, which the manifest does not declare"
            ),
            Self::UnusedAsset { asset } => {
                write!(f, "asset {asset} is declared but no frame uses it")
            }
            Self::WrongAssetSize {
                asset,
                path,
                declared,
                actual,
            } => write!(
                f,
                "asset {asset} at {path:?} is {actual} bytes, not the declared {declared}"
            ),
        }
    }
}

impl std::error::Error for SnapshotLoadError {}

impl From<SnapshotError> for SnapshotLoadError {
    fn from(error: SnapshotError) -> Self {
        Self::Snapshot(error)
    }
}

/// Why the manifest is not a usable `snapshot-manifest` v2 document.
///
/// Where a value is at fault, `at` is the RFC 6901 JSON Pointer to it (for
/// example `/sections/2/items/0/cues/1/id`), so the message names the spot to
/// repair even though JSON has no stable line per value.
#[derive(Clone, Debug, Eq, PartialEq)]
pub enum ManifestReason {
    /// The manifest is larger than [`MAX_MANIFEST_BYTES`]; it was not parsed.
    TooLarge { size: usize, limit: usize },
    /// The manifest starts with a byte-order mark.
    ByteOrderMark,
    /// The 1-based `line` holds a carriage return; the manifest is LF-only.
    CarriageReturn { line: usize },
    /// The manifest does not end with a final LF.
    MissingFinalNewline,
    /// The text is not the document shape: bad JSON, an unknown member, a
    /// repeated member, a missing or mistyped member, or an unknown variant.
    /// `message` is the cause; `line` and `column` (1-based) locate it.
    Malformed {
        line: usize,
        column: usize,
        message: String,
    },
    /// `schema_version` names a version this build does not read.
    UnsupportedVersion { found: u64 },
    /// The value is not a canonical lowercase UUID v4 identity.
    InvalidId {
        at: String,
        value: String,
        error: IdError,
    },
    /// The value is not a canonical lowercase SHA-256 digest.
    InvalidHash {
        at: String,
        value: String,
        error: ContentHashError,
    },
    /// `captured_at` is not `YYYY-MM-DDTHH:MM:SSZ`.
    InvalidTimestamp { at: String, value: String },
    /// A title, label, name, or explanation is empty or whitespace only.
    Blank { at: String },
    /// A text value holds a line break; manifest texts are one line.
    LineBreak { at: String },
    /// A text value ends in whitespace, which an exact byte pin cannot show.
    TrailingWhitespace { at: String },
    /// A dependency path is absolute, backslash-separated, escapes its root,
    /// or is not a portable Windows file name.
    UnsafePath { path: String },
    /// One identity is claimed by two entities of the snapshot (for example a
    /// frame and a Cue), or by one entity twice.
    DuplicateIdentity { at: String, value: String },
    /// The same identity is declared more than once.
    Duplicate { field: &'static str, value: String },
    /// Two dependencies name the same file (ignoring ASCII case).
    DuplicatePath { path: String },
}

impl fmt::Display for ManifestReason {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::TooLarge { size, limit } => {
                write!(f, "the manifest is {size} bytes; the limit is {limit}")
            }
            Self::ByteOrderMark => f.write_str("the manifest starts with a byte-order mark"),
            Self::CarriageReturn { line } => {
                write!(f, "line {line}: carriage return (the manifest is LF-only)")
            }
            Self::MissingFinalNewline => f.write_str("the manifest must end with a final LF"),
            Self::Malformed {
                line,
                column,
                message,
            } => write!(f, "line {line} column {column}: {message}"),
            Self::UnsupportedVersion { found } => write!(
                f,
                "requires an Ordo version supporting snapshot-manifest v{found}; this build supports v{MANIFEST_SCHEMA_VERSION}"
            ),
            Self::InvalidId { at, value, error } => {
                write!(f, "{at}: {value:?} is not a canonical identity: {error}")
            }
            Self::InvalidHash { at, value, error } => {
                write!(f, "{at}: {value:?} is not a canonical digest: {error}")
            }
            Self::InvalidTimestamp { at, value } => {
                write!(f, "{at}: {value:?} is not a UTC timestamp")
            }
            Self::Blank { at } => write!(f, "{at}: the text must not be blank"),
            Self::LineBreak { at } => write!(f, "{at}: the text must be one line"),
            Self::TrailingWhitespace { at } => {
                write!(f, "{at}: the text must not end in whitespace")
            }
            Self::UnsafePath { path } => write!(f, "{path:?} is not a snapshot-relative path"),
            Self::DuplicateIdentity { at, value } => {
                write!(f, "{at}: identity {value} already names something else")
            }
            Self::Duplicate { field, value } => {
                write!(f, "{field} {value} is declared more than once")
            }
            Self::DuplicatePath { path } => {
                write!(f, "{path:?} is named by more than one dependency")
            }
        }
    }
}

/// Why one frame file could not be read as a canonical `ordo-fixture/1` frame.
#[derive(Clone, Debug, Eq, PartialEq)]
pub enum FrameParseError {
    /// The bytes are not UTF-8 text, so there is no line to name.
    NotUtf8,
    /// The 1-based `line` does not hold the record this position expects.
    Line { line: usize, reason: FrameReason },
}

impl fmt::Display for FrameParseError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::NotUtf8 => f.write_str("the frame is not UTF-8 text"),
            Self::Line { line, reason } => write!(f, "line {line}: {reason}"),
        }
    }
}

/// The line-level reason a frame does not match `ordo-fixture/1`.
#[derive(Clone, Debug, Eq, PartialEq)]
pub enum FrameReason {
    /// The first line is not the frame format line.
    UnknownFormat { found: String },
    /// The value is not one of the adopted template kinds.
    InvalidKind { value: String },
    /// A `body-lines` or `footer-lines` value is not a plain decimal count.
    InvalidCount { value: String },
    /// An `image-size` value is not `<width>x<height>` in plain decimal.
    InvalidSize { value: String },
    /// The next record was `found` where `expected` belongs.
    Unexpected {
        expected: &'static str,
        found: String,
    },
    /// The frame ended before `expected` appeared.
    Missing { expected: &'static str },
    /// Content follows the last record.
    Trailing,
    /// The line holds a carriage return; frames are LF-only.
    CarriageReturn,
    /// The frame does not end with a final LF.
    MissingFinalNewline,
    /// An `image:` record on a frame that is not a General Image frame.
    ImageOnNonImageFrame,
}

impl fmt::Display for FrameReason {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::UnknownFormat { found } => write!(
                f,
                "expected the `{FRAME_FORMAT}` format line, found {found:?}"
            ),
            Self::InvalidKind { value } => write!(f, "{value:?} is not an adopted template kind"),
            Self::InvalidCount { value } => {
                write!(f, "{value:?} is not a plain decimal line count")
            }
            Self::InvalidSize { value } => {
                write!(f, "{value:?} is not a plain decimal <width>x<height>")
            }
            Self::Unexpected { expected, found } => {
                write!(f, "expected {expected}, found {found:?}")
            }
            Self::Missing { expected } => write!(f, "expected {expected}"),
            Self::Trailing => f.write_str("content follows the last record"),
            Self::CarriageReturn => f.write_str("carriage return (frames are LF-only)"),
            Self::MissingFinalNewline => f.write_str("the frame must end with a final LF"),
            Self::ImageOnNonImageFrame => {
                f.write_str("only a GeneralImage frame may name an image")
            }
        }
    }
}
