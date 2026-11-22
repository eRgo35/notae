//! Parsing and lookup of the PC sidecar (`.notae/library.json`).

use std::collections::HashMap;

use notae_domain::{SIDECAR_VERSION, Sha256Digest, SidecarEntry, SidecarFile};
use sha2::{Digest, Sha256};

/// Why a sidecar could not be used.
#[derive(Debug, Clone, PartialEq, Eq, thiserror::Error)]
pub enum SidecarError {
    /// The bytes are not a valid sidecar document.
    #[error("malformed sidecar: {detail}")]
    Malformed {
        /// Parser message.
        detail: String,
    },
    /// The sidecar was written in a format version this build does not read.
    #[error("unsupported sidecar version {found}")]
    UnsupportedVersion {
        /// Version found in the file.
        found: u32,
    },
    /// An entry's `sha256` is not a lowercase-hex SHA-256 digest.
    #[error("bad sha256 for {path}")]
    BadHash {
        /// Path of the offending entry.
        path: String,
    },
}

/// A validated sidecar, indexed by score path.
#[derive(Debug, Clone, Default)]
pub struct Sidecar {
    entries: HashMap<String, SidecarEntry>,
}

impl Sidecar {
    /// Parses and validates the bytes of a `library.json`.
    pub fn parse(bytes: &[u8]) -> Result<Sidecar, SidecarError> {
        let file: SidecarFile =
            serde_json::from_slice(bytes).map_err(|e| SidecarError::Malformed {
                detail: e.to_string(),
            })?;
        if file.version != SIDECAR_VERSION {
            return Err(SidecarError::UnsupportedVersion {
                found: file.version,
            });
        }
        let mut entries = HashMap::with_capacity(file.scores.len());
        for entry in file.scores {
            if Sha256Digest::parse(&entry.sha256).is_err() {
                return Err(SidecarError::BadHash { path: entry.path });
            }
            entries.insert(entry.path.clone(), entry);
        }
        Ok(Sidecar { entries })
    }

    /// The entry for `path`, only when its recorded size equals `size`.
    pub fn matching(&self, path: &str, size: u64) -> Option<&SidecarEntry> {
        self.entries.get(path).filter(|e| e.size == size)
    }
}

/// Lowercase hex SHA-256 of the entry's JSON form, to detect metadata changes.
pub fn entry_digest(entry: &SidecarEntry) -> String {
    let bytes = serde_json::to_vec(entry).unwrap_or_default();
    Sha256::digest(&bytes)
        .iter()
        .map(|b| format!("{b:02x}"))
        .collect()
}

#[cfg(test)]
mod tests {
    use super::*;

    const SHA: &str = "0123456789abcdef0123456789abcdef0123456789abcdef0123456789abcdef";

    fn doc(version: u32, sha: &str, tags: &str) -> String {
        format!(
            r#"{{"version":{version},"generator":"t","scores":[{{"path":"a.pdf","size":10,"sha256":"{sha}","number":null,"title":null,"composer":null,"tags":{tags},"text":null}}]}}"#
        )
    }

    #[test]
    fn parses_and_matches_on_path_and_size() {
        let s = Sidecar::parse(doc(1, SHA, "[]").as_bytes()).unwrap();
        assert!(s.matching("a.pdf", 10).is_some());
        assert!(s.matching("a.pdf", 11).is_none());
        assert!(s.matching("b.pdf", 10).is_none());
    }

    #[test]
    fn rejects_unknown_version() {
        let err = Sidecar::parse(doc(2, SHA, "[]").as_bytes()).unwrap_err();
        assert_eq!(err, SidecarError::UnsupportedVersion { found: 2 });
    }

    #[test]
    fn rejects_malformed_json() {
        let err = Sidecar::parse(b"not json").unwrap_err();
        assert!(matches!(err, SidecarError::Malformed { .. }));
    }

    #[test]
    fn rejects_an_entry_with_a_bad_sha() {
        let err = Sidecar::parse(doc(1, "xyz", "[]").as_bytes()).unwrap_err();
        assert_eq!(
            err,
            SidecarError::BadHash {
                path: "a.pdf".into()
            }
        );
    }

    #[test]
    fn digest_changes_when_tags_change() {
        let a = Sidecar::parse(doc(1, SHA, "[]").as_bytes()).unwrap();
        let b = Sidecar::parse(doc(1, SHA, r#"["x"]"#).as_bytes()).unwrap();
        let da = entry_digest(a.matching("a.pdf", 10).unwrap());
        let db = entry_digest(b.matching("a.pdf", 10).unwrap());
        assert_ne!(da, db);
        assert_eq!(da.len(), 64);
    }
}
