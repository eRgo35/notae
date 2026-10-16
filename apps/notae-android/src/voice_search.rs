//! Host-testable decisions for results returned by Android speech recognition.

/// Returns the first nonblank recognized phrase only for a successful activity result.
#[must_use]
pub fn recognized_query(result_ok: bool, results: &[String]) -> Option<&str> {
    if !result_ok {
        return None;
    }
    results
        .iter()
        .map(String::as_str)
        .find(|phrase| !phrase.trim().is_empty())
}

/// Whether a result belongs to the active voice request and a live UI window.
#[must_use]
pub fn voice_result_is_current(
    request_code: i32,
    voice_request_code: i32,
    window_alive: bool,
) -> bool {
    request_code == voice_request_code && window_alive
}
