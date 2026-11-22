//! The `.notae/library.json` sidecar a PC tool writes next to a score library.

use serde::{Deserialize, Serialize};

/// Version of the sidecar format this build reads and writes.
pub const SIDECAR_VERSION: u32 = 1;
/// Sidecar directory, relative to the library root.
pub const SIDECAR_DIR: &str = ".notae";
/// Sidecar metadata file, relative to the library root.
pub const SIDECAR_FILE: &str = ".notae/library.json";
/// Sidecar thumbnail directory, relative to the library root.
pub const SIDECAR_THUMBNAILS: &str = ".notae/thumbnails";

/// The whole `library.json` document.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct SidecarFile {
    /// Format version, see [`SIDECAR_VERSION`].
    pub version: u32,
    /// Name and version of the tool that wrote the file.
    pub generator: String,
    /// One entry per score, sorted by path.
    pub scores: Vec<SidecarEntry>,
}

/// Metadata the PC tool computed for one score.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct SidecarEntry {
    /// Library-relative, `/`-separated path of the PDF.
    pub path: String,
    /// Size of the PDF in bytes.
    pub size: u64,
    /// Lowercase hex SHA-256 of the PDF.
    pub sha256: String,
    /// Catalogue number, when known.
    pub number: Option<u32>,
    /// Title, when known.
    pub title: Option<String>,
    /// Composer, when known.
    pub composer: Option<String>,
    /// Tags; absent in the file means none.
    #[serde(default)]
    pub tags: Vec<String>,
    /// Extracted page text, when available.
    pub text: Option<String>,
}

#[cfg(test)]
mod tests {
    use super::*;

    const EXAMPLE: &str = r#"{
  "version": 1,
  "generator": "notae-sidecar 0.8.0",
  "scores": [
    {
      "path": "001 - Adeste Fideles.pdf",
      "size": 201714,
      "sha256": "0123456789abcdef0123456789abcdef0123456789abcdef0123456789abcdef",
      "number": 1,
      "title": "Adeste Fideles",
      "composer": null,
      "tags": ["Z katalogu"],
      "text": "Adeste Fideles. 1. Adeste fideles laeti triumphantes, …"
    }
  ]
}"#;

    #[test]
    fn sidecar_file_round_trips_the_spec_example() {
        let file: SidecarFile = serde_json::from_str(EXAMPLE).expect("parse");
        assert_eq!(file.version, SIDECAR_VERSION);
        assert_eq!(file.scores[0].number, Some(1));
        assert_eq!(file.scores[0].composer, None);
        assert_eq!(file.scores[0].tags, vec!["Z katalogu".to_owned()]);

        let json = serde_json::to_string(&file).expect("serialize");
        let again: SidecarFile = serde_json::from_str(&json).expect("reparse");
        assert_eq!(again, file);
    }
}
