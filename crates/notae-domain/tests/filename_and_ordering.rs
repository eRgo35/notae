//! Filename convention, default browsing order, and the unique-ID invariant.

use std::cmp::Ordering;

use notae_domain::{
    IndexedNote, MetadataField, MetadataOrigin, NoteId, PdfFingerprint, RelativePdfPath,
    Sha256Digest, compare_library_order, parse_filename, validate_unique_ids,
};

fn fingerprint() -> PdfFingerprint {
    PdfFingerprint::new(Sha256Digest::from_bytes([7; 32]), 1024, None)
}

fn note(filename: &str) -> IndexedNote {
    let parsed = parse_filename(filename);
    IndexedNote::new(
        RelativePdfPath::new(filename).expect("valid relative path"),
        fingerprint(),
        parsed.into_note_metadata().expect("non-empty title"),
    )
}

#[test]
fn parses_leading_id_and_title() {
    let parsed = parse_filename("12 - Autumn Leaves.pdf");
    assert_eq!(parsed.id(), Some(NoteId::new(12)));
    assert_eq!(parsed.title(), "Autumn Leaves");
}

#[test]
fn parses_five_hundred_series_id() {
    let parsed = parse_filename("507 - My Practice Arrangement.pdf");
    assert_eq!(parsed.id(), Some(NoteId::new(507)));
    assert_eq!(parsed.title(), "My Practice Arrangement");
}

#[test]
fn parses_zero_id() {
    let parsed = parse_filename("0 - Zero.pdf");
    assert_eq!(parsed.id(), Some(NoteId::new(0)));
    assert_eq!(parsed.title(), "Zero");
}

#[test]
fn unnumbered_file_keeps_whole_stem_as_title() {
    let parsed = parse_filename("Autumn Leaves.pdf");
    assert_eq!(parsed.id(), None);
    assert_eq!(parsed.title(), "Autumn Leaves");
}

#[test]
fn tolerates_repeated_and_missing_whitespace_around_separator() {
    for filename in [
        "12   -   Autumn Leaves.pdf",
        "12-Autumn Leaves.pdf",
        "12 -Autumn Leaves.pdf",
        "12 - Autumn Leaves   .pdf",
    ] {
        let parsed = parse_filename(filename);
        assert_eq!(parsed.id(), Some(NoteId::new(12)), "{filename}");
        assert_eq!(parsed.title(), "Autumn Leaves", "{filename}");
    }
}

#[test]
fn extension_matching_is_case_insensitive() {
    let parsed = parse_filename("12 - Autumn Leaves.PDF");
    assert_eq!(parsed.id(), Some(NoteId::new(12)));
    assert_eq!(parsed.title(), "Autumn Leaves");
}

#[test]
fn non_pdf_extension_yields_no_id() {
    let parsed = parse_filename("12 - Autumn Leaves.txt");
    assert_eq!(parsed.id(), None);
    assert_eq!(parsed.title(), "12 - Autumn Leaves");
}

#[test]
fn name_without_extension_is_not_a_note_filename() {
    let parsed = parse_filename("12 - Autumn Leaves");
    assert_eq!(parsed.id(), None);
    assert_eq!(parsed.title(), "12 - Autumn Leaves");
}

#[test]
fn empty_title_after_separator_falls_back_to_whole_stem() {
    let parsed = parse_filename("12 - .pdf");
    assert_eq!(parsed.id(), None);
    assert_eq!(parsed.title(), "12 -");

    let parsed = parse_filename("12.pdf");
    assert_eq!(parsed.id(), None);
    assert_eq!(parsed.title(), "12");
}

#[test]
fn digit_leading_title_without_separator_is_not_an_id() {
    let parsed = parse_filename("12 Etudes.pdf");
    assert_eq!(parsed.id(), None);
    assert_eq!(parsed.title(), "12 Etudes");
}

#[test]
fn any_non_negative_integer_is_a_valid_id() {
    let parsed = parse_filename("1975 - a real title.pdf");
    assert_eq!(parsed.id(), Some(NoteId::new(1975)));
    assert_eq!(parsed.title(), "a real title");
}

#[test]
fn number_inside_a_title_is_not_an_id() {
    let parsed = parse_filename("Opus 27 No. 2.pdf");
    assert_eq!(parsed.id(), None);
    assert_eq!(parsed.title(), "Opus 27 No. 2");
}

#[test]
fn overflowing_id_is_not_parsed_as_an_id() {
    let parsed = parse_filename("99999999999 - Too Big.pdf");
    assert_eq!(parsed.id(), None);
    assert_eq!(parsed.title(), "99999999999 - Too Big");
}

#[test]
fn parsed_filename_seeds_metadata_with_filename_origin() {
    let metadata = parse_filename("12 - Autumn Leaves.pdf")
        .into_note_metadata()
        .expect("non-empty title");
    assert_eq!(metadata.id(), Some(NoteId::new(12)));
    assert_eq!(metadata.title(), "Autumn Leaves");
    assert_eq!(
        metadata.origin_of(MetadataField::Id),
        MetadataOrigin::FilenameDerived
    );
    assert_eq!(
        metadata.origin_of(MetadataField::Title),
        MetadataOrigin::FilenameDerived
    );
}

#[test]
fn parsed_filename_seeds_metadata_without_id() {
    let metadata = parse_filename("Autumn Leaves.pdf")
        .into_note_metadata()
        .expect("non-empty title");
    assert_eq!(metadata.id(), None);
    assert_eq!(metadata.title(), "Autumn Leaves");
    assert_eq!(
        metadata.origin_of(MetadataField::Title),
        MetadataOrigin::FilenameDerived
    );
}

#[test]
fn two_sorts_before_ten() {
    let mut notes = [note("10 - Ten.pdf"), note("2 - Two.pdf")];
    notes.sort_by(compare_library_order);
    assert_eq!(notes[0].metadata().id(), Some(NoteId::new(2)));
    assert_eq!(notes[1].metadata().id(), Some(NoteId::new(10)));
}

#[test]
fn unnumbered_records_follow_numbered_ones() {
    let mut notes = [
        note("Alma.pdf"),
        note("12 - Autumn Leaves.pdf"),
        note("Zebra.pdf"),
        note("507 - Custom.pdf"),
    ];
    notes.sort_by(compare_library_order);
    let titles: Vec<&str> = notes.iter().map(|n| n.metadata().title()).collect();
    assert_eq!(
        titles,
        ["Autumn Leaves", "Custom", "Alma", "Zebra"],
        "numbered records ascending, then unnumbered alphabetically"
    );
}

#[test]
fn unnumbered_records_sort_case_insensitively_by_title() {
    let mut notes = [
        note("banana.pdf"),
        note("Apple.pdf"),
        note("cherry.pdf"),
        note("apricot.pdf"),
    ];
    notes.sort_by(compare_library_order);
    let titles: Vec<&str> = notes.iter().map(|n| n.metadata().title()).collect();
    assert_eq!(titles, ["Apple", "apricot", "banana", "cherry"]);
}

#[test]
fn ordering_is_total_for_equal_titles() {
    let left = note("Same Title.pdf");
    let right = note("sub/Same Title.pdf");
    assert_ne!(
        compare_library_order(&left, &right),
        Ordering::Equal,
        "records with equal normalized titles are still totally ordered"
    );
}

#[test]
fn duplicate_ids_are_reported() {
    let notes = [
        note("12 - Autumn Leaves.pdf"),
        note("13 - Another.pdf"),
        note("12 - Autumn Leaves Copy.pdf"),
    ];
    let error = validate_unique_ids(notes.iter()).expect_err("duplicate ID 12");
    assert_eq!(error.id(), NoteId::new(12));
    assert_eq!(
        error.existing_path(),
        &RelativePdfPath::new("12 - Autumn Leaves.pdf").expect("valid path")
    );
    assert_eq!(
        error.conflicting_path(),
        &RelativePdfPath::new("12 - Autumn Leaves Copy.pdf").expect("valid path")
    );
}

#[test]
fn records_without_an_id_never_conflict() {
    let notes = [note("Alma.pdf"), note("Alma Copy.pdf")];
    assert_eq!(validate_unique_ids(notes.iter()), Ok(()));
}

#[test]
fn unique_ids_validate() {
    let notes = [
        note("12 - Autumn Leaves.pdf"),
        note("Alma.pdf"),
        note("507 - Custom.pdf"),
    ];
    assert_eq!(validate_unique_ids(notes.iter()), Ok(()));
}

#[test]
fn empty_library_validates() {
    let notes: [IndexedNote; 0] = [];
    assert_eq!(validate_unique_ids(notes.iter()), Ok(()));
}