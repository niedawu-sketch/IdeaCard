#[derive(Clone, Debug)]
pub struct NativeTextFieldConfig {
    pub field_id: String,
    pub value: String,
    pub placeholder: String,
    pub read_only: bool,
    pub visible: bool,
    pub x: f64,
    pub y: f64,
    pub width: f64,
    pub height: f64,
}

#[cfg(target_os = "macos")]
mod imp {
    use std::{cell::RefCell, collections::HashMap, ptr::NonNull};

    use block2::RcBlock;
    use objc2::{rc::Retained, runtime::ProtocolObject, MainThreadOnly};
    use objc2_app_kit::{
        NSColor, NSControlTextDidBeginEditingNotification, NSControlTextDidChangeNotification,
        NSControlTextDidEndEditingNotification, NSFocusRingType, NSFont, NSTextField, NSWindow,
        NSWindowOrderingMode,
    };
    use objc2_foundation::{
        MainThreadMarker, NSNotification, NSNotificationCenter, NSObjectProtocol, NSPoint, NSRect,
        NSSize, NSString,
    };
    use serde::Serialize;
    use tauri::{AppHandle, Emitter, Manager};

    use super::NativeTextFieldConfig;

    #[derive(Clone, Serialize)]
    #[serde(rename_all = "camelCase")]
    struct ChangedPayload {
        field_id: String,
        value: String,
    }

    #[derive(Clone, Serialize)]
    #[serde(rename_all = "camelCase")]
    struct FocusPayload {
        field_id: String,
        focused: bool,
    }

    struct NativeTextFieldClient {
        field: Retained<NSTextField>,
        observers: Vec<Retained<ProtocolObject<dyn NSObjectProtocol>>>,
    }

    thread_local! {
        static CLIENTS: RefCell<HashMap<String, NativeTextFieldClient>> =
            RefCell::new(HashMap::new());
    }

    pub fn sync(app: &AppHandle, config: NativeTextFieldConfig) {
        let app_for_thread = app.clone();
        let app_for_sync = app_for_thread.clone();
        let _ = app_for_thread.run_on_main_thread(move || {
            sync_on_main(&app_for_sync, config);
        });
    }

    pub fn remove(app: &AppHandle, field_id: String) {
        let app_for_thread = app.clone();
        let _ = app_for_thread.run_on_main_thread(move || {
            remove_on_main(&field_id);
        });
    }

    pub fn focus(app: &AppHandle, field_id: String) {
        let app_for_thread = app.clone();
        let app_for_focus = app_for_thread.clone();
        let _ = app_for_thread.run_on_main_thread(move || {
            focus_on_main(&app_for_focus, &field_id);
        });
    }

    fn sync_on_main(app: &AppHandle, config: NativeTextFieldConfig) {
        let exists = CLIENTS.with(|clients| clients.borrow().contains_key(&config.field_id));
        if !exists && (!config.visible || !create_client(app, &config.field_id)) {
            return;
        }

        let Some(window) = native_window(app) else {
            return;
        };
        let Some(content_view) = window.contentView() else {
            return;
        };
        let bounds = content_view.bounds();
        let frame = frame_for_config(bounds, &config);

        CLIENTS.with(|clients| {
            let clients = clients.borrow();
            let Some(client) = clients.get(&config.field_id) else {
                return;
            };
            let field = &client.field;
            if field.stringValue().to_string() != config.value {
                field.setStringValue(&NSString::from_str(&config.value));
            }
            let placeholder =
                (!config.placeholder.is_empty()).then(|| NSString::from_str(&config.placeholder));
            field.setPlaceholderString(placeholder.as_deref());
            // Reapplying editability can interrupt the active AppKit input session.
            if field.isEditable() == config.read_only {
                field.setEditable(!config.read_only);
            }
            if !field.isSelectable() {
                field.setSelectable(true);
            }
            field.setFrame(frame);
            field.setHidden(!config.visible);
            if !config.visible {
                field.abortEditing();
            }
        });
    }

    fn create_client(app: &AppHandle, field_id: &str) -> bool {
        let Some(window) = native_window(app) else {
            return false;
        };
        let Some(content_view) = window.contentView() else {
            return false;
        };
        let Some(mtm) = MainThreadMarker::new() else {
            return false;
        };

        let field = NSTextField::initWithFrame(
            NSTextField::alloc(mtm),
            NSRect::new(NSPoint::new(0.0, 0.0), NSSize::new(1.0, 1.0)),
        );
        field.setBezeled(false);
        field.setBordered(false);
        field.setDrawsBackground(false);
        field.setFocusRingType(NSFocusRingType::None);
        field.setFont(Some(&NSFont::systemFontOfSize(12.5)));
        field.setTextColor(Some(&NSColor::labelColor()));
        field.setContinuous(true);
        content_view.addSubview_positioned_relativeTo(&field, NSWindowOrderingMode::Above, None);

        let center = NSNotificationCenter::defaultCenter();
        let app_for_change = app.clone();
        let change_id = field_id.to_owned();
        let field_for_change = field.clone();
        let change_block = RcBlock::new(move |_notification: NonNull<NSNotification>| {
            let _ = app_for_change.emit(
                "native-text-field://changed",
                ChangedPayload {
                    field_id: change_id.clone(),
                    value: field_for_change.stringValue().to_string(),
                },
            );
        });
        let change_observer = unsafe {
            center.addObserverForName_object_queue_usingBlock(
                Some(NSControlTextDidChangeNotification),
                Some(&field),
                None,
                &change_block,
            )
        };

        let app_for_focus = app.clone();
        let focus_id = field_id.to_owned();
        let focus_block = RcBlock::new(move |_notification: NonNull<NSNotification>| {
            let _ = app_for_focus.emit(
                "native-text-field://focus-changed",
                FocusPayload {
                    field_id: focus_id.clone(),
                    focused: true,
                },
            );
        });
        let focus_observer = unsafe {
            center.addObserverForName_object_queue_usingBlock(
                Some(NSControlTextDidBeginEditingNotification),
                Some(&field),
                None,
                &focus_block,
            )
        };

        let app_for_blur = app.clone();
        let blur_id = field_id.to_owned();
        let blur_block = RcBlock::new(move |_notification: NonNull<NSNotification>| {
            let _ = app_for_blur.emit(
                "native-text-field://focus-changed",
                FocusPayload {
                    field_id: blur_id.clone(),
                    focused: false,
                },
            );
        });
        let blur_observer = unsafe {
            center.addObserverForName_object_queue_usingBlock(
                Some(NSControlTextDidEndEditingNotification),
                Some(&field),
                None,
                &blur_block,
            )
        };

        CLIENTS.with(|clients| {
            clients.borrow_mut().insert(
                field_id.to_owned(),
                NativeTextFieldClient {
                    field,
                    observers: vec![change_observer, focus_observer, blur_observer],
                },
            );
        });
        true
    }

    pub(super) fn frame_for_config(bounds: NSRect, config: &NativeTextFieldConfig) -> NSRect {
        const HORIZONTAL_INSET: f64 = 8.0;
        const TEXT_FIELD_HEIGHT: f64 = 20.0;
        const BASELINE_OPTICAL_OFFSET: f64 = 2.0;

        let host_width = config.width.max(1.0).min(bounds.size.width.max(1.0));
        let horizontal_inset = HORIZONTAL_INSET.min(host_width / 2.0);
        let width = (host_width - horizontal_inset * 2.0).max(1.0);
        let host_height = config.height.max(1.0).min(bounds.size.height.max(1.0));
        let height = host_height.min(TEXT_FIELD_HEIGHT);
        let centered_inset = (host_height - height) / 2.0;
        let optical_offset = BASELINE_OPTICAL_OFFSET.min(centered_inset);
        let top = config.y + centered_inset + optical_offset;
        let x = (config.x + horizontal_inset).clamp(0.0, (bounds.size.width - width).max(0.0));
        let y =
            (bounds.size.height - top - height).clamp(0.0, (bounds.size.height - height).max(0.0));
        NSRect::new(NSPoint::new(x, y), NSSize::new(width, height))
    }

    fn remove_on_main(field_id: &str) {
        let client = CLIENTS.with(|clients| clients.borrow_mut().remove(field_id));
        if let Some(client) = client {
            destroy_client(client);
        }
    }

    fn focus_on_main(app: &AppHandle, field_id: &str) {
        let field = CLIENTS.with(|clients| {
            clients
                .borrow()
                .get(field_id)
                .map(|client| client.field.clone())
        });
        let (Some(window), Some(field)) = (native_window(app), field) else {
            return;
        };
        if !field.isHidden() && field.isEditable() {
            window.makeFirstResponder(Some(&field));
        }
    }

    fn destroy_client(client: NativeTextFieldClient) {
        client.field.abortEditing();
        let center = NSNotificationCenter::defaultCenter();
        for observer in client.observers {
            unsafe { center.removeObserver(observer.as_ref()) };
        }
        client.field.removeFromSuperview();
    }

    fn native_window(app: &AppHandle) -> Option<&NSWindow> {
        app.get_webview_window("main")
            .and_then(|window| window.ns_window().ok())
            .map(|pointer| unsafe { &*pointer.cast::<NSWindow>() })
    }
}

#[cfg(not(target_os = "macos"))]
mod imp {
    use tauri::AppHandle;

    use super::NativeTextFieldConfig;

    pub fn sync(_app: &AppHandle, _config: NativeTextFieldConfig) {}

    pub fn remove(_app: &AppHandle, _field_id: String) {}

    pub fn focus(_app: &AppHandle, _field_id: String) {}
}

pub use imp::{focus, remove, sync};

#[cfg(all(test, target_os = "macos"))]
mod tests {
    use objc2_foundation::{NSPoint, NSRect, NSSize};

    use super::{imp::frame_for_config, NativeTextFieldConfig};

    #[test]
    fn native_text_is_inset_and_vertically_centered_in_the_html_host() {
        let frame = frame_for_config(
            NSRect::new(NSPoint::new(0.0, 0.0), NSSize::new(800.0, 600.0)),
            &NativeTextFieldConfig {
                field_id: "card-title-1".to_owned(),
                value: "标题".to_owned(),
                placeholder: String::new(),
                read_only: false,
                visible: true,
                x: 10.0,
                y: 20.0,
                width: 300.0,
                height: 36.0,
            },
        );

        assert_eq!(frame.origin.x, 18.0);
        assert_eq!(frame.origin.y, 550.0);
        assert_eq!(frame.size.width, 284.0);
        assert_eq!(frame.size.height, 20.0);
    }
}
