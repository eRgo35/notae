//! Filename convention, default browsing order, and the unique-ID invariant.

use std::cmp::Ordering;

use notae_domain::{
    FilenameError, IndexedNote, IndexStatus, MetadataField, MetadataOrigin, NoteId,
    PdfFingerprint, RelativePdfPath, Sha256Digest, compare_library_order, is_note_filename,
    parse_filename, parse_note_filename, validate_unique_ids,
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

/// A hyphen directly after the digits is the documented separator, so
/// `3-Beat Groove.pdf` is note 3 titled `Beat Groove`. Pinned so the outcome is
/// deliberate rather than a side effect of separator handling.
#[test]
fn hyphen_separator_without_spaces_is_an_id() {
    let parsed = parse_filename("3-Beat Groove.pdf");
    assert_eq!(parsed.id(), Some(NoteId::new(3)));
    assert_eq!(parsed.title(), "Beat Groove");
}

#[test]
fn note_filename_requires_a_pdf_extension() {
    assert!(!is_note_filename("12 - Autumn Leaves.txt"));
    assert!(!is_note_filename("12 - Autumn Leaves"));
    assert!(!is_note_filename("noextension"));
    assert!(is_note_filename("12 - Autumn Leaves.pdf"));
    assert!(is_note_filename("sub/dir/12 - Autumn Leaves.PDF"));
}

#[test]
fn parse_note_filename_rejects_non_pdf_names() {
    assert_eq!(
        parse_note_filename("12 - Autumn Leaves.txt"),
        Err(FilenameError::NotPdf)
    );
    assert_eq!(
        parse_note_filename("12 - Autumn Leaves"),
        Err(FilenameError::NotPdf)
    );
    assert_eq!(parse_note_filename("noextension"), Err(FilenameError::NotPdf));
}

#[test]
fn parse_note_filename_accepts_pdf_names() {
    let parsed = parse_note_filename("12 - Autumn Leaves.pdf").expect("pdf");
    assert_eq!(parsed.id(), Some(NoteId::new(12)));
    assert_eq!(parsed.title(), "Autumn Leaves");
}

#[test]
fn invalid_pdf_name_cannot_become_note_metadata() {
    let parsed = parse_note_filename("12 - Autumn Leaves.txt").expect_err("not a pdf");
    assert_eq!(parsed, FilenameError::NotPdf);
}

/// Equal IDs are only a conflict until they are resolved, so the browsing
/// order still has to place them deterministically.
#[test]
fn duplicate_id_records_are_still_totally_ordered() {
    let left = note("12 - Autumn Leaves.pdf");
    let right = note("12 - Winter Wind.pdf");
    let ordering = compare_library_order(&left, &right);
    assert_ne!(
        ordering,
        Ordering::Equal,
        "two records sharing ID 12 must not compare equal"
    );
    assert_eq!(
        ordering,
        compare_library_order(&right, &left).reverse(),
        "the comparison is antisymmetric"
    );
    assert_eq!(ordering, Ordering::Less, "tie broken by title");
}

#[test]
fn duplicate_id_and_title_records_sort_by_path() {
    let left = note("a/12 - Autumn Leaves.pdf");
    let right = note("b/12 - Autumn Leaves.pdf");
    assert_eq!(compare_library_order(&left, &right), Ordering::Less);
    assert_eq!(compare_library_order(&right, &left), Ordering::Greater);
    assert_eq!(
        compare_library_order(&left, &left),
        Ordering::Equal,
        "a record equals itself"
    );
}

#[test]
fn duplicate_id_records_sort_deterministically() {
    let mut notes = [
        note("c/12 - Same.pdf"),
        note("a/12 - Same.pdf"),
        note("b/12 - Other.pdf"),
    ];
    notes.sort_by(compare_library_order);
    let paths: Vec<&str> = notes.iter().map(|n| n.path().as_str()).collect();
    assert_eq!(paths, ["b/12 - Other.pdf", "a/12 - Same.pdf", "c/12 - Same.pdf"]);
}

fn note_with_fingerprint(filename: &str, seed: u8) -> IndexedNote {
    let parsed = parse_filename(filename);
    IndexedNote::new(
        RelativePdfPath::new(filename).expect("valid relative path"),
        PdfFingerprint::new(Sha256Digest::from_bytes([seed; 32]), 1024, None),
        parsed.into_note_metadata().expect("non-empty title"),
    )
}

/// Two records that agree on ID, title, and path but were fingerprinted
/// differently are still distinct records, so the browsing order must place
/// them deterministically instead of calling them equal.
#[test]
fn same_id_title_and_path_with_different_fingerprints_is_not_equal() {
    let left = note_with_fingerprint("12 - Autumn Leaves.pdf", 7);
    let right = note_with_fingerprint("12 - Autumn Leaves.pdf", 9);
    assert_ne!(left, right, "the records really do differ");
    let ordering = compare_library_order(&left, &right);
    assert_ne!(
        ordering,
        Ordering::Equal,
        "differing fingerprints must not compare equal"
    );
    assert_eq!(
        ordering,
        compare_library_order(&right, &left).reverse(),
        "the comparison stays antisymmetric"
    );
    assert_eq!(
        compare_library_order(&left, &left),
        Ordering::Equal,
        "a record equals itself"
    );
}

/// A status transition alone must not make two otherwise identical records
/// compare equal.
#[test]
fn same_id_title_and_path_with_different_status_is_not_equal() {
    let left = status_note("12 - Autumn Leaves.pdf", IndexStatus::Discovered);
    let right = status_note("12 - Autumn Leaves.pdf", IndexStatus::OcrPending);
    assert_ne!(left, right, "the records really do differ");
    let ordering = compare_library_order(&left, &right);
    assert_ne!(
        ordering,
        Ordering::Equal,
        "a status transition must not compare equal"
    );
    assert_eq!(
        ordering,
        compare_library_order(&right, &left).reverse(),
        "the comparison stays antisymmetric"
    );
}

/// Sorting a library that contains same-path records differing only in
/// fingerprint or status must keep every record and land on one order no
/// matter how the input was arranged.
#[test]
fn a_library_with_colliding_records_sorts_deterministically() {
    let mut library = [
        note_with_fingerprint("12 - Same.pdf", 9),
        status_note("12 - Same.pdf", IndexStatus::Stale),
        note_with_fingerprint("12 - Same.pdf", 7),
        status_note("12 - Same.pdf", IndexStatus::Ready),
        note_with_fingerprint("12 - Same.pdf", 3),
        note("2 - Other.pdf"),
    ];

    let mut expected: Vec<SortKey> = library.iter().map(sort_key).collect();
    library.sort_by(compare_library_order);
    expected.sort();

    let actual: Vec<SortKey> = library.iter().map(sort_key).collect();
    assert_eq!(actual, expected, "sorted order must be canonical");

    // Every record survives the sort.
    let mut count = library
        .iter()
        .map(|n| (n.path().clone(), n.fingerprint().sha256(), n.status()))
        .collect::<Vec<_>>();
    count.sort();
    count.dedup();
    assert_eq!(count.len(), library.len(), "no record is dropped or duplicated");

    // A different input permutation reaches the same order.
    let mut shuffled = [
        status_note("12 - Same.pdf", IndexStatus::Ready),
        note("2 - Other.pdf"),
        note_with_fingerprint("12 - Same.pdf", 3),
        status_note("12 - Same.pdf", IndexStatus::Stale),
        note_with_fingerprint("12 - Same.pdf", 7),
        note_with_fingerprint("12 - Same.pdf", 9),
    ];
    shuffled.sort_by(compare_library_order);
    let shuffled: Vec<SortKey> = shuffled.iter().map(sort_key).collect();
    assert_eq!(actual, shuffled, "input order must not affect the result");
}

/// The documented tie-break chain, spelled out independently of the
/// implementation: ID, case-folded title, exact title, path, fingerprint,
/// then status.
type SortKey = (
    u8,
    Option<NoteId>,
    String,
    String,
    String,
    Sha256Digest,
    u64,
    IndexStatus,
);

fn sort_key(note: &IndexedNote) -> SortKey {
    assert!(
        note.fingerprint().modified().is_none(),
        "the tie-break key models fingerprints without a modification time"
    );
    let id = note.id();
    (
        u8::from(id.is_none()),
        id,
        note.title().to_lowercase(),
        note.title().to_owned(),
        note.path().as_str().to_owned(),
        note.fingerprint().sha256(),
        note.fingerprint().byte_len(),
        note.status(),
    )
}

fn status_note(filename: &str, status: IndexStatus) -> IndexedNote {
    let mut note = note(filename);
    note.set_status(status);
    note
}

#[test]
fn indexed_note_carries_its_index_status() {
    for status in [
        IndexStatus::Discovered,
        IndexStatus::Ready,
        IndexStatus::OcrPending,
        IndexStatus::OcrIncomplete,
        IndexStatus::Stale,
        IndexStatus::DuplicateIdConflict,
        IndexStatus::Inaccessible,
        IndexStatus::InvalidPdf,
        IndexStatus::ImportMismatch,
    ] {
        let note = status_note("12 - Autumn Leaves.pdf", status);
        assert_eq!(note.status(), status);
        assert_eq!(note.with_status(status).status(), status);
    }
}

#[test]
fn a_new_indexed_note_is_discovered() {
    assert_eq!(note("12 - Autumn Leaves.pdf").status(), IndexStatus::Discovered);
    assert_eq!(
        IndexedNote::new(
            RelativePdfPath::new("12 - Autumn Leaves.pdf").expect("valid"),
            fingerprint(),
            parse_note_filename("12 - Autumn Leaves.pdf")
                .expect("pdf")
                .into_note_metadata()
                .expect("non-empty title"),
        )
        .status(),
        IndexStatus::Discovered
    );
}

#[test]
fn setting_status_leaves_metadata_untouched() {
    let original = note("12 - Autumn Leaves.pdf");
    let updated = original.with_status(IndexStatus::OcrPending);
    assert_eq!(updated.status(), IndexStatus::OcrPending);
    assert_eq!(updated.metadata(), original.metadata());
    assert_eq!(updated.path(), original.path());
    assert_eq!(updated.fingerprint(), original.fingerprint());
    assert_eq!(updated.id(), Some(NoteId::new(12)));
    assert_eq!(
        updated.metadata().origin_of(MetadataField::Id),
        MetadataOrigin::FilenameDerived,
        "status changes must not disturb provenance"
    );
    assert_eq!(original.status(), IndexStatus::Discovered);
}

#[test]
fn index_status_round_trips_through_json() {
    let note = status_note("12 - Autumn Leaves.pdf", IndexStatus::DuplicateIdConflict);
    let json = serde_json::to_value(&note).expect("serializes");
    assert_eq!(json["status"], "duplicate_id_conflict");
    assert_eq!(
        serde_json::from_value::<IndexedNote>(json).expect("deserializes"),
        note
    );
}

#[test]
fn a_record_without_status_loads_as_discovered() {
    let note = note("12 - Autumn Leaves.pdf");
    let mut json = serde_json::to_value(&note).expect("serializes");
    json.as_object_mut()
        .expect("object")
        .remove("status");
    assert_eq!(
        serde_json::from_value::<IndexedNote>(json)
            .expect("deserializes")
            .status(),
        IndexStatus::Discovered
    );
}