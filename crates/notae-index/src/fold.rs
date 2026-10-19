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
