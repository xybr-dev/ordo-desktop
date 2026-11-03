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
//! snapshot.txt            the manifest, one record per line (LF)
//! frames/<name>.txt       one canonical `ordo-fixture/1` frame per file
//! assets/<name>           binary assets declared by digest
//! ```
//!
//! The manifest is a deliberately simple line format in the spirit of
//! `ordo-fixture/1`, not the frozen `snapshot-manifest` v1 JSON contract: that
//! contract has no Mass sections or Cues yet, and reconciling the two belongs
//! to the Portability phase. Records appear in this fixed order, one per line:
//!
//! ```text
//! ordo-presentation-snapshot/1
//! snapshot <SnapshotId>
//! title <text>
//! fingerprint <sha256>
//! theme <ThemeId>
//! theme-name <text>
//! theme-kind <Custom|Fallback>
//! ...                                   (one theme record per Theme)
//! mass-theme <ThemeId>
//! frame <FrameId>
//! frame-path <frames/...>
//! frame-sha256 <sha256>
//! ...                                   (one frame record per frame)
//! asset <AssetId>
//! asset-path <assets/...>
//! asset-size <bytes>
//! asset-sha256 <sha256>
//! ...                                   (one asset record per asset)
//! section <MassSectionId>
//! section-title <text>
//! item <MassItemId>
//! item-title <text>
//! item-kind <Song|Reading|Psalm|ResponseSequence|GeneralPresentation|Rite>
//! item-theme <ThemeId>                  (optional: inherit the Mass Theme)
//! item-auto-play <seconds>              (optional: manual advance)
//! cue <CueId>
//! cue-label <text>
//! cue-private-context <text>            (optional: operator-only)
//! cue-behavior <Project|OperatorTrigger|Hold|Wait|Unavailable>
//! cue-frame <FrameId>                   (Project only)
//! cue-duration <seconds>                (Project only, optional)
//! cue-reason <text>                     (Unavailable only)
//! ...                                   (one or more cues per item)
//! ...                                   (one or more items per section)
//! ...                                   (one or more sections)
//! ```
//!
//! Text values are the raw remainder of the line, so punctuation, quotes, and
//! non-Latin scripts pass through untouched and nothing needs escaping. A
//! missing required record, an out-of-order line, a miscount, or trailing
//! content is a loud error naming the 1-based line.
//!
//! # Invariants this boundary enforces
//!
//! - Fingerprint: the SHA-256 of every declared frame and asset digest's hex,
//!   sorted, each followed by LF — the same rule `snapshot-manifest` v1 fixes
//!   for `assets_fingerprint` — so one digest names exactly one captured
//!   snapshot.
//! - Dependency paths are snapshot-relative: frames live under `frames/` and
//!   assets under `assets/` (file names are free), with no absolute path,
//!   drive prefix, backslash separator, empty segment, `.`, or `..`, so a
//!   source can never be asked to read outside its own root.
//! - Every asset's byte length equals its declared `asset-size`.
//! - Every declared frame is shown by at least one Project Cue, and every
//!   Project Cue's frame is declared, so the set of frames is exactly the set
//!   the Presenter can reach.
//! - Every General Image frame names a declared asset, and every declared
//!   asset is named by some frame, so a later raster pass never reaches for
//!   bytes this snapshot did not verify and no unreachable bytes ride along.
//!
//! Reading touches the filesystem through [`SnapshotSource`], never a concrete
//! path, so the later `.ordo` container can feed verified bytes instead. The
//! caller owns the worker thread: this module never spawns one and never
//! blocks the GPUI thread by itself.

use std::collections::{BTreeMap, BTreeSet};
use std::fmt;
use std::fs;
use std::io::ErrorKind;
use std::path::PathBuf;
use std::str::FromStr;

use domain::presentation::{
    AssetId, AutoPlay, ContentHash, ContentHashError, Cue, CueBehavior, CueId, FrameId, IdError,
    MassItem, MassItemId, MassItemKind, MassSection, MassSectionId, PresentationSnapshot,
    SnapshotError, SnapshotId, ThemeId, ThemeKind, ThemeRef,
};
use rendering::{
    ForegroundImage, RenderError, TargetDensity, TemplateContent, TemplateKind, Theme,
    render_template,
};
use sha2::{Digest, Sha256};

/// The one manifest file name a snapshot directory or container must carry.
pub const MANIFEST_PATH: &str = "snapshot.txt";

/// Where every declared frame file lives.
const FRAME_DIRECTORY: &str = "frames";

/// Where every declared asset lives.
const ASSET_DIRECTORY: &str = "assets";

/// The format line every manifest starts with.
const MANIFEST_FORMAT: &str = "ordo-presentation-snapshot/1";

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
}

/// A [`SnapshotSource`] over one directory: the ordinary on-disk snapshot.
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
        match fs::read(self.root.join(path)) {
            Ok(bytes) => Ok(bytes),
            Err(error) if error.kind() == ErrorKind::NotFound => Err(SourceError::NotFound),
            Err(error) => Err(SourceError::Unreadable {
                reason: error.to_string(),
            }),
        }
    }
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

    /// Every verified frame identity, in stable identity order.
    pub fn frame_ids(&self) -> impl Iterator<Item = FrameId> + '_ {
        self.frames.keys().copied()
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
    /// `snapshot-manifest` v1 rule.
    pub fn fingerprint(&self) -> ContentHash {
        self.fingerprint
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
    })?;
    let text = std::str::from_utf8(&bytes).map_err(|_| SnapshotLoadError::ManifestNotUtf8)?;

    // Structure before I/O: a manifest that cannot even describe a snapshot
    // must not make the reader open files.
    let manifest = parse_manifest(text)?;
    let snapshot = build_snapshot(&manifest)?;

    let mut frames = BTreeMap::new();
    for declared in &manifest.frames {
        let (id, path) = (declared.id, &declared.path);
        let (bytes, hash) = read_verified(source, DependencyId::Frame(id), declared)?;
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
    for declared in &manifest.assets {
        let id = declared.id;
        let (bytes, hash) = read_verified(source, DependencyId::Asset(id), declared)?;
        if let Some(size) = declared.size.filter(|size| *size as usize != bytes.len()) {
            return Err(SnapshotLoadError::WrongAssetSize {
                asset: id,
                path: declared.path.clone(),
                declared: size,
                actual: bytes.len(),
            });
        }
        assets.insert(id, LoadedAsset { bytes, hash });
    }

    let fingerprint = fingerprint_of(
        frames
            .values()
            .map(|frame| frame.hash)
            .chain(assets.values().map(|asset| asset.hash)),
    );
    if fingerprint != manifest.fingerprint {
        return Err(SnapshotLoadError::FingerprintMismatch {
            declared: manifest.fingerprint,
            actual: fingerprint,
        });
    }

    verify_references(&snapshot, &frames, &assets)?;

    Ok(LoadedSnapshot {
        snapshot,
        frames,
        assets,
        fingerprint,
    })
}

/// Read one declared dependency and check its bytes against the declared
/// digest, returning both so the caller never re-hashes.
fn read_verified<T>(
    source: &dyn SnapshotSource,
    id: DependencyId,
    declared: &DependencyDecl<T>,
) -> Result<(Vec<u8>, ContentHash), SnapshotLoadError> {
    let dependency = || Dependency {
        id,
        path: declared.path.clone(),
    };
    let bytes = source.read(&declared.path).map_err(|error| match error {
        SourceError::NotFound => SnapshotLoadError::MissingDependency {
            dependency: dependency(),
        },
        SourceError::Unreadable { reason } => SnapshotLoadError::UnreadableDependency {
            dependency: dependency(),
            reason,
        },
    })?;
    let actual = hash_bytes(&bytes);
    if actual != declared.hash {
        return Err(SnapshotLoadError::HashMismatch {
            dependency: dependency(),
            expected: declared.hash,
            actual,
        });
    }
    Ok((bytes, actual))
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

/// The `snapshot-manifest` v1 fingerprint rule: every digest's lowercase hex,
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
/// spellings.
fn is_snapshot_relative(path: &str) -> bool {
    !path.is_empty()
        && !path.starts_with('/')
        && !path.contains('\\')
        && !path.contains(':')
        && !path.contains('\0')
        && path
            .split('/')
            .all(|segment| !segment.is_empty() && segment != "." && segment != "..")
}

/// Why one snapshot could not become usable input.
///
/// Every variant either names the manifest line, the dependency identity and
/// path, or the Cue involved, so the operator-facing message can point at the
/// file to repair instead of reporting that loading failed.
#[derive(Clone, Debug, Eq, PartialEq)]
pub enum SnapshotLoadError {
    /// The source has no `snapshot.txt`.
    MissingManifest,
    /// `snapshot.txt` exists but its bytes could not be read.
    UnreadableManifest { reason: String },
    /// `snapshot.txt` is not UTF-8 text, so no line format can be read.
    ManifestNotUtf8,
    /// The manifest does not follow the documented line format.
    Manifest { line: usize, reason: ManifestReason },
    /// The assembled snapshot violates a domain invariant.
    Snapshot(SnapshotError),
    /// A declared frame or asset is absent from the source.
    MissingDependency { dependency: Dependency },
    /// A declared frame or asset exists but its bytes could not be read.
    UnreadableDependency {
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
            Self::ManifestNotUtf8 => write!(f, "{MANIFEST_PATH} is not UTF-8 text"),
            Self::Manifest { line, reason } => write!(f, "{MANIFEST_PATH} line {line}: {reason}"),
            Self::Snapshot(error) => write!(f, "the snapshot is not usable: {error}"),
            Self::MissingDependency { dependency } => {
                write!(f, "{} at {:?} is missing", dependency.id, dependency.path)
            }
            Self::UnreadableDependency { dependency, reason } => write!(
                f,
                "{} at {:?} cannot be read: {reason}",
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

impl From<ManifestError> for SnapshotLoadError {
    fn from(error: ManifestError) -> Self {
        Self::Manifest {
            line: error.line,
            reason: error.reason,
        }
    }
}

/// Why one manifest line, or the missing end of the manifest, is not the
/// documented format. The line number lives beside it in
/// [`SnapshotLoadError::Manifest`].
#[derive(Clone, Debug, Eq, PartialEq)]
pub enum ManifestReason {
    /// The first line is not the format line.
    UnknownFormat { found: String },
    /// The next record was `found` where `expected` belongs.
    Unexpected {
        expected: &'static str,
        found: String,
    },
    /// The manifest ended before `expected` appeared.
    Missing { expected: &'static str },
    /// The value is not a canonical lowercase UUID v4 identity.
    InvalidId { value: String, error: IdError },
    /// The value is not a canonical lowercase SHA-256 digest.
    InvalidHash {
        value: String,
        error: ContentHashError,
    },
    /// The value is not a plain decimal integer without leading zeros.
    InvalidNumber { value: String },
    /// The value is not one of the values `field` defines.
    InvalidChoice { field: &'static str, value: String },
    /// A title, label, name, or explanation is empty or whitespace only.
    Blank { field: &'static str },
    /// A dependency path is absolute, backslash-separated, or escapes its root.
    UnsafePath { path: String },
    /// The same identity is declared more than once.
    Duplicate { field: &'static str, value: String },
    /// Two dependencies name the same file.
    DuplicatePath { path: String },
}

impl fmt::Display for ManifestReason {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::UnknownFormat { found } => write!(
                f,
                "expected the `{MANIFEST_FORMAT}` format line, found {found:?}"
            ),
            Self::Unexpected { expected, found } => {
                write!(f, "expected {expected}, found {found:?}")
            }
            Self::Missing { expected } => write!(f, "expected {expected}"),
            Self::InvalidId { value, error } => {
                write!(f, "{value:?} is not a canonical identity: {error}")
            }
            Self::InvalidHash { value, error } => {
                write!(f, "{value:?} is not a canonical digest: {error}")
            }
            Self::InvalidNumber { value } => {
                write!(f, "{value:?} is not a plain decimal integer")
            }
            Self::InvalidChoice { field, value } => {
                write!(f, "{value:?} is not a valid {field}")
            }
            Self::Blank { field } => write!(f, "{field} must not be blank"),
            Self::UnsafePath { path } => write!(f, "{path:?} is not a snapshot-relative path"),
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
        }
    }
}

// --- Manifest parsing -------------------------------------------------------

/// One manifest as parsed, before any file is opened.
struct Manifest {
    snapshot: SnapshotId,
    title: String,
    fingerprint: ContentHash,
    themes: Vec<ThemeRef>,
    mass_theme: ThemeId,
    frames: Vec<DependencyDecl<FrameId>>,
    assets: Vec<DependencyDecl<AssetId>>,
    sections: Vec<SectionDecl>,
}

struct DependencyDecl<T> {
    id: T,
    path: String,
    /// Declared byte length; assets only.
    size: Option<u32>,
    hash: ContentHash,
}

struct SectionDecl {
    id: MassSectionId,
    title: String,
    items: Vec<ItemDecl>,
}

struct ItemDecl {
    id: MassItemId,
    title: String,
    kind: MassItemKind,
    theme: Option<ThemeId>,
    auto_play: Option<u32>,
    cues: Vec<CueDecl>,
}

struct CueDecl {
    id: CueId,
    label: String,
    private_context: Option<String>,
    behavior: BehaviorDecl,
}

enum BehaviorDecl {
    Project {
        frame: FrameId,
        duration: Option<u32>,
    },
    OperatorTrigger,
    Hold,
    Wait,
    Unavailable {
        reason: String,
    },
}

/// One line that is not the record the grammar expects at this position.
struct LineError {
    line: usize,
    expected: &'static str,
    found: Option<String>,
}

impl LineError {
    fn into_manifest(self) -> ManifestError {
        ManifestError {
            line: self.line,
            reason: match self.found {
                Some(found) => ManifestReason::Unexpected {
                    expected: self.expected,
                    found,
                },
                None => ManifestReason::Missing {
                    expected: self.expected,
                },
            },
        }
    }

    fn into_frame(self) -> FrameParseError {
        FrameParseError::Line {
            line: self.line,
            reason: match self.found {
                Some(found) => FrameReason::Unexpected {
                    expected: self.expected,
                    found,
                },
                None => FrameReason::Missing {
                    expected: self.expected,
                },
            },
        }
    }
}

struct ManifestError {
    line: usize,
    reason: ManifestReason,
}

/// The manifest's lines, read forward exactly once.
struct Cursor<'a> {
    lines: Vec<&'a str>,
    next: usize,
}

impl<'a> Cursor<'a> {
    fn new(text: &'a str) -> Self {
        Self {
            lines: text.lines().collect(),
            next: 0,
        }
    }

    /// The next line with its 1-based number, consuming it.
    fn take(&mut self) -> Option<(usize, &'a str)> {
        let line = self.lines.get(self.next).copied()?;
        self.next += 1;
        Some((self.next, line))
    }

    /// The next line without consuming it.
    fn peek(&self) -> Option<(usize, &'a str)> {
        self.lines
            .get(self.next)
            .copied()
            .map(|line| (self.next + 1, line))
    }

    fn eof_line(&self) -> usize {
        self.lines.len() + 1
    }
}

/// The key of one line: everything before the first space.
fn key_of(line: &str) -> &str {
    line.split_once(' ').map_or(line, |(key, _)| key)
}

fn take_raw<'a>(
    cursor: &mut Cursor<'a>,
    expected: &'static str,
) -> Result<(usize, &'a str), LineError> {
    cursor.take().ok_or(LineError {
        line: cursor.eof_line(),
        expected,
        found: None,
    })
}

/// Consume the next line, requiring it to be `key` followed by one space.
fn take_prefixed<'a>(
    cursor: &mut Cursor<'a>,
    key: &'static str,
) -> Result<(usize, &'a str), LineError> {
    let (line, text) = take_raw(cursor, key)?;
    let Some(value) = text
        .strip_prefix(key)
        .and_then(|rest| rest.strip_prefix(' '))
    else {
        return Err(LineError {
            line,
            expected: key,
            found: Some(text.to_owned()),
        });
    };
    Ok((line, value))
}

/// Consume the next line, requiring it to start with `prefix`; the value is
/// the raw remainder, so `kind:Lyrics` and `header:` both read without
/// inventing a separator.
fn take_after<'a>(
    cursor: &mut Cursor<'a>,
    prefix: &'static str,
) -> Result<(usize, &'a str), LineError> {
    let (line, text) = take_raw(cursor, prefix)?;
    let Some(value) = text.strip_prefix(prefix) else {
        return Err(LineError {
            line,
            expected: prefix,
            found: Some(text.to_owned()),
        });
    };
    Ok((line, value))
}

fn parse_manifest(text: &str) -> Result<Manifest, ManifestError> {
    let mut cursor = Cursor::new(text);
    let (line, format) =
        take_raw(&mut cursor, MANIFEST_FORMAT).map_err(LineError::into_manifest)?;
    if format != MANIFEST_FORMAT {
        return Err(ManifestError {
            line,
            reason: ManifestReason::UnknownFormat {
                found: format.to_owned(),
            },
        });
    }

    let (_, snapshot) = take_id::<SnapshotId>(&mut cursor, "snapshot")?;
    let (_, title) = take_text(&mut cursor, "title", "title")?;
    let (_, fingerprint) = take_hash(&mut cursor, "fingerprint")?;

    let mut themes = Vec::new();
    while cursor
        .peek()
        .is_some_and(|(_, line)| key_of(line) == "theme")
    {
        themes.push(parse_theme(&mut cursor)?);
    }
    let (_, mass_theme) = take_id::<ThemeId>(&mut cursor, "mass-theme")?;

    let mut paths = BTreeSet::new();
    let frames = parse_dependencies::<FrameId>(
        &mut cursor,
        &mut paths,
        &DependencyKeys {
            record: "frame",
            path: "frame-path",
            size: None,
            hash: "frame-sha256",
            directory: FRAME_DIRECTORY,
        },
    )?;
    let assets = parse_dependencies::<AssetId>(
        &mut cursor,
        &mut paths,
        &DependencyKeys {
            record: "asset",
            path: "asset-path",
            size: Some("asset-size"),
            hash: "asset-sha256",
            directory: ASSET_DIRECTORY,
        },
    )?;

    let mut sections = Vec::new();
    while cursor
        .peek()
        .is_some_and(|(_, line)| key_of(line) == "section")
    {
        sections.push(parse_section(&mut cursor)?);
    }
    if let Some((at, line)) = cursor.peek() {
        return Err(ManifestError {
            line: at,
            reason: ManifestReason::Unexpected {
                expected: "end of manifest",
                found: line.to_owned(),
            },
        });
    }

    Ok(Manifest {
        snapshot,
        title,
        fingerprint,
        themes,
        mass_theme,
        frames,
        assets,
        sections,
    })
}

fn parse_theme(cursor: &mut Cursor<'_>) -> Result<ThemeRef, ManifestError> {
    let (_, id) = take_id::<ThemeId>(cursor, "theme")?;
    let (_, name) = take_text(cursor, "theme-name", "Theme name")?;
    let (at, kind) = take_text(cursor, "theme-kind", "Theme kind")?;
    let kind = match kind.as_str() {
        "Custom" => ThemeKind::Custom,
        "Fallback" => ThemeKind::Fallback,
        _ => {
            return Err(ManifestError {
                line: at,
                reason: ManifestReason::InvalidChoice {
                    field: "Theme kind",
                    value: kind,
                },
            });
        }
    };
    Ok(ThemeRef { id, name, kind })
}

/// The record keys and path directory of one dependency kind.
struct DependencyKeys {
    record: &'static str,
    path: &'static str,
    size: Option<&'static str>,
    hash: &'static str,
    directory: &'static str,
}

/// Parse every consecutive record of one dependency kind, rejecting a repeated
/// identity or a path any earlier dependency (of either kind) already names.
fn parse_dependencies<T>(
    cursor: &mut Cursor<'_>,
    paths: &mut BTreeSet<String>,
    keys: &DependencyKeys,
) -> Result<Vec<DependencyDecl<T>>, ManifestError>
where
    T: FromStr<Err = IdError> + fmt::Display + Ord + Copy,
{
    let mut declared = Vec::new();
    let mut ids = BTreeSet::new();
    while cursor
        .peek()
        .is_some_and(|(_, line)| key_of(line) == keys.record)
    {
        let (at, id) = take_id::<T>(cursor, keys.record)?;
        let (path_at, path) = take_text(cursor, keys.path, "Dependency path")?;
        let under_directory = path
            .strip_prefix(keys.directory)
            .is_some_and(|rest| rest.starts_with('/'));
        if !under_directory || !is_snapshot_relative(&path) {
            return Err(ManifestError {
                line: path_at,
                reason: ManifestReason::UnsafePath { path },
            });
        }
        let size = match keys.size {
            Some(key) => Some(take_number(cursor, key)?.1),
            None => None,
        };
        let (_, hash) = take_hash(cursor, keys.hash)?;
        if !ids.insert(id) {
            return Err(ManifestError {
                line: at,
                reason: ManifestReason::Duplicate {
                    field: keys.record,
                    value: id.to_string(),
                },
            });
        }
        if !paths.insert(path.clone()) {
            return Err(ManifestError {
                line: path_at,
                reason: ManifestReason::DuplicatePath { path },
            });
        }
        declared.push(DependencyDecl {
            id,
            path,
            size,
            hash,
        });
    }
    Ok(declared)
}

fn parse_section(cursor: &mut Cursor<'_>) -> Result<SectionDecl, ManifestError> {
    let (_, id) = take_id::<MassSectionId>(cursor, "section")?;
    let (_, title) = take_text(cursor, "section-title", "Section title")?;
    let mut items = Vec::new();
    while cursor
        .peek()
        .is_some_and(|(_, line)| key_of(line) == "item")
    {
        items.push(parse_item(cursor)?);
    }
    if items.is_empty() {
        // The domain rejects an empty section too, but naming the missing
        // record here points at the line the author must add.
        return Err(ManifestError {
            line: cursor.peek().map_or(cursor.eof_line(), |(line, _)| line),
            reason: ManifestReason::Missing { expected: "item" },
        });
    }
    Ok(SectionDecl { id, title, items })
}

fn parse_item(cursor: &mut Cursor<'_>) -> Result<ItemDecl, ManifestError> {
    let (_, id) = take_id::<MassItemId>(cursor, "item")?;
    let (_, title) = take_text(cursor, "item-title", "Mass item title")?;
    let (at, kind) = take_text(cursor, "item-kind", "Mass item kind")?;
    let kind = match kind.as_str() {
        "Song" => MassItemKind::Song,
        "Reading" => MassItemKind::Reading,
        "Psalm" => MassItemKind::Psalm,
        "ResponseSequence" => MassItemKind::ResponseSequence,
        "GeneralPresentation" => MassItemKind::GeneralPresentation,
        "Rite" => MassItemKind::Rite,
        _ => {
            return Err(ManifestError {
                line: at,
                reason: ManifestReason::InvalidChoice {
                    field: "Mass item kind",
                    value: kind,
                },
            });
        }
    };
    let theme = if cursor
        .peek()
        .is_some_and(|(_, line)| key_of(line) == "item-theme")
    {
        Some(take_id::<ThemeId>(cursor, "item-theme")?.1)
    } else {
        None
    };
    let auto_play = if cursor
        .peek()
        .is_some_and(|(_, line)| key_of(line) == "item-auto-play")
    {
        Some(take_number(cursor, "item-auto-play")?.1)
    } else {
        None
    };
    let mut cues = Vec::new();
    while cursor.peek().is_some_and(|(_, line)| key_of(line) == "cue") {
        cues.push(parse_cue(cursor)?);
    }
    if cues.is_empty() {
        return Err(ManifestError {
            line: cursor.peek().map_or(cursor.eof_line(), |(line, _)| line),
            reason: ManifestReason::Missing { expected: "cue" },
        });
    }
    Ok(ItemDecl {
        id,
        title,
        kind,
        theme,
        auto_play,
        cues,
    })
}

fn parse_cue(cursor: &mut Cursor<'_>) -> Result<CueDecl, ManifestError> {
    let (_, id) = take_id::<CueId>(cursor, "cue")?;
    let (_, label) = take_text(cursor, "cue-label", "Cue label")?;
    let private_context = if cursor
        .peek()
        .is_some_and(|(_, line)| key_of(line) == "cue-private-context")
    {
        Some(take_text(cursor, "cue-private-context", "Cue private context")?.1)
    } else {
        None
    };
    let (at, behavior) = take_text(cursor, "cue-behavior", "Cue behavior")?;
    let behavior = match behavior.as_str() {
        "Project" => {
            let frame = take_id::<FrameId>(cursor, "cue-frame")?.1;
            let duration = if cursor
                .peek()
                .is_some_and(|(_, line)| key_of(line) == "cue-duration")
            {
                Some(take_number(cursor, "cue-duration")?.1)
            } else {
                None
            };
            BehaviorDecl::Project { frame, duration }
        }
        "OperatorTrigger" => BehaviorDecl::OperatorTrigger,
        "Hold" => BehaviorDecl::Hold,
        "Wait" => BehaviorDecl::Wait,
        "Unavailable" => BehaviorDecl::Unavailable {
            reason: take_text(cursor, "cue-reason", "Unavailable reason")?.1,
        },
        _ => {
            return Err(ManifestError {
                line: at,
                reason: ManifestReason::InvalidChoice {
                    field: "Cue behavior",
                    value: behavior,
                },
            });
        }
    };
    Ok(CueDecl {
        id,
        label,
        private_context,
        behavior,
    })
}

fn take_id<T>(cursor: &mut Cursor<'_>, key: &'static str) -> Result<(usize, T), ManifestError>
where
    T: FromStr<Err = IdError>,
{
    let (at, value) = take_prefixed(cursor, key).map_err(LineError::into_manifest)?;
    let id = T::from_str(value).map_err(|error| ManifestError {
        line: at,
        reason: ManifestReason::InvalidId {
            value: value.to_owned(),
            error,
        },
    })?;
    Ok((at, id))
}

fn take_hash(
    cursor: &mut Cursor<'_>,
    key: &'static str,
) -> Result<(usize, ContentHash), ManifestError> {
    let (at, value) = take_prefixed(cursor, key).map_err(LineError::into_manifest)?;
    let hash = ContentHash::parse(value).map_err(|error| ManifestError {
        line: at,
        reason: ManifestReason::InvalidHash {
            value: value.to_owned(),
            error,
        },
    })?;
    Ok((at, hash))
}

fn take_text(
    cursor: &mut Cursor<'_>,
    key: &'static str,
    field: &'static str,
) -> Result<(usize, String), ManifestError> {
    let (at, value) = take_prefixed(cursor, key).map_err(LineError::into_manifest)?;
    if value.trim().is_empty() {
        return Err(ManifestError {
            line: at,
            reason: ManifestReason::Blank { field },
        });
    }
    Ok((at, value.to_owned()))
}

fn take_number(cursor: &mut Cursor<'_>, key: &'static str) -> Result<(usize, u32), ManifestError> {
    let (at, value) = take_prefixed(cursor, key).map_err(LineError::into_manifest)?;
    strict_u32(value)
        .map(|number| (at, number))
        .ok_or_else(|| ManifestError {
            line: at,
            reason: ManifestReason::InvalidNumber {
                value: value.to_owned(),
            },
        })
}

/// A plain decimal integer: digits only, no sign, no whitespace, and no
/// leading zero, so one number has one spelling.
fn strict_u32(text: &str) -> Option<u32> {
    let canonical = !text.is_empty()
        && (text.len() == 1 || !text.starts_with('0'))
        && text.bytes().all(|byte| byte.is_ascii_digit());
    if canonical { text.parse().ok() } else { None }
}

fn build_snapshot(manifest: &Manifest) -> Result<PresentationSnapshot, SnapshotError> {
    let mut sections = Vec::with_capacity(manifest.sections.len());
    for section in &manifest.sections {
        let mut items = Vec::with_capacity(section.items.len());
        for item in &section.items {
            let mut cues = Vec::with_capacity(item.cues.len());
            for cue in &item.cues {
                let behavior = match &cue.behavior {
                    BehaviorDecl::Project { frame, duration } => CueBehavior::Project {
                        frame: *frame,
                        duration_seconds: *duration,
                    },
                    BehaviorDecl::OperatorTrigger => CueBehavior::OperatorTrigger,
                    BehaviorDecl::Hold => CueBehavior::Hold,
                    BehaviorDecl::Wait => CueBehavior::Wait,
                    BehaviorDecl::Unavailable { reason } => CueBehavior::Unavailable {
                        reason: reason.clone(),
                    },
                };
                cues.push(Cue::new(
                    cue.id,
                    cue.label.clone(),
                    cue.private_context.clone(),
                    behavior,
                )?);
            }
            items.push(MassItem::new(
                item.id,
                item.title.clone(),
                item.kind,
                item.theme,
                item.auto_play.map(|seconds| AutoPlay {
                    default_seconds: seconds,
                }),
                cues,
            )?);
        }
        sections.push(MassSection::new(section.id, section.title.clone(), items)?);
    }
    PresentationSnapshot::new(
        manifest.snapshot,
        manifest.title.clone(),
        manifest.mass_theme,
        manifest.themes.clone(),
        sections,
    )
}

// --- Frame parsing ----------------------------------------------------------

/// Parse one canonical `ordo-fixture/1` frame file.
///
/// This is deliberately the same grammar the golden-frame harness reads in
/// `crates/rendering/tests/golden.rs`: the harness proves composition stability,
/// and the reader proves a snapshot's frames are exactly that content. The
/// parser does not render; [`read_snapshot`] does that once the digest matched.
fn parse_frame(text: &str) -> Result<TemplateContent, FrameParseError> {
    let mut cursor = Cursor::new(text);
    let (line, format) = take_raw(&mut cursor, FRAME_FORMAT).map_err(LineError::into_frame)?;
    if format != FRAME_FORMAT {
        return Err(FrameParseError::Line {
            line,
            reason: FrameReason::UnknownFormat {
                found: format.to_owned(),
            },
        });
    }

    let (at, kind) = take_after(&mut cursor, "kind:").map_err(LineError::into_frame)?;
    let kind = match kind {
        "Lyrics" => TemplateKind::Lyrics,
        "Response" => TemplateKind::Response,
        "ReadingIdentification" => TemplateKind::ReadingIdentification,
        "ReadingPassage" => TemplateKind::ReadingPassage,
        "Psalm" => TemplateKind::Psalm,
        "SongIdentification" => TemplateKind::SongIdentification,
        "CelebrationTitle" => TemplateKind::CelebrationTitle,
        "GeneralText" => TemplateKind::GeneralText,
        "GeneralImage" => TemplateKind::GeneralImage,
        "Black" => TemplateKind::Black,
        _ => {
            return Err(FrameParseError::Line {
                line: at,
                reason: FrameReason::InvalidKind {
                    value: kind.to_owned(),
                },
            });
        }
    };

    let (at, header) = take_raw(&mut cursor, "header").map_err(LineError::into_frame)?;
    let header = if header == "header-absent" {
        None
    } else if let Some(text) = header.strip_prefix("header:") {
        Some(text.to_owned())
    } else {
        return Err(FrameParseError::Line {
            line: at,
            reason: FrameReason::Unexpected {
                expected: "`header:<text>` or `header-absent`",
                found: header.to_owned(),
            },
        });
    };

    let body = take_lines(&mut cursor, "body-lines:")?;

    let (at, footer) = take_raw(&mut cursor, "footer").map_err(LineError::into_frame)?;
    let footer = if footer == "footer-absent" {
        None
    } else if let Some(text) = footer.strip_prefix("footer-lines:") {
        Some(take_counted_lines(&mut cursor, text, "footer-lines", at)?.join("\n"))
    } else {
        return Err(FrameParseError::Line {
            line: at,
            reason: FrameReason::Unexpected {
                expected: "`footer-lines:<n>` or `footer-absent`",
                found: footer.to_owned(),
            },
        });
    };

    let (at, image) = take_raw(&mut cursor, "image").map_err(LineError::into_frame)?;
    let image = if image == "image-absent" {
        None
    } else if let Some(id) = image.strip_prefix("image:") {
        if id.is_empty() {
            return Err(FrameParseError::Line {
                line: at,
                reason: FrameReason::Unexpected {
                    expected: "a non-empty `image:<id>`",
                    found: image.to_owned(),
                },
            });
        }
        let (size_at, size) =
            take_after(&mut cursor, "image-size:").map_err(LineError::into_frame)?;
        let (natural_width, natural_height) =
            parse_image_size(size).ok_or_else(|| FrameParseError::Line {
                line: size_at,
                reason: FrameReason::InvalidSize {
                    value: size.to_owned(),
                },
            })?;
        Some(ForegroundImage {
            id: id.to_owned(),
            natural_width,
            natural_height,
        })
    } else {
        return Err(FrameParseError::Line {
            line: at,
            reason: FrameReason::Unexpected {
                expected: "`image:<id>` or `image-absent`",
                found: image.to_owned(),
            },
        });
    };

    if let Some((line, _)) = cursor.peek() {
        return Err(FrameParseError::Line {
            line,
            reason: FrameReason::Trailing,
        });
    }

    Ok(TemplateContent {
        kind,
        header,
        body,
        footer,
        image,
    })
}

fn take_lines(cursor: &mut Cursor<'_>, prefix: &'static str) -> Result<String, FrameParseError> {
    let (at, count) = take_after(cursor, prefix).map_err(LineError::into_frame)?;
    take_counted_lines(cursor, count, prefix, at).map(|lines| lines.join("\n"))
}

fn take_counted_lines(
    cursor: &mut Cursor<'_>,
    count: &str,
    key: &'static str,
    at: usize,
) -> Result<Vec<String>, FrameParseError> {
    let count = strict_u32(count).ok_or_else(|| FrameParseError::Line {
        line: at,
        reason: FrameReason::InvalidCount {
            value: count.to_owned(),
        },
    })? as usize;
    let mut lines = Vec::with_capacity(count);
    for _ in 0..count {
        let (_, line) = take_raw(cursor, key).map_err(LineError::into_frame)?;
        lines.push(line.to_owned());
    }
    Ok(lines)
}

fn parse_image_size(text: &str) -> Option<(u32, u32)> {
    let (width, height) = text.split_once('x')?;
    Some((strict_u32(width)?, strict_u32(height)?))
}
#[cfg(test)]
mod tests {
    use std::fmt::Write as _;

    use super::*;

    fn uuid(n: u16) -> String {
        format!("00000000-0000-4000-8000-{n:012x}")
    }

    fn snapshot_id() -> SnapshotId {
        uuid(1).parse().unwrap()
    }

    fn custom_theme() -> ThemeId {
        uuid(0x101).parse().unwrap()
    }

    fn fallback_theme() -> ThemeId {
        uuid(0x102).parse().unwrap()
    }

    fn frame_id() -> FrameId {
        uuid(0x201).parse().unwrap()
    }

    fn image_frame_id() -> FrameId {
        uuid(0x202).parse().unwrap()
    }

    fn asset_id() -> AssetId {
        uuid(0x301).parse().unwrap()
    }

    fn section_id() -> MassSectionId {
        uuid(0x401).parse().unwrap()
    }

    fn item_id() -> MassItemId {
        uuid(0x501).parse().unwrap()
    }

    fn cue_id() -> CueId {
        uuid(0x601).parse().unwrap()
    }

    fn text_frame(body: &str) -> String {
        format!(
            "ordo-fixture/1\nkind:GeneralText\nheader-absent\nbody-lines:1\n{body}\nfooter-absent\nimage-absent\n"
        )
    }

    fn image_frame(asset: &str) -> String {
        format!(
            "ordo-fixture/1\nkind:GeneralImage\nheader:Image header\nbody-lines:0\nfooter-lines:1\nFixture caption\nimage:{asset}\nimage-size:2x2\n"
        )
    }

    /// An in-memory source: these tests never touch the filesystem.
    #[derive(Default)]
    struct MemorySource {
        files: BTreeMap<String, Vec<u8>>,
    }

    impl MemorySource {
        fn insert(&mut self, path: &str, bytes: impl Into<Vec<u8>>) {
            self.files.insert(path.to_owned(), bytes.into());
        }
    }

    impl SnapshotSource for MemorySource {
        fn read(&self, path: &str) -> Result<Vec<u8>, SourceError> {
            self.files.get(path).cloned().ok_or(SourceError::NotFound)
        }
    }

    #[derive(Clone)]
    struct FrameSpec {
        id: FrameId,
        path: String,
        text: String,
    }

    #[derive(Clone)]
    struct AssetSpec {
        id: AssetId,
        path: String,
        bytes: Vec<u8>,
    }

    #[derive(Clone)]
    struct CueSpec {
        id: CueId,
        label: String,
        private_context: Option<String>,
        behavior: CueBehavior,
    }

    /// A Cue with private context, valid for every behavior.
    fn operator_cue(n: u16, label: &str, behavior: CueBehavior) -> CueSpec {
        CueSpec {
            id: uuid(n).parse().unwrap(),
            label: label.to_owned(),
            private_context: Some("Operator note".to_owned()),
            behavior,
        }
    }

    #[derive(Clone)]
    struct ItemSpec {
        id: MassItemId,
        title: String,
        kind: MassItemKind,
        theme: Option<ThemeId>,
        auto_play: Option<u32>,
        cues: Vec<CueSpec>,
    }

    #[derive(Clone)]
    struct SectionSpec {
        id: MassSectionId,
        title: String,
        items: Vec<ItemSpec>,
    }

    /// A whole manifest as a value, so each test mutates the one fact it is
    /// about and the reader still sees a self-consistent snapshot. Hashes and
    /// the fingerprint are derived, never typed by hand.
    #[derive(Clone)]
    struct Spec {
        snapshot: SnapshotId,
        title: String,
        themes: Vec<ThemeRef>,
        mass_theme: ThemeId,
        frames: Vec<FrameSpec>,
        assets: Vec<AssetSpec>,
        sections: Vec<SectionSpec>,
    }

    impl Spec {
        /// Two items covering every Cue behavior, a text frame, and an image
        /// frame that uses the one declared asset.
        fn minimal() -> Self {
            Self {
                snapshot: snapshot_id(),
                title: "Minimal".to_owned(),
                themes: vec![
                    ThemeRef {
                        id: custom_theme(),
                        name: "Parish Paschal".to_owned(),
                        kind: ThemeKind::Custom,
                    },
                    ThemeRef {
                        id: fallback_theme(),
                        name: "Ordo fallback".to_owned(),
                        kind: ThemeKind::Fallback,
                    },
                ],
                mass_theme: custom_theme(),
                frames: vec![
                    FrameSpec {
                        id: frame_id(),
                        path: "frames/one.txt".to_owned(),
                        text: text_frame("Hello from the fixture"),
                    },
                    FrameSpec {
                        id: image_frame_id(),
                        path: "frames/image.txt".to_owned(),
                        text: image_frame(&asset_id().to_string()),
                    },
                ],
                assets: vec![AssetSpec {
                    id: asset_id(),
                    path: "assets/pixel.png".to_owned(),
                    bytes: vec![0x89, b'P', b'N', b'G'],
                }],
                sections: vec![SectionSpec {
                    id: section_id(),
                    title: "Before Mass".to_owned(),
                    items: vec![
                        ItemSpec {
                            id: item_id(),
                            title: "Announcements".to_owned(),
                            kind: MassItemKind::GeneralPresentation,
                            theme: None,
                            auto_play: Some(8),
                            cues: vec![
                                CueSpec {
                                    id: cue_id(),
                                    label: "Slide one".to_owned(),
                                    private_context: Some("Said only to the operator".to_owned()),
                                    behavior: CueBehavior::Project {
                                        frame: frame_id(),
                                        duration_seconds: Some(3),
                                    },
                                },
                                operator_cue(
                                    0x602,
                                    "Image slide",
                                    CueBehavior::Project {
                                        frame: image_frame_id(),
                                        duration_seconds: None,
                                    },
                                ),
                            ],
                        },
                        ItemSpec {
                            id: uuid(0x502).parse().unwrap(),
                            title: "Rite".to_owned(),
                            kind: MassItemKind::Rite,
                            theme: Some(fallback_theme()),
                            auto_play: None,
                            cues: vec![
                                operator_cue(0x603, "Trigger", CueBehavior::OperatorTrigger),
                                operator_cue(0x604, "Hold", CueBehavior::Hold),
                                operator_cue(0x605, "Wait", CueBehavior::Wait),
                                operator_cue(
                                    0x606,
                                    "Missing",
                                    CueBehavior::Unavailable {
                                        reason: "Image could not be captured".to_owned(),
                                    },
                                ),
                            ],
                        },
                    ],
                }],
            }
        }

        fn hashes(&self) -> impl Iterator<Item = ContentHash> + '_ {
            let frames = self
                .frames
                .iter()
                .map(|frame| hash_bytes(frame.text.as_bytes()));
            let assets = self.assets.iter().map(|asset| hash_bytes(&asset.bytes));
            frames.chain(assets)
        }

        fn fingerprint(&self) -> ContentHash {
            fingerprint_of(self.hashes())
        }

        fn manifest(&self) -> String {
            let mut out = String::new();
            writeln!(out, "{MANIFEST_FORMAT}").unwrap();
            writeln!(out, "snapshot {}", self.snapshot).unwrap();
            writeln!(out, "title {}", self.title).unwrap();
            writeln!(out, "fingerprint {}", self.fingerprint()).unwrap();
            for theme in &self.themes {
                writeln!(out, "theme {}", theme.id).unwrap();
                writeln!(out, "theme-name {}", theme.name).unwrap();
                let kind = match theme.kind {
                    ThemeKind::Custom => "Custom",
                    ThemeKind::Fallback => "Fallback",
                };
                writeln!(out, "theme-kind {kind}").unwrap();
            }
            writeln!(out, "mass-theme {}", self.mass_theme).unwrap();
            for frame in &self.frames {
                writeln!(out, "frame {}", frame.id).unwrap();
                writeln!(out, "frame-path {}", frame.path).unwrap();
                writeln!(out, "frame-sha256 {}", hash_bytes(frame.text.as_bytes())).unwrap();
            }
            for asset in &self.assets {
                writeln!(out, "asset {}", asset.id).unwrap();
                writeln!(out, "asset-path {}", asset.path).unwrap();
                writeln!(out, "asset-size {}", asset.bytes.len()).unwrap();
                writeln!(out, "asset-sha256 {}", hash_bytes(&asset.bytes)).unwrap();
            }
            for section in &self.sections {
                writeln!(out, "section {}", section.id).unwrap();
                writeln!(out, "section-title {}", section.title).unwrap();
                for item in &section.items {
                    writeln!(out, "item {}", item.id).unwrap();
                    writeln!(out, "item-title {}", item.title).unwrap();
                    let kind = match item.kind {
                        MassItemKind::Song => "Song",
                        MassItemKind::Reading => "Reading",
                        MassItemKind::Psalm => "Psalm",
                        MassItemKind::ResponseSequence => "ResponseSequence",
                        MassItemKind::GeneralPresentation => "GeneralPresentation",
                        MassItemKind::Rite => "Rite",
                    };
                    writeln!(out, "item-kind {kind}").unwrap();
                    if let Some(theme) = item.theme {
                        writeln!(out, "item-theme {theme}").unwrap();
                    }
                    if let Some(seconds) = item.auto_play {
                        writeln!(out, "item-auto-play {seconds}").unwrap();
                    }
                    for cue in &item.cues {
                        writeln!(out, "cue {}", cue.id).unwrap();
                        writeln!(out, "cue-label {}", cue.label).unwrap();
                        if let Some(context) = &cue.private_context {
                            writeln!(out, "cue-private-context {context}").unwrap();
                        }
                        match &cue.behavior {
                            CueBehavior::Project {
                                frame,
                                duration_seconds,
                            } => {
                                writeln!(out, "cue-behavior Project").unwrap();
                                writeln!(out, "cue-frame {frame}").unwrap();
                                if let Some(seconds) = duration_seconds {
                                    writeln!(out, "cue-duration {seconds}").unwrap();
                                }
                            }
                            CueBehavior::OperatorTrigger => {
                                writeln!(out, "cue-behavior OperatorTrigger").unwrap();
                            }
                            CueBehavior::Hold => writeln!(out, "cue-behavior Hold").unwrap(),
                            CueBehavior::Wait => writeln!(out, "cue-behavior Wait").unwrap(),
                            CueBehavior::Unavailable { reason } => {
                                writeln!(out, "cue-behavior Unavailable").unwrap();
                                writeln!(out, "cue-reason {reason}").unwrap();
                            }
                        }
                    }
                }
            }
            out
        }

        fn source(&self) -> MemorySource {
            let mut source = MemorySource::default();
            source.insert(MANIFEST_PATH, self.manifest());
            for frame in &self.frames {
                source.insert(&frame.path, frame.text.clone());
            }
            for asset in &self.assets {
                source.insert(&asset.path, asset.bytes.clone());
            }
            source
        }

        fn load(&self) -> Result<LoadedSnapshot, SnapshotLoadError> {
            read_snapshot(&self.source())
        }
    }

    #[test]
    fn a_minimal_snapshot_loads_every_verified_value() {
        let spec = Spec::minimal();
        let loaded = spec.load().unwrap();

        let snapshot = loaded.snapshot();
        assert_eq!(snapshot.id(), snapshot_id());
        assert_eq!(snapshot.title(), "Minimal");
        assert_eq!(snapshot.item_count(), 2);
        assert_eq!(snapshot.cue_count(), 6);
        assert_eq!(snapshot.sections().len(), 1);
        assert_eq!(snapshot.sections()[0].title(), "Before Mass");
        assert_eq!(snapshot.mass_theme().id, custom_theme());

        let item = &snapshot.sections()[0].items()[0];
        assert_eq!(item.kind(), MassItemKind::GeneralPresentation);
        assert_eq!(item.auto_play(), Some(AutoPlay { default_seconds: 8 }));
        assert_eq!(snapshot.effective_theme(0).unwrap().id, custom_theme());

        let cue = snapshot.cue(0).unwrap();
        assert_eq!(cue.label(), "Slide one");
        assert_eq!(cue.private_context(), Some("Said only to the operator"));
        assert_eq!(
            cue.program_effect(),
            domain::presentation::ProgramEffect::Frame(frame_id())
        );
        assert_eq!(
            cue.behavior(),
            &CueBehavior::Project {
                frame: frame_id(),
                duration_seconds: Some(3),
            }
        );

        assert_eq!(
            loaded.frame(frame_id()).unwrap().body,
            "Hello from the fixture"
        );
        assert_eq!(
            loaded.frame_hash(frame_id()),
            Some(hash_bytes(spec.frames[0].text.as_bytes()))
        );
        assert_eq!(
            loaded.frame_ids().collect::<Vec<_>>(),
            vec![frame_id(), image_frame_id()]
        );
        assert_eq!(
            loaded.asset(asset_id()),
            Some(spec.assets[0].bytes.as_slice())
        );
        assert_eq!(
            loaded.asset_hash(asset_id()),
            Some(hash_bytes(&spec.assets[0].bytes))
        );
        assert_eq!(loaded.asset_ids().collect::<Vec<_>>(), vec![asset_id()]);
        assert_eq!(loaded.fingerprint(), spec.fingerprint());
    }

    #[test]
    fn the_fingerprint_follows_the_contract_rule() {
        // No dependencies: the SHA-256 of an empty digest stream.
        assert_eq!(
            fingerprint_of([]).to_string(),
            "e3b0c44298fc1c149afbf4c8996fb92427ae41e4649b934ca495991b7852b855"
        );

        let one = hash_bytes(b"one");
        let two = hash_bytes(b"two");
        let mut sorted = vec![one.to_string(), two.to_string()];
        sorted.sort_unstable();
        let mut expected = Sha256::new();
        for hex in &sorted {
            expected.update(hex.as_bytes());
            expected.update(b"\n");
        }
        assert_eq!(
            fingerprint_of([two, one]),
            fingerprint_of([one, two]),
            "declaration order never changes the fingerprint"
        );
        assert_eq!(
            fingerprint_of([one, two]),
            digest_bytes(expected.finalize())
        );
        assert_ne!(
            fingerprint_of([one, one]),
            fingerprint_of([one]),
            "a second declaration of the same bytes is still a second line"
        );
    }

    #[test]
    fn the_manifest_must_start_with_its_format_line() {
        let spec = Spec::minimal();
        let manifest = spec
            .manifest()
            .replacen(MANIFEST_FORMAT, "ordo-fixture/1", 1);
        let mut source = spec.source();
        source.insert(MANIFEST_PATH, manifest);

        assert_eq!(
            read_snapshot(&source).unwrap_err(),
            SnapshotLoadError::Manifest {
                line: 1,
                reason: ManifestReason::UnknownFormat {
                    found: "ordo-fixture/1".to_owned(),
                },
            }
        );
    }

    #[test]
    fn out_of_order_records_name_their_line() {
        let spec = Spec::minimal();
        let manifest = spec
            .manifest()
            .replace("title Minimal", "themes-are-not-here");
        let mut source = spec.source();
        source.insert(MANIFEST_PATH, manifest);

        assert_eq!(
            read_snapshot(&source).unwrap_err(),
            SnapshotLoadError::Manifest {
                line: 3,
                reason: ManifestReason::Unexpected {
                    expected: "title",
                    found: "themes-are-not-here".to_owned(),
                },
            }
        );
    }

    #[test]
    fn non_canonical_identities_and_digests_are_rejected() {
        let spec = Spec::minimal();
        let manifest =
            spec.manifest()
                .replacen(&format!("snapshot {}", snapshot_id()), "snapshot 0", 1);
        let mut source = spec.source();
        source.insert(MANIFEST_PATH, manifest);
        assert!(matches!(
            read_snapshot(&source).unwrap_err(),
            SnapshotLoadError::Manifest {
                line: 2,
                reason: ManifestReason::InvalidId { .. },
            }
        ));

        let spec = Spec::minimal();
        let manifest = spec.manifest().replacen(
            &spec.fingerprint().to_string(),
            &spec.fingerprint().to_string().to_uppercase(),
            1,
        );
        let mut source = spec.source();
        source.insert(MANIFEST_PATH, manifest);
        assert!(matches!(
            read_snapshot(&source).unwrap_err(),
            SnapshotLoadError::Manifest {
                line: 4,
                reason: ManifestReason::InvalidHash {
                    error: ContentHashError::NotLowercaseHex,
                    ..
                },
            }
        ));
    }

    #[test]
    fn blank_text_is_rejected_rather_than_forwarded() {
        let spec = Spec::minimal();
        let manifest = spec.manifest().replace("title Minimal", "title   ");
        let mut source = spec.source();
        source.insert(MANIFEST_PATH, manifest);

        assert_eq!(
            read_snapshot(&source).unwrap_err(),
            SnapshotLoadError::Manifest {
                line: 3,
                reason: ManifestReason::Blank { field: "title" },
            }
        );
    }

    #[test]
    fn dependency_paths_cannot_leave_the_snapshot_root() {
        for unsafe_path in [
            "../escape.txt",
            "/absolute.txt",
            "frames\\windows.txt",
            "C:/drive.txt",
            "frames//double.txt",
            "frames/./dot.txt",
            "frames/",
            "assets/not-a-frame.txt",
            "other/one.txt",
        ] {
            let mut spec = Spec::minimal();
            spec.frames[0].path = unsafe_path.to_owned();
            assert!(
                matches!(
                    spec.load().unwrap_err(),
                    SnapshotLoadError::Manifest {
                        reason: ManifestReason::UnsafePath { .. },
                        ..
                    }
                ),
                "{unsafe_path:?} must be refused"
            );
        }
    }

    #[test]
    fn duplicate_frame_ids_and_shared_paths_are_rejected() {
        let mut duplicate_id = Spec::minimal();
        duplicate_id.frames.push(FrameSpec {
            id: frame_id(),
            path: "frames/two.txt".to_owned(),
            text: text_frame("Another"),
        });
        assert!(matches!(
            duplicate_id.load().unwrap_err(),
            SnapshotLoadError::Manifest {
                reason: ManifestReason::Duplicate { field: "frame", .. },
                ..
            }
        ));

        let mut shared_path = Spec::minimal();
        shared_path.frames.push(FrameSpec {
            id: uuid(0x2f0).parse().unwrap(),
            path: "frames/one.txt".to_owned(),
            text: text_frame("Another"),
        });
        assert!(matches!(
            shared_path.load().unwrap_err(),
            SnapshotLoadError::Manifest {
                reason: ManifestReason::DuplicatePath { .. },
                ..
            }
        ));
    }

    #[test]
    fn a_missing_dependency_names_the_identity_and_path() {
        let spec = Spec::minimal();
        let mut source = spec.source();
        source.files.remove("frames/one.txt");

        assert_eq!(
            read_snapshot(&source).unwrap_err(),
            SnapshotLoadError::MissingDependency {
                dependency: Dependency {
                    id: DependencyId::Frame(frame_id()),
                    path: "frames/one.txt".to_owned(),
                },
            }
        );
    }

    #[test]
    fn a_mismatched_dependency_names_both_digests() {
        let spec = Spec::minimal();
        let tampered = text_frame("Tampered");
        let mut source = spec.source();
        source.insert("frames/one.txt", tampered.clone());

        assert_eq!(
            read_snapshot(&source).unwrap_err(),
            SnapshotLoadError::HashMismatch {
                dependency: Dependency {
                    id: DependencyId::Frame(frame_id()),
                    path: "frames/one.txt".to_owned(),
                },
                expected: hash_bytes(spec.frames[0].text.as_bytes()),
                actual: hash_bytes(tampered.as_bytes()),
            }
        );
    }

    #[test]
    fn a_tampered_asset_is_rejected_before_any_reference_check() {
        let spec = Spec::minimal();
        let mut source = spec.source();
        source.insert("assets/pixel.png", vec![0xff]);

        assert!(matches!(
            read_snapshot(&source).unwrap_err(),
            SnapshotLoadError::HashMismatch {
                dependency: Dependency {
                    id: DependencyId::Asset(_),
                    ..
                },
                ..
            }
        ));
    }

    #[test]
    fn a_manifest_without_a_fingerprint_match_is_rejected() {
        let spec = Spec::minimal();
        let other = hash_bytes(b"other");
        let manifest =
            spec.manifest()
                .replacen(&spec.fingerprint().to_string(), &other.to_string(), 1);
        let mut source = spec.source();
        source.insert(MANIFEST_PATH, manifest);

        assert_eq!(
            read_snapshot(&source).unwrap_err(),
            SnapshotLoadError::FingerprintMismatch {
                declared: other,
                actual: spec.fingerprint(),
            }
        );
    }

    #[test]
    fn a_frame_that_is_not_canonical_names_the_frame_and_line() {
        let mut spec = Spec::minimal();
        spec.frames[0].text =
            "ordo-fixture/1\nkind:NotAKind\nheader-absent\nbody-lines:0\nfooter-absent\nimage-absent\n"
                .to_owned();

        assert_eq!(
            spec.load().unwrap_err(),
            SnapshotLoadError::InvalidFrame {
                frame: frame_id(),
                path: "frames/one.txt".to_owned(),
                error: FrameParseError::Line {
                    line: 2,
                    reason: FrameReason::InvalidKind {
                        value: "NotAKind".to_owned(),
                    },
                },
            }
        );
    }

    #[test]
    fn a_frame_that_cannot_compose_is_rejected_at_load() {
        let mut spec = Spec::minimal();
        // A General Image frame without a foreground element parses but cannot
        // compose; the reader must catch it here, not on the display path.
        spec.frames[0].text =
            "ordo-fixture/1\nkind:GeneralImage\nheader-absent\nbody-lines:0\nfooter-absent\nimage-absent\n"
                .to_owned();

        assert_eq!(
            spec.load().unwrap_err(),
            SnapshotLoadError::UnrenderableFrame {
                frame: frame_id(),
                path: "frames/one.txt".to_owned(),
                error: RenderError::MissingForegroundImage,
            }
        );
    }

    #[test]
    fn a_project_cue_may_not_name_an_undeclared_frame() {
        let mut spec = Spec::minimal();
        let ghost: FrameId = uuid(0x299).parse().unwrap();
        spec.sections[0].items[0].cues[0].behavior = CueBehavior::Project {
            frame: ghost,
            duration_seconds: None,
        };

        assert_eq!(
            spec.load().unwrap_err(),
            SnapshotLoadError::UndeclaredFrame {
                cue: cue_id(),
                frame: ghost,
            }
        );
    }

    #[test]
    fn a_declared_frame_no_cue_shows_is_rejected() {
        let mut spec = Spec::minimal();
        let ghost: FrameId = uuid(0x299).parse().unwrap();
        spec.frames.push(FrameSpec {
            id: ghost,
            path: "frames/two.txt".to_owned(),
            text: text_frame("Unused"),
        });

        assert_eq!(
            spec.load().unwrap_err(),
            SnapshotLoadError::UnusedFrame { frame: ghost }
        );
    }

    #[test]
    fn a_declared_asset_no_frame_uses_is_rejected() {
        let mut spec = Spec::minimal();
        spec.frames[1].text = text_frame("No image any more");

        assert_eq!(
            spec.load().unwrap_err(),
            SnapshotLoadError::UnusedAsset { asset: asset_id() }
        );
    }

    #[test]
    fn an_asset_must_have_its_declared_size() {
        let spec = Spec::minimal();
        let manifest = spec.manifest().replace("asset-size 4", "asset-size 5");
        let mut source = spec.source();
        source.insert(MANIFEST_PATH, manifest);

        assert_eq!(
            read_snapshot(&source).unwrap_err(),
            SnapshotLoadError::WrongAssetSize {
                asset: asset_id(),
                path: "assets/pixel.png".to_owned(),
                declared: 5,
                actual: 4,
            }
        );
    }

    #[test]
    fn an_image_frame_must_name_a_declared_asset() {
        let mut spec = Spec::minimal();
        let undeclared = uuid(0x399);
        spec.frames[1].text = image_frame(&undeclared);

        assert_eq!(
            spec.load().unwrap_err(),
            SnapshotLoadError::UndeclaredAsset {
                frame: image_frame_id(),
                asset: undeclared,
            }
        );
    }

    #[test]
    fn domain_rejections_surface_as_the_snapshot_error() {
        let mut spec = Spec::minimal();
        // An operator trigger without the private phrase is uninspectable, so
        // the domain rejects it. The reader must forward that rule rather than
        // inventing its own.
        spec.frames.clear();
        spec.sections[0].items[0].cues[0] = CueSpec {
            id: cue_id(),
            label: "The Lord be with you".to_owned(),
            private_context: None,
            behavior: CueBehavior::OperatorTrigger,
        };

        assert_eq!(
            spec.load().unwrap_err(),
            SnapshotLoadError::Snapshot(SnapshotError::MissingPrivateContext { cue: cue_id() })
        );
    }

    #[test]
    fn the_source_sees_the_manifest_first_and_only_declared_paths() {
        struct RecordingSource {
            inner: MemorySource,
            read: std::cell::RefCell<Vec<String>>,
        }

        impl SnapshotSource for RecordingSource {
            fn read(&self, path: &str) -> Result<Vec<u8>, SourceError> {
                self.read.borrow_mut().push(path.to_owned());
                self.inner.read(path)
            }
        }

        let spec = Spec::minimal();
        let source = RecordingSource {
            inner: spec.source(),
            read: std::cell::RefCell::new(Vec::new()),
        };
        read_snapshot(&source).unwrap();
        assert_eq!(
            source.read.borrow().as_slice(),
            [
                MANIFEST_PATH,
                "frames/one.txt",
                "frames/image.txt",
                "assets/pixel.png"
            ]
        );
    }

    #[test]
    fn the_frame_parser_accepts_every_committed_golden_fixture() {
        // The golden corpus is the authority for the canonical frame format;
        // if this parser stops accepting all of it, a snapshot could declare a
        // frame the renderer's own harness still treats as valid.
        let dir =
            std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("../../tests/fixtures/rendering");
        let mut checked = 0;
        for entry in std::fs::read_dir(&dir).expect("golden fixtures must exist") {
            let path = entry.unwrap().path();
            if path.extension().is_none_or(|extension| extension != "txt") {
                continue;
            }
            let text = std::fs::read_to_string(&path).unwrap();
            parse_frame(&text).unwrap_or_else(|error| {
                panic!("{} must parse: {error}", path.display());
            });
            checked += 1;
        }
        assert!(
            checked >= 20,
            "expected the full golden corpus, got {checked}"
        );
    }

    #[test]
    fn the_frame_parser_rejects_miscounts_and_trailing_content() {
        let miscount = "ordo-fixture/1\nkind:Lyrics\nheader-absent\nbody-lines:2\none line\nfooter-absent\nimage-absent\n";
        // A body count that runs past the real body consumes the footer line,
        // so the failure names the record that no longer matches, not the
        // count: a line format cannot know which line the author meant.
        assert!(matches!(
            parse_frame(miscount),
            Err(FrameParseError::Line { line: 7, .. })
        ));

        let trailing = "ordo-fixture/1\nkind:Black\nheader-absent\nbody-lines:0\nfooter-absent\nimage-absent\nextra\n";
        assert_eq!(
            parse_frame(trailing),
            Err(FrameParseError::Line {
                line: 7,
                reason: FrameReason::Trailing,
            })
        );

        let leading_zero = "ordo-fixture/1\nkind:Lyrics\nheader-absent\nbody-lines:01\nx\nfooter-absent\nimage-absent\n";
        assert_eq!(
            parse_frame(leading_zero),
            Err(FrameParseError::Line {
                line: 4,
                reason: FrameReason::InvalidCount {
                    value: "01".to_owned(),
                },
            })
        );

        let bad_size = "ordo-fixture/1\nkind:GeneralImage\nheader-absent\nbody-lines:0\nfooter-absent\nimage:some-asset\nimage-size:2x\n";
        assert_eq!(
            parse_frame(bad_size),
            Err(FrameParseError::Line {
                line: 7,
                reason: FrameReason::InvalidSize {
                    value: "2x".to_owned(),
                },
            })
        );
    }

    #[test]
    fn loaded_snapshots_are_send_and_sync_for_worker_ownership() {
        const fn assert_send_sync<T: Send + Sync>() {}
        assert_send_sync::<LoadedSnapshot>();
        assert_send_sync::<DirectorySource>();
    }
}
