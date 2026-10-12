#[derive(Clone, Copy, Debug)]
pub struct TargetRect {
    pub x: f64,
    pub y: f64,
    pub width: f64,
    pub height: f64,
}

#[cfg(target_os = "macos")]
mod imp {
    use std::{
        cell::{Cell, RefCell},
        ptr::NonNull,
        time::Instant,
    };

    use block2::RcBlock;
    use objc2::{rc::Retained, runtime::ProtocolObject, MainThreadOnly};
    use objc2_app_kit::{
        NSEvent, NSEventMask, NSEventModifierFlags, NSEventType, NSResponder,
        NSTextStorageDidProcessEditingNotification, NSTextView, NSWindow, NSWindowOrderingMode,
    };
    use objc2_foundation::{
        MainThreadMarker, NSNotification, NSNotificationCenter, NSObjectProtocol, NSPoint, NSRect,
        NSRunLoop, NSSize, NSString,
    };
    use serde::Serialize;
    use tauri::{AppHandle, Emitter, Manager};

    use super::TargetRect;
    use crate::AppState;

    const FN_KEY_CODE: u16 = 63;
    const USE_PREARMED_NATIVE_TEXT_CLIENT: bool = false;

    #[derive(Clone, Serialize)]
    #[serde(rename_all = "camelCase")]
    struct StartedPayload {
        session_id: u64,
        target_token: String,
    }

    #[derive(Clone, Serialize)]
    #[serde(rename_all = "camelCase")]
    struct DraftPayload {
        session_id: u64,
        text: String,
    }

    #[derive(Clone, Serialize)]
    #[serde(rename_all = "camelCase")]
    struct ReleasedPayload {
        session_id: u64,
        held_ms: u64,
    }

    #[derive(Clone, Serialize)]
    #[serde(rename_all = "camelCase")]
    struct CancelledPayload {
        session_id: u64,
    }

    struct NativeCapture {
        id: u64,
        started_at: Instant,
    }

    struct NativeTextClient {
        text_view: Retained<NSTextView>,
        previous_responder: Option<Retained<NSResponder>>,
        observer: Retained<ProtocolObject<dyn NSObjectProtocol>>,
        capture: Option<NativeCapture>,
        draft_flush_scheduled: bool,
        armed: bool,
    }

    thread_local! {
        static CLIENT: RefCell<Option<NativeTextClient>> = const { RefCell::new(None) };
        static MONITOR_INSTALLED: Cell<bool> = const { Cell::new(false) };
        static FN_HELD: Cell<bool> = const { Cell::new(false) };
        static FN_STARTED_AT: RefCell<Option<Instant>> = const { RefCell::new(None) };
    }

    pub fn install(app: &AppHandle) {
        if !USE_PREARMED_NATIVE_TEXT_CLIENT {
            let _ = app;
            diagnostic("native_text_event_monitor_disabled");
            return;
        }
        if MainThreadMarker::new().is_none() {
            diagnostic("monitor_install_not_on_main_thread");
            log::warn!("voice_input_monitor_not_on_main_thread");
            return;
        }
        let already_installed = MONITOR_INSTALLED.with(|installed| {
            if installed.get() {
                true
            } else {
                installed.set(true);
                false
            }
        });
        if already_installed {
            return;
        }

        let app = app.clone();
        let block = RcBlock::new(move |event_pointer: NonNull<NSEvent>| -> *mut NSEvent {
            let event = unsafe { event_pointer.as_ref() };
            handle_event(&app, event);
            event_pointer.as_ptr()
        });
        let mask = NSEventMask::FlagsChanged | NSEventMask::KeyDown | NSEventMask::KeyUp;
        let monitor =
            unsafe { NSEvent::addLocalMonitorForEventsMatchingMask_handler(mask, &block) };
        if let Some(monitor) = monitor {
            diagnostic("monitor_installed");
            // The monitor is intentionally kept for the full application lifetime.
            std::mem::forget(monitor);
        } else {
            MONITOR_INSTALLED.with(|installed| installed.set(false));
            log::warn!("voice_input_monitor_unavailable");
        }
    }

    pub fn end_session(app: &AppHandle, session_id: u64) {
        let app_for_thread = app.clone();
        let app_for_finish = app_for_thread.clone();
        let _ = app_for_thread.run_on_main_thread(move || {
            finish_capture(&app_for_finish, session_id, true);
        });
    }

    pub fn sync_target(app: &AppHandle, target_available: bool, target_rect: Option<TargetRect>) {
        if !USE_PREARMED_NATIVE_TEXT_CLIENT {
            let _ = (app, target_available, target_rect);
            return;
        }
        let app_for_thread = app.clone();
        let app_for_sync = app_for_thread.clone();
        let _ = app_for_thread.run_on_main_thread(move || {
            sync_target_on_main(&app_for_sync, target_available, target_rect);
        });
    }

    fn handle_event(app: &AppHandle, event: &NSEvent) {
        match event.r#type() {
            NSEventType::FlagsChanged if event.keyCode() == FN_KEY_CODE => {
                if event
                    .modifierFlags()
                    .contains(NSEventModifierFlags::Function)
                {
                    let was_held = FN_HELD.replace(true);
                    if !was_held {
                        FN_STARTED_AT.with(|started_at| {
                            *started_at.borrow_mut() = Some(Instant::now());
                        });
                    }
                    diagnostic("fn_down");
                } else {
                    FN_HELD.set(false);
                    diagnostic("fn_up");
                    defer_release_session(app);
                }
            }
            NSEventType::KeyDown => route_physical_key_to_webview(app),
            _ => {}
        }
    }

    fn defer_release_session(app: &AppHandle) {
        let app = app.clone();
        let block = RcBlock::new(move || release_session(&app));
        let run_loop = NSRunLoop::mainRunLoop();
        unsafe { run_loop.performBlock(&block) };
    }

    fn begin_session_from_native_text(app: &AppHandle) -> Option<u64> {
        let Some(session) = app
            .state::<AppState>()
            .voice_input
            .lock()
            .ok()
            .and_then(|mut state| state.begin())
        else {
            diagnostic("session_not_started_without_target_or_while_active");
            return None;
        };

        let started_at = FN_STARTED_AT
            .with(|current| current.borrow().as_ref().cloned())
            .unwrap_or_else(Instant::now);
        let capture_started = CLIENT.with(|current| {
            if let Some(client) = current.borrow_mut().as_mut() {
                client.capture = Some(NativeCapture {
                    id: session.id,
                    started_at,
                });
                true
            } else {
                false
            }
        });
        if !capture_started {
            if let Ok(mut state) = app.state::<AppState>().voice_input.lock() {
                state.finish(session.id);
            }
            diagnostic("native_text_client_unavailable_at_session_start");
            return None;
        }
        diagnostic("voice_session_started_from_native_text");

        let _ = app.emit(
            "voice-input://started",
            StartedPayload {
                session_id: session.id,
                target_token: session.target_token,
            },
        );
        Some(session.id)
    }

    fn sync_target_on_main(
        app: &AppHandle,
        target_available: bool,
        target_rect: Option<TargetRect>,
    ) {
        let capture_active = CLIENT.with(|current| {
            current
                .borrow()
                .as_ref()
                .and_then(|client| client.capture.as_ref())
                .is_some()
        });
        if !target_available {
            if !capture_active {
                destroy_native_client(app);
            }
            return;
        }
        if capture_active {
            return;
        }
        if ensure_native_client(app) {
            if let Some(target_rect) = target_rect {
                update_client_frame(app, target_rect);
            }
        }
        if client_is_armed() {
            disarm_client(app);
            diagnostic("native_text_client_refreshed");
        }
        if arm_client(app) {
            diagnostic("native_text_client_prearmed");
        }
    }

    fn ensure_native_client(app: &AppHandle) -> bool {
        if CLIENT.with(|current| current.borrow().is_some()) {
            return true;
        }
        create_native_client(app)
    }

    fn create_native_client(app: &AppHandle) -> bool {
        let Some(window_handle) = app.get_webview_window("main") else {
            return false;
        };
        let Ok(window_pointer) = window_handle.ns_window() else {
            return false;
        };
        let Some(mtm) = MainThreadMarker::new() else {
            return false;
        };
        let window = unsafe { &*window_pointer.cast::<NSWindow>() };
        let Some(content_view) = window.contentView() else {
            return false;
        };

        let text_view = NSTextView::initWithFrame(
            NSTextView::alloc(mtm),
            NSRect::new(NSPoint::new(0.0, 0.0), NSSize::new(2.0, 2.0)),
        );
        text_view.setEditable(true);
        text_view.setSelectable(true);
        text_view.setRichText(false);
        text_view.setDrawsBackground(false);
        text_view.setAllowsUndo(false);
        text_view.setAlphaValue(0.01);
        text_view.setString(&NSString::from_str(""));
        content_view.addSubview_positioned_relativeTo(
            &text_view,
            NSWindowOrderingMode::Below,
            None,
        );

        let app_for_text = app.clone();
        let block = RcBlock::new(move |_notification: NonNull<NSNotification>| {
            schedule_draft_flush(&app_for_text);
        });
        let center = NSNotificationCenter::defaultCenter();
        let Some(text_storage) = (unsafe { text_view.textStorage() }) else {
            text_view.removeFromSuperview();
            return false;
        };
        let observer = unsafe {
            center.addObserverForName_object_queue_usingBlock(
                Some(NSTextStorageDidProcessEditingNotification),
                Some(&text_storage),
                None,
                &block,
            )
        };
        CLIENT.with(|current| {
            *current.borrow_mut() = Some(NativeTextClient {
                text_view,
                previous_responder: window.firstResponder(),
                observer,
                capture: None,
                draft_flush_scheduled: false,
                armed: false,
            });
        });
        true
    }

    fn schedule_draft_flush(app: &AppHandle) {
        let should_schedule = CLIENT.with(|current| {
            let mut current = current.borrow_mut();
            let Some(client) = current.as_mut() else {
                return false;
            };
            if client.draft_flush_scheduled {
                false
            } else {
                client.draft_flush_scheduled = true;
                true
            }
        });
        if !should_schedule {
            return;
        }
        let app = app.clone();
        let block = RcBlock::new(move || flush_current_draft(&app));
        let run_loop = NSRunLoop::mainRunLoop();
        unsafe { run_loop.performBlock(&block) };
    }

    fn flush_current_draft(app: &AppHandle) {
        let snapshot = CLIENT.with(|current| {
            let mut current = current.borrow_mut();
            let client = current.as_mut()?;
            client.draft_flush_scheduled = false;
            Some((
                client.capture.as_ref().map(|capture| capture.id),
                client.text_view.string().to_string(),
            ))
        });
        let Some((existing_session_id, text)) = snapshot else {
            return;
        };
        if existing_session_id.is_none() && text.is_empty() {
            return;
        }
        let session_started_from_text = existing_session_id.is_none();
        let Some(session_id) = existing_session_id.or_else(|| begin_session_from_native_text(app))
        else {
            return;
        };
        let accepted = app
            .state::<AppState>()
            .voice_input
            .lock()
            .map(|mut state| state.record_draft(session_id, text.clone()))
            .unwrap_or(false);
        if accepted {
            diagnostic("native_text_changed");
            let _ = app.emit("voice-input://draft", DraftPayload { session_id, text });
        }
        if session_started_from_text && !FN_HELD.get() {
            release_session(app);
        }
    }

    fn release_session(app: &AppHandle) {
        let Some(session) = app
            .state::<AppState>()
            .voice_input
            .lock()
            .ok()
            .and_then(|mut state| state.release())
        else {
            return;
        };
        let held_ms = CLIENT.with(|current| {
            current
                .borrow()
                .as_ref()
                .and_then(|client| client.capture.as_ref())
                .filter(|capture| capture.id == session.id)
                .map(|capture| capture.started_at.elapsed().as_millis() as u64)
                .unwrap_or_default()
        });
        let _ = app.emit(
            "voice-input://released",
            ReleasedPayload {
                session_id: session.id,
                held_ms,
            },
        );
    }

    fn route_physical_key_to_webview(app: &AppHandle) {
        let cancelled = {
            let app_state = app.state::<AppState>();
            let Ok(mut state) = app_state.voice_input.lock() else {
                disarm_client(app);
                return;
            };
            state.cancel_current()
        };
        if let Some(session) = cancelled {
            finish_cancelled_session(app, session.id);
        } else {
            disarm_client(app);
        }
        diagnostic("physical_key_routed_to_webview");
    }

    fn finish_cancelled_session(app: &AppHandle, session_id: u64) {
        finish_capture(app, session_id, false);
        let _ = app.emit("voice-input://cancelled", CancelledPayload { session_id });
    }

    fn finish_capture(app: &AppHandle, session_id: u64, rearm: bool) {
        let text_view = CLIENT.with(|current| {
            let mut current = current.borrow_mut();
            let client = current.as_mut()?;
            if client.capture.as_ref().map(|capture| capture.id) != Some(session_id) {
                return None;
            }
            client.capture = None;
            Some(client.text_view.clone())
        });
        let Some(text_view) = text_view else {
            return;
        };
        text_view.setString(&NSString::from_str(""));

        let target_available = app
            .state::<AppState>()
            .voice_input
            .lock()
            .map(|state| state.has_target())
            .unwrap_or(false);
        if rearm && target_available {
            let _ = arm_client(app);
        } else {
            disarm_client(app);
        }
        if !target_available {
            destroy_native_client(app);
        }
        diagnostic("voice_session_finished");
    }

    fn client_is_armed() -> bool {
        CLIENT.with(|current| {
            current
                .borrow()
                .as_ref()
                .map(|client| client.armed)
                .unwrap_or(false)
        })
    }

    fn update_client_frame(app: &AppHandle, target_rect: TargetRect) {
        let Some(window) = native_window(app) else {
            return;
        };
        let Some(content_view) = window.contentView() else {
            return;
        };
        let bounds = content_view.bounds();
        let width = target_rect.width.max(80.0).min(bounds.size.width.max(80.0));
        let height = target_rect
            .height
            .max(24.0)
            .min(bounds.size.height.max(24.0));
        let x = target_rect
            .x
            .clamp(0.0, (bounds.size.width - width).max(0.0));
        let y = (bounds.size.height - target_rect.y - target_rect.height)
            .clamp(0.0, (bounds.size.height - height).max(0.0));
        let frame = NSRect::new(NSPoint::new(x, y), NSSize::new(width, height));
        CLIENT.with(|current| {
            if let Some(client) = current.borrow().as_ref() {
                client.text_view.setFrame(frame);
            }
        });
    }

    fn arm_client(app: &AppHandle) -> bool {
        if client_is_armed() {
            return true;
        }
        let Some(window) = native_window(app) else {
            return false;
        };
        CLIENT.with(|current| {
            let mut current = current.borrow_mut();
            let Some(client) = current.as_mut() else {
                return false;
            };
            client.previous_responder = window.firstResponder();
            if window.makeFirstResponder(Some(&client.text_view)) {
                client.armed = true;
                true
            } else {
                false
            }
        })
    }

    fn disarm_client(app: &AppHandle) {
        let Some(window) = native_window(app) else {
            return;
        };
        CLIENT.with(|current| {
            let mut current = current.borrow_mut();
            let Some(client) = current.as_mut() else {
                return;
            };
            if client.armed {
                window.makeFirstResponder(client.previous_responder.as_deref());
                client.armed = false;
            }
        });
    }

    fn destroy_native_client(app: &AppHandle) {
        let native = CLIENT.with(|current| current.borrow_mut().take());
        let Some(native) = native else {
            return;
        };
        if native.armed {
            if let Some(window) = native_window(app) {
                window.makeFirstResponder(native.previous_responder.as_deref());
            }
        }
        let center = NSNotificationCenter::defaultCenter();
        unsafe { center.removeObserver(native.observer.as_ref()) };
        native.text_view.removeFromSuperview();
        diagnostic("native_text_client_destroyed");
    }

    fn native_window(app: &AppHandle) -> Option<&NSWindow> {
        app.get_webview_window("main")
            .and_then(|window| window.ns_window().ok())
            .map(|pointer| unsafe { &*pointer.cast::<NSWindow>() })
    }

    fn diagnostic(event: &str) {
        #[cfg(debug_assertions)]
        eprintln!("voice-input diagnostic: {event}");
        #[cfg(not(debug_assertions))]
        let _ = event;
    }
}

#[cfg(not(target_os = "macos"))]
mod imp {
    use tauri::AppHandle;

    use super::TargetRect;

    pub fn install(_app: &AppHandle) {}

    pub fn end_session(_app: &AppHandle, _session_id: u64) {}

    pub fn sync_target(
        _app: &AppHandle,
        _target_available: bool,
        _target_rect: Option<TargetRect>,
    ) {
    }
}

pub use imp::{end_session, install, sync_target};
