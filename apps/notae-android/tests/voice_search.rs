mod support;

use notae_android::AppWindow;
use notae_android::voice_search::{apply_voice_result, recognized_query, VOICE_REQUEST_CODE};
use slint::ComponentHandle;

#[test]
fn canceled_recognition_does_not_replace_the_query() {
    assert_eq!(recognized_query(false, &["Chopin".into()]), None);
}

#[test]
fn successful_phrase_preserves_polish_text() {
    assert_eq!(
        recognized_query(true, &["Łódź pieśń".into()]),
        Some("Łódź pieśń")
    );
}

#[test]
fn successful_empty_result_does_not_replace_the_query() {
    assert_eq!(recognized_query(true, &[]), None);
    assert_eq!(recognized_query(true, &["  \n".into()]), None);
}

#[test]
fn skips_blank_candidates_and_uses_the_first_nonblank_phrase() {
    let candidates = ["  ".into(), "Chopin".into(), "Bach".into()];
    assert_eq!(recognized_query(true, &candidates), Some("Chopin"));
}

#[test]
fn rejected_voice_results_preserve_the_existing_search_query() {
    use std::{cell::RefCell, rc::Rc};

    let app = support::app();
    app.set_search_text("existing query".into());
    let searches = Rc::new(RefCell::new(Vec::new()));
    let searches_for_callback = Rc::clone(&searches);
    app.on_search_changed(move |query| searches_for_callback.borrow_mut().push(query.to_string()));

    let rejected_results = [
        (VOICE_REQUEST_CODE, false, vec!["ignored".to_owned()]),
        (VOICE_REQUEST_CODE, true, Vec::new()),
        (VOICE_REQUEST_CODE, true, vec![" \n".to_owned()]),
        (VOICE_REQUEST_CODE + 1, true, vec!["ignored".to_owned()]),
    ];
    for (request_code, result_ok, results) in rejected_results {
        assert!(!apply_voice_result(&app, request_code, result_ok, &results));
        assert_eq!(app.get_search_text().as_str(), "existing query");
        assert!(searches.borrow().is_empty());
    }
}

#[test]
fn successful_unicode_voice_result_updates_query_and_search_once() {
    use std::{cell::RefCell, rc::Rc};

    let app = support::app();
    app.set_search_text("old query".into());
    let searches = Rc::new(RefCell::new(Vec::new()));
    let searches_for_callback = Rc::clone(&searches);
    app.on_search_changed(move |query| searches_for_callback.borrow_mut().push(query.to_string()));

    assert!(apply_voice_result(
        &app,
        VOICE_REQUEST_CODE,
        true,
        &["Łódź pieśń".to_owned()]
    ));
    assert_eq!(app.get_search_text().as_str(), "Łódź pieśń");
    assert_eq!(searches.borrow().as_slice(), &["Łódź pieśń"]);
}

#[test]
fn a_destroyed_window_does_not_receive_a_late_voice_result() {
    use std::sync::{
        Arc,
        atomic::{AtomicBool, Ordering},
    };

    i_slint_backend_testing::init_integration_test_with_system_time();
    let delivered = Arc::new(AtomicBool::new(false));
    let delivered_in_loop = Arc::clone(&delivered);
    slint::spawn_local(async move {
        let app = AppWindow::new().expect("generated AppWindow initializes");
        let weak = app.as_weak();
        drop(app);
        weak.upgrade_in_event_loop(move |_| delivered_in_loop.store(true, Ordering::Relaxed))
            .expect("Slint event loop accepts the weak callback");

        slint::invoke_from_event_loop(move || {
            assert!(!delivered.load(Ordering::Relaxed));
            slint::quit_event_loop().expect("event loop stops after the late-result check");
        })
        .expect("Slint event loop accepts the completion callback");
    })
    .expect("Slint starts the integration test future");
    slint::run_event_loop().expect("Slint event loop exits cleanly");
}

#[test]
fn a_window_weak_reference_cannot_be_upgraded_from_a_foreign_thread() {
    let app = support::app();
    let weak = app.as_weak();
    let upgrade = std::thread::spawn(move || weak.upgrade().is_some())
        .join()
        .expect("thread exits normally");
    assert!(!upgrade);
}
