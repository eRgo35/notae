//! `UiModel` crosses threads once a UniFFI object owns it, so it has to be `Send`.

#[test]
fn ui_model_is_send() {
    fn assert_send<T: Send>() {}
    assert_send::<notae_app_core::UiModel>();
}
