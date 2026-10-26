//! Every `@tr("…")` string in `ui/*.slint` has a complete Polish translation, and the
//! Polish file carries no strings the UI no longer uses.

use std::collections::{BTreeMap, BTreeSet};
use std::fs;
use std::path::Path;

/// One translatable message: the English singular and, for counts, the plural.
#[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord)]
struct Message {
    singular: String,
    plural: Option<String>,
}

const POLISH_PLURAL_FORMS: usize = 3;

#[test]
fn every_ui_string_has_a_polish_translation() {
    let sources = source_messages();
    let catalog = polish_catalog();

    let missing: Vec<_> = sources
        .iter()
        .filter(|message| !catalog.contains_key(message))
        .collect();
    assert!(missing.is_empty(), "untranslated strings: {missing:#?}");

    let stale: Vec<_> = catalog
        .keys()
        .filter(|message| !sources.contains(message))
        .collect();
    assert!(
        stale.is_empty(),
        "translations no UI string uses: {stale:#?}"
    );
}

#[test]
fn polish_translations_are_complete_and_keep_placeholders() {
    for (message, forms) in polish_catalog() {
        let expected = if message.plural.is_some() {
            POLISH_PLURAL_FORMS
        } else {
            1
        };
        assert_eq!(forms.len(), expected, "wrong form count for {message:?}");
        for form in &forms {
            assert!(!form.is_empty(), "empty translation for {message:?}");
            let mut english = placeholders(&message.singular);
            if let Some(plural) = &message.plural {
                english.extend(placeholders(plural));
            }
            let polish = placeholders(form);
            // A Polish singular may spell out "one" instead of `{n}`; nothing else may change.
            let english_without_count: BTreeSet<_> =
                english.iter().filter(|p| *p != "{n}").cloned().collect();
            let polish_without_count: BTreeSet<_> =
                polish.iter().filter(|p| *p != "{n}").cloned().collect();
            assert_eq!(
                polish_without_count, english_without_count,
                "placeholders differ in {form:?} for {message:?}"
            );
        }
    }
}

fn source_messages() -> BTreeSet<Message> {
    let ui = Path::new(env!("CARGO_MANIFEST_DIR")).join("ui");
    let mut messages = BTreeSet::new();
    for entry in fs::read_dir(ui).expect("ui directory is readable") {
        let path = entry.expect("ui entry is readable").path();
        if path
            .extension()
            .is_some_and(|extension| extension == "slint")
        {
            let text = fs::read_to_string(&path).expect("slint source is readable");
            messages.extend(tr_calls(&text));
        }
    }
    messages
}

/// Reads the literal arguments of each `@tr("singular" | "plural" % n, …)` call.
fn tr_calls(text: &str) -> Vec<Message> {
    let mut messages = Vec::new();
    let mut rest = text;
    while let Some(start) = rest.find("@tr(") {
        rest = rest[start + 4..].trim_start();
        let (singular, after) = slint_string(rest);
        rest = after.trim_start();
        let plural = if let Some(after_bar) = rest.strip_prefix('|') {
            let (plural, after) = slint_string(after_bar.trim_start());
            rest = after;
            Some(plural)
        } else {
            None
        };
        messages.push(Message { singular, plural });
    }
    messages
}

/// Parses a Slint string literal at the start of `text`; returns it and the remaining text.
/// Escapes are kept as written, which is also how they appear in the `.po` file.
fn slint_string(text: &str) -> (String, &str) {
    let body = text
        .strip_prefix('"')
        .unwrap_or_else(|| panic!("@tr takes a string literal: {text:.40}"));
    let mut escaped = false;
    for (index, character) in body.char_indices() {
        match character {
            '\\' if !escaped => escaped = true,
            '"' if !escaped => return (body[..index].to_owned(), &body[index + 1..]),
            _ => escaped = false,
        }
    }
    panic!("unterminated string literal: {text:.40}")
}

/// Maps each message to its translated forms (one, or one per plural form).
fn polish_catalog() -> BTreeMap<Message, Vec<String>> {
    let path =
        Path::new(env!("CARGO_MANIFEST_DIR")).join("translations/pl/LC_MESSAGES/notae-android.po");
    let text = fs::read_to_string(path).expect("Polish catalog is readable");
    let mut catalog = BTreeMap::new();
    for block in text.split("\n\n") {
        let mut fields: Vec<(String, String)> = Vec::new();
        for line in block.lines().map(str::trim) {
            if line.starts_with('#') || line.is_empty() {
                continue;
            }
            if let Some(continuation) = line.strip_prefix('"') {
                let value = continuation
                    .strip_suffix('"')
                    .expect("continuation line ends with a quote");
                fields
                    .last_mut()
                    .expect("continuation follows a keyword")
                    .1
                    .push_str(value);
            } else {
                let (keyword, value) = line.split_once(' ').expect("keyword and value");
                let value = value
                    .strip_prefix('"')
                    .and_then(|value| value.strip_suffix('"'))
                    .expect("quoted value");
                fields.push((keyword.to_owned(), value.to_owned()));
            }
        }
        let field = |name: &str| {
            fields
                .iter()
                .find(|(keyword, _)| keyword == name)
                .map(|(_, value)| value.clone())
        };
        let Some(singular) = field("msgid") else {
            continue;
        };
        if singular.is_empty() {
            continue; // header
        }
        let plural = field("msgid_plural");
        let forms = if plural.is_some() {
            fields
                .iter()
                .filter(|(keyword, _)| keyword.starts_with("msgstr["))
                .map(|(_, value)| value.clone())
                .collect()
        } else {
            vec![field("msgstr").unwrap_or_default()]
        };
        let message = Message { singular, plural };
        assert!(
            catalog.insert(message.clone(), forms).is_none(),
            "duplicate entry {message:?}"
        );
    }
    catalog
}

/// `{}`, `{0}`, `{n}`… in a message; `\{` is a literal brace, not a placeholder.
fn placeholders(text: &str) -> BTreeSet<String> {
    let mut found = BTreeSet::new();
    let mut previous = '\0';
    let mut open = None;
    for (index, character) in text.char_indices() {
        match character {
            '{' if previous != '\\' => open = Some(index),
            '}' => {
                if let Some(start) = open.take() {
                    found.insert(text[start..=index].to_owned());
                }
            }
            _ => {}
        }
        previous = character;
    }
    found
}
