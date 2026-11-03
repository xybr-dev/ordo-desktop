//! Immutable Presentation snapshot model.
//!
//! A snapshot is the Presenter's complete input: ordered Mass sections, each
//! owning ordered Mass items, each owning ordered Cues. It is independent of
//! planning, persistence, rendering, and the filesystem, so it can be built,
//! inspected, and replayed deterministically.
//!
//! Construction is the only way to obtain a snapshot, and every constructor
//! rejects input that would make presentation ambiguous:
//!
//! - A Cue has exactly one owning Mass item; Mass sections own items only.
//! - Identities are unique inside a snapshot, so navigation and recovery can
//!   name one Cue unambiguously.
//! - Every Theme reference resolves to a Theme the snapshot lists, so
//!   rendering never silently substitutes a different visual identity.
//! - Auto-play is opt-in for General Presentations only, and a duration
//!   override is meaningful only inside an auto-play Mass item (MVP-PLAN 8.4).
//! - Operator-only Cues keep the private context the operator must read.

use std::collections::BTreeSet;
use std::fmt;
use std::str::FromStr;

/// One durable Ordo identity in its canonical text form.
///
/// Ordo identities are UUID version 4 values stored canonically in lowercase
/// (Decision 38). Parsing accepts exactly that form, so one identity has one
/// text form and one byte form: every `Display` output parses back to the same
/// value, and two snapshots cannot disagree about an identity's spelling.
#[derive(Clone, Copy, Eq, Hash, Ord, PartialEq, PartialOrd)]
pub struct EntityId([u8; 16]);

impl EntityId {
    /// Parse the canonical lowercase UUID v4 form.
    ///
    /// Uppercase, braces, shortened groups, a different version, and any other
    /// RFC 4122 variant are rejected rather than normalized, because
    /// normalizing would let two spellings name one identity in different
    /// places.
    pub fn parse(text: &str) -> Result<Self, IdError> {
        let bytes = text.as_bytes();
        if bytes.len() != 36 {
            return Err(IdError::Length(bytes.len()));
        }
        let mut digits = [0u8; 32];
        let mut next = 0;
        for (index, byte) in bytes.iter().copied().enumerate() {
            if matches!(index, 8 | 13 | 18 | 23) {
                if byte != b'-' {
                    return Err(IdError::Malformed);
                }
            } else {
                digits[next] = hex_digit(byte).ok_or(IdError::Malformed)?;
                next += 1;
            }
        }
        let mut raw = [0u8; 16];
        for (byte, pair) in raw.iter_mut().zip(digits.chunks_exact(2)) {
            *byte = (pair[0] << 4) | pair[1];
        }
        let version = raw[6] >> 4;
        if version != 4 {
            return Err(IdError::Version(version));
        }
        let variant = raw[8] >> 4;
        if !(0x8..=0xb).contains(&variant) {
            return Err(IdError::Variant(variant));
        }
        Ok(Self(raw))
    }

    /// Write the one canonical text form: lowercase hex groups separated by
    /// hyphens after four, six, eight, and ten bytes.
    fn write_canonical(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        for (index, byte) in self.0.iter().enumerate() {
            if matches!(index, 4 | 6 | 8 | 10) {
                f.write_str("-")?;
            }
            write!(f, "{byte:02x}")?;
        }
        Ok(())
    }
}

impl FromStr for EntityId {
    type Err = IdError;

    fn from_str(text: &str) -> Result<Self, Self::Err> {
        Self::parse(text)
    }
}

impl fmt::Display for EntityId {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        self.write_canonical(f)
    }
}

impl fmt::Debug for EntityId {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        self.write_canonical(f)
    }
}

/// Why a text value is not a canonical Ordo identity.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum IdError {
    /// The text has this many bytes; a canonical UUID has exactly 36.
    Length(usize),
    /// A character is not a lowercase hexadecimal digit or a hyphen in its
    /// canonical position.
    Malformed,
    /// The version nibble is this value; Ordo identities are version 4.
    Version(u8),
    /// The variant nibble is this value; RFC 4122 allows 8, 9, a, or b.
    Variant(u8),
}

impl fmt::Display for IdError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Length(len) => {
                write!(f, "identity text has {len} bytes; a canonical UUID has 36")
            }
            Self::Malformed => f.write_str("identity text is not a canonical lowercase UUID v4"),
            Self::Version(version) => {
                write!(
                    f,
                    "identity version is {version}; Ordo identities are version 4"
                )
            }
            Self::Variant(variant) => write!(
                f,
                "identity variant nibble is {variant:x}; RFC 4122 allows 8, 9, a, or b"
            ),
        }
    }
}

impl std::error::Error for IdError {}

/// The SHA-256 digest of one declared frame or asset file.
///
/// The snapshot names content by digest, never by a mutable file name alone:
/// the reader compares the bytes it reads against this value before any frame
/// can become Program output.
#[derive(Clone, Copy, Eq, Hash, Ord, PartialEq, PartialOrd)]
pub struct ContentHash([u8; 32]);

impl ContentHash {
    /// Build a digest from bytes a SHA-256 implementation computed.
    ///
    /// Every 32-byte sequence is a valid digest, so this constructor cannot
    /// fail; only text parsing has a canonical form to enforce.
    pub const fn from_bytes(bytes: [u8; 32]) -> Self {
        Self(bytes)
    }

    /// The digest bytes, for fingerprinting and comparison.
    pub const fn as_bytes(&self) -> &[u8; 32] {
        &self.0
    }

    /// Parse 64 lowercase hexadecimal characters.
    pub fn parse(text: &str) -> Result<Self, ContentHashError> {
        let bytes = text.as_bytes();
        if bytes.len() != 64 {
            return Err(ContentHashError::Length(bytes.len()));
        }
        let mut raw = [0u8; 32];
        for (byte, pair) in raw.iter_mut().zip(bytes.chunks_exact(2)) {
            let high = hex_digit(pair[0]).ok_or(ContentHashError::NotLowercaseHex)?;
            let low = hex_digit(pair[1]).ok_or(ContentHashError::NotLowercaseHex)?;
            *byte = (high << 4) | low;
        }
        Ok(Self(raw))
    }
}

impl FromStr for ContentHash {
    type Err = ContentHashError;

    fn from_str(text: &str) -> Result<Self, Self::Err> {
        Self::parse(text)
    }
}

impl fmt::Display for ContentHash {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write_hex(&self.0, f)
    }
}

impl fmt::Debug for ContentHash {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write_hex(&self.0, f)
    }
}

/// Why a text value is not a canonical content hash.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum ContentHashError {
    /// The text has this many bytes; a SHA-256 hash has exactly 64.
    Length(usize),
    /// A character is not a lowercase hexadecimal digit.
    NotLowercaseHex,
}

impl fmt::Display for ContentHashError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Length(len) => write!(
                f,
                "content hash text has {len} bytes; a SHA-256 hash has 64"
            ),
            Self::NotLowercaseHex => f.write_str("content hash text is not lowercase hexadecimal"),
        }
    }
}

impl std::error::Error for ContentHashError {}

/// The lowercase hexadecimal value of one nibble, or `None` when `byte` cannot
/// appear in canonical text.
fn hex_digit(byte: u8) -> Option<u8> {
    match byte {
        b'0'..=b'9' => Some(byte - b'0'),
        b'a'..=b'f' => Some(byte - b'a' + 10),
        _ => None,
    }
}

fn write_hex(bytes: &[u8], f: &mut fmt::Formatter<'_>) -> fmt::Result {
    for byte in bytes {
        write!(f, "{byte:02x}")?;
    }
    Ok(())
}

/// Declare the typed identities that wrap [`EntityId`].
///
/// Each identity is a distinct type, so a Mass item ID can never be passed
/// where a Cue ID belongs and two different kinds of identity can never be
/// compared as if they were the same namespace.
macro_rules! typed_entity_ids {
    ($($(#[$doc:meta])* $name:ident),+ $(,)?) => {
        $(
            $(#[$doc])*
            #[derive(Clone, Copy, Debug, Eq, Hash, Ord, PartialEq, PartialOrd)]
            pub struct $name(EntityId);

            impl $name {
                /// Parse the canonical lowercase UUID v4 text form.
                pub fn parse(text: &str) -> Result<Self, IdError> {
                    EntityId::parse(text).map(Self)
                }

                /// The untyped identity behind this typed handle.
                pub fn entity(self) -> EntityId {
                    self.0
                }
            }

            impl FromStr for $name {
                type Err = IdError;

                fn from_str(text: &str) -> Result<Self, Self::Err> {
                    Self::parse(text)
                }
            }

            impl fmt::Display for $name {
                fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
                    fmt::Display::fmt(&self.0, f)
                }
            }
        )+
    };
}

typed_entity_ids! {
    /// Identifies one immutable Presentation snapshot.
    SnapshotId,
    /// Identifies one Mass section inside one snapshot.
    MassSectionId,
    /// Identifies one Mass item inside one snapshot.
    MassItemId,
    /// Identifies one Cue; Cue IDs are unique across the whole snapshot.
    CueId,
    /// Identifies one Theme a snapshot lists or references.
    ThemeId,
    /// Identifies one canonical frame file a Project Cue shows.
    FrameId,
    /// Identifies one binary asset a snapshot declares by hash.
    AssetId,
}

/// Whether a Theme is parish-authored or Ordo's guaranteed built-in fallback.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum ThemeKind {
    /// A reusable visual identity maintained for this parish.
    Custom,
    /// The built-in fallback Theme (Decision 22): always presentable even
    /// when custom fonts, images, or colors cannot be trusted.
    Fallback,
}

/// A Theme as one snapshot lists it.
///
/// The snapshot carries identity, display name, and kind, not styling: the
/// renderer owns the actual composition, and duplicating it here would let the
/// snapshot and the Theme disagree about what the congregation sees.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct ThemeRef {
    /// Stable identity the snapshot uses for every Theme reference.
    pub id: ThemeId,
    /// The name shown to the operator.
    pub name: String,
    /// Whether this is a parish Theme or the guaranteed fallback.
    pub kind: ThemeKind,
}

/// The kind of content one Mass item presents.
///
/// The kind is preserved from capture rather than inferred from the title,
/// because it decides which items are eligible for auto-play and lets a later
/// phase choose specialized editing and validation without re-parsing text.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum MassItemKind {
    /// A Song's Arrangement.
    Song,
    /// A reading from the lectionary.
    Reading,
    /// A responsorial psalm.
    Psalm,
    /// Ordered congregation responses, each possibly preceded by a trigger.
    ResponseSequence,
    /// Constrained text or image slides: announcements, prayers, intentions.
    GeneralPresentation,
    /// A liturgical rite with no reusable Song, Reading, or Response source.
    Rite,
}

/// The inherited duration for every projectable Cue in one Mass item.
///
/// Only [`MassItemKind::GeneralPresentation`] items may carry auto-play, and a
/// per-Cue duration override may appear only inside such an item, so a
/// snapshot can never claim timed slides for content that never advances
/// automatically (MVP-PLAN 8.4).
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct AutoPlay {
    /// Seconds each projectable Cue shows before auto-play advances. The
    /// owning Mass item rejects zero.
    pub default_seconds: u32,
}

/// What one Cue does to Program when taken live.
///
/// The Program effect is derived from the behaviour (see
/// [`CueBehavior::program_effect`]) rather than stored beside it, so the two
/// can never disagree.
#[derive(Clone, Debug, Eq, PartialEq)]
pub enum CueBehavior {
    /// Show one complete canonical frame. `duration_seconds` overrides the
    /// owning Mass item's auto-play default and is valid only there.
    Project {
        /// The frame file this Cue shows.
        frame: FrameId,
        /// Seconds to show before auto-play advances, or the item default.
        duration_seconds: Option<u32>,
    },
    /// The minister's phrase the operator waits for: the phrase stays private
    /// and the congregation sees black.
    OperatorTrigger,
    /// An ordinary operator-only checkpoint with black output. Unlike
    /// Blackout, taking a Hold live establishes no persistent presentation
    /// state.
    Hold,
    /// Deliberately leave Program unchanged while an unprojected action or
    /// text happens.
    Wait,
    /// Captured content that cannot be produced faithfully: inspectable, but
    /// never presentable.
    Unavailable {
        /// Why the intended output cannot be produced faithfully.
        reason: String,
    },
}

impl CueBehavior {
    /// What Program must show after taking this Cue live.
    pub fn program_effect(&self) -> ProgramEffect {
        match self {
            Self::Project { frame, .. } => ProgramEffect::Frame(*frame),
            Self::OperatorTrigger | Self::Hold => ProgramEffect::Black,
            Self::Wait => ProgramEffect::Unchanged,
            Self::Unavailable { .. } => ProgramEffect::NotPresentable,
        }
    }
}

/// What Program must show after taking one Cue live.
///
/// [`ProgramEffect::Frame`], [`ProgramEffect::Black`], and
/// [`ProgramEffect::Unchanged`] are all valid live outcomes. A
/// [`ProgramEffect::NotPresentable`] Cue stays inspectable and navigable but
/// may never be taken live, so the Engine and Preflight share this single
/// classification instead of re-deriving it from Cue details.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum ProgramEffect {
    /// Exactly this complete canonical frame.
    Frame(FrameId),
    /// The intentional black output.
    Black,
    /// Program keeps whatever it showed before this Cue.
    Unchanged,
    /// No faithful output exists, so the Cue cannot go live.
    NotPresentable,
}

/// One ordered operator-relevant event inside a Mass item.
///
/// `private_context` is operator-only. It is required for
/// [`CueBehavior::OperatorTrigger`] and [`CueBehavior::Wait`], whose whole
/// purpose is something the congregation must not see, and optional for every
/// other behaviour.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct Cue {
    id: CueId,
    label: String,
    private_context: Option<String>,
    behavior: CueBehavior,
}

impl Cue {
    /// Build a Cue, rejecting the behaviours that would be uninspectable.
    ///
    /// `OperatorTrigger` and `Wait` require private context that is more than
    /// whitespace: the operator must be able to read what to wait for. A
    /// project duration of zero is rejected because auto-play would advance
    /// before the frame could ever be seen.
    pub fn new(
        id: CueId,
        label: String,
        private_context: Option<String>,
        behavior: CueBehavior,
    ) -> Result<Self, SnapshotError> {
        let requires_context =
            matches!(&behavior, CueBehavior::OperatorTrigger | CueBehavior::Wait);
        let has_context = private_context
            .as_deref()
            .is_some_and(|text| !text.trim().is_empty());
        if requires_context && !has_context {
            return Err(SnapshotError::MissingPrivateContext { cue: id });
        }
        if let CueBehavior::Project {
            duration_seconds: Some(0),
            ..
        } = &behavior
        {
            return Err(SnapshotError::ZeroCueDuration { cue: id });
        }
        Ok(Self {
            id,
            label,
            private_context,
            behavior,
        })
    }

    /// Stable identity of this Cue; unique across the snapshot.
    pub fn id(&self) -> CueId {
        self.id
    }

    /// The operator-facing name in the Cue timeline.
    pub fn label(&self) -> &str {
        &self.label
    }

    /// Text only the operator may read; never part of Program output.
    pub fn private_context(&self) -> Option<&str> {
        self.private_context.as_deref()
    }

    /// What taking this Cue live does to Program.
    pub fn behavior(&self) -> &CueBehavior {
        &self.behavior
    }

    /// The Program effect of taking this Cue live.
    pub fn program_effect(&self) -> ProgramEffect {
        self.behavior.program_effect()
    }

    /// The per-Cue auto-play override, when this Cue projects.
    fn duration_override(&self) -> Option<u32> {
        match &self.behavior {
            CueBehavior::Project {
                duration_seconds, ..
            } => *duration_seconds,
            _ => None,
        }
    }
}

/// One titled unit of the prepared Ordo: a Song, reading, rite, response
/// sequence, announcement, or hold.
///
/// A Mass item owns every Cue the operator navigates inside it. A Mass section
/// owns items only, so "which Mass item owns this Cue" always has exactly one
/// answer.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct MassItem {
    id: MassItemId,
    title: String,
    kind: MassItemKind,
    theme_override: Option<ThemeId>,
    auto_play: Option<AutoPlay>,
    cues: Vec<Cue>,
}

impl MassItem {
    /// Build a Mass item with at least one Cue.
    ///
    /// `auto_play` is accepted only for a [`MassItemKind::GeneralPresentation`]
    /// with a positive default, and a Cue may override its duration only when
    /// that auto-play exists. Rejecting both here keeps the two facts — "this
    /// item advances itself" and "these Cues are timed" — from drifting apart.
    pub fn new(
        id: MassItemId,
        title: String,
        kind: MassItemKind,
        theme_override: Option<ThemeId>,
        auto_play: Option<AutoPlay>,
        cues: Vec<Cue>,
    ) -> Result<Self, SnapshotError> {
        if cues.is_empty() {
            return Err(SnapshotError::EmptyItem { item: id });
        }
        if auto_play.is_some() && kind != MassItemKind::GeneralPresentation {
            return Err(SnapshotError::AutoPlayOnNonGeneralItem { item: id });
        }
        if auto_play.is_some_and(|auto| auto.default_seconds == 0) {
            return Err(SnapshotError::ZeroAutoPlayDuration { item: id });
        }
        if auto_play.is_none()
            && let Some(cue) = cues.iter().find(|cue| cue.duration_override().is_some())
        {
            return Err(SnapshotError::DurationOutsideAutoPlayItem { cue: cue.id() });
        }
        Ok(Self {
            id,
            title,
            kind,
            theme_override,
            auto_play,
            cues,
        })
    }

    /// Stable identity of this Mass item; unique across the snapshot.
    pub fn id(&self) -> MassItemId {
        self.id
    }

    /// The operator-facing title in the Ordo.
    pub fn title(&self) -> &str {
        &self.title
    }

    /// The kind of content this item presents.
    pub fn kind(&self) -> MassItemKind {
        self.kind
    }

    /// The Theme this item selects instead of the Mass Theme, if any.
    pub fn theme_override(&self) -> Option<ThemeId> {
        self.theme_override
    }

    /// The auto-play default for this item, when it advances its own Cues.
    pub fn auto_play(&self) -> Option<AutoPlay> {
        self.auto_play
    }

    /// Every Cue in the operator's Left/Right order.
    pub fn cues(&self) -> &[Cue] {
        &self.cues
    }
}

/// One independently named grouping of Mass items: an ordered large-scale
/// phase of the prepared Ordo.
///
/// The label carries no inferred liturgical behavior, and the section owns no
/// Cue and no projectable output of its own: everything the congregation can
/// see belongs to an item inside it.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct MassSection {
    id: MassSectionId,
    title: String,
    items: Vec<MassItem>,
}

impl MassSection {
    /// Build a Mass section with at least one Mass item.
    pub fn new(
        id: MassSectionId,
        title: String,
        items: Vec<MassItem>,
    ) -> Result<Self, SnapshotError> {
        if items.is_empty() {
            return Err(SnapshotError::EmptySection { section: id });
        }
        Ok(Self { id, title, items })
    }

    /// Stable identity of this Mass section; unique across the snapshot.
    pub fn id(&self) -> MassSectionId {
        self.id
    }

    /// The operator-facing section label.
    pub fn title(&self) -> &str {
        &self.title
    }

    /// Every Mass item in this section's order.
    pub fn items(&self) -> &[MassItem] {
        &self.items
    }
}

/// One immutable captured version of a Mass plan: the Presenter's whole input.
///
/// The snapshot is self-contained and value-validated: every identity is
/// unique, every Theme reference resolves among [`PresentationSnapshot::themes`],
/// and every Cue belongs to exactly one Mass item. Reading, hashing, and
/// rendering produce these values; nothing downstream may manufacture a
/// partial one.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct PresentationSnapshot {
    id: SnapshotId,
    title: String,
    mass_theme: ThemeRef,
    themes: Vec<ThemeRef>,
    sections: Vec<MassSection>,
}

impl PresentationSnapshot {
    /// Build a snapshot from already-validated sections and Themes.
    ///
    /// `mass_theme` must name one of `themes`; the resolved Theme is stored so
    /// every later lookup is total. Item-local rules (cue presence, auto-play,
    /// required private context) were already enforced by the owning
    /// constructors.
    pub fn new(
        id: SnapshotId,
        title: String,
        mass_theme: ThemeId,
        themes: Vec<ThemeRef>,
        sections: Vec<MassSection>,
    ) -> Result<Self, SnapshotError> {
        if sections.is_empty() {
            return Err(SnapshotError::NoSections);
        }
        if let Some(id) = duplicate(sections.iter().map(MassSection::id)) {
            return Err(SnapshotError::DuplicateSectionId { id });
        }
        if let Some(id) = duplicate(
            sections
                .iter()
                .flat_map(MassSection::items)
                .map(MassItem::id),
        ) {
            return Err(SnapshotError::DuplicateItemId { id });
        }
        if let Some(id) = duplicate(
            sections
                .iter()
                .flat_map(MassSection::items)
                .flat_map(MassItem::cues)
                .map(Cue::id),
        ) {
            return Err(SnapshotError::DuplicateCueId { id });
        }
        if themes.is_empty() {
            return Err(SnapshotError::NoThemes);
        }
        if let Some(id) = duplicate(themes.iter().map(|theme| theme.id)) {
            return Err(SnapshotError::DuplicateThemeId { id });
        }
        let Some(mass_theme) = themes.iter().find(|theme| theme.id == mass_theme).cloned() else {
            return Err(SnapshotError::UnresolvedMassTheme { theme: mass_theme });
        };
        for item in sections.iter().flat_map(MassSection::items) {
            if let Some(theme) = item.theme_override()
                && !themes.iter().any(|listed| listed.id == theme)
            {
                return Err(SnapshotError::UnresolvedItemTheme {
                    item: item.id(),
                    theme,
                });
            }
        }
        Ok(Self {
            id,
            title,
            mass_theme,
            themes,
            sections,
        })
    }

    /// Stable identity of this captured snapshot.
    pub fn id(&self) -> SnapshotId {
        self.id
    }

    /// The snapshot title shown to the operator.
    pub fn title(&self) -> &str {
        &self.title
    }

    /// The Theme every Mass item inherits unless it overrides it.
    ///
    /// Stored resolved: construction proved it names one of
    /// [`PresentationSnapshot::themes`], so this lookup never falls back.
    pub fn mass_theme(&self) -> &ThemeRef {
        &self.mass_theme
    }

    /// Every Theme the snapshot lists or references.
    pub fn themes(&self) -> &[ThemeRef] {
        &self.themes
    }

    /// Every Mass section in the prepared Ordo's order.
    pub fn sections(&self) -> &[MassSection] {
        &self.sections
    }

    /// Every Cue in navigation order, flattened across Mass items.
    ///
    /// Sections and items contribute no positions of their own, so the flat
    /// index is exactly the operator's Left/Right order and no Cue can be
    /// reached twice.
    pub fn positions(&self) -> Vec<CuePosition> {
        let total: usize = self
            .sections
            .iter()
            .flat_map(MassSection::items)
            .map(|item| item.cues().len())
            .sum();
        let mut positions = Vec::with_capacity(total);
        for (section, section_value) in self.sections.iter().enumerate() {
            for (item, item_value) in section_value.items().iter().enumerate() {
                for (cue, cue_value) in item_value.cues().iter().enumerate() {
                    positions.push(CuePosition {
                        flat: positions.len(),
                        section,
                        item,
                        cue,
                        cue_id: cue_value.id(),
                        item_id: item_value.id(),
                    });
                }
            }
        }
        positions
    }

    /// The Cue at one flattened position, or `None` past the last Cue.
    pub fn cue(&self, flat: usize) -> Option<&Cue> {
        self.sections
            .iter()
            .flat_map(MassSection::items)
            .flat_map(MassItem::cues)
            .nth(flat)
    }

    /// Every distinct frame a Project Cue shows.
    ///
    /// Repeated chorus and refrain occurrences deliberately share one frame,
    /// so this is a set rather than one entry per Cue.
    pub fn frame_ids(&self) -> BTreeSet<FrameId> {
        self.sections
            .iter()
            .flat_map(MassSection::items)
            .flat_map(MassItem::cues)
            .filter_map(|cue| match cue.behavior() {
                CueBehavior::Project { frame, .. } => Some(*frame),
                _ => None,
            })
            .collect()
    }

    /// The Theme `item` renders with: its override when this snapshot lists
    /// one, otherwise the Mass Theme.
    ///
    /// An override from an item that does not belong to this snapshot cannot
    /// resolve here, so it falls back to the Mass Theme instead of borrowing an
    /// unrelated visual identity.
    pub fn effective_theme(&self, item: &MassItem) -> &ThemeRef {
        item.theme_override()
            .and_then(|id| self.themes.iter().find(|theme| theme.id == id))
            .unwrap_or(&self.mass_theme)
    }
}

/// One Cue's address in the flattened navigation order.
///
/// `flat` is the operator's Left/Right order; `section`, `item`, and `cue` are
/// the indexes of the Cue's owners, so Page Up/Down and Home/End can move
/// between items and sections without scanning the flattened list.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct CuePosition {
    /// Index in the flattened Cue order.
    pub flat: usize,
    /// Index of the owning Mass section.
    pub section: usize,
    /// Index of the owning Mass item inside its section.
    pub item: usize,
    /// Index of the Cue inside its Mass item.
    pub cue: usize,
    /// Identity of the Cue, for stable selection and recovery.
    pub cue_id: CueId,
    /// Identity of the owning Mass item.
    pub item_id: MassItemId,
}

/// The first identity that repeats, so callers can name the exact collision.
fn duplicate<T: Ord + Copy>(ids: impl IntoIterator<Item = T>) -> Option<T> {
    let mut seen = BTreeSet::new();
    ids.into_iter().find(|id| !seen.insert(*id))
}

/// Why a Presentation snapshot cannot exist as a usable input.
///
/// Every variant names the identity (or the missing relationship) that made
/// construction fail, so the reader can report a path-level rejection instead
/// of exposing a partially valid snapshot.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum SnapshotError {
    /// A snapshot with no Mass sections has nothing to present.
    NoSections,
    /// A Mass section with no Mass items owns no navigable content.
    EmptySection { section: MassSectionId },
    /// A Mass item with no Cues owns nothing the operator can navigate.
    EmptyItem { item: MassItemId },
    /// The same Mass section identity appears more than once.
    DuplicateSectionId { id: MassSectionId },
    /// The same Mass item identity appears in more than one place.
    DuplicateItemId { id: MassItemId },
    /// The same Cue identity appears in more than one place.
    DuplicateCueId { id: CueId },
    /// A snapshot lists no Themes, so it cannot resolve its Mass Theme.
    NoThemes,
    /// The same Theme identity is listed more than once.
    DuplicateThemeId { id: ThemeId },
    /// The named Mass Theme is not one of the snapshot's Themes.
    UnresolvedMassTheme { theme: ThemeId },
    /// A Mass item's Theme override is not one of the snapshot's Themes.
    UnresolvedItemTheme { item: MassItemId, theme: ThemeId },
    /// Auto-play was enabled on an item that is not a General Presentation.
    AutoPlayOnNonGeneralItem { item: MassItemId },
    /// A General Presentation's auto-play default is zero seconds.
    ZeroAutoPlayDuration { item: MassItemId },
    /// A Cue overrides a duration outside any auto-play Mass item.
    DurationOutsideAutoPlayItem { cue: CueId },
    /// A projectable Cue would auto-advance before it could be seen.
    ZeroCueDuration { cue: CueId },
    /// An operator-only Cue has no private phrase or instruction.
    MissingPrivateContext { cue: CueId },
}

impl fmt::Display for SnapshotError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::NoSections => {
                f.write_str("a Presentation snapshot needs at least one Mass section")
            }
            Self::EmptySection { section } => {
                write!(f, "Mass section {section} has no Mass items")
            }
            Self::EmptyItem { item } => write!(f, "Mass item {item} has no Cues"),
            Self::DuplicateSectionId { id } => {
                write!(f, "Mass section {id} appears more than once")
            }
            Self::DuplicateItemId { id } => write!(f, "Mass item {id} appears more than once"),
            Self::DuplicateCueId { id } => write!(f, "Cue {id} appears more than once"),
            Self::NoThemes => f.write_str("a Presentation snapshot needs at least one Theme"),
            Self::DuplicateThemeId { id } => write!(f, "Theme {id} appears more than once"),
            Self::UnresolvedMassTheme { theme } => {
                write!(f, "Mass Theme {theme} is not among the snapshot's Themes")
            }
            Self::UnresolvedItemTheme { item, theme } => write!(
                f,
                "Mass item {item} overrides with Theme {theme}, which the snapshot does not list"
            ),
            Self::AutoPlayOnNonGeneralItem { item } => write!(
                f,
                "Mass item {item} enables auto-play but is not a General Presentation"
            ),
            Self::ZeroAutoPlayDuration { item } => {
                write!(f, "Mass item {item} has a zero auto-play duration")
            }
            Self::DurationOutsideAutoPlayItem { cue } => write!(
                f,
                "Cue {cue} overrides a duration outside an auto-play Mass item"
            ),
            Self::ZeroCueDuration { cue } => write!(f, "Cue {cue} has a zero duration"),
            Self::MissingPrivateContext { cue } => {
                write!(f, "operator-only Cue {cue} has no private context")
            }
        }
    }
}

impl std::error::Error for SnapshotError {}

#[cfg(test)]
mod tests {
    use super::*;

    fn uuid(n: u16) -> String {
        format!("00000000-0000-4000-8000-{n:012x}")
    }

    fn snapshot_id(n: u16) -> SnapshotId {
        uuid(n).parse().unwrap()
    }

    fn section_id(n: u16) -> MassSectionId {
        uuid(0x100 + n).parse().unwrap()
    }

    fn item_id(n: u16) -> MassItemId {
        uuid(0x200 + n).parse().unwrap()
    }

    fn cue_id(n: u16) -> CueId {
        uuid(0x300 + n).parse().unwrap()
    }

    fn theme_id(n: u16) -> ThemeId {
        uuid(0x400 + n).parse().unwrap()
    }

    fn frame_id(n: u16) -> FrameId {
        uuid(0x500 + n).parse().unwrap()
    }

    fn theme(n: u16, name: &str, kind: ThemeKind) -> ThemeRef {
        ThemeRef {
            id: theme_id(n),
            name: name.to_owned(),
            kind,
        }
    }

    /// The two Themes a snapshot must be able to resolve: one parish Theme and
    /// the guaranteed built-in fallback.
    fn listed_themes() -> Vec<ThemeRef> {
        vec![
            theme(1, "Parish Paschal", ThemeKind::Custom),
            theme(2, "Ordo fallback", ThemeKind::Fallback),
        ]
    }

    fn project(n: u16, frame: u16) -> Cue {
        Cue::new(
            cue_id(n),
            format!("Slide {n}"),
            None,
            CueBehavior::Project {
                frame: frame_id(frame),
                duration_seconds: None,
            },
        )
        .unwrap()
    }

    fn item(n: u16, kind: MassItemKind, cues: Vec<Cue>) -> MassItem {
        MassItem::new(item_id(n), format!("Item {n}"), kind, None, None, cues).unwrap()
    }

    fn section(n: u16, items: Vec<MassItem>) -> MassSection {
        MassSection::new(section_id(n), format!("Section {n}"), items).unwrap()
    }

    fn presentation_result(
        sections: Vec<MassSection>,
    ) -> Result<PresentationSnapshot, SnapshotError> {
        PresentationSnapshot::new(
            snapshot_id(1),
            "Normal Mass".to_owned(),
            theme_id(1),
            listed_themes(),
            sections,
        )
    }

    fn presentation(sections: Vec<MassSection>) -> PresentationSnapshot {
        presentation_result(sections).unwrap()
    }

    #[test]
    fn canonical_identities_round_trip_for_every_typed_id() {
        let text = "123e4567-e89b-42d3-a456-426614174000";
        assert_eq!(EntityId::parse(text).unwrap().to_string(), text);
        assert_eq!(SnapshotId::parse(text).unwrap().to_string(), text);
        assert_eq!(MassSectionId::parse(text).unwrap().to_string(), text);
        assert_eq!(MassItemId::parse(text).unwrap().to_string(), text);
        assert_eq!(CueId::parse(text).unwrap().to_string(), text);
        assert_eq!(ThemeId::parse(text).unwrap().to_string(), text);
        assert_eq!(FrameId::parse(text).unwrap().to_string(), text);
        assert_eq!(AssetId::parse(text).unwrap().to_string(), text);

        // A typed identity keeps the exact EntityId it parsed.
        let entity = EntityId::parse(text).unwrap();
        assert_eq!(CueId::parse(text).unwrap().entity(), entity);
    }

    #[test]
    fn non_canonical_identities_are_rejected() {
        assert_eq!(EntityId::parse(""), Err(IdError::Length(0)));
        assert_eq!(
            EntityId::parse("00000000-0000-4000-8000-00000000001"),
            Err(IdError::Length(35))
        );
        assert_eq!(
            EntityId::parse("00000000_0000-4000-8000-000000000001"),
            Err(IdError::Malformed),
            "a separator outside its canonical position is malformed"
        );
        assert_eq!(
            EntityId::parse("00000000-0000-4000-8000-00000000000A"),
            Err(IdError::Malformed),
            "uppercase is not the canonical spelling"
        );
        assert_eq!(
            EntityId::parse("00000000-0000-3000-8000-000000000001"),
            Err(IdError::Version(3))
        );
        assert_eq!(
            EntityId::parse("00000000-0000-4000-c000-000000000001"),
            Err(IdError::Variant(0xc))
        );
        assert_eq!(
            EntityId::parse("00000000-0000-4000-8000-00000000000g"),
            Err(IdError::Malformed)
        );
    }

    #[test]
    fn content_hashes_round_trip_and_reject_non_canonical_text() {
        let text = "a".repeat(64);
        let hash = ContentHash::parse(&text).unwrap();
        assert_eq!(hash.to_string(), text);
        assert_eq!(
            ContentHash::parse(&text.to_uppercase()),
            Err(ContentHashError::NotLowercaseHex)
        );
        assert_eq!(
            ContentHash::parse(&"a".repeat(63)),
            Err(ContentHashError::Length(63))
        );
        assert_eq!(
            ContentHash::from_bytes([0xab; 32]).to_string(),
            "ab".repeat(32)
        );
        assert_eq!(ContentHash::from_bytes([0xab; 32]).as_bytes(), &[0xab; 32]);
    }

    /// A snapshot covering every Cue behaviour: two sections, four items, a
    /// repeated-project frame, an auto-play item, an override Theme, and
    /// operator-only context.
    fn full_snapshot() -> PresentationSnapshot {
        let intro = section(
            1,
            vec![
                item(1, MassItemKind::Song, vec![project(1, 1), project(2, 2)]),
                MassItem::new(
                    item_id(2),
                    "Greeting".to_owned(),
                    MassItemKind::ResponseSequence,
                    None,
                    None,
                    vec![
                        Cue::new(
                            cue_id(3),
                            "The Lord be with you".to_owned(),
                            Some("Presider: The Lord be with you.".to_owned()),
                            CueBehavior::OperatorTrigger,
                        )
                        .unwrap(),
                        project(4, 3),
                    ],
                )
                .unwrap(),
                MassItem::new(
                    item_id(3),
                    "Announcements".to_owned(),
                    MassItemKind::GeneralPresentation,
                    None,
                    Some(AutoPlay { default_seconds: 8 }),
                    vec![
                        project(5, 4),
                        Cue::new(
                            cue_id(6),
                            "Short slide".to_owned(),
                            None,
                            CueBehavior::Project {
                                frame: frame_id(5),
                                duration_seconds: Some(3),
                            },
                        )
                        .unwrap(),
                        Cue::new(
                            cue_id(7),
                            "Damaged image".to_owned(),
                            None,
                            CueBehavior::Unavailable {
                                reason: "captured image is unreadable".to_owned(),
                            },
                        )
                        .unwrap(),
                    ],
                )
                .unwrap(),
            ],
        );
        let word = section(
            2,
            vec![
                MassItem::new(
                    item_id(4),
                    "Silent prayer".to_owned(),
                    MassItemKind::Rite,
                    Some(theme_id(2)),
                    None,
                    vec![
                        Cue::new(
                            cue_id(8),
                            "Hold for silence".to_owned(),
                            Some("Hold until the presider stands.".to_owned()),
                            CueBehavior::Hold,
                        )
                        .unwrap(),
                        Cue::new(
                            cue_id(9),
                            "Cantor verse".to_owned(),
                            Some("Cantor sings the verse unprojected.".to_owned()),
                            CueBehavior::Wait,
                        )
                        .unwrap(),
                    ],
                )
                .unwrap(),
            ],
        );
        presentation(vec![intro, word])
    }

    #[test]
    fn flattened_positions_effects_and_private_context() {
        let snapshot = full_snapshot();
        assert_eq!(snapshot.title(), "Normal Mass");

        let positions = snapshot.positions();
        let order: Vec<_> = positions
            .iter()
            .map(|p| (p.flat, p.section, p.item, p.cue, p.cue_id))
            .collect();
        assert_eq!(
            order,
            vec![
                (0, 0, 0, 0, cue_id(1)),
                (1, 0, 0, 1, cue_id(2)),
                (2, 0, 1, 0, cue_id(3)),
                (3, 0, 1, 1, cue_id(4)),
                (4, 0, 2, 0, cue_id(5)),
                (5, 0, 2, 1, cue_id(6)),
                (6, 0, 2, 2, cue_id(7)),
                (7, 1, 0, 0, cue_id(8)),
                (8, 1, 0, 1, cue_id(9)),
            ]
        );
        assert_eq!(positions[0].item_id, item_id(1));
        assert_eq!(positions[6].item_id, item_id(3));
        assert_eq!(positions[7].item_id, item_id(4));

        for position in &positions {
            assert_eq!(snapshot.cue(position.flat).unwrap().id(), position.cue_id);
        }
        assert_eq!(snapshot.cue(positions.len()), None);

        let effects: Vec<_> = positions
            .iter()
            .map(|p| snapshot.cue(p.flat).unwrap().program_effect())
            .collect();
        assert_eq!(
            effects,
            vec![
                ProgramEffect::Frame(frame_id(1)),
                ProgramEffect::Frame(frame_id(2)),
                ProgramEffect::Black,
                ProgramEffect::Frame(frame_id(3)),
                ProgramEffect::Frame(frame_id(4)),
                ProgramEffect::Frame(frame_id(5)),
                ProgramEffect::NotPresentable,
                ProgramEffect::Black,
                ProgramEffect::Unchanged,
            ]
        );

        assert_eq!(snapshot.cue(0).unwrap().private_context(), None);
        assert_eq!(
            snapshot.cue(2).unwrap().private_context(),
            Some("Presider: The Lord be with you."),
            "an operator trigger keeps the phrase the congregation never sees"
        );
        assert_eq!(
            snapshot.cue(6).unwrap().private_context(),
            None,
            "an unavailable Cue may carry no private context"
        );
        assert_eq!(
            snapshot.cue(7).unwrap().private_context(),
            Some("Hold until the presider stands.")
        );
        assert_eq!(
            snapshot.cue(8).unwrap().private_context(),
            Some("Cantor sings the verse unprojected."),
            "a Wait Cue keeps what happens while Program is unchanged"
        );

        assert_eq!(
            snapshot.frame_ids(),
            BTreeSet::from([
                frame_id(1),
                frame_id(2),
                frame_id(3),
                frame_id(4),
                frame_id(5),
            ])
        );

        let listed = listed_themes();
        assert_eq!(snapshot.themes(), listed.as_slice());
        assert_eq!(snapshot.mass_theme(), &listed[0]);
        let song = &snapshot.sections()[0].items()[0];
        let prayer = &snapshot.sections()[1].items()[0];
        assert_eq!(snapshot.effective_theme(song), &listed[0]);
        assert_eq!(
            snapshot.effective_theme(prayer),
            &listed[1],
            "an item override wins over the Mass Theme"
        );
        assert_eq!(
            snapshot.sections()[0].items()[2].auto_play(),
            Some(AutoPlay { default_seconds: 8 })
        );
    }

    #[test]
    fn repeated_cues_may_share_one_frame() {
        let snapshot = presentation(vec![section(
            1,
            vec![item(
                1,
                MassItemKind::Song,
                vec![project(1, 1), project(2, 1)],
            )],
        )]);

        assert_eq!(snapshot.frame_ids(), BTreeSet::from([frame_id(1)]));
        assert_eq!(snapshot.positions().len(), 2);
    }

    #[test]
    fn duplicate_identities_are_rejected_across_the_snapshot() {
        let first = section(1, vec![item(1, MassItemKind::Song, vec![project(1, 1)])]);
        let second = section(1, vec![item(2, MassItemKind::Song, vec![project(2, 2)])]);
        assert_eq!(
            presentation_result(vec![first, second]),
            Err(SnapshotError::DuplicateSectionId { id: section_id(1) })
        );

        let cross_section = vec![
            section(1, vec![item(1, MassItemKind::Song, vec![project(1, 1)])]),
            section(2, vec![item(1, MassItemKind::Song, vec![project(2, 2)])]),
        ];
        assert_eq!(
            presentation_result(cross_section),
            Err(SnapshotError::DuplicateItemId { id: item_id(1) })
        );

        let same_item = vec![section(
            1,
            vec![item(
                1,
                MassItemKind::Song,
                vec![project(1, 1), project(1, 2)],
            )],
        )];
        assert_eq!(
            presentation_result(same_item),
            Err(SnapshotError::DuplicateCueId { id: cue_id(1) })
        );
    }

    #[test]
    fn empty_sections_items_and_theme_sets_are_rejected() {
        assert_eq!(
            MassSection::new(section_id(1), "Introductory Rites".to_owned(), vec![]),
            Err(SnapshotError::EmptySection {
                section: section_id(1)
            })
        );
        assert_eq!(
            MassItem::new(
                item_id(1),
                "Song".to_owned(),
                MassItemKind::Song,
                None,
                None,
                vec![],
            ),
            Err(SnapshotError::EmptyItem { item: item_id(1) })
        );
        assert_eq!(presentation_result(vec![]), Err(SnapshotError::NoSections));
        assert_eq!(
            PresentationSnapshot::new(
                snapshot_id(1),
                "Normal Mass".to_owned(),
                theme_id(1),
                vec![],
                vec![section(
                    1,
                    vec![item(1, MassItemKind::Song, vec![project(1, 1)])],
                )],
            ),
            Err(SnapshotError::NoThemes)
        );
    }

    #[test]
    fn theme_references_must_resolve_inside_the_snapshot() {
        let sections = || {
            vec![section(
                1,
                vec![item(1, MassItemKind::Song, vec![project(1, 1)])],
            )]
        };
        assert_eq!(
            PresentationSnapshot::new(
                snapshot_id(1),
                "Normal Mass".to_owned(),
                theme_id(9),
                listed_themes(),
                sections(),
            ),
            Err(SnapshotError::UnresolvedMassTheme { theme: theme_id(9) })
        );

        let overridden = MassItem::new(
            item_id(1),
            "Song".to_owned(),
            MassItemKind::Song,
            Some(theme_id(9)),
            None,
            vec![project(1, 1)],
        )
        .unwrap();
        assert_eq!(
            PresentationSnapshot::new(
                snapshot_id(1),
                "Normal Mass".to_owned(),
                theme_id(1),
                listed_themes(),
                vec![section(1, vec![overridden])],
            ),
            Err(SnapshotError::UnresolvedItemTheme {
                item: item_id(1),
                theme: theme_id(9),
            })
        );

        let mut listed = listed_themes();
        listed.push(theme(1, "Duplicate", ThemeKind::Custom));
        assert_eq!(
            PresentationSnapshot::new(
                snapshot_id(1),
                "Normal Mass".to_owned(),
                theme_id(1),
                listed,
                sections(),
            ),
            Err(SnapshotError::DuplicateThemeId { id: theme_id(1) })
        );
    }

    #[test]
    fn auto_play_is_confined_to_general_presentations() {
        assert_eq!(
            MassItem::new(
                item_id(1),
                "Song".to_owned(),
                MassItemKind::Song,
                None,
                Some(AutoPlay { default_seconds: 8 }),
                vec![project(1, 1)],
            ),
            Err(SnapshotError::AutoPlayOnNonGeneralItem { item: item_id(1) })
        );
        assert_eq!(
            MassItem::new(
                item_id(1),
                "Announcements".to_owned(),
                MassItemKind::GeneralPresentation,
                None,
                Some(AutoPlay { default_seconds: 0 }),
                vec![project(1, 1)],
            ),
            Err(SnapshotError::ZeroAutoPlayDuration { item: item_id(1) })
        );
        assert_eq!(
            Cue::new(
                cue_id(1),
                "Zero".to_owned(),
                None,
                CueBehavior::Project {
                    frame: frame_id(1),
                    duration_seconds: Some(0),
                },
            ),
            Err(SnapshotError::ZeroCueDuration { cue: cue_id(1) })
        );
        assert_eq!(
            MassItem::new(
                item_id(1),
                "Song".to_owned(),
                MassItemKind::Song,
                None,
                None,
                vec![
                    Cue::new(
                        cue_id(1),
                        "Timed".to_owned(),
                        None,
                        CueBehavior::Project {
                            frame: frame_id(1),
                            duration_seconds: Some(3),
                        },
                    )
                    .unwrap(),
                ],
            ),
            Err(SnapshotError::DurationOutsideAutoPlayItem { cue: cue_id(1) })
        );

        let timed = MassItem::new(
            item_id(1),
            "Announcements".to_owned(),
            MassItemKind::GeneralPresentation,
            None,
            Some(AutoPlay { default_seconds: 8 }),
            vec![
                project(1, 1),
                Cue::new(
                    cue_id(2),
                    "Short".to_owned(),
                    None,
                    CueBehavior::Project {
                        frame: frame_id(2),
                        duration_seconds: Some(3),
                    },
                )
                .unwrap(),
            ],
        )
        .unwrap();
        assert_eq!(timed.auto_play(), Some(AutoPlay { default_seconds: 8 }));
    }

    #[test]
    fn operator_only_cues_require_private_context() {
        assert_eq!(
            Cue::new(
                cue_id(1),
                "Trigger".to_owned(),
                None,
                CueBehavior::OperatorTrigger,
            ),
            Err(SnapshotError::MissingPrivateContext { cue: cue_id(1) })
        );
        assert_eq!(
            Cue::new(
                cue_id(2),
                "Wait".to_owned(),
                Some("   ".to_owned()),
                CueBehavior::Wait,
            ),
            Err(SnapshotError::MissingPrivateContext { cue: cue_id(2) })
        );

        let trigger = Cue::new(
            cue_id(3),
            "Trigger".to_owned(),
            Some("The Lord be with you.".to_owned()),
            CueBehavior::OperatorTrigger,
        )
        .unwrap();
        assert_eq!(
            trigger.private_context(),
            Some("The Lord be with you."),
            "the trigger phrase stays with the operator"
        );
        assert_eq!(trigger.program_effect(), ProgramEffect::Black);

        assert!(
            Cue::new(cue_id(4), "Hold".to_owned(), None, CueBehavior::Hold).is_ok(),
            "Hold's private context is optional"
        );
        assert!(
            Cue::new(
                cue_id(5),
                "Slide".to_owned(),
                None,
                CueBehavior::Project {
                    frame: frame_id(1),
                    duration_seconds: None,
                },
            )
            .is_ok(),
            "a projectable Cue needs no private context"
        );
    }
}
