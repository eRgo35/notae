//! Score notes: setting, loading, saving and repairing the score notes file.

use notae_app_core::favorites::PieceRef;
use notae_app_core::score_notes::ScoreNotes;

fn key(c: char) -> String {
    c.to_string().repeat(64)
}

fn piece(c: char, path: &str) -> PieceRef {
    PieceRef {
        key: key(c),
        path: path.to_owned(),
    }
}

#[test]
fn set_trims_and_blank_clears() {
    let mut notes = ScoreNotes::default();
    assert!(notes.set(piece('a', "a.pdf"), "  slow start \n"));
    assert_eq!(notes.get(&key('a')), Some("slow start"));
    assert!(!notes.set(piece('a', "a.pdf"), "slow start"));
    assert!(!notes.set(piece('b', "b.pdf"), "   "));
    assert!(notes.set(piece('a', "a.pdf"), "  "));
    assert_eq!(notes.get(&key('a')), None);
    assert!(notes.items().is_empty());
}

#[test]
fn round_trips_json() {
    let mut notes = ScoreNotes::default();
    notes.set(piece('a', "a.pdf"), "one");
    notes.set(piece('b', "b.pdf"), "two");
    let json = notes.to_json();
    assert!(json.starts_with(r#"{"schema_version":1,"notes":["#));
    assert_eq!(ScoreNotes::from_json(&json), notes);
}

#[test]
fn corrupt_or_unknown_version_is_empty() {
    assert!(ScoreNotes::from_json("not json").items().is_empty());
    assert!(
        ScoreNotes::from_json(r#"{"schema_version":2,"notes":[]}"#)
            .items()
            .is_empty()
    );
}

#[test]
fn score_note_relinks_by_path() {
    let mut notes = ScoreNotes::default();
    notes.set(piece('a', "p.pdf"), "text");
    assert!(notes.relink(&[(key('b'), "p.pdf".to_owned())]));
    assert_eq!(notes.get(&key('b')), Some("text"));
    assert_eq!(notes.get(&key('a')), None);
}

#[test]
fn notes_relinked_onto_one_key_keep_both_texts() {
    let mut notes = ScoreNotes::default();
    notes.set(piece('b', "new.pdf"), "breathe");
    notes.set(piece('a', "new.pdf"), "slowly");
    notes.set(piece('c', "copy.pdf"), "same");
    notes.set(piece('d', "copy.pdf"), "same");

    // `a` and `d` left the library; their paths now hold `b` and `c`.
    assert!(notes.relink(&[
        (key('b'), "new.pdf".to_owned()),
        (key('c'), "copy.pdf".to_owned()),
    ]));

    assert_eq!(notes.get(&key('b')), Some("breathe\n\nslowly"));
    assert_eq!(notes.get(&key('c')), Some("same"), "a repeated text once");
    assert_eq!(notes.items().len(), 2);
}

#[test]
fn merge_replaces_same_key() {
    let mut notes = ScoreNotes::default();
    notes.set(piece('a', "a.pdf"), "old");
    let mut incoming = ScoreNotes::default();
    incoming.set(piece('a', "a.pdf"), "new");
    incoming.set(piece('b', "b.pdf"), "other");
    notes.merge(incoming);
    assert_eq!(notes.get(&key('a')), Some("new"));
    assert_eq!(notes.get(&key('b')), Some("other"));
    assert_eq!(notes.items().len(), 2);
}
