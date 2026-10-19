use i_slint_backend_testing::ElementHandle;
use notae_android::AppWindow;
use slint::platform::{PointerEventButton, WindowEvent};
use slint::{ComponentHandle, LogicalPosition, SharedString};
use std::cell::Cell;

thread_local! {
    static TEST_BACKEND_INITIALIZED: Cell<bool> = const { Cell::new(false) };
}

/// Accessible label of the Search field.
#[allow(dead_code)] // Shared by every test binary; only Search tests need it.
pub const SEARCH_LABEL: &str = "Search titles, composers, tags or lyrics";

fn initialize_backend() {
    TEST_BACKEND_INITIALIZED.with(|initialized| {
        if !initialized.replace(true) {
            i_slint_backend_testing::init_no_event_loop();
        }
    });
}

#[allow(dead_code)] // Shared by every test binary; some only need `app_sized`.
pub fn app() -> AppWindow {
    app_sized(1280, 800)
}

pub fn app_sized(width: u32, height: u32) -> AppWindow {
    initialize_backend();
    let app = AppWindow::new().expect("generated AppWindow initializes");
    app.window()
        .set_size(slint::LogicalSize::new(width as f32, height as f32));
    app.set_show_setup(false);
    app.set_show_recovery(false);
    app.set_startup_ready(true);
    app.set_library_display_name("Test library".into());
    app
}

/// Clicks the centre of the Search field so it holds keyboard focus.
#[allow(dead_code)]
pub fn focus_search_field(app: &AppWindow) {
    let field = ElementHandle::find_by_accessible_label(app, SEARCH_LABEL)
        .next()
        .expect("Search field is mounted");
    let position = field.absolute_position();
    let size = field.size();
    let center = LogicalPosition::new(
        position.x + size.width / 2.0,
        position.y + size.height / 2.0,
    );
    app.window()
        .dispatch_event(WindowEvent::PointerMoved { position: center });
    app.window().dispatch_event(WindowEvent::PointerPressed {
        position: center,
        button: PointerEventButton::Left,
    });
    app.window().dispatch_event(WindowEvent::PointerReleased {
        position: center,
        button: PointerEventButton::Left,
    });
}

/// Types `text` one key press at a time, as a hardware keyboard would.
#[allow(dead_code)]
pub fn type_text(app: &AppWindow, text: &str) {
    for character in text.chars() {
        let key = SharedString::from(character.to_string());
        app.window()
            .dispatch_event(WindowEvent::KeyPressed { text: key.clone() });
        app.window()
            .dispatch_event(WindowEvent::KeyReleased { text: key });
    }
}
