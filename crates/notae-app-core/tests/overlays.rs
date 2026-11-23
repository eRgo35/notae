//! Drawings on set pieces: one file per set item in Notae's private storage.

use notae_app_core::overlays::{Overlay, OverlayStore, PageOverlay, Stroke, TextBox, Tool};

fn drawing(content_key: &str) -> Overlay {
    Overlay {
        content_key: content_key.to_owned(),
        pages: vec![PageOverlay {
            page: 0,
            strokes: vec![Stroke {
                tool: Tool::Pen,
                color: 0xFF00_0000,
                width: 0.003,
                points: vec![0.1, 0.2, 0.3, 0.4],
            }],
            texts: vec![TextBox {
                x: 0.5,
                y: 0.5,
                text: "p".to_owned(),
                size: 0.025,
            }],
        }],
    }
}

#[test]
fn overlay_round_trips_through_its_file() {
    let dir = tempfile::tempdir().expect("temporary directory");
    let store = OverlayStore::new(dir.path().join("overlays"));
    store.put("set1", "item1", &drawing("aa")).expect("saved");
    let file = dir.path().join("overlays").join("set1").join("item1.json");
    let text = std::fs::read_to_string(&file).expect("file");
    assert!(text.contains("\"schema_version\":1"), "{text}");
    assert_eq!(store.get("set1", "item1"), Some(drawing("aa")));
    assert!(store.has("set1", "item1"));
    assert_eq!(
        store.all_for("set1"),
        vec![("item1".to_owned(), drawing("aa"))]
    );
}

#[test]
fn an_empty_overlay_deletes_the_file() {
    let dir = tempfile::tempdir().expect("temporary directory");
    let store = OverlayStore::new(dir.path().to_path_buf());
    store.put("set1", "item1", &drawing("aa")).expect("saved");
    let empty = Overlay {
        content_key: "aa".to_owned(),
        pages: vec![PageOverlay {
            page: 0,
            strokes: Vec::new(),
            texts: Vec::new(),
        }],
    };
    store.put("set1", "item1", &empty).expect("removed");
    assert!(!dir.path().join("set1").join("item1.json").exists());
    assert!(!store.has("set1", "item1"));
}

#[test]
fn removing_an_item_or_a_set_deletes_its_files() {
    let dir = tempfile::tempdir().expect("temporary directory");
    let store = OverlayStore::new(dir.path().to_path_buf());
    store.put("set1", "item1", &drawing("aa")).expect("saved");
    store.put("set1", "item2", &drawing("bb")).expect("saved");
    store.remove_item("set1", "item1");
    assert_eq!(store.get("set1", "item1"), None);
    assert!(store.has("set1", "item2"));
    store.remove_set("set1");
    assert!(!dir.path().join("set1").exists());
}

#[test]
fn a_damaged_overlay_file_reads_as_none() {
    let dir = tempfile::tempdir().expect("temporary directory");
    let store = OverlayStore::new(dir.path().to_path_buf());
    std::fs::create_dir_all(dir.path().join("set1")).expect("dir");
    std::fs::write(dir.path().join("set1").join("item1.json"), "{not json").expect("write");
    assert_eq!(store.get("set1", "item1"), None);
}

#[test]
fn ids_cannot_reach_outside_the_overlay_folder() {
    let dir = tempfile::tempdir().expect("temporary directory");
    let store = OverlayStore::new(dir.path().join("overlays"));
    assert!(store.put("..", "x", &drawing("aa")).is_err());
    assert!(store.put("set1", "../x", &drawing("aa")).is_err());
}
