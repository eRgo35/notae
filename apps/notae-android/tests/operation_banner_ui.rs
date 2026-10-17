mod support;

use i_slint_backend_testing::ElementHandle;
use std::time::Duration;
use support::app;

fn banner_app(fraction: f32, animations_enabled: bool) -> notae_android::AppWindow {
    let app = app();
    app.set_animations_enabled(animations_enabled);
    app.set_operation_progress_text("Scanning library…".into());
    app.set_operation_busy(true);
    app.set_operation_fraction(fraction);
    // Change handlers (the Theme motion switch) run on the next tick.
    i_slint_backend_testing::mock_elapsed_time(Duration::from_millis(1));
    app
}

fn find(app: &notae_android::AppWindow, id: &str) -> Option<ElementHandle> {
    ElementHandle::find_by_element_id(app, id).next()
}

#[test]
fn indeterminate_progress_draws_no_fraction_fill() {
    // Regression for #30: an unknown total used to draw a fixed 30 % fill the instant a scan began.
    for animations_enabled in [true, false] {
        let app = banner_app(-1.0, animations_enabled);
        i_slint_backend_testing::mock_elapsed_time(Duration::from_millis(500));
        let fill = find(&app, "OperationBanner::fill").expect("determinate fill element exists");
        assert_eq!(
            fill.size().width,
            0.0,
            "no fraction is shown before real counts exist (animations {animations_enabled})"
        );
    }
}

#[test]
fn indeterminate_progress_sweeps_only_while_motion_is_enabled() {
    let app = banner_app(-1.0, true);
    let track = find(&app, "OperationBanner::track").expect("track exists");
    let sweep = find(&app, "OperationBanner::sweep").expect("moving indeterminate segment exists");
    i_slint_backend_testing::mock_elapsed_time(Duration::from_millis(300));
    let first = sweep.absolute_position().x;
    i_slint_backend_testing::mock_elapsed_time(Duration::from_millis(300));
    let second = sweep.absolute_position().x;
    assert_ne!(
        first, second,
        "the segment moves, so it cannot be read as a value"
    );
    assert!(
        sweep.size().width < track.size().width / 2.0 + 0.1,
        "the moving segment is clipped to the track"
    );

    let still = banner_app(-1.0, false);
    assert!(
        find(&still, "OperationBanner::sweep").is_none(),
        "with animations off an unknown total shows an empty track"
    );
}

#[test]
fn determinate_progress_fills_exactly_the_reported_fraction() {
    let app = banner_app(0.0, true);
    let track = find(&app, "OperationBanner::track").expect("track exists");
    let fill = find(&app, "OperationBanner::fill").expect("fill exists");
    assert_eq!(fill.size().width, 0.0, "0 of N is determinate and empty");
    assert!(
        find(&app, "OperationBanner::sweep").is_none(),
        "a known 0 of N is not indeterminate"
    );
    for fraction in [0.25_f32, 0.5, 1.0] {
        app.set_operation_fraction(fraction);
        i_slint_backend_testing::mock_elapsed_time(Duration::from_millis(500));
        assert!(
            (fill.size().width - track.size().width * fraction).abs() < 0.5,
            "fill follows fraction {fraction}: {} of {}",
            fill.size().width,
            track.size().width
        );
    }
}
