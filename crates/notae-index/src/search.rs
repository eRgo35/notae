//! FTS5 search over the indexed notes.
//!
//! The query runs against the derived `notes_fts` index, which holds the note
//! ID, title, composer/artist, tags, filename, and normalized OCR text of every
//! record. Ranking combines three signals:
//!
//! 1. an exact numeric ID match, which the spec requires to rank highest;
//! 2. an exact title match, which ranks next;
//! 3. the FTS5 `bm25` relevance score for everything else, including broad
//!    OCR-only matches.
//!
//! `bm25` returns negative values where a smaller number is a better match, so
//! the reported [`SearchHit::rank`] negates it and adds the two exact-match
//! bonuses, which are far larger than any relevance score SQLite produces. A
//! higher rank is always a better result.

use notae_domain::{IndexedNote, NoteId};
use rusqlite::Connection;

use crate::store::IndexError;

/// Rank bonus for a record whose numeric ID equals the query.
///
/// Sized so that an exact ID match outranks every possible relevance score.
const EXACT_ID_BONUS: f64 = 1_000.0;

/// Rank bonus for a record whose title equals the query.
///
/// Sized so that an exact title match outranks every relevance score but never
/// an exact ID match.
const EXACT_TITLE_BONUS: f64 = 100.0;

/// Which `notes_fts` columns a search looks at.
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
pub enum SearchScope {
    /// Every indexed column.
    #[default]
    Everything,
    /// Note number, title and filename.
    Titles,
    /// Composer or artist.
    Composers,
    /// Tags.
    Tags,
    /// Lyrics and other page text.
    Text,
}

impl SearchScope {
    /// Stable key used by the Android UI.
    #[must_use]
    pub const fn key(self) -> &'static str {
        match self {
            Self::Everything => "everything",
            Self::Titles => "titles",
            Self::Composers => "composers",
            Self::Tags => "tags",
            Self::Text => "text",
        }
    }

    /// Parses a key; unknown keys search everything.
    #[must_use]
    pub fn from_key(key: &str) -> Self {
        match key {
            "titles" => Self::Titles,
            "composers" => Self::Composers,
            "tags" => Self::Tags,
            "text" => Self::Text,
            _ => Self::Everything,
        }
    }

    /// FTS5 column filter, or `None` for every column.
    const fn columns(self) -> Option<&'static str> {
        match self {
            Self::Everything => None,
            Self::Titles => Some("note_id title filename"),
            Self::Composers => Some("composer"),
            Self::Tags => Some("tags"),
            Self::Text => Some("ocr_text"),
        }
    }
}

/// One search result with its rank.
#[derive(Debug, Clone, PartialEq)]
pub struct SearchHit {
    /// The matching record.
    pub note: IndexedNote,
    /// Relevance of the match; higher is better.
    ///
    /// Scores are comparable within one result set, not across queries: the
    /// bonuses are fixed constants while the relevance term depends on the
    /// corpus and the query.
    pub rank: f64,
    /// Page-text excerpt around the match, with the matched terms between
    /// `\u{2}` and `\u{3}`; `None` when the page text did not match.
    pub text_snippet: Option<String>,
}

/// Selects every record whose indexed text mentions the query.
///
/// `bm25` weights follow the column order of `notes_fts`: note ID, title,
/// composer/artist, tags, filename, OCR text. Structured fields outweigh OCR
/// text so a record titled with the search term beats one that merely prints it.
const SEARCH_SQL: &str = "\
SELECT bm25(notes_fts, 8.0, 10.0, 6.0, 5.0, 4.0, 1.0) AS relevance, notes.*,
       snippet(notes_fts, 5, char(2), char(3), '…', 12) AS text_snippet
FROM notes_fts
JOIN notes ON notes.row_id = notes_fts.rowid
WHERE notes_fts MATCH ?1
ORDER BY
    (CASE WHEN ?2 IS NULL THEN 0 WHEN notes.note_id = ?2 THEN 1 ELSE 0 END) DESC,
    (CASE WHEN lower(notes.title) = ?3 THEN 1 ELSE 0 END) DESC,
    relevance ASC,
    notes.path ASC
LIMIT ?4";

/// Runs `query` against the FTS5 index and returns at most `limit` hits.
///
/// Hits are ordered by rank; records with equal rank fall back to the default
/// browsing order, so the same query and limit always produce the same list.
///
/// # Errors
///
/// Fails when the database cannot be read or holds a value that no longer
/// parses.
pub(crate) fn search(
    conn: &Connection,
    query: &str,
    scope: SearchScope,
    limit: usize,
) -> Result<Vec<SearchHit>, IndexError> {
    let match_expression = match_expression(query, scope);
    if match_expression.is_empty() || limit == 0 {
        return Ok(Vec::new());
    }

    let exact_id = query
        .trim()
        .parse::<NoteId>()
        .ok()
        .map(|id| i64::from(id.get()));
    let exact_title = query.trim().to_lowercase();
    let mut statement = conn.prepare(SEARCH_SQL)?;
    let rows = statement.query_map(
        rusqlite::params![
            match_expression,
            exact_id,
            exact_title,
            i64::try_from(limit).unwrap_or(i64::MAX),
        ],
        |row| {
            let relevance: f64 = row.get(0)?;
            let snippet: Option<String> = row.get("text_snippet")?;
            let note = crate::store::note_from_row(row)?;
            Ok((relevance, snippet, note))
        },
    )?;

    let mut hits = Vec::new();
    for row in rows {
        let (relevance, snippet, note) = row?;
        // `exact_id` is `None` for any query that is not a number; comparing
        // options directly would score every unnumbered record as an exact hit.
        let exact_id_match =
            if exact_id.is_some() && exact_id == note.id().map(|id| i64::from(id.get())) {
                EXACT_ID_BONUS
            } else {
                0.0
            };
        let exact_title_match = if exact_title == note.title().to_lowercase() {
            EXACT_TITLE_BONUS
        } else {
            0.0
        };
        hits.push(SearchHit {
            rank: exact_id_match + exact_title_match - relevance,
            text_snippet: snippet.filter(|text| text.contains('\u{2}')),
            note,
        });
    }
    // The SQL ordering already separates the exact-match bonuses; sorting here
    // gives equal ranks the browsing order rather than the database's row order.
    hits.sort_by(|left, right| {
        right
            .rank
            .total_cmp(&left.rank)
            .then_with(|| notae_domain::compare_library_order(&left.note, &right.note))
    });
    Ok(hits)
}

/// Turns free-form user input into an FTS5 match expression.
///
/// Only runs of alphanumeric characters become terms, and each term is quoted
/// before adding FTS5's prefix operator. This keeps user input literal while
/// matching incomplete words as they are typed. Terms are combined with the
/// implicit `AND`, so a hit matches every query term.
fn match_expression(query: &str, scope: SearchScope) -> String {
    let mut terms = Vec::new();
    let mut current = String::new();
    for character in query.chars() {
        if character.is_alphanumeric() {
            current.push(character);
        } else if !current.is_empty() {
            terms.push(std::mem::take(&mut current));
        }
    }
    if !current.is_empty() {
        terms.push(current);
    }
    let body = terms
        .into_iter()
        .map(|term| format!("\"{term}\"*"))
        .collect::<Vec<_>>()
        .join(" ");
    match (body.is_empty(), scope.columns()) {
        (true, _) | (false, None) => body,
        (false, Some(columns)) => format!("{{{columns}}} : ({body})"),
    }
}
