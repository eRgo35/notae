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

/// Characters FTS5 `snippet()` wraps around a matched term (see `search.rs`).
const MARKERS: [char; 2] = ['\u{2}', '\u{3}'];

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
}
