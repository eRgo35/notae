//! Stable identities: numeric note IDs, file fingerprints, and validated paths.

use std::fmt;
use std::str::FromStr;
use std::time::{Duration, SystemTime, UNIX_EPOCH};

use serde::de::{Error as _, Unexpected};
use serde::{Deserialize, Deserializer, Serialize};

use crate::DomainError;

/// A globally unique, non-negative note number.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash, Serialize, Deserialize)]
#[serde(transparent)]
pub struct NoteId(u32);

impl NoteId {
    /// Creates a note ID.
    #[must_use]
    pub const fn new(value: u32) -> Self {
        Self(value)
    }

    /// Returns the numeric value.
    #[must_use]
    pub const fn get(self) -> u32 {
        self.0
    }

    /// Parses a decimal, non-negative integer.
    pub fn parse(text: &str) -> Result<Self, DomainError> {
        text.parse::<u32>()
            .map(Self)
            .map_err(|_| DomainError::InvalidNoteId(text.to_string()))
    }
}

impl fmt::Display for NoteId {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{}", self.0)
    }
}

impl FromStr for NoteId {
    type Err = DomainError;

    fn from_str(s: &str) -> Result<Self, Self::Err> {
        Self::parse(s)
    }
}

/// A lowercase hex-encoded SHA-256 digest.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash, Serialize, Deserialize)]
#[serde(transparent)]
pub struct Sha256Digest([u8; 32]);

impl Sha256Digest {
    /// Wraps raw digest bytes.
    #[must_use]
    pub const fn from_bytes(bytes: [u8; 32]) -> Self {
        Self(bytes)
    }

    /// Returns the raw digest bytes.
    #[must_use]
    pub const fn as_bytes(&self) -> &[u8; 32] {
        &self.0
    }

    /// Parses exactly 64 lowercase hex characters.
    pub fn parse(text: &str) -> Result<Self, DomainError> {
        if text.len() != 64 {
            return Err(DomainError::InvalidSha256Digest);
        }
        let mut bytes = [0u8; 32];
        let (pairs, rest) = text.as_bytes().as_chunks::<2>();
        if !rest.is_empty() {
            return Err(DomainError::InvalidSha256Digest);
        }
        for (index, pair) in pairs.iter().enumerate() {
            let hi = hex_value(pair[0]).ok_or(DomainError::InvalidSha256Digest)?;
            let lo = hex_value(pair[1]).ok_or(DomainError::InvalidSha256Digest)?;
            bytes[index] = (hi << 4) | lo;
        }
        Ok(Self(bytes))
    }
}

fn hex_value(byte: u8) -> Option<u8> {
    match byte {
        b'0'..=b'9' => Some(byte - b'0'),
        b'a'..=b'f' => Some(byte - b'a' + 10),
        _ => None,
    }
}

impl fmt::Display for Sha256Digest {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        for byte in self.0 {
            write!(f, "{byte:02x}")?;
        }
        Ok(())
    }
}

impl FromStr for Sha256Digest {
    type Err = DomainError;

    fn from_str(s: &str) -> Result<Self, Self::Err> {
        Self::parse(s)
    }
}

const NANOS_PER_SEC: u32 = 1_000_000_000;

/// A point in time expressed as signed seconds and nanoseconds since
/// [`UNIX_EPOCH`].
///
/// Negative seconds are valid and round-trip exactly, which the platform
/// [`SystemTime`] serializer cannot guarantee: it refuses every instant before
/// the epoch. Filesystem timestamps may predate 1970, so the domain stores this
/// explicit representation instead and exposes [`SystemTime`] conversion.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash, Serialize)]
pub struct EpochTimestamp {
    /// Whole seconds since the epoch; negative before 1970.
    secs: i64,
    /// Sub-second nanoseconds, always below one billion.
    #[serde(default)]
    nanos: u32,
}

impl EpochTimestamp {
    /// Creates a timestamp from signed seconds and nanoseconds.
    ///
    /// # Errors
    /// Returns `None` when `nanos` is not below one billion.
    #[must_use]
    pub const fn new(secs: i64, nanos: u32) -> Option<Self> {
        if nanos >= NANOS_PER_SEC {
            return None;
        }
        Some(Self { secs, nanos })
    }

    /// Converts a [`SystemTime`] into a portable timestamp.
    #[must_use]
    pub fn from_system_time(time: SystemTime) -> Option<Self> {
        if let Ok(delta) = time.duration_since(UNIX_EPOCH) {
            return Self::new(
                i64::try_from(delta.as_secs()).ok()?,
                delta.subsec_nanos(),
            );
        }
        let delta = UNIX_EPOCH.duration_since(time).ok()?;
        let secs = i64::try_from(delta.as_secs()).ok()?;
        if delta.subsec_nanos() == 0 {
            // `-i64::MIN` is the one unrepresentable negation, and a
            // pre-epoch `SystemTime` never reaches it, but the checked form
            // keeps the range rule in one place.
            return Self::new(secs.checked_neg()?, 0);
        }
        // Borrow one whole second: `UNIX_EPOCH - (N sec, 500 ms)` is
        // `-(N + 1) sec, 500 ms` counted the same way. At the negative
        // boundary `N` is `i64::MAX`, so the borrowed second does not fit and
        // the instant is reported as unrepresentable instead of overflowing.
        Self::new(
            secs.checked_add(1)?.checked_neg()?,
            NANOS_PER_SEC - delta.subsec_nanos(),
        )
    }

    /// Captures the current wall-clock time as a portable timestamp.
    ///
    /// Returns `None` only when the platform reports a time outside the
    /// representable range — the instants the domain can describe are the
    /// same ones a [`SystemTime`] can describe, so this matches the
    /// behaviour of [`from_system_time`](Self::from_system_time).
    #[must_use]
    pub fn now() -> Option<Self> {
        Self::from_system_time(SystemTime::now())
    }

    /// Returns the seconds since the epoch; negative before 1970.
    #[must_use]
    pub const fn seconds(self) -> i64 {
        self.secs
    }

    /// Returns the sub-second nanoseconds.
    #[must_use]
    pub const fn subsec_nanos(self) -> u32 {
        self.nanos
    }

    /// Converts back to a [`SystemTime`].
    #[must_use]
    pub fn to_system_time(self) -> Option<SystemTime> {
        // The value is `epoch + secs + nanos`; `nanos` is always positive, so
        // negative seconds must be applied before the sub-second remainder.
        let whole = Duration::from_secs(self.secs.unsigned_abs());
        let whole = if self.secs < 0 {
            UNIX_EPOCH.checked_sub(whole)?
        } else {
            UNIX_EPOCH.checked_add(whole)?
        };
        whole.checked_add(Duration::new(0, self.nanos))
    }
}

/// Observed file identity used to recognise renamed or changed PDFs.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct PdfFingerprint {
    /// SHA-256 of the file contents.
    sha256: Sha256Digest,
    /// Size of the file in bytes.
    byte_len: u64,
    /// Observed modification time, when the platform reports one.
    #[serde(default)]
    modified: Option<EpochTimestamp>,
}

impl PdfFingerprint {
    /// Creates a fingerprint from its parts.
    #[must_use]
    pub const fn new(sha256: Sha256Digest, byte_len: u64, modified: Option<EpochTimestamp>) -> Self {
        Self {
            sha256,
            byte_len,
            modified,
        }
    }

    /// Creates a fingerprint from a platform modification time.
    #[must_use]
    pub fn from_system_time(
        sha256: Sha256Digest,
        byte_len: u64,
        modified: Option<SystemTime>,
    ) -> Option<Self> {
        let modified = match modified {
            Some(time) => Some(EpochTimestamp::from_system_time(time)?),
            None => None,
        };
        Some(Self::new(sha256, byte_len, modified))
    }

    /// Returns the SHA-256 of the file contents.
    #[must_use]
    pub const fn sha256(&self) -> Sha256Digest {
        self.sha256
    }

    /// Returns the file size in bytes.
    #[must_use]
    pub const fn byte_len(&self) -> u64 {
        self.byte_len
    }

    /// Returns the observed modification time.
    #[must_use]
    pub const fn modified(&self) -> Option<EpochTimestamp> {
        self.modified
    }
}

/// A validated, forward-slash-separated path relative to the library root.
#[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord, Hash, Serialize)]
#[serde(transparent)]
pub struct RelativePdfPath(String);

impl RelativePdfPath {
    /// Validates and stores a root-relative path.
    ///
    /// # Errors
    /// Returns [`DomainError::InvalidRelativePath`] for absolute paths, empty
    /// paths, backslashes, empty segments, `.` or `..` segments, and Windows
    /// drive prefixes such as `C:/notes.pdf`.
    pub fn new(path: impl Into<String>) -> Result<Self, DomainError> {
        let path = path.into();
        if path.is_empty()
            || path.starts_with('/')
            || path.contains('\\')
            || path.contains("//")
            || path.ends_with('/')
            || has_drive_prefix(&path)
        {
            return Err(DomainError::InvalidRelativePath(path));
        }
        if path
            .split('/')
            .any(|segment| segment.is_empty() || segment == "." || segment == "..")
        {
            return Err(DomainError::InvalidRelativePath(path));
        }
        Ok(Self(path))
    }

    /// Parses a root-relative path from text.
    ///
    /// # Errors
    /// See [`RelativePdfPath::new`].
    pub fn parse(path: &str) -> Result<Self, DomainError> {
        Self::new(path)
    }

    /// Returns the path as a string slice.
    #[must_use]
    pub fn as_str(&self) -> &str {
        &self.0
    }
}

/// Returns `true` for a Windows drive prefix such as `C:`.
///
/// Only a leading ASCII letter followed by `:` is a drive prefix; a colon
/// anywhere else, such as the directory `Scores: Theory`, is a legal
/// POSIX name character.
fn has_drive_prefix(path: &str) -> bool {
    let bytes = path.as_bytes();
    matches!(bytes, [drive, b':', ..] if drive.is_ascii_alphabetic())
}

impl fmt::Display for RelativePdfPath {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(&self.0)
    }
}

impl FromStr for RelativePdfPath {
    type Err = DomainError;

    fn from_str(s: &str) -> Result<Self, Self::Err> {
        Self::parse(s)
    }
}

impl<'de> Deserialize<'de> for RelativePdfPath {
    fn deserialize<D: Deserializer<'de>>(deserializer: D) -> Result<Self, D::Error> {
        struct Visitor;

        impl serde::de::Visitor<'_> for Visitor {
            type Value = RelativePdfPath;

            fn expecting(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
                f.write_str("a validated root-relative PDF path")
            }

            fn visit_str<E: serde::de::Error>(self, value: &str) -> Result<Self::Value, E> {
                RelativePdfPath::new(value).map_err(E::custom)
            }
        }

        deserializer.deserialize_str(Visitor)
    }
}

impl<'de> Deserialize<'de> for EpochTimestamp {
    /// Validates the invariant the constructor enforces, so a hand-written or
    /// hostile sidecar cannot smuggle in out-of-range nanoseconds.
    fn deserialize<D: Deserializer<'de>>(deserializer: D) -> Result<Self, D::Error> {
        #[derive(Deserialize)]
        struct Raw {
            secs: i64,
            #[serde(default)]
            nanos: u32,
        }

        let raw = Raw::deserialize(deserializer)?;
        let nanos = raw.nanos;
        EpochTimestamp::new(raw.secs, nanos)
            .ok_or_else(|| D::Error::custom(Unexpected::Unsigned(u64::from(nanos))))
    }
}