//! Stable identities: numeric note IDs, file fingerprints, and validated paths.

use std::fmt;
use std::str::FromStr;
use std::time::SystemTime;

use serde::{Deserialize, Serialize};

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

/// Observed file identity used to recognise renamed or changed PDFs.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct PdfFingerprint {
    /// SHA-256 of the file contents.
    pub sha256: Sha256Digest,
    /// Size of the file in bytes.
    pub byte_len: u64,
    /// Observed modification time, when the platform reports one.
    #[serde(default)]
    pub modified: Option<SystemTime>,
}

impl PdfFingerprint {
    /// Creates a fingerprint from its parts.
    #[must_use]
    pub const fn new(sha256: Sha256Digest, byte_len: u64, modified: Option<SystemTime>) -> Self {
        Self {
            sha256,
            byte_len,
            modified,
        }
    }
}

/// A validated, forward-slash-separated path relative to the library root.
#[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord, Hash, Serialize, Deserialize)]
#[serde(transparent)]
pub struct RelativePdfPath(String);

impl RelativePdfPath {
    /// Validates and stores a root-relative path.
    ///
    /// # Errors
    /// Returns [`DomainError::InvalidRelativePath`] for absolute paths, empty
    /// paths, backslashes, empty segments, or `..` traversal.
    pub fn new(path: impl Into<String>) -> Result<Self, DomainError> {
        let path = path.into();
        if path.is_empty()
            || path.starts_with('/')
            || path.contains('\\')
            || path.contains("//")
            || path.ends_with('/')
            || path.contains(':')
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