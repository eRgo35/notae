use notae_android::voice_search::{recognized_query, voice_result_is_current};

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
fn unrelated_activity_request_is_not_a_voice_result() {
    assert!(!voice_result_is_current(42, 43, true));
}

#[test]
fn late_voice_result_after_window_destruction_is_ignored() {
    assert!(!voice_result_is_current(42, 42, false));
}
