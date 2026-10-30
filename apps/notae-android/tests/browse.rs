use std::sync::Mutex;

use notae_android::favorites::PieceRef;
use notae_android::language::set_active_bundle;
use notae_android::sets::{SetFields, SetStore};
use notae_android::ui::UiModel;
use notae_android::ui::browse::{
    ComposerCount, LibrarySection, SectionContext, SetEntry, composers, format_edited,
    format_set_date, neighbours, section_rows, sections, set_entries, set_subtitle,
};
use notae_android::ui::search::SearchRow;

fn row(
    key: char,
    id: Option<&str>,
    title: &str,
    composer: Option<&str>,
    pending: bool,
) -> SearchRow {
    SearchRow {
        id: id.map(str::to_owned),
        title: title.to_owned(),
        composer_or_artist: composer.map(str::to_owned),
        tags: Vec::new(),
        status: if pending { "OCR pending" } else { "Ready" }.to_owned(),
        relative_path: format!("{title}.pdf"),
        note_key: key.to_string().repeat(64),
        text_pending: pending,
        title_origin: "From filename".to_owned(),
        composer_origin: String::new(),
        text_snippet: None,
        item_id: None,
        position: None,
        missing: false,
    }
}

fn library() -> Vec<SearchRow> {
    vec![
        row(
            'a',
            Some("12"),
            "Clair de lune",
            Some("Claude Debussy"),
            false,
        ),
        row('b', Some("14"), "Cantique", Some("Gabriel Fauré"), true),
        row('c', None, "Scarborough Fair", Some("traditional"), false),
        row('d', Some("17"), "Pavane", Some("gabriel fauré"), false),
        row('e', Some("20"), "Untitled sketch", None, false),
    ]
}

#[test]
fn section_keys_round_trip() {
    for section in [
        LibrarySection::All,
        LibrarySection::Recent,
        LibrarySection::Favorites,
        LibrarySection::Sets,
        LibrarySection::Set("0123456789abcdef".to_owned()),
        LibrarySection::Composer("Gabriel Fauré".to_owned()),
    ] {
        assert_eq!(LibrarySection::from_key(&section.key()), section);
    }
}

#[test]
fn unknown_or_empty_keys_fall_back_to_all() {
    assert_eq!(LibrarySection::from_key("nonsense"), LibrarySection::All);
    assert_eq!(LibrarySection::from_key("composer:"), LibrarySection::All);
    // Keys of the removed sections (for example from an older saved selection) mean All.
    for removed in ["needs-details", "text-pending"] {
        assert_eq!(LibrarySection::from_key(removed), LibrarySection::All);
    }
}

#[test]
fn composers_merge_case_variants_and_sort_by_name() {
    assert_eq!(
        composers(&library()),
        vec![
            ComposerCount {
                name: "Claude Debussy".to_owned(),
                count: 1
            },
            ComposerCount {
                name: "Gabriel Fauré".to_owned(),
                count: 2
            },
            ComposerCount {
                name: "traditional".to_owned(),
                count: 1
            },
        ]
    );
}

#[test]
fn composer_sections_filter_rows_case_insensitively() {
    let faure = section_rows(
        &library(),
        &LibrarySection::Composer("GABRIEL FAURÉ".to_owned()),
        &SectionContext::default(),
    );
    assert_eq!(faure.len(), 2);
}

#[test]
fn rows_without_a_number_composer_or_text_stay_in_the_library() {
    let all = section_rows(&library(), &LibrarySection::All, &SectionContext::default());
    assert_eq!(all.len(), 5, "no row is hidden for missing details or text");
}

#[test]
fn recent_section_follows_recent_order_and_skips_unknown_keys() {
    let recent = vec!["d".repeat(64), "z".repeat(64), "a".repeat(64)];
    let titles: Vec<String> = section_rows(
        &library(),
        &LibrarySection::Recent,
        &SectionContext {
            recent: &recent,
            favorites: &[],
            ..SectionContext::default()
        },
    )
    .into_iter()
    .map(|row| row.title)
    .collect();
    assert_eq!(titles, ["Pavane", "Clair de lune"]);
}

#[test]
fn fixed_sections_report_counts() {
    let recent = vec!["a".repeat(64)];
    let views = sections(
        &library(),
        &SectionContext {
            recent: &recent,
            favorites: &[],
            ..SectionContext::default()
        },
    );
    let summary: Vec<(String, usize)> = views
        .into_iter()
        .map(|view| (view.key, view.count))
        .collect();
    assert_eq!(
        summary,
        [
            ("all".to_owned(), 5),
            ("recent".to_owned(), 1),
            ("favorites".to_owned(), 0),
            ("sets".to_owned(), 0),
        ]
    );
}

fn piece(key: char, title: &str) -> PieceRef {
    PieceRef {
        key: key.to_string().repeat(64),
        path: format!("{title}.pdf"),
    }
}

#[test]
fn favorites_section_lists_newest_first_and_skips_missing() {
    // `Favorites::items` is newest first; a piece no longer in the library is left out.
    let favorites = vec![
        piece('d', "Pavane"),
        piece('z', "Gone"),
        piece('a', "Clair de lune"),
    ];
    let context = SectionContext {
        recent: &[],
        favorites: &favorites,
        ..SectionContext::default()
    };
    let titles: Vec<String> = section_rows(&library(), &LibrarySection::Favorites, &context)
        .into_iter()
        .map(|row| row.title)
        .collect();
    assert_eq!(titles, ["Pavane", "Clair de lune"]);
}

#[test]
fn sections_include_favorites_with_count() {
    let favorites = vec![piece('b', "Cantique"), piece('z', "Gone")];
    let views = sections(
        &library(),
        &SectionContext {
            recent: &[],
            favorites: &favorites,
            ..SectionContext::default()
        },
    );
    let summary: Vec<(String, usize, &str)> = views
        .into_iter()
        .map(|view| (view.key, view.count, view.kind))
        .collect();
    assert_eq!(
        summary,
        [
            ("all".to_owned(), 5, "library"),
            ("recent".to_owned(), 0, "schedule"),
            ("favorites".to_owned(), 1, "star"),
            ("sets".to_owned(), 0, "folder"),
        ]
    );
}

#[test]
fn favorites_key_round_trips() {
    assert_eq!(LibrarySection::Favorites.key(), "favorites");
    assert_eq!(
        LibrarySection::from_key("favorites"),
        LibrarySection::Favorites
    );
}

fn keys(rows: &[SearchRow]) -> Vec<String> {
    rows.iter().map(|row| row.note_key.clone()).collect()
}

#[test]
fn neighbours_follow_row_order_without_wrapping() {
    let rows = library();
    let keys = keys(&rows);

    let first = neighbours(&rows, &keys[0]).expect("first row is present");
    assert_eq!(
        (first.previous, first.next.as_deref()),
        (None, Some(&*keys[1]))
    );
    assert_eq!((first.position, first.total), (1, 5));

    let middle = neighbours(&rows, &keys[2]).expect("middle row is present");
    assert_eq!(middle.previous.as_deref(), Some(&*keys[1]));
    assert_eq!(middle.next.as_deref(), Some(&*keys[3]));

    let last = neighbours(&rows, &keys[4]).expect("last row is present");
    assert_eq!(
        (last.previous.as_deref(), last.next),
        (Some(&*keys[3]), None)
    );
    assert_eq!((last.position, last.total), (5, 5));
}

#[test]
fn neighbours_of_a_single_or_missing_row() {
    let rows = library();
    assert!(neighbours(&[], "a").is_none());
    assert!(neighbours(&rows, &"z".repeat(64)).is_none());
    let single = neighbours(&rows[..1], &rows[0].note_key).expect("row is present");
    assert_eq!((single.previous, single.next), (None, None));
    assert_eq!((single.position, single.total), (1, 1));
}

fn model_with_library() -> UiModel {
    let mut model = UiModel::new();
    model.set_search_rows(library());
    model
}

#[test]
fn preview_neighbours_follow_the_current_section_not_the_whole_library() {
    let mut model = model_with_library();
    model
        .select_section(LibrarySection::Composer("Gabriel Fauré".to_owned()))
        .expect("no preferences file to save");
    let pavane = "d".repeat(64);
    model.select_note(Some(pavane.clone()));
    let nav = model
        .preview_neighbours()
        .expect("selection is in the section");
    assert_eq!(nav.previous.as_deref(), Some("b".repeat(64).as_str()));
    assert_eq!(nav.next, None, "Pavane is last among the Fauré scores");
    assert_eq!((nav.position, nav.total), (2, 2));
}

#[test]
fn preview_neighbours_are_absent_when_the_selection_is_outside_the_section() {
    // For example a Search result opened while another section is current.
    let mut model = model_with_library();
    model
        .select_section(LibrarySection::Composer("traditional".to_owned()))
        .expect("no preferences file to save");
    model.select_note(Some("a".repeat(64)));
    assert!(
        model.selected_row().is_some(),
        "the preview itself still shows"
    );
    assert!(model.preview_neighbours().is_none());
}

#[test]
fn removed_selection_closes_the_preview_and_is_not_resurrected() {
    let mut model = model_with_library();
    let key = "b".repeat(64);
    model.select_note(Some(key.clone()));
    let mut remaining = library();
    remaining.retain(|row| row.note_key != key);
    model.set_search_rows(remaining);
    assert!(model.selected_row().is_none());
    // The score returns after a later scan: the old selection must not reopen on its own.
    model.set_search_rows(library());
    assert!(model.selected_row().is_none());
}

#[test]
fn empty_library_has_no_preview() {
    let mut model = model_with_library();
    model.select_note(Some("a".repeat(64)));
    model.set_search_rows(Vec::new());
    assert!(model.selected_row().is_none());
    assert!(model.preview_neighbours().is_none());
}

/// The active language is process-wide; tests that switch it take turns.
static LANGUAGE: Mutex<()> = Mutex::new(());

fn set_fields(name: &str) -> SetFields {
    SetFields {
        name: name.to_owned(),
        date: None,
        notes: None,
    }
}

#[test]
fn sets_section_counts_sets() {
    let mut store = SetStore::default();
    store.create(set_fields("Wszystkich Świętych"), 10);
    store.create(set_fields("Dzień Zaduszny"), 20);
    let context = SectionContext {
        sets: &store,
        ..SectionContext::default()
    };
    let summary: Vec<(String, usize, &str)> = sections(&library(), &context)
        .into_iter()
        .map(|view| (view.key, view.count, view.kind))
        .collect();
    assert_eq!(
        summary,
        [
            ("all".to_owned(), 5, "library"),
            ("recent".to_owned(), 0, "schedule"),
            ("favorites".to_owned(), 0, "star"),
            ("sets".to_owned(), 2, "folder"),
        ]
    );
    // The Sets list shows folder cards, not score rows.
    assert!(section_rows(&library(), &LibrarySection::Sets, &context).is_empty());
}

#[test]
fn set_key_round_trips() {
    assert_eq!(LibrarySection::Sets.key(), "sets");
    assert_eq!(LibrarySection::from_key("sets"), LibrarySection::Sets);
    let set = LibrarySection::Set("0123456789abcdef".to_owned());
    assert_eq!(set.key(), "set:0123456789abcdef");
    assert_eq!(LibrarySection::from_key("set:0123456789abcdef"), set);
    assert_eq!(LibrarySection::from_key("set:"), LibrarySection::All);
}

const NOV_1_2026: u64 = 1_793_491_200;
const DAY: u64 = 86_400;

#[test]
fn format_edited_buckets() {
    let _language = LANGUAGE
        .lock()
        .unwrap_or_else(std::sync::PoisonError::into_inner);
    let now = NOV_1_2026 + 30 * DAY + 12 * 3600;
    let today = now - 3600;
    let yesterday = now - DAY;
    let three_days = now - 3 * DAY;
    let long_ago = NOV_1_2026 + 600;

    set_active_bundle("");
    assert_eq!(format_edited(today, now), "Edited today");
    assert_eq!(format_edited(yesterday, now), "Edited yesterday");
    assert_eq!(format_edited(three_days, now), "Edited 3 days ago");
    assert_eq!(format_edited(long_ago, now), "Edited 1 Nov 2026");
    // A clock that went backwards is not "in the future".
    assert_eq!(format_edited(now + DAY, now), "Edited today");

    set_active_bundle("pl");
    assert_eq!(format_edited(today, now), "Edytowano dziś");
    assert_eq!(format_edited(yesterday, now), "Edytowano wczoraj");
    assert_eq!(format_edited(three_days, now), "Edytowano 3 dni temu");
    assert_eq!(format_edited(long_ago, now), "Edytowano 1 lis 2026");
    set_active_bundle("");
}

#[test]
fn format_set_date_en_and_pl() {
    let _language = LANGUAGE
        .lock()
        .unwrap_or_else(std::sync::PoisonError::into_inner);
    set_active_bundle("");
    assert_eq!(format_set_date("2026-11-01"), "1 Nov 2026");
    assert_eq!(format_set_date("2027-05-24"), "24 May 2027");
    set_active_bundle("pl");
    assert_eq!(format_set_date("2026-11-01"), "1 lis 2026");
    assert_eq!(format_set_date("2027-05-24"), "24 maj 2027");
    set_active_bundle("");
    // Anything that is not a stored ISO date is shown as written.
    assert_eq!(format_set_date("soon"), "soon");
}

/// A store with one set holding `items` (a key letter, or `None` for a note), in order.
fn store_with(items: &[Option<char>]) -> (SetStore, String) {
    let mut store = SetStore::default();
    let id = store.create(set_fields("Wszystkich Świętych"), 10);
    for item in items {
        match item {
            Some(key) => {
                let path = library()
                    .into_iter()
                    .find(|row| row.note_key.starts_with(*key))
                    .map_or_else(|| "Psalmy/Psalm 23.pdf".to_owned(), |row| row.relative_path);
                store.add_piece(&id, piece_at(*key, &path), 10);
            }
            None => {
                store.add_note(&id, "Psalm: kantor śpiewa wersety".to_owned(), 10);
            }
        }
    }
    (store, id)
}

fn piece_at(key: char, path: &str) -> PieceRef {
    PieceRef {
        key: key.to_string().repeat(64),
        path: path.to_owned(),
    }
}

fn item_ids(store: &SetStore, id: &str) -> Vec<String> {
    store
        .get(id)
        .expect("set exists")
        .items
        .iter()
        .map(|item| item.id.clone())
        .collect()
}

#[test]
fn set_rows_follow_set_order_with_repeats() {
    let (store, id) = store_with(&[Some('b'), Some('a'), Some('b')]);
    let context = SectionContext {
        sets: &store,
        ..SectionContext::default()
    };
    let rows = section_rows(&library(), &LibrarySection::Set(id.clone()), &context);
    let titles: Vec<&str> = rows.iter().map(|row| row.title.as_str()).collect();
    assert_eq!(titles, ["Cantique", "Clair de lune", "Cantique"]);
    let ids: Vec<String> = rows.iter().filter_map(|row| row.item_id.clone()).collect();
    assert_eq!(
        ids,
        item_ids(&store, &id),
        "each copy carries its own item id"
    );
    assert!(rows.iter().all(|row| !row.missing));
    // Outside a set, rows carry no item id.
    assert!(
        section_rows(&library(), &LibrarySection::All, &context)
            .iter()
            .all(|row| row.item_id.is_none())
    );
    // A set that does not exist has no rows.
    assert!(
        section_rows(
            &library(),
            &LibrarySection::Set("ffff".to_owned()),
            &context
        )
        .is_empty()
    );
}

#[test]
fn missing_piece_becomes_placeholder_in_place() {
    let _language = LANGUAGE
        .lock()
        .unwrap_or_else(std::sync::PoisonError::into_inner);
    set_active_bundle("");
    let (store, id) = store_with(&[Some('a'), Some('f'), Some('c')]);
    let context = SectionContext {
        sets: &store,
        ..SectionContext::default()
    };
    let rows = section_rows(&library(), &LibrarySection::Set(id.clone()), &context);
    let titles: Vec<&str> = rows.iter().map(|row| row.title.as_str()).collect();
    assert_eq!(titles, ["Clair de lune", "Psalm 23", "Scarborough Fair"]);
    let missing = &rows[1];
    assert!(missing.missing);
    assert_eq!(missing.status, "Missing from library");
    assert_eq!(missing.note_key, "f".repeat(64));
    assert_eq!(missing.relative_path, "Psalmy/Psalm 23.pdf");
    assert_eq!(missing.item_id.as_ref(), Some(&item_ids(&store, &id)[1]));
    assert!(!rows[0].missing && !rows[2].missing);

    set_active_bundle("pl");
    let rows = section_rows(&library(), &LibrarySection::Set(id), &context);
    assert_eq!(rows[1].status, "Brak w bibliotece");
    set_active_bundle("");
}

#[test]
fn set_entries_number_notes_and_pieces() {
    let (store, id) = store_with(&[Some('a'), None, Some('f'), Some('a')]);
    let set = store.get(&id).expect("set exists");
    let ids = item_ids(&store, &id);
    let entries = set_entries(&library(), set);
    assert_eq!(entries.len(), 4);
    match &entries[0] {
        SetEntry::Piece { position, row } => {
            assert_eq!(*position, 1);
            assert_eq!(row.title, "Clair de lune");
            assert_eq!(row.item_id.as_ref(), Some(&ids[0]));
        }
        other => panic!("first entry is a piece: {other:?}"),
    }
    assert_eq!(
        entries[1],
        SetEntry::Note {
            position: 2,
            item_id: ids[1].clone(),
            text: "Psalm: kantor śpiewa wersety".to_owned(),
        }
    );
    match &entries[2] {
        SetEntry::Piece { position, row } => {
            assert_eq!(*position, 3);
            assert!(row.missing, "a missing piece keeps its number");
        }
        other => panic!("third entry is a piece: {other:?}"),
    }
    match &entries[3] {
        SetEntry::Piece { position, row } => {
            assert_eq!(*position, 4);
            assert_eq!(row.item_id.as_ref(), Some(&ids[3]));
        }
        other => panic!("fourth entry is a piece: {other:?}"),
    }
}

#[test]
fn set_subtitle_joins_date_and_counts() {
    let _language = LANGUAGE
        .lock()
        .unwrap_or_else(std::sync::PoisonError::into_inner);
    let (mut store, id) =
        store_with(&[Some('a'), None, Some('b'), Some('c'), Some('d'), Some('e')]);
    store.update(
        &id,
        SetFields {
            name: "Wszystkich Świętych".to_owned(),
            date: Some("2026-11-01".to_owned()),
            notes: None,
        },
        10,
    );
    set_active_bundle("");
    assert_eq!(
        set_subtitle(store.get(&id).expect("set")),
        "1 Nov 2026 · 5 pieces · 1 note"
    );
    set_active_bundle("pl");
    assert_eq!(
        set_subtitle(store.get(&id).expect("set")),
        "1 lis 2026 · 5 utworów · 1 notatka"
    );

    // Zero counts and a missing date are left out.
    let (store, id) = store_with(&[Some('a'), Some('b')]);
    set_active_bundle("");
    assert_eq!(set_subtitle(store.get(&id).expect("set")), "2 pieces");
    set_active_bundle("pl");
    assert_eq!(set_subtitle(store.get(&id).expect("set")), "2 utwory");
    let (store, id) = store_with(&[]);
    assert_eq!(set_subtitle(store.get(&id).expect("set")), "");
    set_active_bundle("");
}

#[test]
fn neighbours_in_set_with_repeats_use_item_ids() {
    let (store, id) = store_with(&[Some('a'), Some('b'), Some('a')]);
    let context = SectionContext {
        sets: &store,
        ..SectionContext::default()
    };
    let rows = section_rows(&library(), &LibrarySection::Set(id.clone()), &context);
    let ids = item_ids(&store, &id);

    let second = neighbours(&rows, &ids[2]).expect("the second copy is present");
    assert_eq!(second.previous.as_deref(), Some(ids[1].as_str()));
    assert_eq!(second.next, None, "the second copy is last");
    assert_eq!((second.position, second.total), (3, 3));

    let first = neighbours(&rows, &ids[0]).expect("the first copy is present");
    assert_eq!(first.previous, None);
    assert_eq!(first.next.as_deref(), Some(ids[1].as_str()));
    assert_eq!((first.position, first.total), (1, 3));

    // Inside a set the note key no longer names a row: it would be ambiguous.
    assert!(neighbours(&rows, &"a".repeat(64)).is_none());
}

#[test]
fn neighbours_skip_missing_rows() {
    let (store, id) = store_with(&[Some('a'), Some('f'), Some('c')]);
    let context = SectionContext {
        sets: &store,
        ..SectionContext::default()
    };
    let rows = section_rows(&library(), &LibrarySection::Set(id.clone()), &context);
    let ids = item_ids(&store, &id);
    assert!(rows[1].missing);

    let first = neighbours(&rows, &ids[0]).expect("first piece is present");
    assert_eq!(first.next.as_deref(), Some(ids[2].as_str()));
    let last = neighbours(&rows, &ids[2]).expect("last piece is present");
    assert_eq!(last.previous.as_deref(), Some(ids[0].as_str()));
    assert_eq!(last.next, None);
}

#[test]
fn set_rows_carry_their_set_position() {
    let (store, id) = store_with(&[Some('a'), None, Some('f'), Some('a')]);
    let context = SectionContext {
        sets: &store,
        ..SectionContext::default()
    };
    let rows = section_rows(&library(), &LibrarySection::Set(id), &context);
    let positions: Vec<Option<usize>> = rows.iter().map(|row| row.position).collect();
    assert_eq!(
        positions,
        [Some(1), Some(3), Some(4)],
        "notes keep their number"
    );
    assert!(
        section_rows(&library(), &LibrarySection::All, &context)
            .iter()
            .all(|row| row.position.is_none())
    );
}
