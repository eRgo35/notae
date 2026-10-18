// Java calls these exported JNI entry points; unsafe-code allowance is limited to that FFI boundary.
#![allow(unsafe_code)]
//! Keeps Android's accessibility view of the window in sync with Slint.
//!
//! [`crate::accessibility`] builds and encodes the snapshot; `NotaeAccessibility.java` answers
//! TalkBack from it. This file is the thread glue:
//!
//! * Java reports on the UI thread whether an accessibility client is connected
//!   (`nativeSetActive`) and which action was requested (`nativePerformAction`). Both hop to
//!   the Slint event-loop thread through `invoke_from_event_loop`; the UI thread never waits
//!   for Slint.
//! * On the Slint thread a `PropertyTracker` records every property the snapshot read. When one
//!   changes it only schedules a rebuild (the tracker callback can fire in the middle of a
//!   property update, so nothing is walked there). Rebuilds are coalesced to one per
//!   [`MIN_REBUILD_INTERVAL`] and an unchanged snapshot is not sent again.
//! * Slint input focus is not a property, so a slow timer compares the focused item while a
//!   client is connected.
//!
//! Nothing is tracked or built while no accessibility client is connected.

use std::cell::{Cell, RefCell};
use std::pin::Pin;
use std::sync::atomic::{AtomicBool, Ordering};
use std::time::{Duration, Instant};

use android_activity::AndroidApp;
use i_slint_core::item_tree::ItemWeak;
use i_slint_core::properties::{PropertyDirtyHandler, PropertyTracker};
use i_slint_core::window::WindowInner;
use jni::objects::{JClass, JIntArray, JObjectArray, JString, JValue};
use jni::{EnvUnowned, jni_sig, jni_str};
use slint::{ComponentHandle, Timer, TimerMode};

use crate::AppWindow;
use crate::accessibility::{Action, Registry, Wire};

/// Fastest rebuild cadence while the UI changes continuously (scrolling, animations).
const MIN_REBUILD_INTERVAL: Duration = Duration::from_millis(100);
/// How often Slint's input focus is compared with the last published one.
const FOCUS_POLL_INTERVAL: Duration = Duration::from_millis(300);

// Action kinds sent by `NotaeAccessibility.java`.
const KIND_CLICK: i32 = 0;
const KIND_FOCUS: i32 = 1;
const KIND_SET_TEXT: i32 = 2;
const KIND_SET_SELECTION: i32 = 3;
const KIND_INCREMENT: i32 = 4;
const KIND_DECREMENT: i32 = 5;
const KIND_EXPAND: i32 = 6;

/// Whether Java last reported a connected client. Set from the UI thread; applied to the
/// controller on the Slint thread, which also covers a client that connected before `bind`.
static CLIENT_CONNECTED: AtomicBool = AtomicBool::new(false);

thread_local! {
    static CONTROLLER: RefCell<Option<Controller>> = const { RefCell::new(None) };
    static REBUILD_PENDING: Cell<bool> = const { Cell::new(false) };
}

struct DirtyNotifier;

impl PropertyDirtyHandler for DirtyNotifier {
    fn notify(self: Pin<&Self>) {
        schedule_rebuild();
    }
}

struct Controller {
    app: AndroidApp,
    window: slint::Weak<AppWindow>,
    registry: Registry,
    tracker: Pin<Box<PropertyTracker<false, DirtyNotifier>>>,
    active: bool,
    published: Option<Wire>,
    last_published_at: Option<Instant>,
    focus_seen: Option<ItemWeak>,
    focus_poll: Timer,
}

/// Binds the controller to the window. Call on the Slint thread before the event loop starts.
pub fn bind(app: AndroidApp, window: slint::Weak<AppWindow>) {
    CONTROLLER.with(|slot| {
        *slot.borrow_mut() = Some(Controller {
            app,
            window,
            registry: Registry::default(),
            tracker: Box::pin(PropertyTracker::new_with_dirty_handler(DirtyNotifier)),
            active: false,
            published: None,
            last_published_at: None,
            focus_seen: None,
            focus_poll: Timer::default(),
        });
    });
    // The client may have connected while the native thread was still starting.
    let _ =
        slint::invoke_from_event_loop(|| apply_connected(CLIENT_CONNECTED.load(Ordering::Acquire)));
}

fn schedule_rebuild() {
    if REBUILD_PENDING.with(|pending| pending.replace(true)) {
        return;
    }
    // Properties are thread-bound, so this runs on the Slint thread: queue the work instead of
    // doing it inside the property change that notified us.
    let _ = slint::invoke_from_event_loop(rebuild_when_due);
}

fn rebuild_when_due() {
    let wait = CONTROLLER.with(|slot| {
        let slot = slot.borrow();
        let last = slot.as_ref()?.last_published_at?;
        MIN_REBUILD_INTERVAL.checked_sub(last.elapsed())
    });
    match wait {
        Some(wait) if !wait.is_zero() => Timer::single_shot(wait, rebuild_now),
        _ => rebuild_now(),
    }
}

fn rebuild_now() {
    REBUILD_PENDING.with(|pending| pending.set(false));
    CONTROLLER.with(|slot| {
        // A rebuild never nests: if the controller is busy the change that caused this call
        // re-arms the tracker and schedules another.
        if let Ok(mut slot) = slot.try_borrow_mut()
            && let Some(controller) = slot.as_mut()
        {
            controller.rebuild();
        }
    });
}

fn apply_connected(connected: bool) {
    CONTROLLER.with(|slot| {
        if let Ok(mut slot) = slot.try_borrow_mut()
            && let Some(controller) = slot.as_mut()
        {
            controller.set_active(connected);
        }
    });
}

impl Controller {
    fn set_active(&mut self, active: bool) {
        if self.active == active {
            return;
        }
        self.active = active;
        self.published = None;
        self.last_published_at = None;
        self.focus_seen = None;
        if active {
            self.focus_poll
                .start(TimerMode::Repeated, FOCUS_POLL_INTERVAL, poll_focus);
            self.rebuild();
        } else {
            self.focus_poll.stop();
            self.registry = Registry::default();
            // Dropping the tracker's dependencies stops change notifications.
            self.tracker = Box::pin(PropertyTracker::new_with_dirty_handler(DirtyNotifier));
        }
    }

    fn rebuild(&mut self) {
        if !self.active {
            return;
        }
        let Some(ui) = self.window.upgrade() else {
            return;
        };
        let Self {
            registry, tracker, ..
        } = self;
        let snapshot = tracker
            .as_ref()
            .evaluate_as_dependency_root(|| crate::accessibility::build(ui.window(), registry));
        self.focus_seen = WindowInner::from_pub(ui.window())
            .focus_item
            .borrow()
            .upgrade()
            .map(|item| item.downgrade());

        let wire = snapshot.encode();
        if self.published.as_ref() == Some(&wire) {
            return;
        }
        match publish(&self.app, &wire) {
            Ok(()) => {
                self.published = Some(wire);
                self.last_published_at = Some(Instant::now());
            }
            // Keep `published` unset so the next change retries.
            Err(error) => eprintln!("notae: accessibility publish failed: {error}"),
        }
    }

    fn focus_changed(&self) -> bool {
        let Some(ui) = self.window.upgrade() else {
            return false;
        };
        let now = WindowInner::from_pub(ui.window())
            .focus_item
            .borrow()
            .upgrade();
        let before = self.focus_seen.as_ref().and_then(ItemWeak::upgrade);
        now != before
    }
}

fn poll_focus() {
    let changed = CONTROLLER.with(|slot| {
        slot.try_borrow()
            .ok()
            .and_then(|slot| slot.as_ref().map(Controller::focus_changed))
            .unwrap_or(false)
    });
    if changed {
        schedule_rebuild();
    }
}

fn publish(app: &AndroidApp, wire: &Wire) -> Result<(), jni::errors::Error> {
    let activity_ptr = app.activity_as_ptr();
    super::saf::with_env(app, |env| {
        let activity = super::saf::activity_as_jobject(env, activity_ptr)?;
        let bridge = env
            .call_method(
                &activity,
                jni_str!("notaeAccessibility"),
                jni_sig!("()Lorg/notae/android/NotaeAccessibility;"),
                &[],
            )?
            .l()?;
        if bridge.is_null() {
            return Ok(());
        }
        let ints = JIntArray::new(env, wire.ints.len())?;
        ints.set_region(env, 0, &wire.ints)?;
        let empty = env.new_string("")?;
        let strings = JObjectArray::<JString>::new(env, wire.strings.len(), &empty)?;
        for (index, value) in wire.strings.iter().enumerate() {
            // One frame per string keeps the local-reference table small for large trees.
            env.with_local_frame(4, |env| -> Result<(), jni::errors::Error> {
                let value = env.new_string(value)?;
                strings.set_element(env, index, &value)
            })?;
        }
        env.call_method(
            &bridge,
            jni_str!("publish"),
            jni_sig!("([I[Ljava/lang/String;)V"),
            &[JValue::Object(&ints), JValue::Object(&strings)],
        )?;
        Ok(())
    })
}

fn action_for(kind: i32, text: Option<String>, first: i32, second: i32) -> Option<Action> {
    match kind {
        KIND_CLICK => Some(Action::Click),
        KIND_FOCUS => Some(Action::Focus),
        KIND_SET_TEXT => text.map(Action::SetText),
        KIND_SET_SELECTION => Some(Action::SetSelection(first, second)),
        KIND_INCREMENT => Some(Action::Increment),
        KIND_DECREMENT => Some(Action::Decrement),
        KIND_EXPAND => Some(Action::Expand),
        _ => None,
    }
}

fn perform_on_slint_thread(id: i32, action: Action) {
    CONTROLLER.with(|slot| {
        let Ok(slot) = slot.try_borrow() else { return };
        let Some(controller) = slot.as_ref() else {
            return;
        };
        let Some(ui) = controller.window.upgrade() else {
            return;
        };
        if let Err(refusal) =
            crate::accessibility::perform(ui.window(), &controller.registry, id, &action)
        {
            eprintln!("notae: accessibility action {action:?} on node {id} refused: {refusal:?}");
        }
    });
    // The action usually changed the UI; do not wait for the tracker to notice.
    schedule_rebuild();
}

#[unsafe(no_mangle)]
pub extern "system" fn Java_org_notae_android_NotaeAccessibility_nativeSetActive<'local>(
    _env: EnvUnowned<'local>,
    _class: JClass<'local>,
    active: bool,
) {
    CLIENT_CONNECTED.store(active, Ordering::Release);
    let _ = slint::invoke_from_event_loop(move || apply_connected(active));
}

#[unsafe(no_mangle)]
pub extern "system" fn Java_org_notae_android_NotaeAccessibility_nativePerformAction<'local>(
    mut unowned_env: EnvUnowned<'local>,
    _class: JClass<'local>,
    id: jni::sys::jint,
    kind: jni::sys::jint,
    text: JString<'local>,
    first: jni::sys::jint,
    second: jni::sys::jint,
) {
    let text = unowned_env
        .with_env(|env| -> Result<Option<String>, jni::errors::Error> {
            if text.is_null() {
                return Ok(None);
            }
            Ok(Some(text.mutf8_chars(env)?.to_str().into_owned()))
        })
        .resolve::<jni::errors::ThrowRuntimeExAndDefault>();
    let Some(action) = action_for(kind, text, first, second) else {
        return;
    };
    let _ = slint::invoke_from_event_loop(move || perform_on_slint_thread(id, action));
}
