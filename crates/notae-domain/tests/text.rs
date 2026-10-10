//! Tests for the shared text normalization helpers.

use notae_domain::normalize_whitespace;

#[test]
fn empty_string_collapses_to_empty() {
    assert_eq!(normalize_whitespace(""), "");
}

#[test]
fn trims_leading_and_trailing_whitespace() {
    assert_eq!(normalize_whitespace("   hello   "), "hello");
}

#[test]
fn collapses_runs_of_spaces() {
    assert_eq!(normalize_whitespace("hello   world"), "hello world");
}

#[test]
fn collapses_mixed_whitespace_runs() {
    assert_eq!(normalize_whitespace("a\tb\nc\r\nd\x0ce"), "a b c d e");
}

#[test]
fn all_whitespace_collapses_to_empty() {
    assert_eq!(normalize_whitespace("   \t\n\r  "), "");
}

#[test]
fn preserves_single_whitespace_between_tokens() {
    assert_eq!(normalize_whitespace("one two three"), "one two three");
}

#[test]
fn does_not_alter_interior_punctuation() {
    assert_eq!(
        normalize_whitespace("Zażółć  gęślą  jaźń"),
        "Zażółć gęślą jaźń",
    );
}
