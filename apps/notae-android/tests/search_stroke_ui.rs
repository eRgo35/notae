//! Search finds Polish text typed without `ł`, through the generated UI.
//!
//! The Search screen is driven with key events, over a real in-memory
//! `IndexStore` holding the score `Łódź — Pieśń żółta` (issue #35). The test
//! wires `search_changed`/`search_scope_changed` to the view model and row
//! projection the Android worker uses (`LibraryViewModel`, `library_row`) and
//! reads the results back from the screen's accessibility tree. It does not
//! prove that Samsung Keyboard or the worker thread deliver the same events.

mod support;

use std::{cell::Cell, rc::Rc};

use i_slint_backend_testing::ElementHandle;
use notae_android::ui::library::LibraryViewModel;
use notae_android::ui::search::library_row;
use notae_android::{AppWindow, LibraryRow};
use notae_domain::{
    IndexedNote, MetadataOrigin, NoteId, NoteMetadata, PdfFingerprint, RelativePdfPath,
    Sha256Digest,
};
use notae_index::{IndexStore, OcrText, SearchScope};
use slint::{ComponentHandle, ModelRc, VecModel};
use support::{app, focus_search_field, type_text};

const LODZ_TITLE: &str = "Łódź — Pieśń żółta";
const LODZ_LYRICS: &str =
    "Pałka zagrała kołysankę dla Gałuszki. Części stałe mszy: zmartwychwstał Pan.";

fn note(
    path: &str,
    seed: u8,
    id: u32,
    title: &str,
    composer: Option<&str>,
    tags: &[&str],
) -> IndexedNote {
    let mut metadata = NoteMetadata::new(
        title,
        composer.map(str::to_owned),
        tags.iter().map(|tag| (*tag).to_owned()),
    )
    .expect("valid title");
    metadata.assign_id(Some(NoteId::new(id)), MetadataOrigin::FilenameDerived);
    IndexedNote::new(
        RelativePdfPath::new(path).expect("valid relative path"),
        PdfFingerprint::new(Sha256Digest::from_bytes([seed; 32]), 1_024, None),
        metadata,
    )
}

fn library() -> Rc<IndexStore> {
    let store = IndexStore::open(":memory:").expect("open in-memory index");
    store
        .upsert_note_with_ocr(
            &note(
                "lodz.pdf",
                1,
                303,
                LODZ_TITLE,
                Some("Stanisław Moniuszko"),
                &["kołysanka"],
            ),
            Some(&OcrText::new(LODZ_LYRICS)),
        )
        .expect("insert");
    store
        .upsert_note_with_ocr(
            &note("ave.pdf", 2, 13, "Ave Maria", Some("Schubert"), &["solo"]),
            Some(&OcrText::new("Ave Maria gratia plena")),
        )
        .expect("insert");
    Rc::new(store)
}

/// What the Android worker does for a query: search, project, publish the rows.
fn publish(app: &AppWindow, store: &IndexStore, scope: SearchScope, query: &str) {
    let mut model = LibraryViewModel::new(store);
    model.set_query(query.to_owned());
    model.set_scope(scope);
    let rows: Vec<LibraryRow> = model
        .search()
        .expect("search the index")
        .into_iter()
        .map(|row| library_row(row, None))
        .collect();
    app.set_library_rows(ModelRc::new(VecModel::from(rows)));
}

fn open_search(store: &Rc<IndexStore>) -> AppWindow {
    let app = app();
    app.set_show_search(true);
    let scope = Rc::new(Cell::new(SearchScope::Everything));
    {
        let (weak, store, scope) = (app.as_weak(), Rc::clone(store), Rc::clone(&scope));
        app.on_search_changed(move |query| {
            if let Some(app) = weak.upgrade() {
                publish(&app, &store, scope.get(), &query);
            }
        });
    }
    {
        let (weak, store, scope) = (app.as_weak(), Rc::clone(store), Rc::clone(&scope));
        app.on_search_scope_changed(move |key| {
            let Some(app) = weak.upgrade() else { return };
            scope.set(SearchScope::from_key(&key));
            app.set_search_scope(key);
            publish(&app, &store, scope.get(), app.get_search_text().as_str());
        });
    }
    slint::platform::update_timers_and_animations();
    app
}

/// `(title, snippet)` of every result row on the Search screen, best first.
fn results(app: &AppWindow) -> Vec<(String, String)> {
    ElementHandle::find_by_element_type_name(app, "ResultRow")
        .map(|row| {
            (
                row.accessible_label().unwrap_or_default().to_string(),
                row.accessible_description().unwrap_or_default().to_string(),
            )
        })
        .collect()
}

/// Picks the scope chip, then types `query` into the Search field.
fn search(store: &Rc<IndexStore>, chip: &str, query: &str) -> Vec<(String, String)> {
    let app = open_search(store);
    ElementHandle::find_by_accessible_label(&app, chip)
        .next()
        .unwrap_or_else(|| panic!("`{chip}` scope chip is exposed"))
        .invoke_accessible_default_action();
    focus_search_field(&app);
    type_text(&app, query);
    slint::platform::update_timers_and_animations();
    results(&app)
}

fn titles(found: &[(String, String)]) -> Vec<&str> {
    found.iter().map(|(title, _)| title.as_str()).collect()
}

#[test]
fn queries_with_and_without_the_stroke_find_the_score_in_every_scope() {
    let store = library();
    // (scope chip, queries); every query must show exactly the Łódź score.
    let cases: [(&str, &[&str]); 5] = [
        (
            "Titles",
            &["lodz", "Lodz", "ŁÓDŹ", "łódź", "zolta piesn", "żółta"],
        ),
        ("Composers", &["stanislaw", "STANISŁAW", "Stanisław"]),
        ("Tags", &["kolysanka", "kołysanka"]),
        (
            "Lyrics",
            &[
                "palka",
                "Pałka",
                "galuszki",
                "czesci stale",
                "części stałe",
                "zmartwychwstal",
                "ZMARTWYCHWSTAŁ",
            ],
        ),
        (
            "All",
            &[
                "lodz",
                "stanislaw",
                "kolysanka",
                "palka",
                "zmartwychwstal",
                "Łódź",
            ],
        ),
    ];
    for (chip, queries) in cases {
        for query in queries {
            let found = search(&store, chip, query);
            assert_eq!(titles(&found), [LODZ_TITLE], "{query:?} under {chip}");
        }
    }
}

#[test]
fn lyric_results_show_the_stored_spelling_not_the_folded_one() {
    let store = library();
    // (query, the word as printed in the lyrics)
    for (query, printed) in [
        ("palka", "Pałka"),
        ("galuszki", "Gałuszki"),
        ("czesci stale", "stałe"),
        ("zmartwychwstal", "zmartwychwstał"),
    ] {
        let found = search(&store, "Lyrics", query);
        assert_eq!(titles(&found), [LODZ_TITLE], "{query:?}");
        let snippet = &found[0].1;
        assert!(snippet.contains(printed), "{query:?} -> {snippet:?}");
        let folded = printed.replace('ł', "l").replace('Ł', "L");
        assert!(!snippet.contains(&folded), "{query:?} -> {snippet:?}");
        assert!(
            !snippet.contains(['\u{2}', '\u{3}']),
            "markers leaked: {snippet:?}"
        );
    }
}

#[test]
fn folding_does_not_leak_across_scopes_and_the_control_score_stays_findable() {
    let store = library();
    assert!(search(&store, "Composers", "lodz").is_empty());
    assert!(search(&store, "Titles", "palka").is_empty());
    assert!(search(&store, "Tags", "stanislaw").is_empty());
    assert_eq!(titles(&search(&store, "All", "ave")), ["Ave Maria"]);
}
