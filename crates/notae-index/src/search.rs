//! FTS5 search over the indexed notes.
//!
//! The query runs against the derived `notes_fts` index, which holds the note
//! ID, title, composer/artist, tags, filename, and normalized OCR text of every
//! record. Ranking combines four signals, each outweighing all the ones after:
//!
//! 1. an exact numeric ID match, which the spec requires to rank highest;
//! 2. an exact title match, which ranks next;
//! 3. the field the query matches ([`Tier`]): the start of the title, words
//!    of the title, the composer, the tags, number or filename, and last the
//!    page text alone;
//! 4. among page-text-only matches, the FTS5 `bm25` relevance score.
//!
//! Matches in the same structured field are equally good: they keep the
//! browsing order (by number), since `bm25` would rank them by title length.
//! `bm25` returns negative values where a smaller number is a better match, so
//! the reported [`SearchHit::rank`] squashes its negation into `[0, 1)` under
//! the tier and exact-match bonuses. A higher rank is always a better result.
//!
//! The index holds text with `ł`/`Ł` folded to `l`/`L` (see `crate::fold`), so
//! the query is folded the same way before it is split into terms, the
//! exact-title bonus compares folded titles, and the lyric snippet — which
//! `snippet()` cuts from the folded column — is mapped back onto the stored
//! page text before it is returned.

use notae_domain::{IndexedNote, NoteId};
use rusqlite::Connection;

use crate::fold::{fold_l_stroke, has_l_stroke, restore_l_stroke};
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

/// Rank bonus for each step of [`Tier`]; the four steps stay under
/// [`EXACT_TITLE_BONUS`].
const TIER_BONUS: f64 = 10.0;

/// The best field a query matches, as the column filters of its FTS5 test.
/// The first that matches gives the tier; none gives the page-text tier, 0.
const TIERS: [(f64, &str); 4] = [
    (4.0, "{title} : ^"),
    (3.0, "{title} : "),
    (2.0, "{composer} : "),
    (1.0, "{tags note_id filename} : "),
];

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
    /// `\u{2}` and `\u{3}`, in the spelling of the stored text (`ł` stays `ł`
    /// although the index folds it); `None` when the page text did not match.
    pub text_snippet: Option<String>,
}

/// Text `snippet()` puts at a cut end of its window; also what
/// `restore_l_stroke` must strip before it locates the window.
const SNIPPET_ELLIPSIS: &str = "…";

/// Selects every record whose indexed text mentions the query.
///
/// `bm25` weights follow the column order of `notes_fts`: note ID, title,
/// composer/artist, tags, filename, OCR text. Structured fields outweigh OCR
/// text so a record titled with the search term beats one that merely prints it.
/// The tier tests ?3–?6 follow [`TIERS`].
const SEARCH_SQL: &str = "\
SELECT bm25(notes_fts, 8.0, 10.0, 6.0, 5.0, 4.0, 1.0) AS relevance, notes.*,
       snippet(notes_fts, 5, char(2), char(3), ?2, 12) AS text_snippet,
       CASE
           WHEN notes_fts.rowid IN (SELECT rowid FROM notes_fts WHERE notes_fts MATCH ?3) THEN 4.0
           WHEN notes_fts.rowid IN (SELECT rowid FROM notes_fts WHERE notes_fts MATCH ?4) THEN 3.0
           WHEN notes_fts.rowid IN (SELECT rowid FROM notes_fts WHERE notes_fts MATCH ?5) THEN 2.0
           WHEN notes_fts.rowid IN (SELECT rowid FROM notes_fts WHERE notes_fts MATCH ?6) THEN 1.0
           ELSE 0.0
       END AS tier
FROM notes_fts
JOIN notes ON notes.row_id = notes_fts.rowid
WHERE notes_fts MATCH ?1";

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
    let terms = terms(query);
    if terms.is_empty() || limit == 0 {
        return Ok(Vec::new());
    }
    let match_expression = match_expression(&terms, scope);
    // The start of the title as one phrase, its last word a prefix; every other
    // tier as the terms in any order, like the search itself.
    let phrase = format!("\"{}\"*", terms.join(" "));
    let words = terms
        .iter()
        .map(|term| format!("\"{term}\"*"))
        .collect::<Vec<_>>()
        .join(" ");
    let [start, title, composer, other] = TIERS.map(|(_, columns)| {
        if columns.ends_with('^') {
            format!("{columns}{phrase}")
        } else {
            format!("{columns}({words})")
        }
    });

    let exact_id = query
        .trim()
        .parse::<NoteId>()
        .ok()
        .map(|id| i64::from(id.get()));
    // The same fold as the indexed text and the match expression, so an exact
    // title is exact up to case and the stroke of `ł`.
    let exact_title = fold_l_stroke(&query.trim().to_lowercase()).into_owned();
    let mut statement = conn.prepare(SEARCH_SQL)?;
    let rows = statement.query_map(
        rusqlite::params![
            match_expression,
            SNIPPET_ELLIPSIS,
            start,
            title,
            composer,
            other
        ],
        |row| {
            let relevance: f64 = row.get(0)?;
            let tier: f64 = row.get("tier")?;
            let snippet = match row.get::<_, Option<String>>("text_snippet")? {
                Some(text) if text.contains('\u{2}') => Some(with_original_spelling(row, text)?),
                _ => None,
            };
            let note = crate::store::note_from_row(row)?;
            Ok((relevance, tier, snippet, note))
        },
    )?;

    let mut hits = Vec::new();
    for row in rows {
        let (relevance, tier, snippet, note) = row?;
        // `exact_id` is `None` for any query that is not a number; comparing
        // options directly would score every unnumbered record as an exact hit.
        let exact_id_match =
            if exact_id.is_some() && exact_id == note.id().map(|id| i64::from(id.get())) {
                EXACT_ID_BONUS
            } else {
                0.0
            };
        let exact_title_match =
            if fold_l_stroke(&note.title().to_lowercase()) == exact_title.as_str() {
                EXACT_TITLE_BONUS
            } else {
                0.0
            };
        // Page text alone is ordered by relevance, squashed under one tier step;
        // structured fields are not (see the module docs).
        let text_relevance = if tier == 0.0 {
            let score = (-relevance).max(0.0);
            score / (1.0 + score)
        } else {
            0.0
        };
        hits.push(SearchHit {
            rank: exact_id_match + exact_title_match + tier * TIER_BONUS + text_relevance,
            text_snippet: snippet,
            note,
        });
    }
    // Ranked and cut here rather than in SQL: SQLite's `lower()` folds ASCII
    // only, so it would miss an exact title such as "Święty Boże". Equal ranks
    // keep the browsing order rather than the database's row order.
    hits.sort_by(|left, right| {
        right
            .rank
            .total_cmp(&left.rank)
            .then_with(|| notae_domain::compare_library_order(&left.note, &right.note))
    });
    hits.truncate(limit);
    Ok(hits)
}

/// Splits free-form user input into search terms: its runs of alphanumeric
/// characters, which keeps user input literal.
fn terms(query: &str) -> Vec<String> {
    // Indexed text is folded as it is written (`write_fts_row`), so the query
    // must be folded the same way before it is split into terms.
    let query = fold_l_stroke(query);
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
    terms
}

/// Turns search terms into an FTS5 match expression.
///
/// Each term is quoted before adding FTS5's prefix operator, matching
/// incomplete words as they are typed. Terms are combined with the implicit
/// `AND`, so a hit matches every query term.
fn match_expression(terms: &[String], scope: SearchScope) -> String {
    let body = terms
        .iter()
        .map(|term| format!("\"{term}\"*"))
        .collect::<Vec<_>>()
        .join(" ");
    match (body.is_empty(), scope.columns()) {
        (true, _) | (false, None) => body,
        (false, Some(columns)) => format!("{{{columns}}} : ({body})"),
    }
}

/// Returns `snippet`, cut from the folded FTS column, with the stored text's
/// own spelling.
///
/// `notes.*` already carries the stored page text, so it is read in place
/// (`get_ref`, no copy) and only inspected when it contains `ł`/`Ł`; every other
/// hit keeps the snippet as SQLite produced it.
fn with_original_spelling(row: &rusqlite::Row<'_>, snippet: String) -> rusqlite::Result<String> {
    let original = match row.get_ref("ocr_text")? {
        rusqlite::types::ValueRef::Text(bytes) => std::str::from_utf8(bytes).ok(),
        _ => None,
    };
    Ok(match original {
        Some(original) if has_l_stroke(original) => {
            restore_l_stroke(&snippet, SNIPPET_ELLIPSIS, original)
        }
        _ => snippet,
    })
}
