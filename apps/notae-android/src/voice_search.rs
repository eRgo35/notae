//! Host-testable routing and application of Android speech-recognition results.

use crate::AppWindow;

/// Reserved in the Activity result-code namespace for voice recognition.
///
/// Keep in sync with `NotaeActivity.VOICE_REQUEST_CODE`.
pub const VOICE_REQUEST_CODE: i32 = 0x4e4f;

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

/// Applies a current voice result on the Slint event-loop thread.
///
/// Returns `true` only when the query was replaced and search was requested.
pub fn apply_voice_result(
    app: &AppWindow,
    request_code: i32,
    result_ok: bool,
    results: &[String],
) -> bool {
    if request_code != VOICE_REQUEST_CODE {
        return false;
    }
    let Some(query) = recognized_query(result_ok, results) else {
        return false;
    };
    let query = slint::SharedString::from(query);
    app.set_search_text(query.clone());
    app.invoke_search_changed(query);
    true
}

