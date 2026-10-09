//! The filename convention `<ID> - <Title>.pdf` and how it seeds metadata.

use serde::{Deserialize, Serialize};

use crate::{DomainError, MetadataOrigin, NoteId, NoteMetadata};

/// The result of parsing one note filename.
///
/// A filename either follows the convention, carrying a numeric ID and a
/// title, or it does not, in which case the whole stem becomes the title and
/// the ID stays unknown.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct ParsedFilename {
    /// The leading numeric ID, when the filename follows the convention.
    id: Option<NoteId>,
    /// The title: the text after the separator, or the whole stem.
    title: String,
}

impl ParsedFilename {
    /// Returns the parsed ID, when the filename carried one.
    #[must_use]
    pub const fn id(&self) -> Option<NoteId> {
        self.id
    }

    /// Returns the parsed title.
    #[must_use]
    pub fn title(&self) -> &str {
        &self.title
    }

    /// Builds metadata from this filename, marking both the ID and the title
    /// [`MetadataOrigin::FilenameDerived`].
    ///
    /// # Errors
    /// Returns [`DomainError::EmptyTitle`] when the filename has no title text
    /// at all, for example a bare `.pdf`.
    pub fn into_note_metadata(self) -> Result<NoteMetadata, DomainError> {
        let mut metadata = NoteMetadata::new(&self.title, None, [])?;
        metadata.assign_id(self.id, MetadataOrigin::FilenameDerived);
        metadata.assign_title(&self.title, MetadataOrigin::FilenameDerived)?;
        Ok(metadata)
    }
}

/// Parses one note filename into an ID and a title.
///
/// # Rules
///
/// - Any leading directory is ignored; only the final path component is read.
/// - A leading ASCII digit run is an ID only when the rest of the stem starts
///   with a `-` separator, optionally padded with whitespace on either side,
///   and is followed by a non-empty title. `12 - Autumn Leaves.pdf` yields ID
///   `12`; `12 Etudes.pdf` and `Opus 27 No. 2.pdf` yield no ID, because the
///   number is part of the title rather than a leading ID.
/// - The numeric ranges in use (`1-299`, `500-599`) are conventions, not
///   rules: any `u32` is a valid ID. A digit run too large for `u32` is not an
///   ID, since a note number must be storable and comparable.
/// - When the separator is present but the title is empty, the name does not
///   follow the convention and the whole trimmed stem becomes the title.
/// - The extension must be `.pdf`, matched case-insensitively (`.PDF` works).
///   A name with any other extension is not a note filename: it has no ID and
///   its stem, or the whole name when there is no extension, becomes the
///   title.
#[must_use]
pub fn parse_filename(filename: &str) -> ParsedFilename {
    let base = final_component(filename);
    let (stem, is_pdf) = match base.rsplit_once('.') {
        Some((stem, extension)) => (stem, extension.eq_ignore_ascii_case("pdf")),
        None => (base, false),
    };
    let stem = stem.trim();
    if is_pdf
        && let Some((id, title)) = split_id_and_title(stem)
    {
        return ParsedFilename {
            id: Some(id),
            title: title.to_string(),
        };
    }
    ParsedFilename {
        id: None,
        title: stem.to_string(),
    }
}

/// Returns the final path component, ignoring both separators so that a path
/// built on Windows parses the same way as on POSIX.
fn final_component(name: &str) -> &str {
    let cut = name
        .rfind(['/', '\\'])
        .map_or(0, |index| index + 1);
    &name[cut..]
}

/// Splits `<digits><separator><title>` into its parts.
///
/// Returns `None` unless the stem begins with digits, is followed by a `-`
/// separator with optional surrounding whitespace, and then a non-empty title.
fn split_id_and_title(stem: &str) -> Option<(NoteId, &str)> {
    let digits = stem
        .find(|character: char| !character.is_ascii_digit())
        .unwrap_or(stem.len());
    if digits == 0 {
        return None;
    }
    let rest = stem[digits..].trim_start();
    let title = rest.strip_prefix('-')?.trim_start();
    if title.is_empty() {
        return None;
    }
    Some((NoteId::parse(&stem[..digits]).ok()?, title))
}