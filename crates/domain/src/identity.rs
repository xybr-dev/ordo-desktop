//! Stable entity identity and creation time.
//!
//! Ordo identities are UUID version 4 values stored canonically in lowercase
//! (Decision 38). Parsing accepts exactly that form, so one identity has one
//! text form and one byte form: every `Display` output parses back to the same
//! value, and two records cannot disagree about an identity's spelling.
//!
//! Random bytes come from the caller, never from this module: the domain stays
//! free of OS entropy sources. [`EntityId::from_random_bytes`] applies the
//! version and RFC 4122 variant bits, so a caller-supplied random array always
//! becomes a valid version 4 identity.
//!
//! [`Timestamp`] is UTC milliseconds since the Unix epoch with one canonical
//! text form, `YYYY-MM-DDTHH:MM:SS.sssZ`. The codec is dependency-free civil
//! arithmetic so the domain crate keeps an empty dependency list.

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

    /// Build a version 4 identity from caller-supplied random bytes.
    ///
    /// The version nibble is set to 4 and the variant nibble to the RFC 4122
    /// range, so the result always survives [`EntityId::parse`]. This is the
    /// only constructor for new identities; storage injects the random source
    /// so a test can force a collision.
    pub const fn from_random_bytes(mut bytes: [u8; 16]) -> Self {
        bytes[6] = (bytes[6] & 0x0f) | 0x40;
        bytes[8] = (bytes[8] & 0x3f) | 0x80;
        Self(bytes)
    }

    /// The 16 identity bytes, for fingerprinting and comparison.
    pub const fn as_bytes(&self) -> &[u8; 16] {
        &self.0
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

/// One UTC instant in milliseconds since the Unix epoch.
///
/// The canonical text form is `YYYY-MM-DDTHH:MM:SS.sssZ`, always with three
/// fractional digits and an uppercase `T` and `Z`. Parsing rejects every other
/// spelling, including an offset, a missing fraction, a space separator, and a
/// date the Gregorian calendar does not contain. Installation clocks are
/// expected to stay inside years 0000 through 9999, the range the four-digit
/// year field can spell.
#[derive(Clone, Copy, Eq, Hash, Ord, PartialEq, PartialOrd)]
pub struct Timestamp(i64);

impl Timestamp {
    /// Build from UTC milliseconds since `1970-01-01T00:00:00.000Z`.
    pub const fn from_millis(millis: i64) -> Self {
        Self(millis)
    }

    /// UTC milliseconds since `1970-01-01T00:00:00.000Z`.
    pub const fn millis(self) -> i64 {
        self.0
    }

    /// Parse the one canonical text form.
    ///
    /// Every field is range-checked, including leap-year rules for February,
    /// so a parsed value always prints back as the same text.
    pub fn parse(text: &str) -> Result<Self, TimestampError> {
        let bytes = text.as_bytes();
        if bytes.len() != 24 {
            return Err(TimestampError::Length(bytes.len()));
        }
        if bytes[4] != b'-'
            || bytes[7] != b'-'
            || bytes[10] != b'T'
            || bytes[13] != b':'
            || bytes[16] != b':'
            || bytes[19] != b'.'
            || bytes[23] != b'Z'
        {
            return Err(TimestampError::Malformed);
        }
        let year = decimal(bytes, 0, 4)?;
        let month = decimal(bytes, 5, 2)?;
        let day = decimal(bytes, 8, 2)?;
        let hour = decimal(bytes, 11, 2)?;
        let minute = decimal(bytes, 14, 2)?;
        let second = decimal(bytes, 17, 2)?;
        let millis = decimal(bytes, 20, 3)?;
        let calendar_valid = (1..=12).contains(&month)
            && day != 0
            && day <= days_in_month(year, month)
            && hour <= 23
            && minute <= 59
            && second <= 59;
        if !calendar_valid {
            return Err(TimestampError::InvalidDate);
        }

        let days = days_from_civil(i64::from(year), month, day);
        let seconds = i64::from(hour) * 3_600 + i64::from(minute) * 60 + i64::from(second);
        Ok(Self(
            days * 86_400_000 + seconds * 1_000 + i64::from(millis),
        ))
    }

    /// The canonical text form, or `None` when the instant falls outside years
    /// 0000 through 9999.
    ///
    /// This is the fallible counterpart to the `Display` text form for
    /// callers that must not emit a year the four-digit canonical field cannot
    /// spell.
    pub fn canonical_text(self) -> Option<String> {
        let (year, month, day, hour, minute, second, millis) = self.canonical_parts();
        if !(0..=9999).contains(&year) {
            return None;
        }
        Some(format!(
            "{year:04}-{month:02}-{day:02}T{hour:02}:{minute:02}:{second:02}.{millis:03}Z"
        ))
    }

    /// The fields of the canonical text form.
    fn canonical_parts(self) -> (i64, u32, u32, i64, i64, i64, i64) {
        let total_seconds = self.0.div_euclid(1_000);
        let millis = self.0.rem_euclid(1_000);
        let days = total_seconds.div_euclid(86_400);
        let seconds_of_day = total_seconds.rem_euclid(86_400);
        let (year, month, day) = civil_from_days(days);
        let hour = seconds_of_day / 3_600;
        let minute = (seconds_of_day % 3_600) / 60;
        let second = seconds_of_day % 60;
        (year, month, day, hour, minute, second, millis)
    }

    /// Write the canonical text form.
    fn write_canonical(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        let (year, month, day, hour, minute, second, millis) = self.canonical_parts();
        write!(
            f,
            "{year:04}-{month:02}-{day:02}T{hour:02}:{minute:02}:{second:02}.{millis:03}Z"
        )
    }
}

impl FromStr for Timestamp {
    type Err = TimestampError;

    fn from_str(text: &str) -> Result<Self, Self::Err> {
        Self::parse(text)
    }
}

/// Write the canonical form when the instant is inside years 0000 through 9999.
///
/// Values outside that range still print, but their year is not the canonical
/// four-digit field; use [`Timestamp::canonical_text`] when a canonical string
/// is required.
impl fmt::Display for Timestamp {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        self.write_canonical(f)
    }
}

impl fmt::Debug for Timestamp {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        self.write_canonical(f)
    }
}

/// Why a text value is not a canonical Ordo timestamp.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum TimestampError {
    /// The text has this many bytes; the canonical form has exactly 24.
    Length(usize),
    /// A separator is missing, misplaced, or not the canonical character.
    Malformed,
    /// A field is outside its calendar range, including non-leap Februaries.
    InvalidDate,
}

impl fmt::Display for TimestampError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Length(len) => write!(
                f,
                "timestamp text has {len} bytes; the canonical form has 24"
            ),
            Self::Malformed => f.write_str("timestamp text is not YYYY-MM-DDTHH:MM:SS.sssZ"),
            Self::InvalidDate => {
                f.write_str("timestamp names a date or time the calendar does not contain")
            }
        }
    }
}

impl std::error::Error for TimestampError {}

/// The lowercase hexadecimal value of one nibble, or `None` when `byte` cannot
/// appear in canonical text.
pub(crate) fn hex_digit(byte: u8) -> Option<u8> {
    match byte {
        b'0'..=b'9' => Some(byte - b'0'),
        b'a'..=b'f' => Some(byte - b'a' + 10),
        _ => None,
    }
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

                /// Wrap an untyped identity as this kind of handle.
                pub const fn from_entity(entity: EntityId) -> Self {
                    Self(entity)
                }

                /// The untyped identity behind this typed handle.
                pub const fn entity(self) -> EntityId {
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

pub(crate) use typed_entity_ids;

typed_entity_ids! {
    /// Identifies the single Parish held by one local store.
    ParishId,
    /// Identifies one Choir.
    ChoirId,
    /// Identifies one Song work.
    SongId,
    /// Identifies one Arrangement of a Song work.
    ArrangementId,
    /// Identifies one reusable section inside an Arrangement.
    SectionId,
    /// Identifies one ordered occurrence of a section in an Arrangement.
    OccurrenceId,
    /// Identifies one reusable visual Theme.
    ThemeId,
    /// Identifies one managed binary asset.
    ManagedAssetId,
    /// Identifies one Edit revision.
    RevisionId,
    /// Groups the Edit revisions of one editing session.
    HistorySessionId,
}

/// Parse one decimal field at a fixed position in canonical timestamp text.
fn decimal(bytes: &[u8], start: usize, len: usize) -> Result<u32, TimestampError> {
    let mut value = 0_u32;
    for byte in &bytes[start..start + len] {
        let digit = match byte {
            b'0'..=b'9' => u32::from(byte - b'0'),
            _ => return Err(TimestampError::Malformed),
        };
        value = value * 10 + digit;
    }
    Ok(value)
}

fn is_leap_year(year: u32) -> bool {
    year.is_multiple_of(4) && (!year.is_multiple_of(100) || year.is_multiple_of(400))
}

fn days_in_month(year: u32, month: u32) -> u32 {
    match month {
        1 | 3 | 5 | 7 | 8 | 10 | 12 => 31,
        4 | 6 | 9 | 11 => 30,
        2 if is_leap_year(year) => 29,
        2 => 28,
        _ => 0,
    }
}

/// Days since 1970-01-01 for a proleptic Gregorian date (Howard Hinnant's
/// civil-from-days inverse), implemented locally so the domain keeps no date
/// dependency.
fn days_from_civil(year: i64, month: u32, day: u32) -> i64 {
    let year = year - i64::from(month <= 2);
    let era = if year >= 0 { year } else { year - 399 } / 400;
    let year_of_era = year - era * 400;
    let month_prime = if month > 2 {
        i64::from(month) - 3
    } else {
        i64::from(month) + 9
    };
    let day_of_year = (153 * month_prime + 2) / 5 + i64::from(day) - 1;
    let day_of_era = year_of_era * 365 + year_of_era / 4 - year_of_era / 100 + day_of_year;
    era * 146_097 + day_of_era - 719_468
}

/// The proleptic Gregorian date for a count of days since 1970-01-01.
fn civil_from_days(days: i64) -> (i64, u32, u32) {
    let shifted = days + 719_468;
    let era = if shifted >= 0 {
        shifted
    } else {
        shifted - 146_096
    } / 146_097;
    let day_of_era = shifted - era * 146_097;
    let year_of_era =
        (day_of_era - day_of_era / 1_460 + day_of_era / 36_524 - day_of_era / 146_096) / 365;
    let year = year_of_era + era * 400;
    let day_of_year = day_of_era - (365 * year_of_era + year_of_era / 4 - year_of_era / 100);
    let month_prime = (5 * day_of_year + 2) / 153;
    let day = day_of_year - (153 * month_prime + 2) / 5 + 1;
    let month = if month_prime < 10 {
        month_prime + 3
    } else {
        month_prime - 9
    };
    (
        year + i64::from(month <= 2),
        u32::try_from(month).unwrap_or(0),
        u32::try_from(day).unwrap_or(0),
    )
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn random_bytes_become_a_canonical_version_four_identity() {
        let id = EntityId::from_random_bytes([0xff; 16]);
        assert_eq!(
            id.to_string(),
            "ffffffff-ffff-4fff-bfff-ffffffffffff",
            "version and variant bits are forced, everything else is preserved"
        );
        assert_eq!(
            id.as_bytes(),
            &[
                0xff, 0xff, 0xff, 0xff, 0xff, 0xff, 0x4f, 0xff, 0xbf, 0xff, 0xff, 0xff, 0xff, 0xff,
                0xff, 0xff
            ]
        );
        assert_eq!(EntityId::parse(&id.to_string()), Ok(id));
    }

    #[test]
    fn typed_identities_wrap_and_recover_the_untyped_identity() {
        let entity = EntityId::parse("123e4567-e89b-42d3-a456-426614174000").unwrap();
        assert_eq!(ParishId::from_entity(entity).entity(), entity);
        assert_eq!(ChoirId::from_entity(entity).entity(), entity);
        assert_eq!(SongId::from_entity(entity).entity(), entity);
        assert_eq!(ArrangementId::from_entity(entity).entity(), entity);
        assert_eq!(SectionId::from_entity(entity).entity(), entity);
        assert_eq!(OccurrenceId::from_entity(entity).entity(), entity);
        assert_eq!(ThemeId::from_entity(entity).entity(), entity);
        assert_eq!(ManagedAssetId::from_entity(entity).entity(), entity);
        assert_eq!(RevisionId::from_entity(entity).entity(), entity);
    }

    #[test]
    fn timestamps_round_trip_through_canonical_text() {
        for (text, millis) in [
            ("1970-01-01T00:00:00.000Z", 0),
            ("1969-12-31T23:59:59.999Z", -1),
            ("2000-01-01T00:00:00.000Z", 946_684_800_000),
            ("2024-02-29T00:00:00.000Z", 1_709_164_800_000),
            ("2024-02-29T12:34:56.789Z", 1_709_210_096_789),
            ("9999-12-31T23:59:59.999Z", 253_402_300_799_999),
        ] {
            let parsed = Timestamp::parse(text).expect(text);
            assert_eq!(parsed.millis(), millis, "{text}");
            assert_eq!(parsed.to_string(), text);
            assert_eq!(Timestamp::from_millis(millis).to_string(), text);
        }
    }

    #[test]
    fn canonical_text_reports_the_supported_year_range() {
        let first = Timestamp::parse("0000-01-01T00:00:00.000Z").expect("first canonical");
        let last = Timestamp::parse("9999-12-31T23:59:59.999Z").expect("last canonical");

        assert_eq!(
            first.canonical_text(),
            Some("0000-01-01T00:00:00.000Z".to_owned())
        );
        assert_eq!(
            last.canonical_text(),
            Some("9999-12-31T23:59:59.999Z".to_owned())
        );
        assert_eq!(
            Timestamp::from_millis(first.millis()).canonical_text(),
            Some("0000-01-01T00:00:00.000Z".to_owned())
        );
        assert_eq!(
            Timestamp::from_millis(last.millis()).canonical_text(),
            Some("9999-12-31T23:59:59.999Z".to_owned())
        );
        assert_eq!(first.to_string(), "0000-01-01T00:00:00.000Z");
        assert_eq!(last.to_string(), "9999-12-31T23:59:59.999Z");

        assert!(
            Timestamp::from_millis(first.millis() - 1)
                .canonical_text()
                .is_none(),
            "one millisecond before year 0000 is out of range"
        );
        assert!(
            Timestamp::from_millis(last.millis() + 1)
                .canonical_text()
                .is_none(),
            "one millisecond after year 9999 is out of range"
        );
        assert!(Timestamp::from_millis(i64::MIN).canonical_text().is_none());
        assert!(Timestamp::from_millis(i64::MAX).canonical_text().is_none());
    }

    #[test]
    fn timestamp_parsing_enforces_leap_years_and_field_ranges() {
        assert!(Timestamp::parse("2000-02-29T00:00:00.000Z").is_ok());
        assert_eq!(
            Timestamp::parse("1900-02-29T00:00:00.000Z"),
            Err(TimestampError::InvalidDate),
            "1900 is divisible by 100 but not 400"
        );
        assert_eq!(
            Timestamp::parse("2023-02-29T00:00:00.000Z"),
            Err(TimestampError::InvalidDate)
        );
        for text in [
            "2024-00-01T00:00:00.000Z",
            "2024-13-01T00:00:00.000Z",
            "2024-01-00T00:00:00.000Z",
            "2024-01-32T00:00:00.000Z",
            "2024-01-01T24:00:00.000Z",
            "2024-01-01T00:60:00.000Z",
            "2024-01-01T00:00:60.000Z",
        ] {
            assert_eq!(
                Timestamp::parse(text),
                Err(TimestampError::InvalidDate),
                "{text}"
            );
        }
    }

    #[test]
    fn timestamp_parsing_rejects_non_canonical_spellings() {
        assert_eq!(
            Timestamp::parse("2024-01-01T00:00:00.000"),
            Err(TimestampError::Length(23))
        );
        assert_eq!(
            Timestamp::parse("2024-01-01T00:00:00.000ZZ"),
            Err(TimestampError::Length(25))
        );
        for text in [
            "2024-01-01 00:00:00.000Z",
            "2024-01-01t00:00:00.000Z",
            "2024-01-01T00:00:00.000z",
            "2024-01-01T00:00:00Z",
            "2024-01-01T00:00:00.000+00:00",
            "2024-01-01T00:00:00.00aZ",
            "2024-01-01T00:00:00.000X",
        ] {
            assert!(
                matches!(
                    Timestamp::parse(text),
                    Err(TimestampError::Malformed | TimestampError::Length(_))
                ),
                "{text}: {:?}",
                Timestamp::parse(text)
            );
        }
    }

    #[test]
    fn timestamp_errors_display_a_message() {
        for error in [
            TimestampError::Length(3),
            TimestampError::Malformed,
            TimestampError::InvalidDate,
        ] {
            assert!(!error.to_string().is_empty());
        }
    }
}
