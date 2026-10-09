#![allow(missing_docs)]

use std::collections::BTreeSet;

use notae_domain::{
    DomainError, IndexStatus, MetadataOrigin, NoteId, NoteMetadata, PdfFingerprint,
    RelativePdfPath, Sha256Digest,
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

    assert_eq!(meta.title, "Autumn   Leaves");
    assert_eq!(meta.composer_or_artist.as_deref(), Some("Duke Ellington "));
    assert_eq!(
        meta.tags,
        BTreeSet::from([
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
        r#"{"id":null,"title":"Autumn Leaves","composer_or_artist":"Duke Ellington","tags":["jazz"],"origin":"unset"}"#
    );
    let back: NoteMetadata = serde_json::from_str(&json).unwrap();
    assert_eq!(back.id, meta.id);
    assert_eq!(back.title, meta.title);
    assert_eq!(back.tags, meta.tags);
}

#[test]
fn note_metadata_with_id_round_trips() {
    let mut meta = NoteMetadata::new("Study No. 2", None::<String>, Vec::new()).unwrap();
    meta.id = Some(NoteId::new(7));
    let json = serde_json::to_string(&meta).unwrap();
    let back: NoteMetadata = serde_json::from_str(&json).unwrap();
    assert_eq!(back.id, Some(NoteId::new(7)));
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
        assert_eq!(serde_json::from_str::<MetadataOrigin>(&json).unwrap(), origin);
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
    assert_eq!(
        serde_json::to_string(&IndexStatus::DuplicateIdConflict).unwrap(),
        "\"duplicate_id_conflict\""
    );
    assert_ne!(IndexStatus::Ready, IndexStatus::Stale);
}