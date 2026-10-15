#![allow(missing_docs)]

use std::collections::BTreeSet;
use std::time::{Duration, UNIX_EPOCH};

use notae_domain::{
    DomainError, EpochTimestamp, FieldProvenance, IndexStatus, MetadataField, MetadataOrigin,
    NoteId, NoteMetadata, PdfFingerprint, RelativePdfPath, Sha256Digest,
};

fn digest(byte: u8) -> Sha256Digest {
    Sha256Digest::parse(&byte.to_string().repeat(64)).unwrap()
}

#[test]
fn note_id_parses_non_negative_integers() {
    assert_eq!(NoteId::parse("0").unwrap(), NoteId::new(0));
    assert_eq!(NoteId::parse("507").unwrap(), NoteId::new(507));
    assert!(NoteId::parse("-1").is_err());
    assert!(NoteId::parse("1.5").is_err());
    assert!(NoteId::parse("").is_err());
    assert!(NoteId::parse(" 12").is_err());
}

#[test]
fn note_id_display_and_from_str_round_trip() {
    let id = NoteId::new(12);
    assert_eq!(id.to_string(), "12");
    assert_eq!("12".parse::<NoteId>().unwrap(), id);
}

#[test]
fn note_id_orders_numerically() {
    assert!(NoteId::new(2) < NoteId::new(10));
    let mut ids = vec![NoteId::new(10), NoteId::new(2), NoteId::new(507)];
    ids.sort();
    assert_eq!(ids, vec![NoteId::new(2), NoteId::new(10), NoteId::new(507)]);
}

#[test]
fn note_id_serde_round_trip_is_stable() {
    let id = NoteId::new(42);
    let json = serde_json::to_string(&id).unwrap();
    assert_eq!(json, "42");
    assert_eq!(serde_json::from_str::<NoteId>(&json).unwrap(), id);
}

#[test]
fn note_metadata_construction_normalizes_tags() {
    let meta = NoteMetadata::new(
        "  Autumn   Leaves  ",
        Some("Duke Ellington ".to_string()),
        vec![
            "  jazz ".to_string(),
            "swing".to_string(),
            "jazz".to_string(),
            String::new(),
            "  ".to_string(),
            "big\tband".to_string(),
        ],
    )
    .unwrap();

    assert_eq!(meta.title(), "Autumn   Leaves");
    assert_eq!(meta.composer_or_artist(), Some("Duke Ellington "));
    assert_eq!(
        meta.tags(),
        &BTreeSet::from([
            "jazz".to_string(),
            "swing".to_string(),
            "big band".to_string(),
        ])
    );
}

#[test]
fn note_metadata_requires_non_empty_title() {
    assert!(matches!(
        NoteMetadata::new("   \t ", None::<String>, Vec::new()),
        Err(DomainError::EmptyTitle)
    ));
}

#[test]
fn note_metadata_serde_round_trip() {
    let meta = NoteMetadata::new(
        "Autumn Leaves",
        Some("Duke Ellington".to_string()),
        vec!["jazz".to_string()],
    )
    .unwrap();
    let json = serde_json::to_string(&meta).unwrap();
    assert_eq!(
        json,
        r#"{"id":null,"title":"Autumn Leaves","composer_or_artist":"Duke Ellington","tags":["jazz"],"provenance":{"id":"unset","title":"unset","composer_or_artist":"unset","tags":"unset"}}"#
    );
    let back: NoteMetadata = serde_json::from_str(&json).unwrap();
    assert_eq!(back, meta);
}

#[test]
fn note_metadata_with_id_round_trips() {
    let mut meta = NoteMetadata::new("Study No. 2", None::<String>, Vec::new()).unwrap();
    meta.set_id(Some(NoteId::new(7)));
    let json = serde_json::to_string(&meta).unwrap();
    let back: NoteMetadata = serde_json::from_str(&json).unwrap();
    assert_eq!(back.id(), Some(NoteId::new(7)));
    assert_eq!(back.title(), "Study No. 2");
}

#[test]
fn pdf_fingerprint_round_trips() {
    let hash = digest(1);
    let fingerprint = PdfFingerprint::new(hash, 4096, None);
    let json = serde_json::to_string(&fingerprint).unwrap();
    assert!(json.contains("4096"));
    let back: PdfFingerprint = serde_json::from_str(&json).unwrap();
    assert_eq!(back, fingerprint);
}

#[test]
fn sha256_digest_hex_display_and_parse() {
    let hex = "a".repeat(64);
    let digest = Sha256Digest::parse(&hex).unwrap();
    assert_eq!(digest.to_string(), hex);
    assert_eq!(Sha256Digest::parse(&hex).unwrap(), digest);
    assert!(Sha256Digest::parse("abc").is_err());
    assert!(Sha256Digest::parse(&"zz".repeat(32)).is_err());
}

#[test]
fn relative_pdf_path_accepts_root_relative_paths() {
    let path = RelativePdfPath::parse("Jazz/12 - Autumn Leaves.pdf").unwrap();
    assert_eq!(path.as_str(), "Jazz/12 - Autumn Leaves.pdf");
    let back: RelativePdfPath =
        serde_json::from_str(&serde_json::to_string(&path).unwrap()).unwrap();
    assert_eq!(back, path);
}

#[test]
fn relative_pdf_path_rejects_absolute_and_traversal_paths() {
    assert!(RelativePdfPath::parse("/etc/passwd.pdf").is_err());
    assert!(RelativePdfPath::parse("C:/notes/a.pdf").is_err());
    assert!(RelativePdfPath::parse("../secrets.pdf").is_err());
    assert!(RelativePdfPath::parse("Jazz/../../secrets.pdf").is_err());
    assert!(RelativePdfPath::parse("Jazz//a.pdf").is_err());
    assert!(RelativePdfPath::parse("Jazz/").is_err());
    assert!(RelativePdfPath::parse("").is_err());
    assert!(RelativePdfPath::parse("Jazz\\a.pdf").is_err());
}

#[test]
fn metadata_origin_equality_and_serde() {
    assert_eq!(MetadataOrigin::Manual, MetadataOrigin::Manual);
    assert_ne!(MetadataOrigin::FilenameDerived, MetadataOrigin::PcImported);
    assert_ne!(MetadataOrigin::Unset, MetadataOrigin::Manual);
    for origin in [
        MetadataOrigin::FilenameDerived,
        MetadataOrigin::PcImported,
        MetadataOrigin::Manual,
        MetadataOrigin::Unset,
    ] {
        let json = serde_json::to_string(&origin).unwrap();
        assert_eq!(
            serde_json::from_str::<MetadataOrigin>(&json).unwrap(),
            origin
        );
    }
    assert_eq!(
        serde_json::to_string(&MetadataOrigin::PcImported).unwrap(),
        "\"pc_imported\""
    );
}

#[test]
fn index_status_serializes_with_stable_names() {
    for status in [
        IndexStatus::Discovered,
        IndexStatus::Indexing,
        IndexStatus::Ready,
        IndexStatus::OcrPending,
        IndexStatus::OcrIncomplete,
        IndexStatus::Stale,
        IndexStatus::DuplicateIdConflict,
        IndexStatus::Inaccessible,
        IndexStatus::InvalidPdf,
        IndexStatus::ImportMismatch,
    ] {
        let json = serde_json::to_string(&status).unwrap();
        assert_eq!(serde_json::from_str::<IndexStatus>(&json).unwrap(), status);
    }
    assert_ne!(IndexStatus::Ready, IndexStatus::Stale);
}

/// Every `IndexStatus` variant is pinned to an exact wire name, so a rename
/// fails even though serialization and deserialization stay mutually
/// consistent.
#[test]
fn index_status_wire_names_are_pinned() {
    let expected = [
        (IndexStatus::Discovered, "\"discovered\""),
        (IndexStatus::Indexing, "\"indexing\""),
        (IndexStatus::Ready, "\"ready\""),
        (IndexStatus::OcrPending, "\"ocr_pending\""),
        (IndexStatus::OcrIncomplete, "\"ocr_incomplete\""),
        (IndexStatus::Stale, "\"stale\""),
        (
            IndexStatus::DuplicateIdConflict,
            "\"duplicate_id_conflict\"",
        ),
        (IndexStatus::Inaccessible, "\"inaccessible\""),
        (IndexStatus::InvalidPdf, "\"invalid_pdf\""),
        (IndexStatus::ImportMismatch, "\"import_mismatch\""),
    ];

    for (status, json) in expected {
        assert_eq!(serde_json::to_string(&status).unwrap(), json);
    }
}

#[test]
fn metadata_origin_wire_names_are_pinned() {
    assert_eq!(
        serde_json::to_string(&MetadataOrigin::FilenameDerived).unwrap(),
        "\"filename_derived\""
    );
    assert_eq!(
        serde_json::to_string(&MetadataOrigin::PcImported).unwrap(),
        "\"pc_imported\""
    );
    assert_eq!(
        serde_json::to_string(&MetadataOrigin::Manual).unwrap(),
        "\"manual\""
    );
    assert_eq!(
        serde_json::to_string(&MetadataOrigin::Unset).unwrap(),
        "\"unset\""
    );
}

// --- Defect 1: deserialization must not bypass path validation -------------

#[test]
fn relative_pdf_path_deserialization_rejects_invalid_paths() {
    for raw in [
        "../secrets.pdf",
        "C:/notes/a.pdf",
        "./notes.pdf",
        "Jazz/./notes.pdf",
        "/etc/passwd.pdf",
        "Jazz/../../secrets.pdf",
        "Jazz//a.pdf",
        "Jazz/",
        "",
        "Jazz\\a.pdf",
    ] {
        let json = serde_json::to_string(raw).unwrap();
        assert!(
            serde_json::from_str::<RelativePdfPath>(&json).is_err(),
            "deserializing {raw:?} must be rejected"
        );
        assert!(RelativePdfPath::parse(raw).is_err());
    }
}

#[test]
fn relative_pdf_path_deserialization_accepts_valid_paths() {
    for raw in [
        "notes.pdf",
        "Jazz/12 - Autumn Leaves.pdf",
        "Scores: Theory/12 - Etude.pdf",
    ] {
        let json = serde_json::to_string(raw).unwrap();
        let path: RelativePdfPath = serde_json::from_str(&json).unwrap();
        assert_eq!(path.as_str(), raw);
    }
}

// --- Defect 5: colons are only rejected as Windows drive prefixes ----------

#[test]
fn relative_pdf_path_allows_colons_outside_a_drive_prefix() {
    let accepted = [
        "Scores: Theory/12 - Etude.pdf",
        "notes: part 1.pdf",
        "1:2.pdf",
        "Jazz/12 - Etude.pdf",
    ];
    for raw in accepted {
        assert_eq!(
            RelativePdfPath::parse(raw).map(|p| p.as_str().to_string()),
            Ok(raw.to_string()),
            "{raw:?} must be accepted"
        );
    }

    let rejected = ["C:/notes/a.pdf", "z:a.pdf", "C:a.pdf"];
    for raw in rejected {
        assert!(
            RelativePdfPath::parse(raw).is_err(),
            "{raw:?} must be rejected as a drive prefix"
        );
    }
}

// --- Defect 2: deserialization must not bypass metadata invariants --------

#[test]
fn note_metadata_deserialization_rejects_blank_title() {
    for raw in [
        r#"{"title":"","composer_or_artist":null,"tags":[]}"#,
        r#"{"title":"   ","composer_or_artist":null,"tags":[]}"#,
        r#"{"title":"\t\n ","composer_or_artist":null,"tags":[]}"#,
    ] {
        assert!(
            serde_json::from_str::<NoteMetadata>(raw).is_err(),
            "{raw} must be rejected"
        );
    }
}

#[test]
fn note_metadata_deserialization_normalizes_tags() {
    let raw = r#"{"id":null,"title":"Study","composer_or_artist":null,"tags":["  jazz ","big\tband","jazz","","   "],"provenance":{"id":"unset","title":"unset","composer_or_artist":"unset","tags":"unset"}}"#;
    let meta: NoteMetadata = serde_json::from_str(raw).unwrap();
    assert_eq!(
        meta.tags(),
        &BTreeSet::from(["jazz".to_string(), "big band".to_string()])
    );
}

#[test]
fn note_metadata_mutation_api_preserves_invariants() {
    let mut meta = NoteMetadata::new("Study No. 2", None::<String>, Vec::new()).unwrap();
    assert!(matches!(
        meta.set_title("   "),
        Err(DomainError::EmptyTitle)
    ));
    meta.set_title("  Study No. 3  ").unwrap();
    assert_eq!(meta.title(), "Study No. 3");

    meta.add_tag("  swing ");
    assert_eq!(meta.tags(), &BTreeSet::from(["swing".to_string()]));

    meta.set_composer_or_artist(Some("Bill Evans ".to_string()));
    assert_eq!(meta.composer_or_artist(), Some("Bill Evans "));
}

// --- Defect 3: provenance is tracked per field ----------------------------

#[test]
fn note_metadata_tracks_provenance_per_field() {
    let mut meta = NoteMetadata::new(
        "Autumn Leaves",
        Some("Duke Ellington".to_string()),
        vec!["jazz".into()],
    )
    .unwrap();
    // Filename-derived IDs must not look like manual edits, so the import
    // facing path is the one that takes an explicit origin.
    meta.assign_id(Some(NoteId::new(12)), MetadataOrigin::FilenameDerived);

    assert_eq!(
        meta.origin_of(MetadataField::Id),
        MetadataOrigin::FilenameDerived
    );
    for field in MetadataField::ALL
        .into_iter()
        .filter(|f| *f != MetadataField::Id)
    {
        assert_eq!(meta.origin_of(field), MetadataOrigin::Unset);
    }

    // Editing only the title must not mark the other fields as manual.
    meta.set_title("Autumn Leaves (remastered)").unwrap();

    assert_eq!(meta.origin_of(MetadataField::Title), MetadataOrigin::Manual);
    assert_eq!(meta.origin_of(MetadataField::Tags), MetadataOrigin::Unset);
    assert_eq!(
        meta.origin_of(MetadataField::ComposerOrArtist),
        MetadataOrigin::Unset
    );
    assert_eq!(
        meta.origin_of(MetadataField::Id),
        MetadataOrigin::FilenameDerived
    );

    // A tag edit marks only the tags field.
    meta.add_tag("swing");
    assert_eq!(meta.origin_of(MetadataField::Tags), MetadataOrigin::Manual);
    assert_eq!(meta.origin_of(MetadataField::Title), MetadataOrigin::Manual);

    // PC import is tracked per field as well.
    let mut imported = NoteMetadata::new("PC Title", None::<String>, Vec::new()).unwrap();
    imported.set_origin(MetadataField::Title, MetadataOrigin::PcImported);
    imported.set_origin(MetadataField::ComposerOrArtist, MetadataOrigin::PcImported);
    assert_eq!(
        imported.origin_of(MetadataField::Title),
        MetadataOrigin::PcImported
    );
    assert_eq!(imported.origin_of(MetadataField::Id), MetadataOrigin::Unset);
}

#[test]
fn note_metadata_provenance_survives_serde_round_trip() {
    let mut meta = NoteMetadata::new("Autumn Leaves", None::<String>, vec!["jazz".into()]).unwrap();
    meta.set_origin(MetadataField::Title, MetadataOrigin::PcImported);
    meta.mark_manual(MetadataField::Tags);
    meta.set_origin(MetadataField::Id, MetadataOrigin::FilenameDerived);

    let json = serde_json::to_string(&meta).unwrap();
    assert_eq!(
        json,
        r#"{"id":null,"title":"Autumn Leaves","composer_or_artist":null,"tags":["jazz"],"provenance":{"id":"filename_derived","title":"pc_imported","composer_or_artist":"unset","tags":"manual"}}"#
    );

    let back: NoteMetadata = serde_json::from_str(&json).unwrap();
    for field in MetadataField::ALL {
        assert_eq!(back.origin_of(field), meta.origin_of(field));
    }
    assert_eq!(back, meta);
}

#[test]
fn field_provenance_defaults_to_unset_and_is_queryable() {
    let provenance = FieldProvenance::default();
    for field in MetadataField::ALL {
        assert_eq!(provenance.origin_of(field), MetadataOrigin::Unset);
    }

    let mut provenance = FieldProvenance::default();
    provenance.mark_manual(MetadataField::Title);
    provenance.set_origin(MetadataField::Id, MetadataOrigin::PcImported);
    assert_eq!(
        provenance.origin_of(MetadataField::Title),
        MetadataOrigin::Manual
    );
    assert_eq!(
        provenance.origin_of(MetadataField::Id),
        MetadataOrigin::PcImported
    );
    assert_eq!(
        provenance.origin_of(MetadataField::Tags),
        MetadataOrigin::Unset
    );
    assert!(provenance.is_manual(MetadataField::Title));
    assert!(!provenance.is_manual(MetadataField::Tags));

    let json = serde_json::to_string(&provenance).unwrap();
    assert_eq!(
        json,
        r#"{"id":"pc_imported","title":"manual","composer_or_artist":"unset","tags":"unset"}"#
    );
    assert_eq!(
        serde_json::from_str::<FieldProvenance>(&json).unwrap(),
        provenance
    );
}

#[test]
fn metadata_field_wire_names_are_pinned() {
    assert_eq!(serde_json::to_string(&MetadataField::Id).unwrap(), "\"id\"");
    assert_eq!(
        serde_json::to_string(&MetadataField::Title).unwrap(),
        "\"title\""
    );
    assert_eq!(
        serde_json::to_string(&MetadataField::ComposerOrArtist).unwrap(),
        "\"composer_or_artist\""
    );
    assert_eq!(
        serde_json::to_string(&MetadataField::Tags).unwrap(),
        "\"tags\""
    );
    assert_eq!(MetadataField::ALL.len(), 4);
}

// --- Defect 4: modification times must be portable ------------------------

#[test]
fn pdf_fingerprint_round_trips_pre_epoch_timestamp() {
    let modified = UNIX_EPOCH - Duration::from_secs(86_400);
    let stamp = EpochTimestamp::from_system_time(modified).unwrap();
    assert_eq!(stamp.seconds(), -86_400);

    let fingerprint = PdfFingerprint::new(digest(2), 2048, Some(stamp));
    let json = serde_json::to_string(&fingerprint).unwrap();
    assert!(json.contains("\"secs\":-86400"), "{json}");

    let back: PdfFingerprint = serde_json::from_str(&json).unwrap();
    assert_eq!(back, fingerprint);
    assert_eq!(back.modified().unwrap().to_system_time(), Some(modified));
}

#[test]
fn epoch_timestamp_round_trips_across_the_epoch() {
    let cases = [
        UNIX_EPOCH - Duration::from_nanos(1),
        UNIX_EPOCH - Duration::from_secs(1),
        UNIX_EPOCH,
        UNIX_EPOCH + Duration::from_nanos(1),
        UNIX_EPOCH + Duration::from_secs(1_700_000_000),
    ];
    for case in cases {
        let stamp = EpochTimestamp::from_system_time(case).unwrap();
        let json = serde_json::to_string(&stamp).unwrap();
        let back: EpochTimestamp = serde_json::from_str(&json).unwrap();
        assert_eq!(back, stamp);
        assert_eq!(back.to_system_time(), Some(case));
    }
}

#[test]
fn epoch_timestamp_rejects_out_of_range_nanos() {
    assert!(EpochTimestamp::new(-1, 1_000_000_000).is_none());
    assert!(EpochTimestamp::from_system_time(UNIX_EPOCH - Duration::from_nanos(1)).is_some());
}

// --- Follow-up 1: a user ID edit is manual, an import assignment is not ---

#[test]
fn user_id_edit_marks_manual_and_import_assignment_does_not() {
    let mut meta = NoteMetadata::new("Autumn Leaves", None::<String>, Vec::new()).unwrap();

    // The user-facing setter always records a manual edit, including a clear.
    meta.set_id(Some(NoteId::new(12)));
    assert_eq!(meta.id(), Some(NoteId::new(12)));
    assert_eq!(meta.origin_of(MetadataField::Id), MetadataOrigin::Manual);

    meta.set_id(None);
    assert_eq!(meta.id(), None);
    assert_eq!(
        meta.origin_of(MetadataField::Id),
        MetadataOrigin::Manual,
        "clearing the ID is a manual edit and must stay authoritative"
    );

    // The filename/import-facing setter records the origin it is given.
    meta.assign_id(Some(NoteId::new(12)), MetadataOrigin::FilenameDerived);
    assert_eq!(
        meta.origin_of(MetadataField::Id),
        MetadataOrigin::FilenameDerived
    );
    meta.assign_id(None, MetadataOrigin::PcImported);
    assert_eq!(
        meta.origin_of(MetadataField::Id),
        MetadataOrigin::PcImported
    );
    meta.assign_id(Some(NoteId::new(3)), MetadataOrigin::Manual);
    assert_eq!(meta.origin_of(MetadataField::Id), MetadataOrigin::Manual);
}

// --- Follow-up 3: clearing an optional field is still a manual edit -------

#[test]
fn clearing_optional_fields_keeps_the_manual_marker() {
    let mut meta =
        NoteMetadata::new("Autumn Leaves", Some("Duke Ellington".to_string()), vec![]).unwrap();

    meta.set_composer_or_artist(Some("Duke Ellington".to_string()));
    assert_eq!(
        meta.origin_of(MetadataField::ComposerOrArtist),
        MetadataOrigin::Manual
    );
    meta.set_composer_or_artist(None);
    assert_eq!(meta.composer_or_artist(), None);
    assert_eq!(
        meta.origin_of(MetadataField::ComposerOrArtist),
        MetadataOrigin::Manual,
        "a composer removed by the user must not be refilled by an import"
    );

    // Non-manual clearing exists for import and initialization paths only.
    meta.assign_composer_or_artist(None, MetadataOrigin::PcImported);
    assert_eq!(
        meta.origin_of(MetadataField::ComposerOrArtist),
        MetadataOrigin::PcImported
    );

    // Emptying the tag set is a manual edit too.
    let mut tagged = NoteMetadata::new("Etude", None::<String>, vec!["jazz".into()]).unwrap();
    tagged.set_tags(Vec::new());
    assert!(tagged.tags().is_empty());
    assert_eq!(meta_origin_of_tags(&tagged), MetadataOrigin::Manual);

    // The import path applies the given origin, even when the value is empty.
    let mut imported = NoteMetadata::new("Etude", None::<String>, vec!["jazz".into()]).unwrap();
    imported.assign_tags(Vec::new(), MetadataOrigin::PcImported);
    assert!(imported.tags().is_empty());
    assert_eq!(
        imported.origin_of(MetadataField::Tags),
        MetadataOrigin::PcImported
    );
}

fn meta_origin_of_tags(meta: &NoteMetadata) -> MetadataOrigin {
    meta.origin_of(MetadataField::Tags)
}

// --- Follow-up 2: provenance is required, or migrated from `origin` -------

#[test]
fn deserialization_requires_provenance_or_a_legacy_origin() {
    // No provenance object and no legacy scalar: the record cannot be shown to
    // hold no manual edits, so it must not be accepted.
    for raw in [
        r#"{"title":"Study"}"#,
        r#"{"id":7,"title":"Study","composer_or_artist":null,"tags":[]}"#,
    ] {
        assert!(
            serde_json::from_str::<NoteMetadata>(raw).is_err(),
            "{raw} must be rejected: absent provenance looks like no manual edits"
        );
    }
}

#[test]
fn legacy_scalar_origin_is_migrated_to_every_field() {
    // Legacy `origin: "manual"` means the whole record was hand-edited, so
    // every field must stay authoritative over an import.
    let raw = r#"{"id":7,"title":"Study","composer_or_artist":null,"tags":[],"origin":"manual"}"#;
    let meta: NoteMetadata = serde_json::from_str(raw).unwrap();
    for field in MetadataField::ALL {
        assert_eq!(
            meta.origin_of(field),
            MetadataOrigin::Manual,
            "legacy origin must migrate {field:?}"
        );
    }
    assert_eq!(meta.id(), Some(NoteId::new(7)));

    // The other legacy scalars migrate uniformly as well.
    for (wire, expected) in [
        ("pc_imported", MetadataOrigin::PcImported),
        ("filename_derived", MetadataOrigin::FilenameDerived),
        ("unset", MetadataOrigin::Unset),
    ] {
        let raw = format!(r#"{{"title":"Study","origin":"{wire}"}}"#);
        let meta: NoteMetadata = serde_json::from_str(&raw).unwrap();
        for field in MetadataField::ALL {
            assert_eq!(
                meta.origin_of(field),
                expected,
                "legacy {wire} must migrate {field:?}"
            );
        }
    }

    // The explicit per-field object wins over the legacy scalar, so a modern
    // record mixed with a stale scalar keeps its real provenance.
    let raw = r#"{"title":"Study","origin":"manual","provenance":{"id":"manual","title":"pc_imported","composer_or_artist":"unset","tags":"unset"}}"#;
    let meta: NoteMetadata = serde_json::from_str(raw).unwrap();
    assert_eq!(meta.origin_of(MetadataField::Id), MetadataOrigin::Manual);
    assert_eq!(
        meta.origin_of(MetadataField::Title),
        MetadataOrigin::PcImported
    );
    assert_eq!(
        meta.origin_of(MetadataField::ComposerOrArtist),
        MetadataOrigin::Unset
    );
}

// --- Follow-up 4: extreme pre-epoch instants return `None` ----------------

#[test]
fn epoch_timestamp_returns_none_for_unrepresentable_pre_epoch_instants() {
    // `UNIX_EPOCH - (i64::MAX secs, 1 ns)` is the last instant the platform
    // can express before the epoch, and it is one second further back than
    // `EpochTimestamp` can represent in `i64` seconds.
    let extreme = UNIX_EPOCH
        .checked_sub(Duration::new(i64::MAX as u64, 1))
        .unwrap();
    assert!(
        EpochTimestamp::from_system_time(extreme).is_none(),
        "an unrepresentable instant must return None, not panic"
    );
    assert!(
        PdfFingerprint::from_system_time(digest(3), 4096, Some(extreme)).is_none(),
        "a fingerprint must not panic on any filesystem timestamp"
    );

    // The representable boundary one nanosecond later still converts.
    let representable = extreme + Duration::from_nanos(1);
    let stamp = EpochTimestamp::from_system_time(representable).unwrap();
    assert_eq!(stamp.seconds(), -i64::MAX);
    assert_eq!(stamp.subsec_nanos(), 0);
    assert_eq!(stamp.to_system_time(), Some(representable));
    assert!(PdfFingerprint::from_system_time(digest(4), 4096, Some(representable)).is_some());
}
