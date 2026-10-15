use notae_android::ui::browse::{ComposerCount, LibrarySection, composers, section_rows, sections};
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
        LibrarySection::NeedsDetails,
        LibrarySection::TextPending,
        LibrarySection::Composer("Gabriel Fauré".to_owned()),
    ] {
        assert_eq!(LibrarySection::from_key(&section.key()), section);
    }
}

#[test]
fn unknown_or_empty_keys_fall_back_to_all() {
    assert_eq!(LibrarySection::from_key("nonsense"), LibrarySection::All);
    assert_eq!(LibrarySection::from_key("composer:"), LibrarySection::All);
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
fn needs_details_means_missing_number_or_composer() {
    let titles: Vec<String> = section_rows(&library(), &LibrarySection::NeedsDetails, &[])
        .into_iter()
        .map(|row| row.title)
        .collect();
    assert_eq!(titles, ["Scarborough Fair", "Untitled sketch"]);
}

#[test]
fn text_pending_and_composer_sections_filter_rows() {
    let pending = section_rows(&library(), &LibrarySection::TextPending, &[]);
    assert_eq!(pending.len(), 1);
    assert_eq!(pending[0].title, "Cantique");

    let faure = section_rows(
        &library(),
        &LibrarySection::Composer("GABRIEL FAURÉ".to_owned()),
        &[],
    );
    assert_eq!(faure.len(), 2);
}

#[test]
fn recent_section_follows_recent_order_and_skips_unknown_keys() {
    let recent = vec!["d".repeat(64), "z".repeat(64), "a".repeat(64)];
    let titles: Vec<String> = section_rows(&library(), &LibrarySection::Recent, &recent)
        .into_iter()
        .map(|row| row.title)
        .collect();
    assert_eq!(titles, ["Pavane", "Clair de lune"]);
}

#[test]
fn fixed_sections_report_counts() {
    let recent = vec!["a".repeat(64)];
    let views = sections(&library(), &recent);
    let summary: Vec<(String, usize)> = views
        .into_iter()
        .map(|view| (view.key, view.count))
        .collect();
    assert_eq!(
        summary,
        [
            ("all".to_owned(), 5),
            ("recent".to_owned(), 1),
            ("needs-details".to_owned(), 2),
            ("text-pending".to_owned(), 1),
        ]
    );
}
