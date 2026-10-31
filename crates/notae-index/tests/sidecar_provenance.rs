//! Databases written while the desktop sidecar import existed must keep opening.
//!
//! The importer is gone, but its rows are still in users' indexes: fields it
//! filled carry the stored provenance `pc_imported`. This test opens such a
//! database and checks that migration, search, and provenance reading all
//! still work.
#![allow(clippy::expect_used)]

use std::path::PathBuf;

use notae_domain::{MetadataField, MetadataOrigin, NoteId};
use notae_index::{IndexStore, SCHEMA_VERSION, SearchScope};

/// A copy of `tests/fixtures/sidecar-provenance.sqlite` in a fresh directory.
///
/// Opening migrates in place, so the committed fixture is never opened itself.
///
/// The fixture was produced at commit 7f2355e, before the importer was removed,
/// by a throwaway test that indexed two local notes (101, 102) and ran
/// `SidecarImporter::preview` and `apply` with a manifest that proposed a
/// composer, tags, and OCR text for 101 and a new note, 103. Row 101 therefore
/// holds `pc_imported` composer and tag provenance; 103 holds OCR text that
/// came from the sidecar.
fn fixture_copy() -> (tempfile_dir::Dir, PathBuf) {
    let dir = tempfile_dir::Dir::new();
    let copy = dir.path().join("index.sqlite");
    let source =
        PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("tests/fixtures/sidecar-provenance.sqlite");
    std::fs::copy(source, &copy).expect("copy the committed fixture");
    (dir, copy)
}

/// A scratch directory removed on drop.
mod tempfile_dir {
    use std::path::{Path, PathBuf};

    pub struct Dir(PathBuf);

    impl Dir {
        pub fn new() -> Self {
            let nanos = std::time::SystemTime::now()
                .duration_since(std::time::UNIX_EPOCH)
                .expect("clock after epoch")
                .as_nanos();
            let path = std::env::temp_dir().join(format!(
                "notae-sidecar-fixture-{}-{nanos}",
                std::process::id()
            ));
            std::fs::create_dir_all(&path).expect("create scratch directory");
            Self(path)
        }

        pub fn path(&self) -> &Path {
            &self.0
        }
    }

    impl Drop for Dir {
        fn drop(&mut self) {
            let _ = std::fs::remove_dir_all(&self.0);
        }
    }
}

#[test]
fn opens_database_written_with_sidecar_provenance() {
    let (_dir, database) = fixture_copy();
    let store = IndexStore::open(&database).expect("an index written with sidecar rows opens");
    assert_eq!(store.schema_version().expect("version"), SCHEMA_VERSION);
    assert_eq!(store.note_count().expect("count"), 3);

    // Search over the imported composer finds the row that carries the
    // sidecar provenance.
    let hits = store
        .search_in("Kosma", SearchScope::Composers, 10)
        .expect("search by imported composer");
    assert_eq!(hits.len(), 1);
    let imported = &hits[0].note;
    assert_eq!(imported.metadata().id(), Some(NoteId::new(101)));
    assert_eq!(
        imported
            .metadata()
            .origin_of(MetadataField::ComposerOrArtist),
        MetadataOrigin::PcImported
    );
    assert_eq!(
        imported.metadata().origin_of(MetadataField::Tags),
        MetadataOrigin::PcImported
    );

    // Imported tags and OCR text are searchable too.
    let tagged = store
        .search("standard", 10)
        .expect("search by imported tag");
    assert_eq!(tagged.len(), 1);
    assert_eq!(tagged[0].note.path(), imported.path());

    let ocr_hits = store.search("sidecarfixture", 10).expect("search OCR text");
    let mut titles: Vec<_> = ocr_hits
        .iter()
        .map(|hit| hit.note.title().to_owned())
        .collect();
    titles.sort();
    assert_eq!(titles, ["Autumn Leaves", "Cantabile"]);
}
