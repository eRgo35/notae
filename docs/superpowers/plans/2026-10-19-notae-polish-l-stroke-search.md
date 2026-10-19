# Polish ł/Ł Search Folding Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Make Search treat Polish `ł`/`Ł` as `l`/`L` (so `palka` finds `Pałka`, `lodz` finds `Łódź`) in titles, composers, tags and lyrics in every `SearchScope`, without breaking existing indexes.

**Architecture:** One pure function, `fold_l_stroke` (`ł`→`l`, `Ł`→`L`, one `char` in, one `char` out), in a new private `notae-index` module. It is applied on the write side (`write_fts_row`, the only place `notes_fts` is written) and on the query side (`match_expression`, exact-title bonus). Rows already in an index are folded in place by a new pure-SQL migration `0002` that bumps `SCHEMA_VERSION` from 1 to 2. The canonical `notes` table, the sidecar manifest and everything shown to the user stay in the original spelling; because the fold is 1:1 per `char`, the FTS5 lyric snippet (which is cut from the folded column) is mapped back onto the stored text before it leaves `notae-index`.

**Tech Stack:** Rust 1.98.1, rusqlite 0.40 (bundled SQLite with FTS5, `unicode61 remove_diacritics 2`), Slint 1.18.1 testing backend (`i_slint_backend_testing`).

**Spec:** GitHub issue #35 (`gh issue view 35`; it is authoritative). Vocabulary follows the code and `README.md` (index, sidecar, Search scope, lyrics = OCR text); the repo has no `GLOSSARY.md` and `docs/adr/` only holds `0001-on-device-ocr.md`, which this change does not touch.

## Global Constraints

- Use the workspace-pinned Rust 1.98.1 toolchain (`rust-toolchain.toml`). CI runs the same gates as `docs/engineering-standards.md`.
- `unsafe` is forbidden in the workspace (`unsafe_code = "forbid"`; only the Android entry-point shim is exempt). A custom FTS5 tokenizer needs the C FFI (`fts5_api`) and is therefore **not acceptable**; this is the reason the tokenizer option of acceptance item 5 is rejected.
- Production code must not use `unwrap`/`expect` (`cargo clippy --workspace --lib --bins -- -D warnings -D clippy::unwrap_used -D clippy::expect_used`). Test targets may use them for fixtures and assertions. `missing_docs = "warn"` and the CI rustdoc gate run with `-D warnings`: document every new `pub` item.
- Use Conventional Commits: `<type>(optional-scope): imperative summary`, with `Refs #35` in the body. No mandatory Git hook is installed.
- Scope is `ł` (U+0142) and `Ł` (U+0141) only (issue acceptance 7). Do not fold `đ ø æ œ ß` (see *Notes for the PR*).
- Do **not** change `SIDECAR_SCHEMA_VERSION` (stays `2`). `crates/notae-index/src/sidecar.rs` was read: `SidecarWriter`/`SidecarImporter` read and write the canonical `notes` columns and `OcrText` through `IndexStore` (`list_all`, `ocr_text`, `reindex_note_with_ocr`, `set_ocr`); nothing in the manifest reads `notes_fts`, and `grep -rn notes_fts crates apps` finds it only in the schema, `store.rs`, `search.rs`, `lib.rs` docs and tests. The manifest therefore carries the original spelling and needs no version change, no migration note and no re-export from the desktop indexer.
- Do change `notae_index::SCHEMA_VERSION` (the SQLite index schema, **not** the sidecar): `1` → `2`, with migration `0002_fold_l_stroke.sql`. This needs the migration notes written in Task 5 (CHANGELOG, `apps/notae-index/README.md`, `docs/android-debugging.md`, the `schema.rs` and SQL comments).
- Required display behaviour: titles, composers, tags and lyric snippets shown to the user keep the original spelling (`Łódź`, `Pałka`). Only the derived `notes_fts` rows and the query are folded.
- The fold is applied through exactly one function. The SQL migration contains a frozen copy of the mapping (a released migration must never change); a test (Task 2) asserts the migration and the Rust function agree.
- Task order matters inside one PR: Tasks 1 and 2 must land together. After Task 1 alone, newly written rows are folded but rows already in a version 1 index are not. Do not cut a release between them.

## Decisions and rejected options

| Question | Decision | Why |
| --- | --- | --- |
| Tokenizer change vs. normalise both sides | Normalise index side and query side with one function. | A tokenizer that folds `ł` must be a custom FTS5 tokenizer: C FFI, `unsafe`, forbidden by `AGENTS.md`. `unicode61` has no option that folds a letter without a canonical decomposition (`remove_diacritics` only removes combining marks; `tokenchars`/`separators`/`categories` classify, they do not map). `trigram` does not fold either. |
| Where the fold lives | `crates/notae-index/src/fold.rs`, `pub(crate)`. | The Android crate only calls `IndexStore::search_in`; it never needs the function. Keeping it private keeps the public API unchanged. |
| Query-side-only expansion (`"lodz"*` OR every `ł` spelling) | Rejected. | Combinatorial (every `l` may be `ł`), and still cannot match `ł` against a stored `l`. |
| SQLite scalar function registered on the connection (`rusqlite` `functions` feature) so the migration can call the Rust fold | Rejected. | New cargo feature on the Android build, and a migration that depends on a connection-local function being registered first. A frozen SQL `replace()` plus a parity test is simpler and has the same single source of truth *at runtime*. |
| External-content or contentless `notes_fts` (index folded text, read original text for `snippet()`) | Rejected. | Rebuilds the table, makes `delete`/`rebuild` commands depend on exact old values, changes the schema shape. Far more risk than a 1:1 char mapping. |
| Bring old FTS rows up to date | Migration `0002` = one in-place `UPDATE notes_fts … replace(replace(col,'ł','l'),'Ł','L')` on the rows that contain `ł`/`Ł`; `SCHEMA_VERSION` 2. | Pure SQL, so it fits the existing `MIGRATIONS` runner, runs once inside the migration transaction and fails atomically with it. **Correction to the brief:** a Rust-side rebuild is *not* needed. The `filename` column does not have to be derived from `path`: folding the stored FTS text in place gives the same bytes. Task 2 proves it by comparing the migrated rows with rows written by the Rust writer. |
| Keep the stored lyric snippet in the original spelling | Map the folded snippet back onto `notes.ocr_text` in Rust (`restore_l_stroke`), only for hits whose stored text contains `ł`/`Ł`. | `snippet()` can only return text of the FTS column, which is now folded, so without this users would read `Palka zagrala kolysanke` for `Pałka zagrała kołysankę`. The fold keeps char count, so the snippet is a contiguous char range of the folded text and the same range of the original gives the original spelling. `notes.*` is already selected, so `ocr_text` is read with `get_ref` (borrowed, no copy) and nothing else is queried. Cost only for ł-bearing hits: one fold plus one substring search of that note's page text. Rejected: showing folded snippets (loses the letter in displayed lyrics), reimplementing `unicode61` matching in Rust to compute snippets, and external content (above). |
| Exact-title bonus | Also `ł`-insensitive, via the same function. | A title that differs from the query only by the stroke (`Łaska` for `laska`) must rank first the way `łaska` already does. The bonus has never folded the other diacritics, so `lodz` does not earn it for `Łódź` (ó, ź); that is unchanged. Rust side: `fold(lowercase(title)) == fold(lowercase(query))`. SQL `ORDER BY` side: compare `lower(notes_fts.title)` (already folded) with the folded query. **Pre-existing, left alone:** SQLite `lower()` is ASCII-only, so the SQL term never matched non-ASCII titles; the Rust bonus is what ranks results, the SQL term only matters when `LIMIT` truncates (the Android app passes `usize::MAX`). |

### Migration and the Android index

- `IndexStore::from_connection` runs `schema::migrate`, so every opener upgrades in place: the Android app (`apps/notae-android/src/android_main.rs:1013`, when the library is opened) and the desktop indexer (`apps/notae-index/src/main.rs:126`, `<library>/.notae/index.sqlite`).
- The Android index (`<internal data>/notae/index.sqlite`) is **not** rebuilt by a rescan: it persists and a rescan updates it incrementally by content hash. So the migration is what brings it up to date; the app does not "rebuild anyway".
- The Android app imports `manifest.json`, never the CLI's `index.sqlite` (see `apps/notae-index/README.md`), and the manifest format does not change.
- **Forward-only upgrade (accepted by the maintainer).** `migrate()` refuses an index recorded at a newer version (`IndexError::UnsupportedSchemaVersion`), so an older APK or `notae-index` refuses a version 2 index; no downgrade path is provided. That refusal is the *correct* behaviour here: an older build writing unfolded rows into a folded index would silently break search.
- The migration touches only derived data; `notes` is not modified, so there is no data-loss path.

## File Structure

| File | Change | Responsibility |
| --- | --- | --- |
| `crates/notae-index/src/fold.rs` | Create | `fold_l_stroke`, `has_l_stroke`, `restore_l_stroke`; unit tests. The only definition of the fold. |
| `crates/notae-index/src/lib.rs` | Modify | `mod fold;` and crate docs. |
| `crates/notae-index/src/store.rs` | Modify | `write_fts_row` folds every indexed column; doc of `OcrText`. |
| `crates/notae-index/src/search.rs` | Modify | Fold the query; exact-title bonus; bound snippet ellipsis; restore the snippet spelling; module docs. |
| `crates/notae-index/migrations/0002_fold_l_stroke.sql` | Create | In-place fold of existing `notes_fts` rows. |
| `crates/notae-index/src/schema.rs` | Modify | `SCHEMA_VERSION = 2`, register migration, verify recorded version before applying newer migrations, docs. |
| `crates/notae-index/tests/store_and_search.rs` | Modify | Real-store `Łódź — Pieśń żółta` regression tests, migration tests; replace the "limitation" test. |
| `apps/notae-android/src/ui/search.rs` | Modify | `library_row` becomes a host-visible `pub fn` so the UI test uses the real row projection. |
| `apps/notae-android/tests/support/mod.rs` | Modify | Shared `SEARCH_LABEL`, `focus_search_field`, `type_text`. |
| `apps/notae-android/tests/search_input.rs` | Modify | Reuse the shared helper (no behaviour change). |
| `apps/notae-android/tests/search_stroke_ui.rs` | Create | Generated-UI behavioural test: Search screen over a real index, every scope. |
| `README.md`, `docs/android-debugging.md`, `CHANGELOG.md`, `apps/notae-index/README.md` | Modify | Remove the limitation, record the migration. |

---

### Task 1: Shared fold, index side and query side

**Files:**
- Create: `crates/notae-index/src/fold.rs`
- Modify: `crates/notae-index/src/lib.rs` (add `mod fold;` next to `mod import;`)
- Modify: `crates/notae-index/src/store.rs:20` (import), `:659-680` (`write_fts_row`), `:75-77` (`OcrText` doc)
- Modify: `crates/notae-index/src/search.rs:107-118` (SQL), `:140-177` (`search`), `:195-201` (`match_expression`)
- Test: `crates/notae-index/tests/store_and_search.rs`

**Interfaces:**
- Consumes: `IndexStore::{open, upsert_note_with_ocr, search_in}`, test helpers `note`, `digest` (already in `store_and_search.rs`).
- Produces: `pub(crate) fn fold_l_stroke(text: &str) -> std::borrow::Cow<'_, str>` in `crate::fold`.

- [x] **Step 1: Write the failing integration tests**

In `crates/notae-index/tests/store_and_search.rs`, delete the whole test `accentless_queries_follow_the_unicode61_folding_rules_without_a_tokenizer_change` (it asserts the old limitation; it starts at the `#[test]` before it, currently line 980, and runs to the end of the file) and append:

```rust
const LODZ_TITLE: &str = "Łódź — Pieśń żółta";
const LODZ_LYRICS: &str = "Pałka zagrała kołysankę dla Gałuszki. Części stałe mszy: zmartwychwstał Pan.";

/// The real-store fixture of issue #35: one score whose title, composer, tag and
/// lyrics all contain `ł`, and one control score without any.
fn lodz_store() -> IndexStore {
    let store = IndexStore::open(":memory:").expect("open");
    store
        .upsert_note_with_ocr(
            &note(
                "score.pdf",
                3,
                Some(NoteId::new(303)),
                LODZ_TITLE,
                Some("Stanisław Moniuszko"),
                &["kołysanka"],
            ),
            Some(&OcrText::new(LODZ_LYRICS)),
        )
        .expect("insert");
    store
        .upsert_note_with_ocr(
            &note(
                "ave.pdf",
                4,
                Some(NoteId::new(13)),
                "Ave Maria",
                Some("Schubert"),
                &["solo"],
            ),
            Some(&OcrText::new("Ave Maria gratia plena")),
        )
        .expect("insert");
    store
}

fn titles(store: &IndexStore, query: &str, scope: SearchScope) -> Vec<String> {
    store
        .search_in(query, scope, 10)
        .expect("search")
        .iter()
        .map(|hit| hit.note.title().to_owned())
        .collect()
}

#[test]
fn queries_with_and_without_the_stroke_find_polish_text_in_every_scope() {
    let store = lodz_store();
    let cases: [(SearchScope, &[&str]); 5] = [
        (
            SearchScope::Titles,
            &[
                // Without the stroke or the other diacritics.
                "lodz", "Lodz", "LODZ", "zolta", "zolta piesn", "piesn",
                // As typed on a Polish keyboard, any case.
                "łódź", "Łódź", "ŁÓDŹ", "łÓdŹ", "żółta", "ŻÓŁTA", "Łódź — Pieśń żółta",
                // Mixed: stroke kept, other diacritics dropped, and the reverse.
                "łodz", "ŁODZ", "lódź", "zołta",
            ],
        ),
        (
            SearchScope::Composers,
            &["stanislaw", "Stanislaw", "STANISLAW", "Stanisław", "STANISŁAW", "moniuszko"],
        ),
        (
            SearchScope::Tags,
            &["kolysanka", "Kolysanka", "kołysanka", "KOŁYSANKA", "kolys"],
        ),
        (
            SearchScope::Text,
            &[
                "palka", "Palka", "Pałka", "galuszki", "Galuszki", "Gałuszki",
                "czesci stale", "części stałe", "Czesci Stale",
                "zmartwychwstal", "zmartwychwstał", "ZMARTWYCHWSTAŁ",
            ],
        ),
        (
            SearchScope::Everything,
            &["lodz", "ŁÓDŹ", "stanislaw", "Stanisław", "kolysanka", "palka", "Pałka", "czesci stale"],
        ),
    ];
    for (scope, queries) in cases {
        for query in queries {
            assert_eq!(
                titles(&store, query, scope),
                [LODZ_TITLE],
                "query {query:?} in {scope:?}"
            );
        }
    }
}

#[test]
fn the_examples_of_issue_35_find_their_scores() {
    let store = IndexStore::open(":memory:").expect("open");
    let titles_and_queries = [
        ("Pałka", "palka"),
        ("Gałuszka", "Galuszka"),
        ("Części stałe", "czesci stale"),
        ("Zmartwychwstał", "zmartwychwstal"),
        ("Łódź", "lodz"),
    ];
    for (seed, (title, _)) in (1_u8..).zip(titles_and_queries) {
        store
            .upsert_note(&note(&format!("s{seed}.pdf"), seed, None, title, None, &[]))
            .expect("insert");
    }
    for (title, query) in titles_and_queries {
        assert_eq!(titles(&store, query, SearchScope::Everything), [title], "{query:?}");
        assert_eq!(titles(&store, title, SearchScope::Everything), [title], "{title:?}");
    }
}

#[test]
fn plain_l_and_stroke_l_are_one_letter_in_both_directions() {
    let store = IndexStore::open(":memory:").expect("open");
    store
        .upsert_note(&note("a.pdf", 1, None, "Lodz", None, &[]))
        .expect("insert");
    store
        .upsert_note(&note("b.pdf", 2, None, "Łódź", None, &[]))
        .expect("insert");
    for query in ["lodz", "Lodz", "łodz", "Łódź", "ŁÓDŹ"] {
        let mut found = titles(&store, query, SearchScope::Titles);
        found.sort();
        assert_eq!(found, ["Lodz", "Łódź"], "{query:?}");
    }
}

#[test]
fn scopes_stay_separate_after_folding() {
    let store = lodz_store();
    assert!(titles(&store, "lodz", SearchScope::Composers).is_empty());
    assert!(titles(&store, "palka", SearchScope::Titles).is_empty());
    assert!(titles(&store, "stanislaw", SearchScope::Tags).is_empty());
    assert!(titles(&store, "kolysanka", SearchScope::Text).is_empty());
    assert_eq!(titles(&store, "ave", SearchScope::Everything), ["Ave Maria"]);
}

#[test]
fn the_exact_title_bonus_ignores_the_stroke() {
    // Only the stroke differs between the query and the exact title: the exact
    // title bonus has never folded the other diacritics.
    let store = IndexStore::open(":memory:").expect("open");
    store
        .upsert_note(&note("a.pdf", 1, None, "Łaska łaskawa", None, &[]))
        .expect("insert");
    store
        .upsert_note(&note("b.pdf", 2, None, "Łaska", None, &[]))
        .expect("insert");
    for query in ["laska", "Laska", "łaska", "ŁASKA", " Łaska "] {
        let hits = store
            .search_in(query, SearchScope::Titles, 10)
            .expect("search");
        assert_eq!(hits.len(), 2, "{query:?}");
        assert_eq!(hits[0].note.title(), "Łaska", "{query:?}");
        assert!(hits[0].rank > 50.0, "exact title bonus for {query:?}: {}", hits[0].rank);
        assert!(hits[1].rank < 50.0, "no bonus for the longer title: {}", hits[1].rank);
        let limited = store
            .search_in(query, SearchScope::Titles, 1)
            .expect("search");
        assert_eq!(limited[0].note.title(), "Łaska", "limit 1 for {query:?}");
    }
}

#[test]
fn only_the_l_stroke_is_folded() {
    // Scope of issue #35: `ł`/`Ł` only. These letters have no decomposition either
    // and stay distinct until a separate change decides otherwise.
    let store = IndexStore::open(":memory:").expect("open");
    for (seed, title) in (1_u8..).zip(["Øre", "Đorđe", "Cœli", "Cæli", "Straße"]) {
        store
            .upsert_note(&note(&format!("s{seed}.pdf"), seed, None, title, None, &[]))
            .expect("insert");
    }
    for query in ["ore", "dorde", "coeli", "caeli", "strasse"] {
        assert!(
            titles(&store, query, SearchScope::Titles).is_empty(),
            "{query:?} must not match yet"
        );
    }
}
```

`cargo fmt` will reflow the long arrays; run it in Step 5.

- [x] **Step 2: Run the tests to verify they fail**

Run: `cargo test -p notae-index --test store_and_search -- queries_with_and_without the_examples_of_issue_35 plain_l_and_stroke_l scopes_stay_separate the_exact_title_bonus only_the_l_stroke`
Expected: FAIL — `queries_with_and_without_the_stroke…` (e.g. `query "lodz" in Titles`), `the_examples_of_issue_35…`, `plain_l_and_stroke_l…`, and `the_exact_title_bonus_ignores_the_stroke` fail; `scopes_stay_separate_after_folding` and `only_the_l_stroke_is_folded` already pass (they pin behaviour that must not change).

- [x] **Step 3: Create the fold module**

Create `crates/notae-index/src/fold.rs`:

```rust
//! Spelling normalization shared by the index writer and the query parser.
//!
//! FTS5's `unicode61 remove_diacritics 2` tokenizer folds the Polish letters
//! `ą ć ę ń ó ś ź ż` because they decompose into a base letter plus a combining
//! mark. `ł` (U+0142) and `Ł` (U+0141) have no decomposition, so the tokenizer
//! keeps them distinct from `l`/`L` and a query typed without the stroke finds
//! nothing. A custom tokenizer would need `unsafe` FFI, which this workspace
//! forbids, so the fold is applied to the text *before* it reaches the
//! tokenizer: every `notes_fts` row is written through [`fold_l_stroke`] and
//! every query is folded the same way. The stored `notes` text is never folded.
//!
//! The fold maps one `char` to one `char`, so it preserves the char count and
//! char positions of a text. That is what lets `restore_l_stroke` (added with
//! the lyric snippet work) map an FTS5 snippet of the folded text back onto the
//! original spelling.

use std::borrow::Cow;

/// The two letters that [`fold_l_stroke`] folds.
const STROKES: [char; 2] = ['ł', 'Ł'];

/// Folds `ł` to `l` and `Ł` to `L`; every other character is unchanged.
///
/// Borrows the input when it contains neither letter, so the common case does
/// not allocate. The function is idempotent and preserves the number of
/// characters.
#[must_use]
pub(crate) fn fold_l_stroke(text: &str) -> Cow<'_, str> {
    if !has_l_stroke(text) {
        return Cow::Borrowed(text);
    }
    Cow::Owned(
        text.chars()
            .map(|character| match character {
                'ł' => 'l',
                'Ł' => 'L',
                other => other,
            })
            .collect(),
    )
}

/// Returns whether `text` contains a letter [`fold_l_stroke`] would change.
#[must_use]
pub(crate) fn has_l_stroke(text: &str) -> bool {
    text.contains(STROKES)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn folds_both_cases() {
        assert_eq!(fold_l_stroke("ł"), "l");
        assert_eq!(fold_l_stroke("Ł"), "L");
        assert_eq!(fold_l_stroke("Łódź — Pieśń żółta"), "Lódź — Pieśń żólta");
        assert_eq!(fold_l_stroke("Pałka Gałuszka"), "Palka Galuszka");
    }

    #[test]
    fn leaves_every_other_character_untouched() {
        for text in [
            "",
            "lL",
            "ąćęńóśźż ĄĆĘŃÓŚŹŻ",
            "đ ø æ œ ß Đ Ø Æ Œ",
            "Ave Maria 303 — …",
            "l\u{0337}", // `l` with a combining overlay is not the precomposed letter
        ] {
            assert_eq!(fold_l_stroke(text), text, "{text:?}");
        }
    }

    #[test]
    fn borrows_when_there_is_nothing_to_fold() {
        assert!(matches!(fold_l_stroke("Ave Maria"), Cow::Borrowed(_)));
        assert!(matches!(fold_l_stroke("Pałka"), Cow::Owned(_)));
    }

    #[test]
    fn is_idempotent_and_preserves_the_character_count() {
        for text in ["ł", "ŁÓDŹ łódź", "zmartwychwstał", "plain", ""] {
            let once = fold_l_stroke(text).into_owned();
            assert_eq!(fold_l_stroke(&once), once, "{text:?}");
            assert_eq!(once.chars().count(), text.chars().count(), "{text:?}");
        }
    }

    #[test]
    fn reports_whether_a_text_has_a_stroke() {
        assert!(has_l_stroke("Pałka"));
        assert!(has_l_stroke("ŁÓDŹ"));
        assert!(!has_l_stroke("Palka"));
        assert!(!has_l_stroke(""));
    }
}
```

In `crates/notae-index/src/lib.rs` add `mod fold;` between `mod import;` and `mod provenance;`:

```rust
mod fold;
mod import;
```

- [x] **Step 4: Fold on the index side and the query side**

`crates/notae-index/src/store.rs`: add the import below `use crate::schema;`:

```rust
use crate::fold::fold_l_stroke;
```

Replace the body of `write_fts_row` (the `params!` list):

```rust
fn write_fts_row(
    conn: &Connection,
    row_id: i64,
    note: &IndexedNote,
    ocr_text: &str,
) -> Result<(), rusqlite::Error> {
    conn.execute("DELETE FROM notes_fts WHERE rowid = ?1", [row_id])?;
    // Every indexed column goes through the same fold the query uses; see
    // `crate::fold`. The canonical `notes` row keeps the original spelling.
    conn.execute(
        "INSERT INTO notes_fts (rowid, note_id, title, composer, tags, filename, ocr_text)
         VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7)",
        params![
            row_id,
            note.id().map(|id| i64::from(id.get())),
            fold_l_stroke(note.title()),
            note.metadata().composer_or_artist().map(fold_l_stroke),
            fold_l_stroke(&tags_text(note)),
            fold_l_stroke(&file_name(note.path())),
            fold_l_stroke(ocr_text),
        ],
    )?;
    Ok(())
}
```

In the `OcrText` doc comment (`store.rs:75-77`) replace

```rust
/// Case folding is deliberately not applied: the FTS5 `unicode61` tokenizer
/// already matches case-insensitively, so storing the text as recognized keeps
/// it readable and directly comparable.
```

with

```rust
/// Case folding is deliberately not applied: the FTS5 `unicode61` tokenizer
/// already matches case-insensitively, so storing the text as recognized keeps
/// it readable and directly comparable. The same holds for the `ł`→`l` fold of
/// `crate::fold`: it is applied to the derived `notes_fts` row only, never to
/// the stored text.
```

`crates/notae-index/src/search.rs`: add `use crate::fold::fold_l_stroke;` after the existing `use crate::store::IndexError;` line (line 20), change the exact-title term of `SEARCH_SQL` to read the already folded FTS title:

```rust
    (CASE WHEN lower(notes_fts.title) = ?3 THEN 1 ELSE 0 END) DESC,
```

and in `search()` replace the two exact-title computations:

```rust
    // The same fold as the indexed text and the match expression, so an exact
    // title is exact up to case and the stroke of `ł`.
    let exact_title = fold_l_stroke(&query.trim().to_lowercase()).into_owned();
```

```rust
        let exact_title_match = if fold_l_stroke(&note.title().to_lowercase()) == exact_title.as_str()
        {
            EXACT_TITLE_BONUS
        } else {
            0.0
        };
```

and fold the query at the top of `match_expression`:

```rust
fn match_expression(query: &str, scope: SearchScope) -> String {
    // Indexed text is folded as it is written (`write_fts_row`), so the query
    // must be folded the same way before it is split into terms.
    let query = fold_l_stroke(query);
    let mut terms = Vec::new();
    let mut current = String::new();
    for character in query.chars() {
```

(the rest of the function is unchanged).

- [x] **Step 5: Run the tests to verify they pass**

Run: `cargo fmt && cargo test -p notae-index`
Expected: PASS for the new tests and the unit tests in `fold.rs`. Everything else in `notae-index` still passes (the schema version is still 1 at this point; `polish_titles_match_exact_and_case_varied_queries_in_every_scope` still passes).

- [x] **Step 6: Commit**

```bash
git add crates/notae-index/src/fold.rs crates/notae-index/src/lib.rs crates/notae-index/src/store.rs crates/notae-index/src/search.rs crates/notae-index/tests/store_and_search.rs
git commit -m "feat(index): fold Polish ł/Ł on both sides of the FTS index" -m "Refs #35"
```

---

### Task 2: Migration 0002 brings existing indexes up to date

**Files:**
- Create: `crates/notae-index/migrations/0002_fold_l_stroke.sql`
- Modify: `crates/notae-index/src/schema.rs:18-19`, `:28-41`, `:54-59`, `:92-123`
- Modify: `crates/notae-index/src/lib.rs` (crate docs bullet for `notes_fts`)
- Test: `crates/notae-index/tests/store_and_search.rs` (imports at line 4-11; hollow-database test at lines 752-783)

**Interfaces:**
- Consumes: `MIGRATIONS`, `SCHEMA_VERSION`, `Migration` (public, `crates/notae-index/src/lib.rs`), `IndexStore::{open, schema_version, upsert_note_with_ocr, ocr_text, get_by_hash, search}`, fixtures `note`, `digest`, `temp_db`.
- Produces: `SCHEMA_VERSION == 2`; `MIGRATIONS` has `Migration { version: 2, name: "fold_l_stroke", sql: … }`.

- [x] **Step 1: Write the failing tests**

In `crates/notae-index/tests/store_and_search.rs` change the import `use std::path::PathBuf;` to `use std::path::{Path, PathBuf};` and the `notae_index` import to include `MIGRATIONS`:

```rust
use notae_index::{
    IndexError, IndexStore, MIGRATIONS, OcrText, ReindexOutcome, SCHEMA_VERSION, SearchScope,
};
```

Replace the existing test `opening_a_database_that_claims_the_version_without_the_schema_is_refused` (currently lines 752-783, from its `#[test]` to its closing brace) with a version that checks every recorded version, because a database recorded at version 1 must still be reported as incomplete rather than failing inside migration 0002:

```rust
#[test]
fn opening_a_database_that_claims_a_version_without_the_schema_is_refused() {
    for recorded in 1..=SCHEMA_VERSION {
        let file = temp_db(&format!("hollow-{recorded}"));
        let conn = rusqlite::Connection::open(&file).expect("create raw");
        let versions: String = (1..=recorded)
            .map(|version| format!("INSERT INTO schema_migrations (version) VALUES ({version});"))
            .collect();
        conn.execute_batch(&format!(
            "CREATE TABLE schema_migrations (
                 version INTEGER PRIMARY KEY,
                 applied_at TEXT NOT NULL DEFAULT (strftime('%Y-%m-%dT%H:%M:%fZ', 'now'))
             );
             {versions}"
        ))
        .expect("seed the bookkeeping only");
        drop(conn);

        let error = IndexStore::open(&file)
            .expect_err("a version row without the schema it names must not be trusted");
        match error {
            IndexError::IncompleteSchema {
                version,
                ref missing,
            } => {
                assert_eq!(version, recorded);
                assert!(
                    missing.contains("notes") && missing.contains("notes_fts"),
                    "the missing objects must be named: {missing}"
                );
            }
            other => panic!("unexpected error at recorded version {recorded}: {other}"),
        }
        let _ = std::fs::remove_file(&file);
    }
}
```

Append the migration tests:

```rust
#[test]
fn the_schema_version_is_the_newest_migration() {
    let versions: Vec<u32> = MIGRATIONS.iter().map(|migration| migration.version).collect();
    assert!(versions.windows(2).all(|pair| pair[0] < pair[1]), "{versions:?}");
    assert_eq!(versions.last().copied(), Some(SCHEMA_VERSION));
    assert_eq!(SCHEMA_VERSION, 2, "the ł/Ł fold is schema version 2");
}

/// One SQL expression per `notes_fts` row, comparable across databases.
fn fts_dump(file: &Path) -> Vec<String> {
    let conn = rusqlite::Connection::open(file).expect("open raw");
    let mut statement = conn
        .prepare(
            "SELECT rowid || '|' || quote(CAST(note_id AS TEXT)) || '|' || quote(title)
                    || '|' || quote(composer) || '|' || quote(tags) || '|' || quote(filename)
                    || '|' || quote(ocr_text)
             FROM notes_fts ORDER BY rowid",
        )
        .expect("prepare");
    statement
        .query_map([], |row| row.get::<_, String>(0))
        .expect("query")
        .collect::<Result<Vec<_>, _>>()
        .expect("rows")
}

fn upgrade_fixture() -> (IndexedNote, OcrText) {
    (
        note(
            "Pałka.pdf",
            5,
            Some(NoteId::new(35)),
            LODZ_TITLE,
            Some("Stanisław Moniuszko"),
            &["kołysanka"],
        ),
        OcrText::new(LODZ_LYRICS),
    )
}

#[test]
fn a_version_1_index_is_folded_in_place_when_it_is_opened() {
    // Put a current database back into the state a version 1 build left behind:
    // unfolded FTS rows and no version 2 row in the bookkeeping.
    let legacy = temp_db("legacy-v1");
    let (record, ocr) = upgrade_fixture();
    let store = IndexStore::open(&legacy).expect("create");
    store
        .upsert_note_with_ocr(&record, Some(&ocr))
        .expect("insert");
    drop(store);
    let conn = rusqlite::Connection::open(&legacy).expect("open raw");
    conn.execute_batch(
        "DELETE FROM schema_migrations WHERE version = 2;
         UPDATE notes_fts SET
             title    = (SELECT title FROM notes WHERE notes.row_id = notes_fts.rowid),
             composer = (SELECT composer_or_artist FROM notes WHERE notes.row_id = notes_fts.rowid),
             tags     = (SELECT tags FROM notes WHERE notes.row_id = notes_fts.rowid),
             filename = (SELECT path FROM notes WHERE notes.row_id = notes_fts.rowid),
             ocr_text = (SELECT ocr_text FROM notes WHERE notes.row_id = notes_fts.rowid);",
    )
    .expect("rewrite the rows the way version 1 stored them");
    let unfolded: i64 = conn
        .query_row(
            "SELECT count(*) FROM notes_fts WHERE notes_fts MATCH '\"lodz\"*'",
            [],
            |row| row.get(0),
        )
        .expect("count");
    assert_eq!(unfolded, 0, "the simulated version 1 rows must be unfolded");
    drop(conn);

    let upgraded = IndexStore::open(&legacy).expect("opening migrates");
    assert_eq!(upgraded.schema_version().expect("version"), SCHEMA_VERSION);
    for query in ["lodz", "palka", "stanislaw", "kolysanka", "czesci stale", "Pałka"] {
        assert_eq!(upgraded.search(query, 10).expect("search").len(), 1, "{query:?}");
    }
    // Canonical data is untouched: only the derived index was folded.
    let kept = upgraded
        .get_by_hash(&digest(5))
        .expect("read")
        .expect("record present");
    assert_eq!(kept.title(), LODZ_TITLE);
    assert_eq!(
        upgraded.ocr_text(&digest(5)).expect("ocr").expect("text").text(),
        LODZ_LYRICS
    );
    drop(upgraded);

    // The frozen SQL in the migration and the Rust writer produce the same rows.
    let fresh = temp_db("fresh-v2");
    let store = IndexStore::open(&fresh).expect("create");
    store
        .upsert_note_with_ocr(&record, Some(&ocr))
        .expect("insert");
    drop(store);
    let migrated = fts_dump(&legacy);
    assert_eq!(migrated, fts_dump(&fresh));
    assert!(migrated[0].contains("'Palka.pdf'"), "{migrated:?}");
    let _ = std::fs::remove_file(&legacy);
    let _ = std::fs::remove_file(&fresh);
}

#[test]
fn reopening_a_migrated_index_does_not_change_it() {
    let file = temp_db("migrated-twice");
    let (record, ocr) = upgrade_fixture();
    let store = IndexStore::open(&file).expect("create");
    store
        .upsert_note_with_ocr(&record, Some(&ocr))
        .expect("insert");
    drop(store);
    let first = fts_dump(&file);
    drop(IndexStore::open(&file).expect("reopen"));
    assert_eq!(fts_dump(&file), first);
    let _ = std::fs::remove_file(&file);
}
```

- [x] **Step 2: Run the tests to verify they fail**

Run: `cargo test -p notae-index --test store_and_search -- the_schema_version_is_the_newest_migration a_version_1_index_is_folded reopening_a_migrated opening_a_database_that_claims_a_version`
Expected: FAIL — `the_schema_version_is_the_newest_migration` (`1 != 2`); `a_version_1_index_is_folded_in_place_when_it_is_opened` (the open leaves rows unfolded: `lodz` finds 0 hits, and `SCHEMA_VERSION` is still 1 so the deleted `version = 2` row never existed). The hollow-database loop passes for version 1 today.

- [x] **Step 3: Write the migration and bump the version**

Create `crates/notae-index/migrations/0002_fold_l_stroke.sql`:

```sql
-- Notae index schema, version 2.
--
-- Version 1 wrote the stored text into `notes_fts` unchanged. FTS5
-- `unicode61 remove_diacritics 2` cannot fold U+0142/U+0141 (ł/Ł: no canonical
-- decomposition), so a query typed without the stroke found nothing. From
-- version 2 every writer folds ł to l and Ł to L before the text reaches the
-- tokenizer (`crate::fold::fold_l_stroke`) and every query is folded the same
-- way. This migration brings the rows written by version 1 up to the same form,
-- in place, inside the migration transaction.
--
-- Only the derived `notes_fts` table changes; the canonical `notes` rows keep
-- the original spelling, and the sidecar manifest (which is built from `notes`)
-- is unaffected. Rows without ł/Ł are not rewritten.
--
-- The upgrade is forward-only: builds that only know version 1 refuse a
-- version 2 database.
--
-- A released migration is frozen. The mapping below is a copy of
-- `fold_l_stroke`; `a_version_1_index_is_folded_in_place_when_it_is_opened` in
-- `tests/store_and_search.rs` checks that the two agree.

UPDATE notes_fts
SET title    = replace(replace(title,    'ł', 'l'), 'Ł', 'L'),
    composer = replace(replace(composer, 'ł', 'l'), 'Ł', 'L'),
    tags     = replace(replace(tags,     'ł', 'l'), 'Ł', 'L'),
    filename = replace(replace(filename, 'ł', 'l'), 'Ł', 'L'),
    ocr_text = replace(replace(ocr_text, 'ł', 'l'), 'Ł', 'L')
WHERE instr(coalesce(title, '') || coalesce(composer, '') || coalesce(tags, '')
            || coalesce(filename, '') || coalesce(ocr_text, ''), 'ł') > 0
   OR instr(coalesce(title, '') || coalesce(composer, '') || coalesce(tags, '')
            || coalesce(filename, '') || coalesce(ocr_text, ''), 'Ł') > 0;
```

`crates/notae-index/src/schema.rs`:

Line 18-19:

```rust
/// Version of the schema this build creates and expects.
///
/// Version 2 folds the Polish `ł`/`Ł` in `notes_fts` (migration
/// `0002_fold_l_stroke.sql`). A database at version 2 is refused by builds that
/// only know version 1: the upgrade is forward-only.
pub const SCHEMA_VERSION: u32 = 2;
```

Lines 28-41 (rename the doc, keep the data; the list is what *every* version since 1 has):

```rust
/// Objects every schema version since 1 is made of, as `sqlite_master` type
/// and name.
///
/// Checked when opening, so a database that recorded a version without
/// actually having it is rejected. Version 2 only rewrites rows of `notes_fts`
/// and adds no object. The FTS5 shadow tables are deliberately not listed: they
/// are SQLite's own bookkeeping for `notes_fts` and are dropped and recreated
/// with it. The check cannot tell whether the rows of `notes_fts` are folded;
/// that is guaranteed by the migration transaction, not verified at open.
const SCHEMA_OBJECTS: &[(&str, &str)] = &[
    ("table", "schema_migrations"),
    ("table", "notes"),
    ("table", "notes_fts"),
    ("index", "notes_note_id_unique"),
    ("index", "notes_status_idx"),
];
```

(rename every use of `V1_OBJECTS` to `SCHEMA_OBJECTS`: only `verify_objects`).

Lines 54-59:

```rust
/// Every migration, ordered by ascending version.
pub const MIGRATIONS: &[Migration] = &[
    Migration {
        version: 1,
        name: "initial",
        sql: include_str!("../migrations/0001_initial.sql"),
    },
    Migration {
        version: 2,
        name: "fold_l_stroke",
        sql: include_str!("../migrations/0002_fold_l_stroke.sql"),
    },
];
```

In `migrate()`, verify the recorded version *before* applying newer migrations, so a database that claims a version without the schema is reported as incomplete instead of failing inside migration 0002 with "no such table". Insert after the `UnsupportedSchemaVersion` check (current lines 96-101):

```rust
    // A recorded version is a claim: check it before building on it, so a
    // hollow database is reported as such rather than failing inside the first
    // migration that touches a table it does not have.
    if current > 0 {
        verify_objects(conn, current)?;
    }
```

and update the doc of `migrate` (the `# Errors` section already names both errors) by adding a paragraph above it:

```rust
/// A database upgraded to a newer version is refused by older builds
/// ([`IndexError::UnsupportedSchemaVersion`]): that is intentional, because an
/// older build would write rows the newer schema's migrations had normalized.
```

`crates/notae-index/src/lib.rs`: extend the `notes_fts` bullet:

```rust
//! - `notes_fts` — an FTS5 index over note ID, title, composer/artist, tags,
//!   filename, and OCR text, sharing row identifiers with `notes`. Its text is
//!   written with Polish `ł`/`Ł` folded to `l`/`L` (schema version 2, see
//!   `fold`) and queries are folded the same way; `notes` keeps the original
//!   spelling.
```

- [x] **Step 4: Run the tests to verify they pass**

Run: `cargo fmt && cargo test -p notae-index`
Expected: PASS, including `open_creates_schema_at_the_current_version`, `reopening_an_existing_database_reapplies_no_migration`, `opening_a_database_from_a_newer_schema_is_refused` (seeded `99`; still `found == 99`, `supported == SCHEMA_VERSION`) and `a_failed_migration_leaves_the_database_unchanged`.

- [x] **Step 5: Commit**

```bash
git add crates/notae-index
git commit -m "feat(index): migrate existing FTS rows to the folded form" -m "Schema version 1 -> 2 (0002_fold_l_stroke.sql). Opening a version 2 index with an older build is refused. Refs #35"
```

---

### Task 3: Keep the lyric snippet in the original spelling

**Files:**
- Modify: `crates/notae-index/src/fold.rs` (add `restore_l_stroke`, `MARKERS`, tests)
- Modify: `crates/notae-index/src/search.rs:98-100`, `:107-118`, `:129-183`
- Test: `crates/notae-index/tests/store_and_search.rs`

**Interfaces:**
- Consumes: `fold_l_stroke`, `has_l_stroke` (Task 1); `SearchHit::text_snippet`.
- Produces: `pub(crate) fn restore_l_stroke(snippet: &str, ellipsis: &str, original: &str) -> String` — `snippet` is an FTS5 `snippet()` result over the folded text (markers `\u{2}`/`\u{3}`, optional leading/trailing `ellipsis`), `original` is the stored text; returns the snippet with the original characters, the snippet unchanged when it cannot be located.

- [x] **Step 1: Write the failing tests**

Append to `crates/notae-index/tests/store_and_search.rs`:

```rust
#[test]
fn lyric_snippets_keep_the_original_spelling() {
    let store = IndexStore::open(":memory:").expect("open");
    let lyrics = "Pałka zagrała kołysankę dla Łodzi i wszystkich dzieci które nie chciały spać aż do samego rana";
    store
        .upsert_note_with_ocr(
            &note("a.pdf", 1, None, "Kołysanka", None, &[]),
            Some(&OcrText::new(lyrics)),
        )
        .expect("insert");
    for query in ["palka", "Pałka", "PAŁKA"] {
        let hits = store.search_in(query, SearchScope::Text, 10).expect("search");
        let snippet = hits[0].text_snippet.as_deref().expect("snippet");
        assert_eq!(
            snippet,
            "\u{2}Pałka\u{3} zagrała kołysankę dla Łodzi i wszystkich dzieci które nie chciały spać…",
            "{query:?}"
        );
    }
}

#[test]
fn a_snippet_with_a_leading_ellipsis_keeps_the_original_spelling() {
    let store = IndexStore::open(":memory:").expect("open");
    store
        .upsert_note_with_ocr(
            &note("a.pdf", 1, None, "Kołysanka", None, &[]),
            Some(&OcrText::new(
                "a b c d e f g h i j k l m n o p q r Pałka zagrała x y z w",
            )),
        )
        .expect("insert");
    let hits = store
        .search_in("palka", SearchScope::Text, 10)
        .expect("search");
    assert_eq!(
        hits[0].text_snippet.as_deref(),
        Some("…m n o p q r \u{2}Pałka\u{3} zagrała x y z w")
    );
}
```

(The window is the 12-token window FTS5 `snippet(…, 12)` produces; the expected strings were derived by running the same statement against SQLite 3.51 with the folded text. If the bundled SQLite picks another window, correct the expected string to the snippet of the *folded* text with `ł`/`Ł` put back — the assertion that matters is that no `ł`/`Ł` of the original is lost and no folded `l` replaces one. The existing `text_scope_matches_only_page_text_and_returns_a_marked_snippet` covers text without a stroke.)

- [x] **Step 2: Run the tests to verify they fail**

Run: `cargo test -p notae-index --test store_and_search -- lyric_snippets_keep a_snippet_with_a_leading`
Expected: FAIL — the snippet reads `\u{2}Palka\u{3} zagrala kolysankę dla Lodzi …` (folded).

- [x] **Step 3: Implement `restore_l_stroke`**

In `crates/notae-index/src/fold.rs` add the marker constant next to `STROKES` (this was deferred from Task 1 to keep `-D warnings` green):

```rust
/// Characters FTS5 `snippet()` wraps around a matched term (see `search.rs`).
const MARKERS: [char; 2] = ['\u{2}', '\u{3}'];
```

and, above `#[cfg(test)]`:

```rust
/// Puts the original `ł`/`Ł` back into an FTS5 snippet of the folded text.
///
/// `snippet` is what `snippet()` returned for the folded column: a contiguous
/// piece of the folded text, with [`MARKERS`] around matches and `ellipsis`
/// added at either end when the window does not reach the end of the text.
/// Because [`fold_l_stroke`] keeps the char count, locating that piece in the
/// folded `original` gives the char range whose original spelling is wanted.
///
/// The ellipsis could also be a real character of the text, so the variants
/// "both ends stripped" down to "none stripped" are tried in turn. When the
/// piece occurs more than once, the first occurrence is used; occurrences fold
/// to identical text, so they differ only in spelling. When the piece cannot be
/// found (a stale index) the snippet is returned unchanged.
#[must_use]
pub(crate) fn restore_l_stroke(snippet: &str, ellipsis: &str, original: &str) -> String {
    let folded = fold_l_stroke(original);
    for (leading, trailing) in [(true, true), (true, false), (false, true), (false, false)] {
        let Some(body) = trim_ellipsis(snippet, ellipsis, leading, trailing) else {
            continue;
        };
        let plain: String = body
            .chars()
            .filter(|character| !MARKERS.contains(character))
            .collect();
        let Some(offset) = folded.find(&plain) else {
            continue;
        };
        let skipped = folded[..offset].chars().count();
        let mut source = original.chars().skip(skipped);
        let mut restored = String::with_capacity(snippet.len());
        if leading {
            restored.push_str(ellipsis);
        }
        for character in body.chars() {
            if MARKERS.contains(&character) {
                restored.push(character);
            } else {
                restored.push(source.next().unwrap_or(character));
            }
        }
        if trailing {
            restored.push_str(ellipsis);
        }
        return restored;
    }
    snippet.to_owned()
}

/// Strips the ellipsis from the requested ends, or `None` when it is absent.
fn trim_ellipsis<'a>(
    snippet: &'a str,
    ellipsis: &str,
    leading: bool,
    trailing: bool,
) -> Option<&'a str> {
    let mut body = snippet;
    if leading {
        body = body.strip_prefix(ellipsis)?;
    }
    if trailing {
        body = body.strip_suffix(ellipsis)?;
    }
    Some(body)
}
```

Add to the `tests` module in `fold.rs`:

```rust
    #[test]
    fn restores_the_original_letters_in_a_snippet() {
        let original = "Pałka zagrała kołysankę dla Łodzi";
        assert_eq!(
            restore_l_stroke("\u{2}Palka\u{3} zagrala kolysankę…", "…", original),
            "\u{2}Pałka\u{3} zagrała kołysankę…"
        );
        assert_eq!(
            restore_l_stroke("…kolysankę dla \u{2}Lodzi\u{3}", "…", original),
            "…kołysankę dla \u{2}Łodzi\u{3}"
        );
        assert_eq!(
            restore_l_stroke("…zagrala \u{2}kolysankę\u{3} dla…", "…", original),
            "…zagrała \u{2}kołysankę\u{3} dla…"
        );
    }

    #[test]
    fn an_ellipsis_that_is_part_of_the_text_is_kept() {
        assert_eq!(restore_l_stroke("…lodz", "…", "…łodz"), "…łodz");
        assert_eq!(restore_l_stroke("lodz…", "…", "łodz…"), "łodz…");
    }

    #[test]
    fn a_snippet_that_cannot_be_located_is_returned_unchanged() {
        assert_eq!(restore_l_stroke("zzz", "…", "abc"), "zzz");
        assert_eq!(restore_l_stroke("", "…", "abc"), "");
    }

    #[test]
    fn the_first_occurrence_is_used_when_the_piece_repeats() {
        // Folded text is "pal pal"; the first occurrence covers the first three
        // chars of the original, "pał".
        assert_eq!(
            restore_l_stroke("\u{2}pal\u{3}", "…", "pał pal"),
            "\u{2}pał\u{3}"
        );
    }
```

- [x] **Step 4: Use it in `search`**

`crates/notae-index/src/search.rs`: import `use crate::fold::{fold_l_stroke, has_l_stroke, restore_l_stroke};` and add above `SEARCH_SQL`:

```rust
/// Text `snippet()` puts at a cut end of its window; also what
/// `restore_l_stroke` must strip before it locates the window.
const SNIPPET_ELLIPSIS: &str = "…";
```

In `SEARCH_SQL` replace the literal ellipsis by a bound parameter:

```rust
       snippet(notes_fts, 5, char(2), char(3), ?5, 12) AS text_snippet
```

Pass it as the fifth parameter (after the limit):

```rust
        rusqlite::params![
            match_expression,
            exact_id,
            exact_title,
            i64::try_from(limit).unwrap_or(i64::MAX),
            SNIPPET_ELLIPSIS,
        ],
```

Replace the snippet read in the row closure and the final `text_snippet` assignment:

```rust
        |row| {
            let relevance: f64 = row.get(0)?;
            let snippet = match row.get::<_, Option<String>>("text_snippet")? {
                Some(text) if text.contains('\u{2}') => Some(with_original_spelling(row, text)?),
                _ => None,
            };
            let note = crate::store::note_from_row(row)?;
            Ok((relevance, snippet, note))
        },
```

```rust
        hits.push(SearchHit {
            rank: exact_id_match + exact_title_match - relevance,
            text_snippet: snippet,
            note,
        });
```

and add below `match_expression`:

```rust
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
```

Update the `SearchHit::text_snippet` doc (lines 97-99):

```rust
    /// Page-text excerpt around the match, with the matched terms between
    /// `\u{2}` and `\u{3}`, in the spelling of the stored text (`ł` stays `ł`
    /// although the index folds it); `None` when the page text did not match.
```

and add to the module docs of `search.rs` (after the ranking paragraph):

```rust
//! The index holds text with `ł`/`Ł` folded to `l`/`L` (see `crate::fold`), so
//! the query is folded the same way before it is split into terms, the
//! exact-title bonus compares folded titles, and the lyric snippet — which
//! `snippet()` cuts from the folded column — is mapped back onto the stored
//! page text before it is returned.
```

- [x] **Step 5: Run the tests to verify they pass**

Run: `cargo fmt && cargo test -p notae-index`
Expected: PASS (new snippet tests, `fold.rs` unit tests, and the existing `text_scope_matches_only_page_text_and_returns_a_marked_snippet`).

- [x] **Step 6: Commit**

```bash
git add crates/notae-index
git commit -m "fix(index): keep ł in lyric snippets of the folded index" -m "Refs #35"
```

---

### Task 4: Android Search generated-UI behavioural test

**Files:**
- Modify: `apps/notae-android/src/ui/search.rs:59-60` (`library_row` visibility)
- Modify: `apps/notae-android/tests/support/mod.rs`
- Modify: `apps/notae-android/tests/search_input.rs:19-20`, `:34-58`
- Create: `apps/notae-android/tests/search_stroke_ui.rs`

**Interfaces:**
- Consumes: `LibraryViewModel::{new, set_query, set_scope, search}` (`apps/notae-android/src/ui/library.rs`), `notae_android::{AppWindow, LibraryRow}`, the Search screen of `ui/search.slint` (`ResultRow` exposes `accessible-label` = title and `accessible-description` = snippet; scope chips are labelled `All`, `Titles`, `Composers`, `Lyrics`, `Tags`; results render only for a non-empty query).
- Produces: `notae_android::ui::search::library_row(row: SearchRow, thumbnail: Option<slint::Image>) -> LibraryRow` (now `pub`, host-visible); `support::{SEARCH_LABEL, focus_search_field, type_text}`.

`library_row` is the real projection `SearchRow` → Slint `LibraryRow` that `android_main.rs` pushes through `sync_window`; it is `#[cfg(target_os = "android")] pub(crate)` only because nothing else called it. The Android runtime wiring (`on_search_changed` → worker → `UiModel` → `sync_window`) cannot run on the host, so the test wires `on_search_changed`/`on_search_scope_changed` to the same `LibraryViewModel` + `library_row` the worker uses, over a real in-memory `IndexStore`, and observes the Search screen the way a TalkBack/uiautomator user does (accessible labels).

- [x] **Step 1: Write the failing test**

Create `apps/notae-android/tests/search_stroke_ui.rs`:

```rust
//! Search finds Polish text typed without `ł`, through the generated UI.
//!
//! The Search screen is driven with key events, over a real in-memory
//! `IndexStore` holding the score `Łódź — Pieśń żółta` (issue #35). The test
//! wires `search_changed`/`search_scope_changed` to the view model and row
//! projection the Android worker uses (`LibraryViewModel`, `library_row`) and
//! reads the results back from the screen's accessibility tree. It does not
//! prove that Samsung Keyboard or the worker thread deliver the same events.

mod support;

use std::{cell::Cell, rc::Rc};

use i_slint_backend_testing::ElementHandle;
use notae_android::ui::library::LibraryViewModel;
use notae_android::ui::search::library_row;
use notae_android::{AppWindow, LibraryRow};
use notae_domain::{
    IndexedNote, MetadataOrigin, NoteId, NoteMetadata, PdfFingerprint, RelativePdfPath,
    Sha256Digest,
};
use notae_index::{IndexStore, OcrText, SearchScope};
use slint::{ComponentHandle, ModelRc, VecModel};
use support::{app, focus_search_field, type_text};

const LODZ_TITLE: &str = "Łódź — Pieśń żółta";
const LODZ_LYRICS: &str = "Pałka zagrała kołysankę dla Gałuszki. Części stałe mszy: zmartwychwstał Pan.";

fn note(
    path: &str,
    seed: u8,
    id: u32,
    title: &str,
    composer: Option<&str>,
    tags: &[&str],
) -> IndexedNote {
    let mut metadata = NoteMetadata::new(
        title,
        composer.map(str::to_owned),
        tags.iter().map(|tag| (*tag).to_owned()),
    )
    .expect("valid title");
    metadata.assign_id(Some(NoteId::new(id)), MetadataOrigin::FilenameDerived);
    IndexedNote::new(
        RelativePdfPath::new(path).expect("valid relative path"),
        PdfFingerprint::new(Sha256Digest::from_bytes([seed; 32]), 1_024, None),
        metadata,
    )
}

fn library() -> Rc<IndexStore> {
    let store = IndexStore::open(":memory:").expect("open in-memory index");
    store
        .upsert_note_with_ocr(
            &note(
                "lodz.pdf",
                1,
                303,
                LODZ_TITLE,
                Some("Stanisław Moniuszko"),
                &["kołysanka"],
            ),
            Some(&OcrText::new(LODZ_LYRICS)),
        )
        .expect("insert");
    store
        .upsert_note_with_ocr(
            &note("ave.pdf", 2, 13, "Ave Maria", Some("Schubert"), &["solo"]),
            Some(&OcrText::new("Ave Maria gratia plena")),
        )
        .expect("insert");
    Rc::new(store)
}

/// What the Android worker does for a query: search, project, publish the rows.
fn publish(app: &AppWindow, store: &IndexStore, scope: SearchScope, query: &str) {
    let mut model = LibraryViewModel::new(store);
    model.set_query(query.to_owned());
    model.set_scope(scope);
    let rows: Vec<LibraryRow> = model
        .search()
        .expect("search the index")
        .into_iter()
        .map(|row| library_row(row, None))
        .collect();
    app.set_library_rows(ModelRc::new(VecModel::from(rows)));
}

fn open_search(store: &Rc<IndexStore>) -> AppWindow {
    let app = app();
    app.set_show_search(true);
    let scope = Rc::new(Cell::new(SearchScope::Everything));
    {
        let (weak, store, scope) = (app.as_weak(), Rc::clone(store), Rc::clone(&scope));
        app.on_search_changed(move |query| {
            if let Some(app) = weak.upgrade() {
                publish(&app, &store, scope.get(), &query);
            }
        });
    }
    {
        let (weak, store, scope) = (app.as_weak(), Rc::clone(store), Rc::clone(&scope));
        app.on_search_scope_changed(move |key| {
            let Some(app) = weak.upgrade() else { return };
            scope.set(SearchScope::from_key(&key));
            app.set_search_scope(key);
            publish(&app, &store, scope.get(), app.get_search_text().as_str());
        });
    }
    slint::platform::update_timers_and_animations();
    app
}

/// `(title, snippet)` of every result row on the Search screen, best first.
fn results(app: &AppWindow) -> Vec<(String, String)> {
    ElementHandle::find_by_element_type_name(app, "ResultRow")
        .map(|row| {
            (
                row.accessible_label().unwrap_or_default().to_string(),
                row.accessible_description().unwrap_or_default().to_string(),
            )
        })
        .collect()
}

/// Picks the scope chip, then types `query` into the Search field.
fn search(store: &Rc<IndexStore>, chip: &str, query: &str) -> Vec<(String, String)> {
    let app = open_search(store);
    ElementHandle::find_by_accessible_label(&app, chip)
        .next()
        .unwrap_or_else(|| panic!("`{chip}` scope chip is exposed"))
        .invoke_accessible_default_action();
    focus_search_field(&app);
    type_text(&app, query);
    slint::platform::update_timers_and_animations();
    results(&app)
}

fn titles(found: &[(String, String)]) -> Vec<&str> {
    found.iter().map(|(title, _)| title.as_str()).collect()
}

#[test]
fn queries_with_and_without_the_stroke_find_the_score_in_every_scope() {
    let store = library();
    // (scope chip, queries); every query must show exactly the Łódź score.
    let cases: [(&str, &[&str]); 5] = [
        ("Titles", &["lodz", "Lodz", "ŁÓDŹ", "łódź", "zolta piesn", "żółta"]),
        ("Composers", &["stanislaw", "STANISŁAW", "Stanisław"]),
        ("Tags", &["kolysanka", "kołysanka"]),
        (
            "Lyrics",
            &["palka", "Pałka", "galuszki", "czesci stale", "części stałe", "zmartwychwstal", "ZMARTWYCHWSTAŁ"],
        ),
        ("All", &["lodz", "stanislaw", "kolysanka", "palka", "zmartwychwstal", "Łódź"]),
    ];
    for (chip, queries) in cases {
        for query in queries {
            let found = search(&store, chip, query);
            assert_eq!(titles(&found), [LODZ_TITLE], "{query:?} under {chip}");
        }
    }
}

#[test]
fn lyric_results_show_the_stored_spelling_not_the_folded_one() {
    let store = library();
    // (query, the word as printed in the lyrics)
    for (query, printed) in [
        ("palka", "Pałka"),
        ("galuszki", "Gałuszki"),
        ("czesci stale", "stałe"),
        ("zmartwychwstal", "zmartwychwstał"),
    ] {
        let found = search(&store, "Lyrics", query);
        assert_eq!(titles(&found), [LODZ_TITLE], "{query:?}");
        let snippet = &found[0].1;
        assert!(snippet.contains(printed), "{query:?} -> {snippet:?}");
        let folded = printed.replace('ł', "l").replace('Ł', "L");
        assert!(!snippet.contains(&folded), "{query:?} -> {snippet:?}");
        assert!(!snippet.contains(['\u{2}', '\u{3}']), "markers leaked: {snippet:?}");
    }
}

#[test]
fn folding_does_not_leak_across_scopes_and_the_control_score_stays_findable() {
    let store = library();
    assert!(search(&store, "Composers", "lodz").is_empty());
    assert!(search(&store, "Titles", "palka").is_empty());
    assert!(search(&store, "Tags", "stanislaw").is_empty());
    assert_eq!(titles(&search(&store, "All", "ave")), ["Ave Maria"]);
}
```

In `apps/notae-android/src/ui/search.rs` (needed for the file above to compile), replace

```rust
#[cfg(target_os = "android")]
pub(crate) fn library_row(row: SearchRow, thumbnail: Option<slint::Image>) -> crate::LibraryRow {
```

with

```rust
/// Projects a search result onto the Slint row the Library and Search screens show.
///
/// `android_main` calls this for every row it publishes; host tests call it to
/// drive the generated UI with the real projection.
pub fn library_row(row: SearchRow, thumbnail: Option<slint::Image>) -> crate::LibraryRow {
```

Replace the `SEARCH_LABEL` const and `focus_search` helper of `apps/notae-android/tests/search_input.rs` (see Step 3) and add the shared helpers to `apps/notae-android/tests/support/mod.rs`:

```rust
use i_slint_backend_testing::ElementHandle;
use notae_android::AppWindow;
use slint::platform::{PointerEventButton, WindowEvent};
use slint::{ComponentHandle, LogicalPosition, SharedString};
use std::cell::Cell;

/// Accessible label of the Search field.
#[allow(dead_code)] // Shared by every test binary; only Search tests need it.
pub const SEARCH_LABEL: &str = "Search titles, composers, tags or lyrics";

/// Clicks the centre of the Search field so it holds keyboard focus.
#[allow(dead_code)]
pub fn focus_search_field(app: &AppWindow) {
    let field = ElementHandle::find_by_accessible_label(app, SEARCH_LABEL)
        .next()
        .expect("Search field is mounted");
    let position = field.absolute_position();
    let size = field.size();
    let center = LogicalPosition::new(
        position.x + size.width / 2.0,
        position.y + size.height / 2.0,
    );
    app.window()
        .dispatch_event(WindowEvent::PointerMoved { position: center });
    app.window().dispatch_event(WindowEvent::PointerPressed {
        position: center,
        button: PointerEventButton::Left,
    });
    app.window().dispatch_event(WindowEvent::PointerReleased {
        position: center,
        button: PointerEventButton::Left,
    });
}

/// Types `text` one key press at a time, as a hardware keyboard would.
#[allow(dead_code)]
pub fn type_text(app: &AppWindow, text: &str) {
    for character in text.chars() {
        let key = SharedString::from(character.to_string());
        app.window()
            .dispatch_event(WindowEvent::KeyPressed { text: key.clone() });
        app.window()
            .dispatch_event(WindowEvent::KeyReleased { text: key });
    }
}
```

(Keep the existing `use slint::ComponentHandle; use std::cell::Cell;` lines only once; merge imports into the block above. The existing `app`, `app_sized` and `initialize_backend` are unchanged.)

- [x] **Step 2: Run the new test to verify it fails for the right reason**

Red check (the production fix from Tasks 1-3 is already in the tree, so break it for a moment): in `crates/notae-index/src/fold.rs` change the body of `fold_l_stroke` to `Cow::Borrowed(text)`, then run:

Run: `cargo test -p notae-android --test search_stroke_ui`
Expected: FAIL — `queries_with_and_without_the_stroke_find_the_score_in_every_scope` with `left: [], right: ["Łódź — Pieśń żółta"]` for `"lodz" under Titles`. Revert the temporary change and run again: all three tests PASS. If `ElementHandle` finds no `ResultRow`, the `ListView` has not instantiated its rows yet: call `i_slint_backend_testing::mock_elapsed_time(std::time::Duration::from_millis(500))` after `type_text` in `search`, as `library_ui.rs::settle` does.

- [x] **Step 3: Make `search_input.rs` reuse the shared helper**

In `apps/notae-android/tests/search_input.rs` replace the `use support::{app, app_sized};` import and the `SEARCH_LABEL` const with:

```rust
use support::{SEARCH_LABEL, app, app_sized, focus_search_field};
```

and replace the body of `focus_search` after the callback registration — the block from `let field = …` through the three `dispatch_event` calls (lines 40-57) — with one call:

```rust
fn focus_search(app: AppWindow) -> (AppWindow, Rc<RefCell<Vec<String>>>) {
    app.set_show_search(true);
    let queries = Rc::new(RefCell::new(Vec::new()));
    let queries_for_callback = Rc::clone(&queries);
    app.on_search_changed(move |query| queries_for_callback.borrow_mut().push(query.to_string()));
    slint::platform::update_timers_and_animations();

    focus_search_field(&app);
    (app, queries)
}
```

Remove the imports that become unused (`LogicalPosition`, `PointerEventButton`) only if the compiler reports them unused; `WindowEvent`, `SharedString` and `ElementHandle` are still used by the other tests in that file.

- [x] **Step 4: Run the Android host tests**

Run: `cargo test -p notae-android --test search_stroke_ui --test search_input --test search_state --test voice_search`
Expected: PASS.

- [x] **Step 5: Commit**

```bash
git add apps/notae-android
git commit -m "test(android): drive Search over a real index for ł-less queries" -m "Refs #35"
```

---

### Task 5: Documentation, changelog and full verification

**Files:**
- Modify: `README.md:58-59`
- Modify: `docs/android-debugging.md:59-60` (Issue #13 section, `### Task 1: startup and input feedback (#1, #2, #3, #13)`)
- Modify: `CHANGELOG.md:11-17` (`## [Unreleased]`)
- Modify: `apps/notae-index/README.md:27-29`

**Interfaces:** none (documentation).

- [x] **Step 1: README**

Replace (lines 57-59, `**Search** …` bullet tail):

```
  hardware keyboard is attached; a microphone action appears when Android
  provides a speech recognizer. FTS5 folds combining diacritics, so
  `ą ć ę ń ó ś ź ż` match their plain letters; `ł` does **not** match `l`.
```

with

```
  hardware keyboard is attached; a microphone action appears when Android
  provides a speech recognizer. Search ignores case and Polish diacritics:
  `ą ć ę ń ó ś ź ż` match their plain letters and `ł`/`Ł` match `l`/`L`, so
  `lodz` finds `Łódź` in titles, composers, tags and lyrics.
```

(keep the surrounding lines byte-identical; check the first line's exact wrapping with `read README.md:54-60` before editing.)

- [x] **Step 2: `docs/android-debugging.md`**

Replace the whole of line 60 (`- Observed FTS5 `unicode61` folding (no tokenizer or schema change was made): …`) with:

```
- Search folding (#35, supersedes the limitation first observed for #13): FTS5 `unicode61 remove_diacritics 2` removes combining diacritics (`ą ć ę ń ó ś ź ż`) but `ł`/`Ł` have no decomposition, so `lodz`, `Lodz` and `zolta` found nothing in 0.2.0/0.2.1. Notae now folds `ł`→`l` and `Ł`→`L` itself (`crates/notae-index/src/fold.rs`) on every `notes_fts` row it writes (title, composer, tags, filename, page text) and on every query. Migration `0002_fold_l_stroke.sql` (index schema version 2) folds rows written by 0.2.x in place the first time the index is opened; the stored `notes` text, displayed titles and lyric snippets keep the original spelling. `lodz`, `palka`, `Galuszka`, `czesci stale` and `zmartwychwstal` now find `Łódź — Pieśń żółta`, `Pałka`, `Gałuszka`, `części stałe` and `zmartwychwstał`. `l` and `ł` are one letter for search, so `lodz` also finds a score titled `Lodz`. The schema upgrade is forward-only: an older build refuses a version 2 index. Verified on host tests only; not yet on the Galaxy Tab S10 FE+.
```

Append to the end of line 59 (after `…in Titles and Everything scopes.`); the text below is what was committed:

```
 For #35 the same file indexes `Łódź — Pieśń żółta` with composer, tag and lyrics containing `ł` and checks the issue's queries (`palka`, `Galuszka`, `czesci stale`, `zmartwychwstal`, `lodz`) and their diacritic forms in every scope, the migration of a version 1 index, and snippet spelling; `apps/notae-android/tests/search_stroke_ui.rs` types the queries into the Search screen over a real in-memory index and reads the result rows (title, lyric snippet) from the accessibility tree in every scope.
```

- [x] **Step 3: CHANGELOG**

In `## [Unreleased]` add under the existing `### Fixed` list, then a new `### Changed` section (blank line between sections, same bullet/wrapping style as the entry above it):

```markdown
- Search ([#35](https://github.com/eRgo35/notae/issues/35)): the Polish letters
  `ł` and `Ł` now match `l` and `L`, so `palka`, `Galuszka`, `czesci stale`,
  `zmartwychwstal` and `lodz` find `Pałka`, `Gałuszka`, `części stałe`,
  `zmartwychwstał` and `Łódź` in titles, composers, tags and lyrics, in every
  Search scope. Queries typed with the diacritics keep working, and titles and
  lyric snippets are still shown in their original spelling. `l` and `ł` are
  now one letter for search, so `lodz` also finds a score titled `Lodz`.

### Changed

- Index schema version 2 ([#35](https://github.com/eRgo35/notae/issues/35)):
  the search index folds `ł`/`Ł` to `l`/`L` as it is written, and existing
  indexes are migrated in place the first time this build opens them. The
  upgrade is forward-only: older builds refuse a version 2 index. The sidecar
  format is unchanged (`SIDECAR_SCHEMA_VERSION` stays `2`).
```

- [x] **Step 4: `apps/notae-index/README.md`**

After the bullet list of the two files (line 29, after `the Android app reads. This is the cross-platform contract.`) add:

```markdown
`index.sqlite` is upgraded in place by the first build that knows a newer schema
version (version 2 folds the Polish `ł`/`Ł` in its search table, issue #35); the
upgrade is forward-only, so an older `notae-index` refuses the file. The
`manifest.json` format is unchanged.
```

- [x] **Step 5: Full verification**

Run, in this order (from `docs/engineering-standards.md`; on a Fedora/RHEL host without system pkg-config files first export `PKG_CONFIG_PATH=/tmp/tesseract-pkgconfig` as that document describes):

```sh
cargo fmt --check
cargo clippy --workspace --all-targets -- -D warnings -A clippy::unwrap_used -A clippy::expect_used
cargo clippy --workspace --lib --bins -- -D warnings -D clippy::unwrap_used -D clippy::expect_used
cargo test --workspace
RUSTDOCFLAGS="-D warnings" cargo doc --workspace --no-deps
cargo deny check
cargo llvm-cov --workspace --summary-only --fail-under-lines 84
cargo run -p notae-index-cli -- --help
cargo apk2 check -p notae-android --target aarch64-linux-android
```

Expected: all exit 0; `cargo test --workspace` includes `store_and_search` (new tests), the `fold` unit tests and `search_stroke_ui`. No dependency changed, so `cargo deny check` and `Cargo.lock` are unchanged.

- [x] **Step 6: Commit**

```bash
git add README.md docs/android-debugging.md CHANGELOG.md apps/notae-index/README.md docs/superpowers/plans/2026-10-19-notae-polish-l-stroke-search.md
git commit -m "docs: record ł/Ł search folding and index schema 2" -m "Closes the documented limitation. Refs #35"
```

---

## Implementation notes

The plan was executed as written except for three corrections, already folded into the code above:

1. The exact-title test uses `Łaska` / `Łaska łaskawa` because the bonus never folded ó/ź (a `lodz` query cannot earn it for `Łódź`).
2. The Android lyric case is `("czesci stale", "stałe")`: `Części` contains no `ł`, so folding cannot change it.
3. The `SCHEMA_VERSION` doc comment has no `[`migrate`]` link (`migrate` is private; the rustdoc `-D warnings` gate rejects it).

Also: `cargo fmt` found rustfmt drift in two unrelated test files that failed `cargo fmt --check` on a clean `main`; it was committed separately as `style:`.

## Notes for the PR

- Summary: `ł`/`Ł` are folded on both sides of the FTS index through one function; existing indexes migrate in place (schema 1 → 2); displayed text keeps its spelling; sidecar format unchanged.
- Maintainer decisions flagged: (1) accepting the one-way schema bump (forward-only: an older APK/CLI refuses a version 2 index; accepted by the maintainer); (2) restoring the original spelling in lyric snippets instead of showing folded text.
- **Later work, not in this change (issue acceptance 7):** the same tokenizer gap exists for `đ`, `ø`, `æ`, `œ`, `ß` (verified with `unicode61 remove_diacritics 2` on SQLite 3.51: `ore`, `dorde`, `caeli`, `coeli`, `strasse` match none of `Øre`, `Đorđe`, `Cæli`, `Cœli`, `Straße`; `only_the_l_stroke_is_folded` pins that). For a Polish music library the relevant ones are `æ`/`œ` (Latin liturgical text: `cæli`/`cœli` vs `caeli`) and `ß` (German lyrics). `đ`, `ø` are rarer (Croatian/Scandinavian composers). `æ`→`ae`, `œ`→`oe`, `ß`→`ss` are one-to-many, so they would break the 1:1 char property that `restore_l_stroke` relies on; that change needs a different snippet strategy, not just more entries in `fold_l_stroke`. `đ`→`d`, `ø`→`o` are 1:1 and could reuse the mechanism.
- Not verified: the Galaxy Tab S10 FE+; the Android worker thread and `sync_window` wiring (the host UI test reuses `LibraryViewModel` and `library_row` but not the worker); the cost of `restore_l_stroke` on a very large library (one fold and one substring search per ł-bearing hit that has a lyric match; `search_in(…, usize::MAX)` on a broad query is the worst case) was reasoned about, not measured.

## Self-Review

**1. Spec coverage (issue #35 acceptance):**

| # | Requirement | Where |
| --- | --- | --- |
| 1 | `palka`, `Galuszka`, `czesci stale`, `zmartwychwstal`, `lodz` find `Pałka`, `Gałuszka`, `części stałe`, `zmartwychwstał`, `Łódź` | Task 1 `the_examples_of_issue_35_find_their_scores`; Task 4 UI test cases |
| 2 | Diacritic queries still work | Task 1 table (`Łódź`, `ŁÓDŹ`, `żółta`, `Pałka`, mixed forms); Task 4 (`łódź`, `Stanisław`, `kołysanka`, `części stałe`) |
| 3 | Ł ł L l all work | Task 1 table (`ŁÓDŹ`, `łÓdŹ`, `LODZ`, `lodz`), `plain_l_and_stroke_l_are_one_letter_in_both_directions`; fold unit tests |
| 4 | Titles, composers, tags, OCR text in every scope | Task 1 `queries_with_and_without_the_stroke_find_polish_text_in_every_scope` (all five `SearchScope`s); Task 4 chips `Titles/Composers/Tags/Lyrics/All`; filename column via migration parity dump (Task 2) |
| 5 | Existing indexes keep working; one shared function; migration/rebuild path; no silent sidecar change | Task 2 (migration 0002, `SCHEMA_VERSION` 2, parity test, hollow-version test); *Migration and the Android index*; Global Constraints (sidecar stays 2, reasons); Task 5 notes in CHANGELOG, READMEs, SQL header |
| 6 | Regression tests in `crates/notae-index` with the `Łódź — Pieśń żółta` fixture (exact and diacritic-less) and an Android Search generated-UI test | Task 1 `lodz_store` (`LODZ_TITLE`), Task 4 `search_stroke_ui.rs` |
| 7 | ł/Ł only; note đ ø æ œ ß | Global Constraints; `only_the_l_stroke_is_folded`; *Notes for the PR* |
| – | Update README, `docs/android-debugging.md`, CHANGELOG | Task 5 Steps 1-4 |

Exact-title bonus (brief): Task 1 (`the_exact_title_bonus_ignores_the_stroke`, SQL and Rust side). Snippet spelling (brief): Task 3. Verification commands: Task 5 Step 5. Gaps: none open.

**2. Placeholder scan:** no `TBD`/`TODO`/"add appropriate…" left. Two places depend on observed runtime output and say how to resolve it: the FTS5 snippet window strings in Task 3 (derived from the same statement on SQLite 3.51, with a fallback rule) and the possible `mock_elapsed_time` call in Task 4 Step 2. The README and `docs/android-debugging.md` edits quote the exact old text to replace; re-read the file range before editing because earlier edits do not shift these files but line numbers may differ.

**3. Type consistency:** `fold_l_stroke(&str) -> Cow<'_, str>`, `has_l_stroke(&str) -> bool`, `restore_l_stroke(&str, &str, &str) -> String` are defined in Tasks 1 and 3 and used with those signatures in `store.rs`, `search.rs` and the tests; `SNIPPET_ELLIPSIS` is the one source for the SQL parameter `?5` and the `ellipsis` argument; `LODZ_TITLE`/`LODZ_LYRICS` are defined once in `store_and_search.rs` (Task 1) and reused by Task 2's `upgrade_fixture`; `library_row`, `focus_search_field`, `type_text`, `SEARCH_LABEL` are defined in Task 4 and used only there. `SCHEMA_OBJECTS` replaces `V1_OBJECTS` in `schema.rs` only.
