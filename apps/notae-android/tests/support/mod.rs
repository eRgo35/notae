use notae_android::AppWindow;
use slint::ComponentHandle;
use std::cell::Cell;

thread_local! {
    static TEST_BACKEND_INITIALIZED: Cell<bool> = const { Cell::new(false) };
}

fn initialize_backend() {
    TEST_BACKEND_INITIALIZED.with(|initialized| {
        if !initialized.replace(true) {
            i_slint_backend_testing::init_no_event_loop();
        }
    });
}

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
