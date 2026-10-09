//! Integration tests for the portable sidecar manifest and the import preview.
//!
//! Every test here builds a local index, writes a sidecar for it, and drives
//! [`SidecarImporter`]: `preview` is checked first because it is the read-only
//! view the Android import screen renders, and `apply` afterwards because
//! nothing is written until the user confirms.

use std::collections::HashSet;
use std::path::{Path, PathBuf};

use notae_domain::{
    EpochTimestamp, IndexStatus, IndexedNote, MetadataField, MetadataOrigin, NoteId, NoteMetadata,
    PdfFingerprint, RelativePdfPath, Sha256Digest,
};
use notae_index::{
    CatalogError, EntryChange, FileCondition, IndexError, IndexStore, OcrAssessment, OcrText,
    PdfCatalog, PdfFile, SIDECAR_SCHEMA_VERSION, SidecarEntry, SidecarError, SidecarImporter,
    SidecarManifest, SidecarOcr, SidecarWriter,
};
use rusqlite::params;

fn digest(seed: u8) -> Sha256Digest {
    Sha256Digest::from_bytes([seed; 32])
}

fn path(value: &str) -> RelativePdfPath {
    RelativePdfPath::new(value).expect("valid relative path")
}

/// Filename-derived metadata, as a rescan would leave it.
fn metadata(id: Option<NoteId>, title: &str, composer: Option<&str>, tags: &[&str]) -> NoteMetadata {
    let mut metadata = NoteMetadata::new(
        title,
        composer.map(str::to_owned),
        tags.iter().map(|tag| (*tag).to_owned()),
    )
    .expect("valid title");
    metadata.assign_id(id, MetadataOrigin::FilenameDerived);
    metadata
        .assign_title(title, MetadataOrigin::FilenameDerived)
        .expect("valid title");
    metadata.assign_composer_or_artist(composer.map(str::to_owned), MetadataOrigin::Unset);
    metadata.assign_tags(tags.iter().map(|tag| (*tag).to_owned()), MetadataOrigin::Unset);
    metadata
}

fn note(
    relative: &str,
    seed: u8,
    id: Option<NoteId>,
    title: &str,
    composer: Option<&str>,
    tags: &[&str],
) -> IndexedNote {
    record(relative, seed, metadata(id, title, composer, tags))
}

fn record(relative: &str, seed: u8, metadata: NoteMetadata) -> IndexedNote {
    IndexedNote::new(
        path(relative),
        PdfFingerprint::new(digest(seed), 1_024 + u64::from(seed), None),
        metadata,
    )
}

/// A record the user edited by hand, which an import must not undo.
fn edited_note(relative: &str, seed: u8, id: NoteId, title: &str) -> IndexedNote {
    let mut metadata = metadata(Some(id), title, None, &[]);
    metadata.set_title(title).expect("valid title");
    metadata.set_id(Some(id));
    IndexedNote::new(
        path(relative),
        PdfFingerprint::new(digest(seed), 1_024 + u64::from(seed), None),
        metadata,
    )
}

fn entry(relative: &str, seed: u8, id: Option<NoteId>, title: &str) -> SidecarEntry {
    SidecarEntry::new(&note(relative, seed, id, title, None, &[]), None)
}

fn entry_with_ocr(
    relative: &str,
    seed: u8,
    id: Option<NoteId>,
    title: &str,
    ocr: &str,
) -> SidecarEntry {
    SidecarEntry::new(
        &note(relative, seed, id, title, None, &[]),
        Some(&OcrText::with_language(ocr, "eng")),
    )
}

fn timestamp() -> EpochTimestamp {
    EpochTimestamp::new(1_700_000_000, 0).expect("valid timestamp")
}

fn manifest(entries: Vec<SidecarEntry>) -> SidecarManifest {
    SidecarManifest::new(timestamp(), "notae-index 0.1.0", entries)
}

fn temp_file(label: &str, extension: &str) -> PathBuf {
    let nanos = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .expect("clock after epoch")
        .as_nanos();
    std::env::temp_dir().join(format!(
        "notae-sidecar-{label}-{}-{nanos}.{extension}",
        std::process::id()
    ))
}

fn open(label: &str) -> (IndexStore, PathBuf) {
    let file = temp_file(label, "sqlite");
    let store = IndexStore::open(&file).expect("open creates the database");
    (store, file)
}

/// Reads the write timestamp of every record, which is the observable proof of
/// whether an import wrote anything at all.
fn write_times(database: &Path) -> Vec<(String, String)> {
    let connection = rusqlite::Connection::open(database).expect("open raw");
    let mut statement = connection
        .prepare("SELECT path, indexed_at FROM notes ORDER BY path")
        .expect("prepare");
    statement
        .query_map([], |row| Ok((row.get(0)?, row.get(1)?)))
        .expect("query")
        .collect::<Result<Vec<_>, _>>()
        .expect("rows")
}

/// A catalog over a fixed list of files, standing in for the library root.
struct TestCatalog {
    files: Vec<PdfFile>,
}

impl TestCatalog {
    /// A catalog containing exactly `files`.
    fn new(files: Vec<PdfFile>) -> Self {
        Self { files }
    }

    /// A catalog whose every file is readable under the hash its seed names.
    fn agreeing(seeds: &[(&str, u8)]) -> Self {
        Self::new(
            seeds
                .iter()
                .map(|(relative, seed)| PdfFile::from_path(path(relative), fingerprint(*seed)))
                .collect(),
        )
    }
}

impl PdfCatalog for TestCatalog {
    fn enumerate(&self) -> Result<Vec<PdfFile>, CatalogError> {
        Ok(self.files.clone())
    }
}

/// The fingerprint the test helpers use for `seed`.
fn fingerprint(seed: u8) -> PdfFingerprint {
    PdfFingerprint::new(digest(seed), 1_024 + u64::from(seed), None)
}

/// The catalog a sidecar's own entries describe: every file it names is
/// present, and each agrees with the hash the entry claims.
///
/// This is the ordinary case — a library that matches the sidecar — and it is
/// what keeps the classification tests about classification.
fn catalog_of(sidecar: &SidecarManifest) -> TestCatalog {
    TestCatalog::new(
        sidecar
            .entries()
            .iter()
            .map(|entry| PdfFile::from_path(entry.path().clone(), entry.fingerprint().clone()))
            .collect(),
    )
}

/// The record filed under `relative`, which is what a status assertion reads.
fn record_at(store: &IndexStore, relative: &str) -> IndexedNote {
    store
        .list_all()
        .expect("list")
        .into_iter()
        .find(|note| note.path() == &path(relative))
        .unwrap_or_else(|| panic!("no record filed under {relative}"))
}

#[test]
fn manifest_round_trips_through_json() {
    let original = manifest(vec![
        entry_with_ocr("12 - Autumn Leaves.pdf", 1, Some(NoteId::new(12)), "Autumn Leaves", "le  feuilles  d'automne"),
        entry("Sibelius - Valse Triste.pdf", 2, None, "Valse Triste"),
    ]);

    let file = temp_file("round-trip", "json");
    original.write(&file).expect("write the manifest");
    let read_back = SidecarManifest::read(&file).expect("read the manifest");

    assert_eq!(read_back, original);
    assert_eq!(read_back.schema_version(), SIDECAR_SCHEMA_VERSION);
    assert_eq!(read_back.entries().len(), 2);
    assert_eq!(read_back.entries()[0].path(), &path("12 - Autumn Leaves.pdf"));
    assert_eq!(
        read_back.entries()[0].ocr().map(SidecarOcr::text),
        Some("le feuilles d'automne")
    );
    assert_eq!(read_back.entries()[0].metadata().id(), Some(NoteId::new(12)));

    let _ = std::fs::remove_file(&file);
}

#[test]
fn manifest_uses_stable_snake_case_keys() {
    let json = manifest(vec![entry("a.pdf", 1, None, "A")])
        .to_json()
        .expect("serialize");

    for key in [
        "\"schema_version\"",
        "\"generated_at\"",
        "\"tool_version\"",
        "\"entries\"",
        "\"relative_path\"",
        "\"fingerprint\"",
        "\"metadata\"",
        "\"provenance\"",
    ] {
        assert!(json.contains(key), "missing {key} in {json}");
    }
    assert!(!json.contains("relativePath"), "keys must be snake_case");

    // The same manifest read back is the same value, so the wire form really is
    // the contract rather than an accident of the writer.
    assert_eq!(
        serde_json::from_str::<SidecarManifest>(&json).expect("parse the manifest"),
        manifest(vec![entry("a.pdf", 1, None, "A")])
    );
}

#[test]
fn malformed_manifest_is_rejected() {
    let not_json = temp_file("malformed-json", "json");
    std::fs::write(&not_json, "{not json at all").expect("write the file");
    assert!(matches!(
        SidecarManifest::read(&not_json),
        Err(SidecarError::Json(_))
    ));

    let missing_entries = temp_file("malformed-fields", "json");
    std::fs::write(
        &missing_entries,
        r#"{"schema_version": 1, "generated_at": {"secs": 0}, "tool_version": "x"}"#,
    )
    .expect("write the file");
    assert!(matches!(
        SidecarManifest::read(&missing_entries),
        Err(SidecarError::Json(_))
    ));

    // A record with no provenance at all cannot be told apart from one the user
    // never edited, so it is refused instead of silently trusted.
    let without_provenance = temp_file("malformed-provenance", "json");
    std::fs::write(
        &without_provenance,
        r#"{
            "schema_version": 1,
            "generated_at": {"secs": 0},
            "tool_version": "notae-index 0.1.0",
            "entries": [{
                "relative_path": "a.pdf",
                "fingerprint": {"sha256": "0000000000000000000000000000000000000000000000000000000000000001", "byte_len": 10},
                "metadata": {"id": null, "title": "A"},
                "ocr": null
            }]
        }"#,
    )
    .expect("write the file");
    assert!(matches!(
        SidecarManifest::read(&without_provenance),
        Err(SidecarError::Json(_))
    ));

    for file in [&not_json, &missing_entries, &without_provenance] {
        let _ = std::fs::remove_file(file);
    }
}

#[test]
fn unsupported_schema_version_is_rejected_before_any_mutation() {
    let (store, database) = open("unsupported");
    store
        .upsert_note(&note("12 - Autumn Leaves.pdf", 1, Some(NoteId::new(12)), "Autumn Leaves", None, &[]))
        .expect("index the local record");

    let mut rejected = manifest(vec![entry(
        "12 - Autumn Leaves.pdf",
        1,
        Some(NoteId::new(12)),
        "Autumn Leaves",
    )]);
    rejected.set_schema_version(SIDECAR_SCHEMA_VERSION + 1);

    let before = write_times(&database);
    let error = SidecarImporter::preview(&store, &rejected, &catalog_of(&rejected)).expect_err("older build must refuse");
    assert!(matches!(
        error,
        SidecarError::UnsupportedSchemaVersion { found, supported }
            if found == SIDECAR_SCHEMA_VERSION + 1 && supported == SIDECAR_SCHEMA_VERSION
    ));
    assert_eq!(write_times(&database), before, "preview must not write");

    let _ = std::fs::remove_file(&database);
}

#[test]
fn writer_emits_a_manifest_of_the_stored_records() {
    let (store, database) = open("writer");
    store
        .upsert_note_with_ocr(
            &note("12 - Autumn Leaves.pdf", 1, Some(NoteId::new(12)), "Autumn Leaves", Some("Miles Davis"), &["jazz"]),
            Some(&OcrText::with_language("autumn leaves", "eng")),
        )
        .expect("index the first record");
    store
        .upsert_note(&note("507 - My Practice.pdf", 2, Some(NoteId::new(507)), "My Practice", None, &[]))
        .expect("index the second record");

    let file = temp_file("writer", "json");
    SidecarWriter::new("notae-index 0.1.0", timestamp())
        .write(&store, &file)
        .expect("write the sidecar");

    let written = SidecarManifest::read(&file).expect("read the sidecar");
    assert_eq!(written.schema_version(), SIDECAR_SCHEMA_VERSION);
    assert_eq!(written.tool_version(), "notae-index 0.1.0");
    assert_eq!(written.generated_at(), timestamp());
    assert_eq!(
        written
            .entries()
            .iter()
            .map(SidecarEntry::path)
            .collect::<Vec<_>>(),
        vec![
            &path("12 - Autumn Leaves.pdf"),
            &path("507 - My Practice.pdf")
        ],
        "entries are written in a stable, path-sorted order"
    );
    assert_eq!(
        written.entries()[0].ocr().map(SidecarOcr::text),
        Some("autumn leaves")
    );
    assert_eq!(written.entries()[1].ocr(), None);
    assert_eq!(written.entries()[0].ocr().and_then(SidecarOcr::language), Some("eng"));
    assert_eq!(
        written.entries()[0].metadata().composer_or_artist(),
        Some("Miles Davis")
    );
    assert_eq!(
        written
            .entry(&path("507 - My Practice.pdf"))
            .expect("the entry is found by path"),
        &written.entries()[1]
    );
    assert_eq!(written.entry(&path("absent.pdf")), None);

    // The entry rebuilds the record it describes, with its provenance intact.
    let rebuilt = written.entries()[0].to_note();
    assert_eq!(rebuilt.title(), "Autumn Leaves");
    assert_eq!(rebuilt.id(), Some(NoteId::new(12)));
    assert_eq!(
        rebuilt.metadata().origin_of(MetadataField::Title),
        MetadataOrigin::FilenameDerived
    );
    assert_eq!(
        written.entries()[0].ocr_text().map(|ocr| ocr.text().to_string()),
        Some("autumn leaves".to_string())
    );
    assert_eq!(written.entries()[1].ocr_text(), None);

    // The sidecar is portable on its own: it re-reads without the source store.
    drop(store);
    let reread = SidecarManifest::read(&file).expect("read the sidecar again");
    assert_eq!(reread.entries().len(), 2);

    let _ = std::fs::remove_file(&file);
    let _ = std::fs::remove_file(&database);
}

#[test]
fn preview_classifies_new_matched_moved_and_missing() {
    let (store, database) = open("classification");
    // Indexed under the old name; the sidecar has it under the new one.
    store
        .upsert_note(&note("Autumn Leaves.pdf", 1, Some(NoteId::new(12)), "Autumn Leaves", None, &[]))
        .expect("index the moved record");
    store
        .upsert_note(&note("Sibelius - Valse Triste.pdf", 2, None, "Valse Triste", None, &[]))
        .expect("index the matched record");
    store
        .upsert_note(&note("local-only.pdf", 3, None, "Local Only", None, &[]))
        .expect("index the record the sidecar does not mention");

    let sidecar = manifest(vec![
        entry("12 - Autumn Leaves.pdf", 1, Some(NoteId::new(12)), "Autumn Leaves"),
        entry("Sibelius - Valse Triste.pdf", 2, None, "Valse Triste"),
        entry("44 - New Piece.pdf", 4, Some(NoteId::new(44)), "New Piece"),
    ]);

    let before = write_times(&database);
    let preview = SidecarImporter::preview(&store, &sidecar, &catalog_of(&sidecar)).expect("preview");

    assert_eq!(preview.new_entries().len(), 1);
    assert_eq!(preview.new_entries()[0].path(), &path("44 - New Piece.pdf"));
    assert_eq!(preview.changed_entries().len(), 1);
    let moved = &preview.changed_entries()[0];
    assert_eq!(moved.path(), &path("12 - Autumn Leaves.pdf"));
    assert!(matches!(
        moved.change(),
        EntryChange::Moved { from } if from == &path("Autumn Leaves.pdf")
    ));
    assert!(moved.change().writes());
    assert_eq!(
        moved.resolved().map(IndexedNote::path),
        Some(&path("12 - Autumn Leaves.pdf")),
        "a move writes the record at the sidecar's path"
    );
    assert_eq!(
        moved.local().map(IndexedNote::path),
        Some(&path("Autumn Leaves.pdf")),
        "and names the record it follows"
    );
    assert_eq!(preview.unchanged_entries().len(), 1);
    assert_eq!(
        preview.unchanged_entries()[0].path(),
        &path("Sibelius - Valse Triste.pdf")
    );
    assert_eq!(preview.mismatches().len(), 0);
    assert_eq!(preview.missing().len(), 1);
    assert_eq!(
        preview.missing()[0].path(),
        &path("local-only.pdf"),
        "a record the sidecar does not mention is reported, not deleted"
    );
    assert!(preview.writes(), "a new entry and a move both write");
    assert_eq!(preview.entries().len(), 3, "every entry is classified exactly once");

    assert_eq!(write_times(&database), before, "preview must not write");
    let _ = std::fs::remove_file(&database);
}

#[test]
fn an_import_that_changes_nothing_writes_nothing() {
    let (store, database) = open("no-op");
    store
        .upsert_note(&note("a.pdf", 1, Some(NoteId::new(1)), "A", None, &[]))
        .expect("index the record");

    let sidecar = manifest(vec![entry("a.pdf", 1, Some(NoteId::new(1)), "A")]);
    let before = write_times(&database);

    let preview = SidecarImporter::preview(&store, &sidecar, &catalog_of(&sidecar)).expect("preview");
    assert!(!preview.writes());
    assert_eq!(preview.unchanged_entries().len(), 1);

    let report = SidecarImporter::apply(preview, &store).expect("apply");
    assert!(report.is_empty());
    assert_eq!(write_times(&database), before);

    let _ = std::fs::remove_file(&database);
}

#[test]
fn preview_reports_hash_mismatches_and_never_applies_them() {
    let (store, database) = open("mismatch");
    store
        .upsert_note(&note("12 - Autumn Leaves.pdf", 1, Some(NoteId::new(12)), "Autumn Leaves", None, &[]))
        .expect("index the local record");

    // The file on the phone is the one the store fingerprinted; the sidecar
    // describes different bytes, so the entry cannot be verified against it.
    let sidecar = manifest(vec![entry_with_ocr(
        "12 - Autumn Leaves.pdf",
        9,
        Some(NoteId::new(12)),
        "Autumn Leaves (PC)",
        "pc text",
    )]);

    let before = write_times(&database);
    let preview =
        SidecarImporter::preview(&store, &sidecar, &TestCatalog::agreeing(&[("12 - Autumn Leaves.pdf", 1)]))
            .expect("preview");

    assert_eq!(preview.mismatches().len(), 1);
    let mismatch = &preview.mismatches()[0];
    assert_eq!(mismatch.path(), &path("12 - Autumn Leaves.pdf"));
    assert!(matches!(
        mismatch.change(),
        EntryChange::HashMismatch { local } if *local == digest(1)
    ));
    assert_eq!(mismatch.sidecar().fingerprint().sha256(), digest(9));
    assert!(matches!(mismatch.ocr(), OcrAssessment::NotApplied));
    assert_eq!(
        mismatch.resolved(),
        None,
        "a mismatch is applied to no record at all"
    );
    assert_eq!(mismatch.local().map(IndexedNote::title), Some("Autumn Leaves"));
    assert!(!mismatch.change().writes());
    assert_eq!(preview.new_entries().len(), 0);
    assert_eq!(preview.changed_entries().len(), 0);
    assert!(!preview.writes(), "a mismatch alone writes nothing");

    let report = SidecarImporter::apply(preview, &store).expect("apply");
    assert_eq!(report.imported().len(), 0);
    assert_eq!(report.mismatches().len(), 1);
    assert_eq!(report.unchanged().len(), 0);
    assert_eq!(report.missing().len(), 0);
    assert_eq!(report.conflicts().len(), 0);
    assert_eq!(report.failures().len(), 0);

    let record = store
        .get_by_hash(&digest(1))
        .expect("read the local record")
        .expect("the local record survives");
    assert_eq!(record.title(), "Autumn Leaves");
    assert_eq!(
        store
            .ocr_text(&digest(1))
            .expect("read ocr")
            .map(|ocr| ocr.text().to_string()),
        Some(String::new()),
        "a mismatched entry contributes no OCR text"
    );
    assert_eq!(
        write_times(&database),
        before,
        "a mismatch is reported and nothing is written for it"
    );
    let _ = std::fs::remove_file(&database);
}

#[test]
fn preview_reports_duplicate_id_conflicts_with_both_paths() {
    let (store, database) = open("conflicts");
    // The record already indexed holds the ID, even though the newcomer's path
    // sorts first: ownership comes from the index as it was before the import.
    store
        .upsert_note(&note("12 - Autumn Leaves.pdf", 1, Some(NoteId::new(12)), "Autumn Leaves", None, &[]))
        .expect("index the record that already holds the id");
    let sidecar = manifest(vec![
        entry("12 - Autumn Leaves (copy).pdf", 2, Some(NoteId::new(12)), "Autumn Leaves"),
        entry("12 - Autumn Leaves.pdf", 1, Some(NoteId::new(12)), "Autumn Leaves"),
    ]);

    let preview = SidecarImporter::preview(&store, &sidecar, &catalog_of(&sidecar)).expect("preview");

    assert_eq!(preview.conflicts().len(), 1);
    let conflict = &preview.conflicts()[0];
    assert_eq!(conflict.id(), NoteId::new(12));
    assert_eq!(conflict.existing(), &path("12 - Autumn Leaves.pdf"));
    assert_eq!(conflict.conflicting(), &path("12 - Autumn Leaves (copy).pdf"));
    assert_eq!(
        preview.conflict_for(&path("12 - Autumn Leaves (copy).pdf")),
        Some(conflict),
        "both paths find the conflict by path"
    );
    assert_eq!(preview.conflict_for(&path("12 - Autumn Leaves.pdf")), Some(conflict));
    assert_eq!(preview.conflict_for(&path("99 - Unrelated.pdf")), None);
    // Both entries carry the conflict, so the screen can mark either file
    // without joining two lists.
    for entry in preview.entries() {
        let takes_part = entry.path() == conflict.existing()
            || entry.path() == conflict.conflicting();
        assert_eq!(
            entry.conflict().is_some(),
            takes_part,
            "every entry names its conflict state"
        );
    }

    let report = SidecarImporter::apply(preview, &store).expect("apply");
    assert_eq!(report.conflicts().len(), 1);
    assert_eq!(report.failures().len(), 0);

    let holder = store
        .get_by_id(NoteId::new(12))
        .expect("read by id")
        .expect("one record holds the id");
    assert_eq!(holder.path(), &path("12 - Autumn Leaves.pdf"));
    assert_eq!(
        store.list_all().expect("list").len(),
        2,
        "the loser stays in the library without the contested id"
    );
    assert!(store
        .list_all()
        .expect("list")
        .iter()
        .all(|note| note.id() != Some(NoteId::new(12)) || note.path() == &path("12 - Autumn Leaves.pdf")));

    let _ = std::fs::remove_file(&database);
}

#[test]
fn manual_metadata_survives_a_sidecar_that_proposes_other_values() {
    let (store, database) = open("manual");
    let edited = edited_note(
        "12 - Autumn Leaves.pdf",
        1,
        NoteId::new(12),
        "Autumn Leaves (my edit)",
    );
    let mut edited_metadata = edited.metadata().clone();
    edited_metadata.set_composer_or_artist(Some("Miles Davis".to_string()));
    store
        .upsert_note(&record("12 - Autumn Leaves.pdf", 1, edited_metadata))
        .expect("index the edited record");

    // The sidecar proposes a different title and a different composer; both are
    // fields the user edited by hand.
    let proposed = SidecarEntry::new(
        &note(
            "12 - Autumn Leaves.pdf",
            1,
            Some(NoteId::new(12)),
            "Autumn Leaves (from PC)",
            Some("Bill Evans"),
            &[],
        ),
        None,
    );
    let sidecar = manifest(vec![proposed]);

    let preview = SidecarImporter::preview(&store, &sidecar, &catalog_of(&sidecar)).expect("preview");
    let planned = &preview.unchanged_entries()[0];

    assert!(
        planned
            .metadata()
            .preserved()
            .contains(&MetadataField::Title),
        "a manually edited field is preserved: {:?}",
        planned.metadata().preserved()
    );
    assert!(
        planned
            .metadata()
            .preserved()
            .contains(&MetadataField::ComposerOrArtist)
    );
    assert_eq!(
        planned.metadata().imported(),
        &[],
        "nothing the sidecar proposes may be applied to a manual field"
    );

    let report = SidecarImporter::apply(preview, &store).expect("apply");
    assert_eq!(report.imported().len(), 0);
    assert_eq!(report.unchanged().len(), 1);

    let stored = store
        .get_by_hash(&digest(1))
        .expect("read")
        .expect("record");
    assert_eq!(stored.title(), "Autumn Leaves (my edit)");
    assert_eq!(stored.metadata().composer_or_artist(), Some("Miles Davis"));
    assert_eq!(
        stored.metadata().origin_of(MetadataField::Title),
        MetadataOrigin::Manual
    );

    let _ = std::fs::remove_file(&database);
}

#[test]
fn imported_metadata_is_applied_to_non_manual_fields() {
    let (store, database) = open("imported");
    let mut local_metadata = metadata(Some(NoteId::new(12)), "Autumn Leaves", None, &["jazz"]);
    local_metadata
        .set_title("Autumn Leaves (my edit)")
        .expect("valid title");
    store
        .upsert_note(&record("12 - Autumn Leaves.pdf", 1, local_metadata))
        .expect("index the record");

    let mut proposed_metadata = metadata(
        Some(NoteId::new(12)),
        "Autumn Leaves (from PC)",
        Some("Miles Davis"),
        &["blues", "jazz"],
    );
    proposed_metadata.set_tags(["blues", "jazz"].map(str::to_owned));
    let sidecar = manifest(vec![SidecarEntry::new(
        &record("12 - Autumn Leaves.pdf", 1, proposed_metadata),
        None,
    )]);

    let preview = SidecarImporter::preview(&store, &sidecar, &catalog_of(&sidecar)).expect("preview");
    let planned = &preview.changed_entries()[0];
    assert!(planned.metadata().preserved().contains(&MetadataField::Title));
    assert!(
        planned
            .metadata()
            .imported()
            .contains(&MetadataField::ComposerOrArtist)
    );
    assert!(planned.metadata().imported().contains(&MetadataField::Tags));
    assert!(!planned.metadata().imported().contains(&MetadataField::Title));

    let report = SidecarImporter::apply(preview, &store).expect("apply");
    assert_eq!(report.imported().len(), 1);

    let stored = store.get_by_hash(&digest(1)).expect("read").expect("record");
    assert_eq!(stored.title(), "Autumn Leaves (my edit)");
    assert_eq!(stored.metadata().composer_or_artist(), Some("Miles Davis"));
    assert_eq!(
        stored.metadata().tags().iter().collect::<Vec<_>>(),
        vec!["blues", "jazz"]
    );
    assert_eq!(
        stored.metadata().origin_of(MetadataField::ComposerOrArtist),
        MetadataOrigin::PcImported
    );

    let _ = std::fs::remove_file(&database);
}

#[test]
fn explicit_override_replaces_manual_metadata() {
    let (store, database) = open("override");
    let edited = edited_note(
        "12 - Autumn Leaves.pdf",
        1,
        NoteId::new(12),
        "Autumn Leaves (my edit)",
    );
    store.upsert_note(&edited).expect("index the edited record");

    let sidecar = manifest(vec![entry(
        "12 - Autumn Leaves.pdf",
        1,
        Some(NoteId::new(12)),
        "Autumn Leaves (from PC)",
    )]);

    let preview = SidecarImporter::preview(&store, &sidecar, &catalog_of(&sidecar)).expect("preview");
    assert!(preview.unchanged_entries().len() == 1, "manual metadata blocks the write");

    let overridden = preview
        .with_override(&path("12 - Autumn Leaves.pdf"), [MetadataField::Title])
        .expect("the entry exists");
    assert_eq!(overridden.changed_entries().len(), 1);
    assert!(
        overridden.changed_entries()[0]
            .metadata()
            .overridden()
            .contains(&MetadataField::Title)
    );

    assert!(
        SidecarImporter::preview(&store, &sidecar, &catalog_of(&sidecar))
            .expect("the untouched preview still refuses")
            .unchanged_entries()
            .len()
            == 1,
        "an override never mutates the preview it came from"
    );

    SidecarImporter::apply(overridden, &store).expect("apply");
    let stored = store.get_by_hash(&digest(1)).expect("read").expect("record");
    assert_eq!(stored.title(), "Autumn Leaves (from PC)");
    assert_eq!(
        stored.metadata().origin_of(MetadataField::Title),
        MetadataOrigin::Manual,
        "an explicit replacement is the user's decision and stays authoritative"
    );

    let _ = std::fs::remove_file(&database);
}

#[test]
fn imported_ocr_fills_missing_and_replaces_stale_text() {
    let (store, database) = open("ocr-import");
    store
        .upsert_note_with_ocr(
            &note("a.pdf", 1, None, "A", None, &[]),
            Some(&OcrText::new("")),
        )
        .expect("index a record with no OCR text");
    store
        .upsert_note_with_ocr(
            &note("b.pdf", 2, None, "B", None, &[]),
            Some(&OcrText::with_language("text from the old bytes", "eng")),
        )
        .expect("index a record with stale OCR text");

    let sidecar = manifest(vec![
        entry_with_ocr("a.pdf", 1, None, "A", "recognized on the PC"),
        entry_with_ocr("b.pdf", 2, None, "B", "text from the new bytes"),
    ]);

    let preview = SidecarImporter::preview(&store, &sidecar, &catalog_of(&sidecar)).expect("preview");
    assert!(preview
        .entries()
        .iter()
        .all(|item| matches!(item.ocr(), OcrAssessment::Importing)));

    let report = SidecarImporter::apply(preview, &store).expect("apply");
    assert_eq!(report.imported().len(), 2);

    assert_eq!(
        store.ocr_text(&digest(1)).expect("read").map(|t| t.text().to_string()),
        Some("recognized on the PC".to_string())
    );
    let b = store.ocr_text(&digest(2)).expect("read").expect("ocr text");
    assert_eq!(b.text(), "text from the new bytes");
    assert_eq!(b.language(), Some("eng"));

    let _ = std::fs::remove_file(&database);
}

#[test]
fn current_local_ocr_is_not_reimported() {
    let (store, database) = open("ocr-current");
    store
        .upsert_note_with_ocr(
            &note("a.pdf", 1, None, "A", None, &[]),
            Some(&OcrText::with_language("already indexed", "eng")),
        )
        .expect("index the record");

    let sidecar = manifest(vec![entry_with_ocr("a.pdf", 1, None, "A", "already indexed")]);
    let before = write_times(&database);

    let preview = SidecarImporter::preview(&store, &sidecar, &catalog_of(&sidecar)).expect("preview");
    assert_eq!(preview.unchanged_entries().len(), 1);
    assert!(matches!(
        preview.unchanged_entries()[0].ocr(),
        OcrAssessment::Current
    ));

    let report = SidecarImporter::apply(preview, &store).expect("apply");
    assert_eq!(report.imported().len(), 0);
    assert_eq!(report.unchanged().len(), 1);
    assert_eq!(write_times(&database), before, "current OCR costs no write");

    let stored = store.ocr_text(&digest(1)).expect("read").expect("ocr");
    assert_eq!(stored.text(), "already indexed");

    let _ = std::fs::remove_file(&database);
}

#[test]
fn preview_does_not_touch_the_local_store() {
    let (store, database) = open("read-only");
    store
        .upsert_note_with_ocr(
            &note("12 - Autumn Leaves.pdf", 1, Some(NoteId::new(12)), "Autumn Leaves", None, &[]),
            Some(&OcrText::new("autumn leaves")),
        )
        .expect("index the record");

    let sidecar = manifest(vec![
        entry_with_ocr("12 - Autumn Leaves.pdf", 1, Some(NoteId::new(12)), "Autumn Leaves", "new pc text"),
        entry("44 - New Piece.pdf", 4, Some(NoteId::new(12)), "New Piece"),
    ]);

    let before_times = write_times(&database);
    let before_notes = store.list_all().expect("list");

    let preview = SidecarImporter::preview(&store, &sidecar, &catalog_of(&sidecar)).expect("preview");
    assert_eq!(preview.conflicts().len(), 1, "the preview still reports everything");
    assert!(!preview.new_entries().is_empty());
    let _ = preview.with_override(&path("44 - New Piece.pdf"), [MetadataField::Title])
        .expect("override");

    assert_eq!(write_times(&database), before_times);
    assert_eq!(store.list_all().expect("list"), before_notes);
    assert_eq!(
        store.ocr_text(&digest(1)).expect("read").map(|t| t.text().to_string()),
        Some("autumn leaves".to_string())
    );

    let _ = std::fs::remove_file(&database);
}

#[test]
fn apply_keeps_at_most_one_record_per_numeric_id() {
    let (store, database) = open("unique-ids");
    store
        .upsert_note(&note("existing.pdf", 1, Some(NoteId::new(12)), "Existing", None, &[]))
        .expect("index the local record");
    store
        .upsert_note(&note("local-only.pdf", 2, Some(NoteId::new(7)), "Local Only", None, &[]))
        .expect("index a record the sidecar omits");

    let sidecar = manifest(vec![
        // Claims an ID the local library already holds.
        entry("12 - Autumn Leaves.pdf", 3, Some(NoteId::new(12)), "Autumn Leaves"),
        // Two new files competing for one number.
        entry("99 - Take One.pdf", 4, Some(NoteId::new(99)), "Take One"),
        entry("99 - Take Two.pdf", 5, Some(NoteId::new(99)), "Take Two"),
        // A record that does not mention an ID at all.
        entry("100 - Unnumbered.pdf", 6, None, "Unnumbered"),
    ]);

    let preview = SidecarImporter::preview(&store, &sidecar, &catalog_of(&sidecar)).expect("preview");
    assert_eq!(preview.conflicts().len(), 2);

    let report = SidecarImporter::apply(preview, &store).expect("apply");
    assert_eq!(report.conflicts().len(), 2);
    assert_eq!(
        report.failures().len(),
        0,
        "no record is stranded by a conflict: {:?}",
        report.failures()
    );

    let notes = store.list_all().expect("list");
    let mut seen: HashSet<NoteId> = HashSet::new();
    for note in &notes {
        let Some(id) = note.id() else { continue };
        assert!(seen.insert(id), "id {id} is held by two records");
        assert_eq!(
            store.get_by_id(id).expect("read by id").map(|found| found.path().clone()),
            Some(note.path().clone())
        );
    }
    assert_eq!(seen.len(), 3, "the library keeps every distinct id it had");

    // The record indexed first keeps the contested number.
    assert_eq!(
        store.get_by_id(NoteId::new(12)).expect("read").expect("held").path(),
        &path("existing.pdf")
    );

    let _ = std::fs::remove_file(&database);
}

#[test]
fn apply_reports_a_store_refusal_as_a_failure() {
    let (store, database) = open("failure");
    // The number 12 is already held by a local record.
    store
        .upsert_note(&note("existing.pdf", 1, Some(NoteId::new(12)), "Existing", None, &[]))
        .expect("index the record that holds the id");

    // The newcomer also carries 12, and the user typed it in by hand on the PC,
    // so it is a manual ID there too. An import may not withhold a manual ID,
    // so the store refuses the write — and the refusal is reported instead of
    // the file disappearing.
    let mut manual = metadata(Some(NoteId::new(12)), "Autumn Leaves", None, &[]);
    manual.set_id(Some(NoteId::new(12)));
    let sidecar = manifest(vec![
        SidecarEntry::new(&record("12 - Autumn Leaves.pdf", 2, manual), None),
        entry("77 - Fine Piece.pdf", 3, Some(NoteId::new(77)), "Fine Piece"),
    ]);

    let preview = SidecarImporter::preview(&store, &sidecar, &catalog_of(&sidecar)).expect("preview");
    assert_eq!(preview.conflicts().len(), 1);
    let report = SidecarImporter::apply(preview, &store).expect("apply");

    assert_eq!(report.failures().len(), 1);
    assert_eq!(report.failures()[0].path(), &path("12 - Autumn Leaves.pdf"));
    assert!(matches!(
        report.failures()[0].reason(),
        IndexError::DuplicateNoteId(id) if *id == NoteId::new(12)
    ));
    assert_eq!(
        report.imported().len(),
        1,
        "one refused record does not strand the rest"
    );
    assert_eq!(report.imported()[0].path(), &path("77 - Fine Piece.pdf"));

    let _ = std::fs::remove_file(&database);
}

#[test]
fn duplicate_paths_and_content_in_a_manifest_are_rejected() {
    let (store, database) = open("duplicates");

    let duplicate_path = manifest(vec![
        entry("a.pdf", 1, None, "A"),
        entry("a.pdf", 2, None, "A again"),
    ]);
    assert!(matches!(
        SidecarImporter::preview(&store, &duplicate_path, &catalog_of(&duplicate_path)),
        Err(SidecarError::DuplicatePath(_))
    ));

    let duplicate_content = manifest(vec![entry("a.pdf", 1, None, "A"), entry("b.pdf", 1, None, "B")]);
    assert!(matches!(
        SidecarImporter::preview(&store, &duplicate_content, &catalog_of(&duplicate_content)),
        Err(SidecarError::DuplicateContent(_))
    ));
    assert!(store.list_all().expect("list").is_empty());

    // A manifest is rejected by reading it too, not only by importing it.
    let file = temp_file("invalid-manifest", "json");
    assert!(duplicate_content.write(&file).is_ok());
    assert!(matches!(
        SidecarManifest::read(&file),
        Err(SidecarError::DuplicateContent(_))
    ));
    let _ = std::fs::remove_file(&file);

    // An override naming a path the manifest does not have is refused.
    let sidecar = manifest(vec![entry("a.pdf", 1, None, "A")]);
    let preview = SidecarImporter::preview(&store, &sidecar, &catalog_of(&sidecar)).expect("preview");
    assert!(matches!(
        preview.with_override(&path("absent.pdf"), [MetadataField::Title]),
        Err(SidecarError::UnknownEntry(_))
    ));

    let _ = std::fs::remove_file(&database);
}

#[test]
fn search_covers_imported_metadata_and_ocr_text() {
    let (store, database) = open("search");
    let sidecar = manifest(vec![entry_with_ocr(
        "12 - Autumn Leaves.pdf",
        1,
        Some(NoteId::new(12)),
        "Autumn Leaves",
        "miles davis autumn in new york",
    )]);

    SidecarImporter::apply(
        SidecarImporter::preview(&store, &sidecar, &catalog_of(&sidecar)).expect("preview"),
        &store,
    )
    .expect("apply");

    assert!(!store.search("davis", 10).expect("search").is_empty());
    assert!(!store.search("12", 10).expect("search").is_empty());

    let _ = std::fs::remove_file(&database);
}

#[test]
fn import_does_not_delete_records_the_sidecar_omits() {
    let (store, database) = open("no-delete");
    store
        .upsert_note(&note("local-only.pdf", 1, None, "Local Only", None, &[]))
        .expect("index the record");

    let sidecar = manifest(vec![entry("new.pdf", 2, None, "New")]);
    let preview = SidecarImporter::preview(&store, &sidecar, &catalog_of(&sidecar)).expect("preview");
    assert_eq!(preview.missing().len(), 1);

    let report = SidecarImporter::apply(preview, &store).expect("apply");
    assert_eq!(report.missing().len(), 1);

    let notes = store.list_all().expect("list");
    assert_eq!(notes.len(), 2);
    assert!(notes.iter().any(|note| note.path() == &path("local-only.pdf")));

    let _ = std::fs::remove_file(&database);
}

#[test]
fn ocr_text_from_a_sidecar_is_readable_through_the_store() {
    let (store, database) = open("ocr-read");
    let mut local = note("a.pdf", 1, None, "A", None, &[]);
    local.set_status(notae_domain::IndexStatus::OcrPending);
    store.upsert_note_with_ocr(&local, Some(&OcrText::new(""))).expect("index");

    let sidecar = manifest(vec![entry_with_ocr("a.pdf", 1, None, "A", "printed words")]);
    SidecarImporter::apply(
        SidecarImporter::preview(&store, &sidecar, &catalog_of(&sidecar)).expect("preview"),
        &store,
    )
    .expect("apply");

    let text = store.ocr_text(&digest(1)).expect("read").expect("ocr");
    assert_eq!(text.text(), "printed words");

    let row: (String, i64) = rusqlite::Connection::open(&database)
        .expect("open raw")
        .query_row("SELECT ocr_text, length(ocr_text) FROM notes", params![], |row| {
            Ok((row.get(0)?, row.get(1)?))
        })
        .expect("query");
    assert_eq!(row.0, "printed words");
    assert_eq!(row.1, i64::try_from("printed words".len()).expect("length fits"));

    let _ = std::fs::remove_file(&database);
}

#[test]
fn a_file_that_changed_on_disk_is_a_hash_mismatch_and_is_not_applied() {
    let (store, database) = open("disk-mismatch");
    store
        .upsert_note(&note("a.pdf", 1, Some(NoteId::new(12)), "Autumn Leaves", None, &[]))
        .expect("index the record");

    // The sidecar and the store both still describe the bytes the phone had
    // yesterday; the file itself is different today. Comparing the sidecar
    // against the store would call this `Unchanged` and write the PC's stale
    // metadata over a file that no longer holds it.
    let sidecar = manifest(vec![entry_with_ocr(
        "a.pdf",
        1,
        Some(NoteId::new(12)),
        "Autumn Leaves (PC)",
        "text from the old bytes",
    )]);
    let catalog = TestCatalog::agreeing(&[("a.pdf", 7)]);

    let before = write_times(&database);
    let preview = SidecarImporter::preview(&store, &sidecar, &catalog).expect("preview");

    assert_eq!(preview.mismatches().len(), 1);
    let mismatch = &preview.mismatches()[0];
    assert_eq!(mismatch.path(), &path("a.pdf"));
    assert!(
        matches!(mismatch.change(), EntryChange::HashMismatch { local } if *local == digest(7)),
        "the reported hash is the one the file actually has: {:?}",
        mismatch.change()
    );
    assert_eq!(mismatch.resolved(), None);
    assert!(matches!(mismatch.ocr(), OcrAssessment::NotApplied));
    assert!(!preview.writes());

    let report = SidecarImporter::apply(preview, &store).expect("apply");
    assert_eq!(report.imported().len(), 0);
    assert_eq!(report.mismatches().len(), 1);

    let stored = store.get_by_hash(&digest(1)).expect("read").expect("record");
    assert_eq!(
        stored.title(),
        "Autumn Leaves",
        "the changed file's record keeps its own metadata"
    );
    assert_eq!(
        store
            .ocr_text(&digest(1))
            .expect("read")
            .map(|ocr| ocr.text().to_string()),
        Some(String::new()),
        "a mismatched entry contributes no OCR text"
    );
    assert_eq!(write_times(&database), before);

    let _ = std::fs::remove_file(&database);
}

#[test]
fn an_entry_for_a_file_the_library_does_not_have_is_reported_as_missing() {
    let (store, database) = open("absent-file");

    let sidecar = manifest(vec![
        entry("present.pdf", 1, None, "Present"),
        entry("absent.pdf", 2, Some(NoteId::new(99)), "Absent"),
    ]);
    // Only `present.pdf` is in the library root.
    let catalog = TestCatalog::agreeing(&[("present.pdf", 1)]);

    let preview = SidecarImporter::preview(&store, &sidecar, &catalog).expect("preview");

    assert_eq!(preview.absent_entries().len(), 1);
    let absent = &preview.absent_entries()[0];
    assert_eq!(absent.path(), &path("absent.pdf"));
    assert_eq!(absent.change(), &EntryChange::Missing);
    assert_eq!(absent.resolved(), None);
    assert!(matches!(absent.ocr(), OcrAssessment::NotApplied));
    assert_eq!(preview.new_entries().len(), 1, "the file that is there still imports");

    let report = SidecarImporter::apply(preview, &store).expect("apply");
    assert_eq!(report.absent_entries().len(), 1);
    assert_eq!(report.imported().len(), 1);

    let notes = store.list_all().expect("list");
    assert_eq!(notes.len(), 1, "no record is created for a file that is not there");
    assert_eq!(notes[0].path(), &path("present.pdf"));

    let _ = std::fs::remove_file(&database);
}

#[test]
fn a_file_the_catalog_cannot_read_is_not_imported() {
    let (store, database) = open("unreadable-file");

    let sidecar = manifest(vec![entry("a.pdf", 1, Some(NoteId::new(12)), "A")]);
    let catalog = TestCatalog::new(vec![PdfFile::from_path_with_condition(
        path("a.pdf"),
        FileCondition::Unreadable,
    )]);

    let preview = SidecarImporter::preview(&store, &sidecar, &catalog).expect("preview");

    assert_eq!(preview.unreadable_entries().len(), 1);
    let unreadable = &preview.unreadable_entries()[0];
    assert_eq!(unreadable.path(), &path("a.pdf"));
    assert_eq!(unreadable.resolved(), None);
    assert!(!preview.writes(), "a hash that could not be read is not a hash");

    SidecarImporter::apply(preview, &store).expect("apply");
    assert!(
        store.list_all().expect("list").is_empty(),
        "no record is created for a file whose hash could not be read"
    );

    let _ = std::fs::remove_file(&database);
}

#[test]
fn a_manifest_with_an_unknown_field_is_rejected() {
    let extra = temp_file("unknown-field", "json");
    std::fs::write(
        &extra,
        r#"{
            "schema_version": 2,
            "generated_at": {"secs": 0},
            "tool_version": "notae-index 0.9.0",
            "entries": [],
            "producer_id": "a-different-indexer"
        }"#,
    )
    .expect("write the file");
    assert!(
        matches!(SidecarManifest::read(&extra), Err(SidecarError::Json(_))),
        "a producer this build does not understand must fail before import, \
         not have its field dropped"
    );

    // The same rule reaches the entries themselves.
    let entry_extra = temp_file("unknown-entry-field", "json");
    std::fs::write(
        &entry_extra,
        r#"{
            "schema_version": 2,
            "generated_at": {"secs": 0},
            "tool_version": "notae-index 0.9.0",
            "entries": [{
                "relative_path": "a.pdf",
                "fingerprint": {"sha256": "0000000000000000000000000000000000000000000000000000000000000001", "byte_len": 10},
                "metadata": {"id": null, "title": "A", "provenance": {"id": "filename_derived", "title": "filename_derived", "composer_or_artist": "unset", "tags": "unset"}},
                "status": "ready",
                "row_id": 7
            }]
        }"#,
    )
    .expect("write the file");
    assert!(matches!(
        SidecarManifest::read(&entry_extra),
        Err(SidecarError::Json(_))
    ));

    let _ = std::fs::remove_file(&extra);
    let _ = std::fs::remove_file(&entry_extra);
}

/// A manifest carrying exactly one entry, built from the JSON fragments the
/// test varies.
fn manifest_json(entry: String) -> String {
    format!(
        r#"{{
            "schema_version": 2,
            "generated_at": {{"secs": 1700000000, "nanos": 0}},
            "tool_version": "notae-index 0.1.0",
            "entries": [{entry}]
        }}"#
    )
}

/// One entry carrying the given fingerprint and metadata objects.
fn entry_json(fingerprint: String, metadata: String) -> String {
    format!(
        r#"{{
            "relative_path": "a.pdf",
            "fingerprint": {fingerprint},
            "metadata": {metadata},
            "status": "ready"
        }}"#
    )
}

/// The provenance object as the writer emits it.
fn provenance_json() -> String {
    r#"{"id": "filename_derived", "title": "filename_derived", "composer_or_artist": "unset", "tags": "unset"}"#.to_string()
}

#[test]
fn an_unknown_field_in_any_nested_value_is_rejected() {
    // The digest crosses the wire as the 32 raw bytes it is, not as hex.
    let digest = format!("[{}]", vec!["1"; 32].join(", "));
    let good_fingerprint = format!(r#"{{"sha256": {digest}, "byte_len": 10}}"#);
    let good_metadata =
        format!(r#"{{"id": null, "title": "A", "provenance": {}}}"#, provenance_json());

    let cases: [(&str, String); 4] = [
        (
            "fingerprint",
            manifest_json(entry_json(
                format!(r#"{{"sha256": {digest}, "byte_len": 10, "future": 1}}"#),
                good_metadata.clone(),
            )),
        ),
        (
            "metadata",
            manifest_json(entry_json(
                good_fingerprint.clone(),
                format!(
                    r#"{{"id": null, "title": "A", "provenance": {}, "future": 1}}"#,
                    provenance_json()
                ),
            )),
        ),
        (
            "provenance",
            manifest_json(entry_json(
                good_fingerprint.clone(),
                r#"{"id": null, "title": "A", "provenance": {"id": "filename_derived", "title": "filename_derived", "composer_or_artist": "unset", "tags": "unset", "future": 1}}"#.to_string(),
            )),
        ),
        (
            "timestamp",
            r#"{
                "schema_version": 2,
                "generated_at": {"secs": 1700000000, "nanos": 0, "future": 1},
                "tool_version": "notae-index 0.1.0",
                "entries": []
            }"#
            .to_string(),
        ),
    ];

    for (where_, json) in cases {
        let file = temp_file(&format!("nested-extra-{where_}"), "json");
        std::fs::write(&file, json).expect("write the file");
        assert!(
            matches!(SidecarManifest::read(&file), Err(SidecarError::Json(_))),
            "an unknown field inside {where_} must be refused before import, not \
             silently dropped: a producer and a consumer that do not agree have \
             to find out"
        );
        let _ = std::fs::remove_file(&file);
    }

    // The same nested shapes without the extra field still load, so strictness
    // is about unknown fields and nothing else.
    let file = temp_file("nested-clean", "json");
    std::fs::write(
        &file,
        manifest_json(entry_json(good_fingerprint, good_metadata)),
    )
    .expect("write the file");
    assert!(SidecarManifest::read(&file).is_ok());
    let _ = std::fs::remove_file(&file);
}

#[test]
fn a_manifest_without_a_schema_version_is_rejected() {
    let file = temp_file("no-version", "json");
    std::fs::write(
        &file,
        r#"{
            "generated_at": {"secs": 0},
            "tool_version": "notae-index 0.1.0",
            "entries": []
        }"#,
    )
    .expect("write the file");

    assert!(
        matches!(SidecarManifest::read(&file), Err(SidecarError::Json(_))),
        "the schema version is a required part of the contract, so a manifest \
         that omits it is refused instead of being read as the current one"
    );

    let _ = std::fs::remove_file(&file);
}

#[test]
fn the_writer_preserves_contested_and_condition_failed_status() {
    let (store, database) = open("status-round-trip");
    // A record the PC could not index, and one it could not read: both carry a
    // status that says something about the record itself.
    let mut contested = note("12 - Autumn Leaves (copy).pdf", 1, None, "Autumn Leaves", None, &[]);
    contested.set_status(IndexStatus::DuplicateIdConflict);
    store.upsert_note(&contested).expect("index the contested record");

    let mut unreadable = note("44 - Broken.pdf", 2, Some(NoteId::new(44)), "Broken", None, &[]);
    unreadable.set_status(IndexStatus::InvalidPdf);
    store.upsert_note(&unreadable).expect("index the unreadable record");

    let file = temp_file("status-round-trip", "json");
    SidecarWriter::new("notae-index 0.1.0", timestamp())
        .write(&store, &file)
        .expect("write the sidecar");

    let written = SidecarManifest::read(&file).expect("read the sidecar");
    assert_eq!(
        written.entry(&path("12 - Autumn Leaves (copy).pdf")).expect("entry").status(),
        IndexStatus::DuplicateIdConflict,
        "a record the writer dropped the status of reads as a free unnumbered entry"
    );
    assert_eq!(
        written.entry(&path("44 - Broken.pdf")).expect("entry").status(),
        IndexStatus::InvalidPdf,
        "a condition a record was left in survives the round trip"
    );

    // The status reaches the receiving library too, so a withheld ID is read as
    // withheld rather than as a number nobody claimed.
    let catalog = catalog_of(&written);
    let importing = IndexStore::open(temp_file("status-round-trip-target", "sqlite"))
        .expect("open the receiving library");
    let preview = SidecarImporter::preview(&importing, &written, &catalog).expect("preview");
    let offered = preview.new_entries();
    let contested = offered
        .iter()
        .find(|item| item.path() == &path("12 - Autumn Leaves (copy).pdf"))
        .expect("the contested record is offered as new");
    assert_eq!(
        contested.resolved().map(IndexedNote::status),
        Some(IndexStatus::DuplicateIdConflict)
    );
    assert_eq!(
        contested.resolved().map(IndexedNote::id),
        Some(None),
        "the PC withheld the ID because it was contested, and the phone sees it as such"
    );

    let _ = std::fs::remove_file(&file);
    let _ = std::fs::remove_file(&database);
}

#[test]
fn a_conflict_with_an_omitted_incumbent_marks_both_records() {
    let (store, database) = open("omitted-incumbent");
    // The incumbent holds 30 and the sidecar does not mention it at all.
    store
        .upsert_note(&note("held.pdf", 1, Some(NoteId::new(30)), "Held", None, &[]))
        .expect("index the record that holds the id");

    let sidecar = manifest(vec![entry("loser.pdf", 2, Some(NoteId::new(30)), "Loser")]);
    let preview = SidecarImporter::preview(&store, &sidecar, &catalog_of(&sidecar)).expect("preview");

    assert_eq!(preview.conflicts().len(), 1);
    assert_eq!(
        preview.contested_records().len(),
        1,
        "the record the sidecar omits is still part of the conflict"
    );
    assert_eq!(
        preview.contested_records()[0].path(),
        &path("held.pdf"),
        "and the preview says so before anything is written"
    );

    let report = SidecarImporter::apply(preview, &store).expect("apply");
    assert_eq!(report.conflicts().len(), 1);
    assert_eq!(report.failures().len(), 0);

    let held = record_at(&store, "held.pdf");
    assert_eq!(
        held.status(),
        IndexStatus::DuplicateIdConflict,
        "both sides of a conflict carry the marker, not just the entry"
    );
    assert_eq!(held.id(), Some(NoteId::new(30)));

    let loser = record_at(&store, "loser.pdf");
    assert_eq!(loser.status(), IndexStatus::DuplicateIdConflict);
    assert_eq!(loser.id(), None, "the newcomer is written without the contested id");

    let _ = std::fs::remove_file(&database);
}

#[test]
fn a_contested_incumbent_write_is_not_an_empty_report() {
    // The incumbent is omitted by the sidecar and still holds the number, so
    // applying the preview writes its status even when no entry is imported at
    // all. A report that calls that empty hides a real write.
    let (store, database) = open("contested-not-empty");
    store
        .upsert_note(&note("held.pdf", 1, Some(NoteId::new(30)), "Held", None, &[]))
        .expect("index the record that holds the id");

    // The only entry is the incumbent's own file, imported under a manual ID the
    // store refuses: nothing is imported, one record fails, and the omitted
    // incumbent is still written with the conflict marker.
    let mut manual = metadata(Some(NoteId::new(30)), "Held", None, &[]);
    manual.set_id(Some(NoteId::new(30)));
    let sidecar = manifest(vec![SidecarEntry::new(
        &record("held-copy.pdf", 2, manual),
        None,
    )]);
    let preview = SidecarImporter::preview(&store, &sidecar, &catalog_of(&sidecar)).expect("preview");

    let report = SidecarImporter::apply(preview, &store).expect("apply");
    assert_eq!(
        report.contested_records().len(),
        1,
        "the omitted incumbent is written with the conflict marker"
    );
    assert!(
        !report.is_empty(),
        "a report that wrote a contested incumbent is not empty: \
         imported={} contested={} failures={}",
        report.imported().len(),
        report.contested_records().len(),
        report.failures().len()
    );

    let _ = std::fs::remove_file(&database);
}

#[test]
fn an_empty_report_stays_empty_without_a_contested_write() {
    let (store, database) = open("still-empty");
    store
        .upsert_note(&note("a.pdf", 1, Some(NoteId::new(1)), "A", None, &[]))
        .expect("index the record");

    let sidecar = manifest(vec![entry("a.pdf", 1, Some(NoteId::new(1)), "A")]);
    let preview = SidecarImporter::preview(&store, &sidecar, &catalog_of(&sidecar)).expect("preview");
    let report = SidecarImporter::apply(preview, &store).expect("apply");

    assert_eq!(report.contested_records().len(), 0);
    assert!(report.is_empty(), "nothing was written, so the report is empty");

    let _ = std::fs::remove_file(&database);
}

#[test]
fn a_condition_failed_incumbent_still_takes_part_in_the_conflict() {
    let (store, database) = open("condition-incumbent");
    // The incumbent holds 7 and the catalog cannot read its file this run, so
    // its entry is unverified and contributes no candidate of its own. It still
    // exists in the library and still holds the number.
    let mut incumbent = note("inc.pdf", 1, Some(NoteId::new(7)), "Incumbent", None, &[]);
    incumbent.set_status(IndexStatus::Inaccessible);
    store.upsert_note(&incumbent).expect("index the incumbent");

    let sidecar = manifest(vec![
        entry("inc.pdf", 1, Some(NoteId::new(7)), "Incumbent"),
        entry("new.pdf", 2, Some(NoteId::new(7)), "Newcomer"),
    ]);
    let catalog = TestCatalog::new(vec![
        PdfFile::from_path_with_condition(path("inc.pdf"), FileCondition::Unreadable),
        PdfFile::from_path(path("new.pdf"), fingerprint(2)),
    ]);

    let preview = SidecarImporter::preview(&store, &sidecar, &catalog).expect("preview");

    assert_eq!(preview.unreadable_entries().len(), 1);
    assert_eq!(
        preview.conflicts().len(),
        1,
        "an unverified incumbent still holds its id, so the collision is real"
    );
    let conflict = &preview.conflicts()[0];
    assert_eq!(conflict.id(), NoteId::new(7));
    assert_eq!(conflict.existing(), &path("inc.pdf"));
    assert_eq!(conflict.conflicting(), &path("new.pdf"));
    assert!(
        preview.conflict_for(&path("inc.pdf")).is_some(),
        "the condition-failed incumbent is reported as one side of the conflict"
    );
    assert!(
        preview.conflict_for(&path("new.pdf")).is_some(),
        "and so is the newcomer"
    );

    let newcomer = preview.new_entries()[0];
    assert_eq!(
        newcomer.resolved().and_then(IndexedNote::id),
        None,
        "the newcomer is written without the number the incumbent already holds"
    );
    assert_eq!(
        newcomer.resolved().map(IndexedNote::status),
        Some(IndexStatus::DuplicateIdConflict)
    );

    let report = SidecarImporter::apply(preview, &store).expect("apply");
    assert_eq!(report.conflicts().len(), 1);
    assert_eq!(
        report.failures().len(),
        0,
        "the collision is resolved by the import instead of by a store refusal: \
         {:?}",
        report.failures()
    );

    let written = record_at(&store, "new.pdf");
    assert_eq!(written.status(), IndexStatus::DuplicateIdConflict);
    assert_eq!(written.id(), None);

    let kept = record_at(&store, "inc.pdf");
    assert_eq!(
        kept.status(),
        IndexStatus::Inaccessible,
        "the observed condition is kept, not overwritten by a conflict marker"
    );
    assert_eq!(kept.id(), Some(NoteId::new(7)));

    let _ = std::fs::remove_file(&database);
}